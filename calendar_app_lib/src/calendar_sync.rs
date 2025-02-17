use anyhow::{format_err, Error};
use futures::{future::try_join_all, Stream, TryStreamExt};
use log::debug;
use postgres_query::Error as PqError;
use stack_string::{format_sstr, StackString};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use stdout_channel::StdoutChannel;
use time::{Date, Duration, OffsetDateTime, PrimitiveDateTime};
use time_tz::{OffsetDateTimeExt, PrimitiveDateTimeExt};
use tokio::try_join;

use gcal_lib::gcal_instance::{compare_gcal_events, Event as GCalEvent, GCalendarInstance};

use crate::{
    calendar::{Calendar, Event},
    config::Config,
    models::{CalendarCache, CalendarList},
    parse_hashnyc::parse_hashnyc,
    parse_nycruns::parse_nycruns,
    pgpool::PgPool,
    timezone::TimeZone,
};

#[derive(Clone)]
pub struct CalendarSync {
    pub config: Config,
    pub gcal: Option<GCalendarInstance>,
    pub pool: PgPool,
    pub stdout: StdoutChannel<StackString>,
}

impl CalendarSync {
    pub async fn new(config: Config, pool: PgPool) -> Self {
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        )
        .await
        .ok();
        Self {
            config,
            gcal,
            pool,
            stdout: StdoutChannel::new(),
        }
    }

    /// # Errors
    /// Returns error if any `upsert` call fails
    pub async fn sync_calendar_list(&self) -> Result<Vec<CalendarList>, Error> {
        let calendar_list = self
            .gcal
            .as_ref()
            .ok_or_else(|| format_err!("No gcal instance found"))?
            .list_gcal_calendars()
            .await?;

        #[allow(clippy::manual_filter_map)]
        let futures = calendar_list
            .into_iter()
            .filter_map(|item| Calendar::from_gcal_entry(&item))
            .map(|calendar| async move {
                let cal: CalendarList = calendar.into();
                cal.upsert(&self.pool).await?;
                Ok(cal)
            });

        try_join_all(futures).await
    }

    async fn import_calendar_events<'a>(
        &'a self,
        gcal_id: &'a impl AsRef<str>,
        calendar_events: impl IntoIterator<Item = &'a GCalEvent>,
        upsert: bool,
    ) -> Result<Vec<CalendarCache>, Error> {
        let futures = calendar_events.into_iter().map(|item| async move {
            let gcal_id = gcal_id.as_ref();
            if item.start.is_none() {
                return Ok(None);
            } else if item.summary.is_none() {
                self.stdout
                    .send(format_sstr!("{:?} {:?}", item.start, item.description));
                return Ok(None);
            }
            let event: CalendarCache = Event::from_gcal_event(item, gcal_id)
                .ok_or_else(|| format_err!("Failed to convert event"))?
                .into();
            if upsert {
                event.upsert(&self.pool).await?;
                Ok(Some(event))
            } else if CalendarCache::get_by_gcal_id_event_id(gcal_id, &event.event_id, &self.pool)
                .await?
                .is_none()
            {
                event.insert(&self.pool).await?;
                Ok(Some(event))
            } else {
                Ok(None)
            }
        });
        let inserted: Result<Vec<_>, Error> = try_join_all(futures).await;
        Ok(inserted?.into_iter().flatten().collect())
    }

    async fn export_calendar_events<'a>(
        &self,
        calendar_events: impl IntoIterator<Item = &'a GCalEvent>,
        database_events: impl IntoIterator<Item = &'a CalendarCache>,
        update: bool,
    ) -> Result<Vec<GCalEvent>, Error> {
        let event_map: HashMap<_, _> = calendar_events
            .into_iter()
            .filter_map(|item| {
                let event_id: StackString = item.id.as_ref()?.into();
                Some((event_id, item))
            })
            .collect();
        let event_map = Arc::new(event_map);

        #[allow(clippy::manual_filter_map)]
        let futures = database_events.into_iter().map(|item| {
            let event_map = event_map.clone();
            async move {
                let event_id = item.event_id.as_str();
                let event: Event = item.clone().into();
                let (gcal_id, event) = event.to_gcal_event();
                if let Some(gcal_event) = event_map.get(event_id) {
                    let update = update
                        && gcal_event
                            .organizer
                            .as_ref()
                            .and_then(|o| o.email.as_deref())
                            != Some("unknownorganizer@calendar.google.com");
                    if !compare_gcal_events(gcal_event, &event) && update {
                        if let Ok(new_event) = self
                            .gcal
                            .as_ref()
                            .ok_or_else(|| format_err!("No gcal instance found"))?
                            .update_gcal_event(&gcal_id, event)
                            .await
                        {
                            Ok(Some(new_event))
                        } else {
                            Ok(None)
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(self
                        .gcal
                        .as_ref()
                        .ok_or_else(|| format_err!("No gcal instance found"))?
                        .insert_gcal_event(&gcal_id, event)
                        .await
                        .ok())
                }
            }
        });
        let result: Result<Vec<_>, Error> = try_join_all(futures).await;
        Ok(result?.into_iter().flatten().collect())
    }

    /// # Errors
    /// Returns error if api calls fail
    pub async fn sync_full_calendar(
        &self,
        gcal_id: &str,
        edit: bool,
    ) -> Result<(Vec<GCalEvent>, Vec<CalendarCache>), Error> {
        let calendar_events = self
            .gcal
            .as_ref()
            .ok_or_else(|| format_err!("No gcal instance found"))?
            .get_gcal_events(gcal_id, None, None)
            .await?;
        let exported = if edit {
            let database_events: Vec<_> =
                CalendarCache::get_by_gcal_id_datetime(gcal_id, None, None, &self.pool)
                    .await?
                    .try_collect()
                    .await?;
            self.export_calendar_events(&calendar_events, &database_events, false)
                .await?
        } else {
            Vec::new()
        };
        let imported = self
            .import_calendar_events(&gcal_id, calendar_events.iter(), false)
            .await?;
        Ok((exported, imported))
    }

    /// # Errors
    /// Returns error if api calls fail
    pub async fn sync_future_events(
        &self,
        gcal_id: &str,
        edit: bool,
    ) -> Result<(Vec<GCalEvent>, Vec<CalendarCache>), Error> {
        let calendar_events = self
            .gcal
            .as_ref()
            .ok_or_else(|| format_err!("No gcal instance found"))?
            .get_gcal_events(gcal_id, Some(OffsetDateTime::now_utc()), None)
            .await?;
        let exported = if edit {
            let database_events: Vec<_> = CalendarCache::get_by_gcal_id_datetime(
                gcal_id,
                Some(OffsetDateTime::now_utc()),
                None,
                &self.pool,
            )
            .await?
            .try_collect()
            .await?;
            self.export_calendar_events(&calendar_events, &database_events, true)
                .await?
        } else {
            Vec::new()
        };
        let imported = self
            .import_calendar_events(&gcal_id, calendar_events.iter(), true)
            .await?;
        Ok((exported, imported))
    }

    /// # Errors
    /// Returns error if api calls fail
    pub async fn run_syncing(&self, full: bool) -> Result<Vec<StackString>, Error> {
        let mut output = Vec::new();

        let hashnyc_future = parse_hashnyc(&self.pool);
        let nycruns_future = parse_nycruns(&self.pool);

        let (hashnyc_events, nycruns_events) = try_join!(hashnyc_future, nycruns_future)?;

        output.push(format_sstr!("parse_hashnyc {}", hashnyc_events.len()));
        output.push(format_sstr!("parse_nycruns {}", nycruns_events.len()));

        let inserted = self.sync_calendar_list().await?;
        output.push(format_sstr!("inserted {} calendars", inserted.len()));

        let gcal_set: HashSet<_> = inserted.iter().map(|cal| cal.gcal_id.clone()).collect();
        let gcal_set = Arc::new(gcal_set);

        let results: Result<Vec<_>, Error> = CalendarList::get_calendars(&self.pool)
            .await?
            .map_err(Into::into)
            .try_filter_map(|calendar| {
                let gcal_set = gcal_set.clone();
                async move {
                    if calendar.sync && gcal_set.contains(&calendar.gcal_id) {
                        let (exported, inserted) = if full {
                            self.sync_full_calendar(&calendar.gcal_id, calendar.edit)
                                .await?
                        } else {
                            debug!("gcal_id {}", calendar.gcal_id);
                            self.sync_future_events(&calendar.gcal_id, calendar.edit)
                                .await?
                        };
                        let result = format_sstr!(
                            "future events {} {} {}",
                            calendar.calendar_name,
                            exported.len(),
                            inserted.len()
                        );
                        Ok(Some(result))
                    } else {
                        Ok(None)
                    }
                }
            })
            .try_collect()
            .await;
        output.extend_from_slice(&results?);

        Ok(output)
    }

    /// # Errors
    /// Returns error if api calls fail
    pub async fn list_agenda(
        &self,
        days_before: i64,
        days_after: i64,
    ) -> Result<Vec<Event>, Error> {
        let min_time = OffsetDateTime::now_utc() - Duration::days(days_before);
        let max_time = OffsetDateTime::now_utc() + Duration::days(days_after);

        let (calendar_map, events) = try_join!(
            self.list_calendars(),
            CalendarCache::get_by_datetime(min_time, max_time, &self.pool)
        )?;

        let display_map: HashMap<_, _> = calendar_map
            .try_filter_map(|cal| async move {
                if cal.display {
                    Ok(Some((cal.gcal_id, cal.display)))
                } else {
                    Ok(None)
                }
            })
            .try_collect()
            .await?;
        let display_map = Arc::new(display_map);

        let events: Vec<Event> = events
            .try_filter_map(|event| {
                let display_map = display_map.clone();
                async move {
                    if display_map.get(&event.gcal_id).is_some_and(|x| *x) {
                        let event: Event = event.into();
                        Ok(Some(event))
                    } else {
                        Ok(None)
                    }
                }
            })
            .try_collect()
            .await?;

        Ok(events)
    }

    /// # Errors
    /// Returns error if `get_calendars` fails
    pub async fn list_calendars(
        &self,
    ) -> Result<impl Stream<Item = Result<Calendar, PqError>>, Error> {
        CalendarList::get_calendars(&self.pool)
            .await
            .map(|s| s.map_ok(Into::into))
    }

    /// # Errors
    /// Returns error if `get_by_gcal_id_datetime` fails
    pub async fn list_events(
        &self,
        gcal_id: &str,
        min_date: Option<Date>,
        max_date: Option<Date>,
    ) -> Result<Vec<Event>, Error> {
        let min_date = min_date
            .and_then(|d| d.with_hms(0, 0, 0).ok().map(PrimitiveDateTime::assume_utc))
            .unwrap_or_else(|| (OffsetDateTime::now_utc() - Duration::weeks(1)));
        let max_date = max_date
            .and_then(|d| d.checked_add(Duration::days(1)))
            .and_then(|d| {
                d.with_hms(0, 0, 0)
                    .ok()
                    .map(|dt| dt.assume_timezone(TimeZone::local().into()))
            })
            .map_or_else(
                || (OffsetDateTime::now_utc() + Duration::weeks(2)),
                |d| d.unwrap().to_timezone(TimeZone::utc().into()),
            );
        let mut events: Vec<Event> = CalendarCache::get_by_gcal_id_datetime(
            gcal_id,
            Some(min_date),
            Some(max_date),
            &self.pool,
        )
        .await?
        .map_ok(Into::into)
        .try_collect()
        .await?;
        events.sort_by_key(|event| event.start_time);
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use futures::TryStreamExt;

    use crate::{calendar_sync::CalendarSync, config::Config, pgpool::PgPool};

    #[tokio::test]
    async fn test_list_events() -> Result<(), Error> {
        let config = Config::init_config()?;
        let pool = PgPool::new(&config.database_url)?;

        let cal_sync = CalendarSync::new(config, pool).await;

        let calendars: Vec<_> = cal_sync.list_calendars().await?.try_collect().await?;
        println!("{}", calendars.len());
        assert!(calendars.len() > 0);

        Ok(())
    }
}
