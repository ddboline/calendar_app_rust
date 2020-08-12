use anyhow::Error;
use chrono::{Duration, Local, NaiveDate, TimeZone, Utc};
use futures::future::try_join_all;
use itertools::Itertools;
use stack_string::StackString;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{task::spawn_blocking, try_join};

use gcal_lib::gcal_instance::{compare_gcal_events, Event as GCalEvent, GCalendarInstance};

use crate::{
    calendar::{Calendar, Event},
    config::Config,
    models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList},
    parse_hashnyc::parse_hashnyc,
    parse_nycruns::parse_nycruns,
    pgpool::PgPool,
    stdout_channel::StdoutChannel,
};

#[derive(Clone)]
pub struct CalendarSync {
    pub config: Config,
    pub gcal: GCalendarInstance,
    pub pool: PgPool,
    pub stdout: StdoutChannel,
}

impl CalendarSync {
    pub fn new(config: Config, pool: PgPool) -> Self {
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        );
        Self {
            config,
            gcal,
            pool,
            stdout: StdoutChannel::new(),
        }
    }

    pub async fn sync_calendar_list(&self) -> Result<Vec<InsertCalendarList>, Error> {
        let calendar_list = {
            let gcal = self.gcal.clone();
            spawn_blocking(move || gcal.list_gcal_calendars()).await?
        }?;

        #[allow(clippy::filter_map)]
        let futures = calendar_list
            .into_iter()
            .filter_map(|item| Calendar::from_gcal_entry(&item))
            .map(|calendar| async move {
                let cal: InsertCalendarList = calendar.into();
                cal.upsert(&self.pool).await
            });

        let result: Result<Vec<_>, Error> = try_join_all(futures).await;
        let inserted = result?;
        Ok(inserted)
    }

    async fn import_calendar_events(
        &self,
        gcal_id: &str,
        calendar_events: &[GCalEvent],
        upsert: bool,
    ) -> Result<Vec<InsertCalendarCache>, Error> {
        let futures = calendar_events.iter().map(|item| async move {
            if item.start.is_none() {
                return Ok(None);
            } else if item.summary.is_none() {
                self.stdout
                    .send(format!("{:?} {:?}", item.start, item.description).into())?;
                return Ok(None);
            }
            let event: InsertCalendarCache = Event::from_gcal_event(&item, &gcal_id)?.into();
            if upsert {
                let event = event.upsert(&self.pool).await?;
                Ok(Some(event))
            } else if CalendarCache::get_by_gcal_id_event_id(&gcal_id, &event.event_id, &self.pool)
                .await?
                .is_empty()
            {
                let event = event.insert(&self.pool).await?;
                Ok(Some(event))
            } else {
                Ok(None)
            }
        });
        let result: Result<Vec<_>, Error> = try_join_all(futures).await;
        let inserted: Vec<_> = result?.into_iter().filter_map(|x| x).collect();
        Ok(inserted)
    }

    async fn export_calendar_events(
        &self,
        calendar_events: &[GCalEvent],
        database_events: &[CalendarCache],
        update: bool,
    ) -> Result<Vec<GCalEvent>, Error> {
        let event_map: HashMap<_, _> = calendar_events
            .iter()
            .filter_map(|item| item.id.as_ref().map(|event_id| (event_id.as_str(), item)))
            .collect();
        let event_map = Arc::new(event_map);

        #[allow(clippy::filter_map)]
        let futures = database_events.iter().map(|item| {
            let event_map = event_map.clone();
            async move {
                let event_id = item.event_id.as_str();
                let event: Event = item.clone().into();
                let (gcal_id, event) = event.to_gcal_event()?;
                if let Some(gcal_event) = event_map.get(event_id) {
                    if !compare_gcal_events(gcal_event, &event) && update {
                        let gcal = self.gcal.clone();
                        Ok(Some(
                            spawn_blocking(move || gcal.update_gcal_event(&gcal_id, event))
                                .await??,
                        ))
                    } else {
                        Ok(None)
                    }
                } else {
                    let gcal = self.gcal.clone();
                    Ok(Some(
                        spawn_blocking(move || gcal.insert_gcal_event(&gcal_id, event)).await??,
                    ))
                }
            }
        });
        let result: Result<Vec<_>, Error> = try_join_all(futures).await;
        Ok(result?.into_iter().filter_map(|x| x).collect())
    }

    pub async fn sync_full_calendar(
        &self,
        gcal_id: &str,
        edit: bool,
    ) -> Result<(Vec<GCalEvent>, Vec<InsertCalendarCache>), Error> {
        let calendar_events = {
            let gcal = self.gcal.clone();
            let gcal_id = gcal_id.to_string();
            spawn_blocking(move || gcal.get_gcal_events(&gcal_id, None, None)).await?
        }?;
        let exported = if edit {
            let database_events =
                CalendarCache::get_by_gcal_id_datetime(gcal_id, None, None, &self.pool).await?;
            self.export_calendar_events(&calendar_events, &database_events, false)
                .await?
        } else {
            Vec::new()
        };
        let imported = self
            .import_calendar_events(gcal_id, &calendar_events, false)
            .await?;
        Ok((exported, imported))
    }

    pub async fn sync_future_events(
        &self,
        gcal_id: &str,
        edit: bool,
    ) -> Result<(Vec<GCalEvent>, Vec<InsertCalendarCache>), Error> {
        let calendar_events = {
            let gcal = self.gcal.clone();
            let gcal_id = gcal_id.to_string();
            spawn_blocking(move || gcal.get_gcal_events(&gcal_id, Some(Utc::now()), None)).await?
        }?;
        let exported = if edit {
            let database_events =
                CalendarCache::get_by_gcal_id_datetime(gcal_id, Some(Utc::now()), None, &self.pool)
                    .await?;
            self.export_calendar_events(&calendar_events, &database_events, true)
                .await?
        } else {
            Vec::new()
        };
        let imported = self
            .import_calendar_events(gcal_id, &calendar_events, true)
            .await?;
        Ok((exported, imported))
    }

    pub async fn run_syncing(&self, full: bool) -> Result<Vec<StackString>, Error> {
        let mut output = Vec::new();

        let hashnyc_future = parse_hashnyc(&self.pool);
        let nycruns_future = parse_nycruns(&self.pool);

        let (hashnyc_events, nycruns_events) = try_join!(hashnyc_future, nycruns_future)?;

        output.push(format!("parse_hashnyc {}", hashnyc_events.len()).into());
        output.push(format!("parse_nycruns {}", nycruns_events.len()).into());

        let inserted = self.sync_calendar_list().await?;
        output.push(format!("inserted {} caledars", inserted.len()).into());

        let calendar_list = CalendarList::get_calendars(&self.pool)
            .await?
            .into_iter()
            .filter(|calendar| calendar.sync);

        let futures = calendar_list.map(|calendar| {
            let mut output = Vec::new();
            async move {
                let (exported, inserted) = if full {
                    self.sync_full_calendar(&calendar.gcal_id, calendar.edit)
                        .await?
                } else {
                    self.sync_future_events(&calendar.gcal_id, calendar.edit)
                        .await?
                };
                output.push(
                    format!(
                        "future events {} {} {}",
                        calendar.calendar_name,
                        exported.len(),
                        inserted.len()
                    )
                    .into(),
                );
                Ok(output)
            }
        });
        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
        let results: Vec<_> = results?.into_iter().flatten().collect();
        output.extend_from_slice(&results);

        Ok(output)
    }

    pub async fn list_agenda(&self) -> Result<Vec<Event>, Error> {
        let min_time = Utc::now() - Duration::days(1);
        let max_time = Utc::now() + Duration::days(2);

        let (calendar_map, events) = try_join!(
            self.list_calendars(),
            CalendarCache::get_by_datetime(min_time, max_time, &self.pool)
        )?;

        let display_map: HashMap<_, _> = calendar_map
            .into_iter()
            .filter_map(|cal| {
                if cal.display {
                    Some((cal.gcal_id, cal.display))
                } else {
                    None
                }
            })
            .collect();

        let events: Vec<_> = events
            .into_iter()
            .filter(|event| display_map.get(&event.gcal_id).map_or(false, |x| *x))
            .sorted_by_key(|event| event.event_start_time)
            .map(Into::into)
            .collect();
        Ok(events)
    }

    pub async fn list_calendars(&self) -> Result<Vec<Calendar>, Error> {
        let calendars: Vec<_> = CalendarList::get_calendars(&self.pool)
            .await?
            .into_iter()
            .map(|c| c.into())
            .collect();
        Ok(calendars)
    }

    pub async fn list_events(
        &self,
        gcal_id: &str,
        min_date: Option<NaiveDate>,
        max_date: Option<NaiveDate>,
    ) -> Result<Vec<Event>, Error> {
        let min_date = min_date.map_or_else(
            || (Utc::now() - Duration::weeks(1)),
            |d| {
                Local
                    .from_local_datetime(&d.and_hms(0, 0, 0))
                    .single()
                    .unwrap()
                    .with_timezone(&Utc)
            },
        );
        let max_date = max_date.map_or_else(
            || (Utc::now() + Duration::weeks(2)),
            |d| {
                Local
                    .from_local_datetime(&d.and_hms(0, 0, 0))
                    .single()
                    .unwrap()
                    .with_timezone(&Utc)
            },
        );
        let events: Vec<_> = CalendarCache::get_by_gcal_id_datetime(
            &gcal_id,
            Some(min_date),
            Some(max_date),
            &self.pool,
        )
        .await?
        .into_iter()
        .sorted_by_key(|event| event.event_start_time)
        .map(|c| c.into())
        .collect();
        Ok(events)
    }
}
