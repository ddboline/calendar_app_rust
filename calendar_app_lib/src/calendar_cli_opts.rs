use anyhow::Error;
use chrono::NaiveDate;
use chrono::{Duration, Local, TimeZone, Utc};
use structopt::StructOpt;
use tokio::task::spawn_blocking;

use crate::calendar::{Calendar, Event};
use crate::calendar_sync::CalendarSync;
use crate::config::Config;
use crate::models::{CalendarCache, CalendarList};
use crate::pgpool::PgPool;

#[derive(StructOpt, Debug)]
pub enum CalendarActions {
    /// Print today's Agenda
    PrintAgenda,
    /// Sync all Calendars for future events
    SyncCalendars,
    /// Sync all Calendars for All events
    SyncCalendarsFull,
    /// Delete Calendar Event
    Delete {
        #[structopt(short, long)]
        /// Google Calendar Id
        gcal_id: String,
        #[structopt(short, long)]
        /// Google Event Id
        event_id: String,
    },
    /// List All Calendars
    ListCalendars,
    /// List Events in a Single Calendar
    List {
        #[structopt(short, long)]
        /// Google Calendar Id
        gcal_id: String,
        #[structopt(long)]
        /// Earliest date to consider (defaults to 1 week in the past)
        min_date: Option<NaiveDate>,
        #[structopt(long)]
        /// Latest date to consider (default to 1 week from today)
        max_date: Option<NaiveDate>,
    },
}

#[derive(StructOpt, Debug)]
pub struct CalendarCliOpts {
    #[structopt(subcommand)]
    action: Option<CalendarActions>,
}

impl CalendarCliOpts {
    pub async fn parse_opts() -> Result<(), Error> {
        let opts = Self::from_args();
        let action = opts.action.unwrap_or(CalendarActions::PrintAgenda);
        println!("{:?}", action);

        let config = Config::init_config()?;
        let pool = PgPool::new(&config.database_url);
        let cal_sync = CalendarSync::new(config, pool);

        match action {
            CalendarActions::PrintAgenda => {
                list_agenda(&cal_sync.pool).await?;
            }
            CalendarActions::SyncCalendars => {
                cal_sync.run_syncing().await?;
            }
            CalendarActions::SyncCalendarsFull => {
                for calendar in cal_sync.sync_calendar_list().await? {
                    cal_sync.sync_full_calendar(&calendar.gcal_id).await?;
                }
            }
            CalendarActions::Delete { gcal_id, event_id } => {
                {
                    spawn_blocking(move || cal_sync.gcal.delete_gcal_event(&gcal_id, &event_id))
                        .await?
                }?;
            }
            CalendarActions::ListCalendars => {
                let calendar_list = CalendarList::get_calendars(&cal_sync.pool).await?;
                for calendar in calendar_list {
                    let calendar: Calendar = calendar.into();
                    println!("{}", calendar);
                }
            }
            CalendarActions::List {
                gcal_id,
                min_date,
                max_date,
            } => {
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
                    || (Utc::now() + Duration::weeks(1)),
                    |d| {
                        Local
                            .from_local_datetime(&d.and_hms(0, 0, 0))
                            .single()
                            .unwrap()
                            .with_timezone(&Utc)
                    },
                );
                let events = CalendarCache::get_by_gcal_id_datetime(
                    &gcal_id,
                    min_date,
                    max_date,
                    &cal_sync.pool,
                )
                .await?;
                for event in events {
                    let event: Event = event.into();
                    println!("{}", event);
                }
            }
        }

        Ok(())
    }
}

async fn list_agenda(pool: &PgPool) -> Result<(), Error> {
    let min_time = Utc::now() - Duration::days(1);
    let max_time = Utc::now() + Duration::days(1);
    let events = CalendarCache::get_by_datetime(min_time, max_time, &pool).await?;
    for event in events {
        let event: Event = event.into();
        println!("{}", event);
    }
    Ok(())
}
