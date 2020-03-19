use anyhow::Error;
use chrono::Utc;
use futures::future::try_join_all;
use tokio::task::spawn_blocking;

use gcal_lib::gcal_instance::{Event as GCalEvent, GCalendarInstance};

use crate::calendar::{Calendar, Event};
use crate::config::Config;
use crate::models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList};
use crate::parse_hashnyc::parse_hashnyc;
use crate::parse_nycruns::ParseNycRuns;
use crate::pgpool::PgPool;

#[derive(Clone)]
pub struct CalendarSync {
    pub config: Config,
    pub gcal: GCalendarInstance,
    pub pool: PgPool,
}

impl CalendarSync {
    pub fn new(config: Config, pool: PgPool) -> Self {
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        );
        Self { config, gcal, pool }
    }

    pub async fn sync_calendar_list(&self) -> Result<Vec<InsertCalendarList>, Error> {
        let calendar_list = {
            let gcal = self.gcal.clone();
            spawn_blocking(move || gcal.list_gcal_calendars()).await?
        }?;

        let futures = calendar_list.into_iter().map(|item| async move {
            if let Some(calendar) = Calendar::from_gcal_entry(&item) {
                let cal: InsertCalendarList = calendar.into();
                match cal.upsert(&self.pool).await {
                    Ok(ev) => Ok(Some(ev)),
                    Err(e) => Err(e),
                }
            } else {
                Ok(None)
            }
        });
        let result: Result<Vec<_>, Error> = try_join_all(futures).await;
        let inserted: Vec<_> = result?.into_iter().filter_map(|x| x).collect();
        Ok(inserted)
    }

    async fn sync_calendar_events(
        &self,
        gcal_id: &str,
        calendar_events: &[GCalEvent],
        upsert: bool,
    ) -> Result<Vec<InsertCalendarCache>, Error> {
        let futures = calendar_events.iter().map(|item| async move {
            if item.start.is_none() {
                return Ok(None);
            } else if item.summary.is_none() {
                println!("{:?} {:?}", item.start, item.description);
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

    pub async fn sync_full_calendar(
        &self,
        gcal_id: &str,
    ) -> Result<Vec<InsertCalendarCache>, Error> {
        let calendar_events = {
            let gcal = self.gcal.clone();
            let gcal_id = gcal_id.to_string();
            spawn_blocking(move || gcal.get_gcal_events(&gcal_id, None, None)).await?
        }?;
        self.sync_calendar_events(gcal_id, &calendar_events, false)
            .await
    }

    pub async fn sync_future_events(
        &self,
        gcal_id: &str,
    ) -> Result<Vec<InsertCalendarCache>, Error> {
        let calendar_events = {
            let gcal = self.gcal.clone();
            let gcal_id = gcal_id.to_string();
            spawn_blocking(move || gcal.get_gcal_events(&gcal_id, Some(Utc::now()), None)).await?
        }?;
        self.sync_calendar_events(gcal_id, &calendar_events, true)
            .await
    }

    pub async fn run_syncing(&self) -> Result<(), Error> {
        let inserted = self.sync_calendar_list().await?;
        println!("inserted {} caledars", inserted.len());
        let calendar_list = CalendarList::get_calendars(&self.pool).await?;
        for calendar in calendar_list {
            if !calendar.sync {
                continue;
            }
            println!("starting calendar {}", calendar.calendar_name);
            let inserted = self.sync_future_events(&calendar.gcal_id).await?;
            println!("{} {}", calendar.calendar_name, inserted.len());
        }

        let pool = self.pool.clone();
        let events = parse_hashnyc(&pool).await?;
        println!("events {:#?}", events);
        println!("events {}", events.len());

        let pool = self.pool.clone();
        let nycruns = ParseNycRuns::new(pool);
        let results = nycruns.parse_nycruns().await?;
        println!("{:#?}", results);

        Ok(())
    }
}
