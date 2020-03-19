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
                for event in cal_sync.list_agenda().await? {
                    println!("{}", event);
                }
            }
            CalendarActions::SyncCalendars => {
                for line in cal_sync.run_syncing().await? {
                    println!("{}", line);
                }
            }
            CalendarActions::SyncCalendarsFull => {
                for calendar in cal_sync.sync_calendar_list().await? {
                    let events = cal_sync.sync_full_calendar(&calendar.gcal_id).await?;
                    println!("{} {}", calendar.calendar_name, events.len());
                }
            }
            CalendarActions::Delete { gcal_id, event_id } => {
                {
                    println!("delete {} {}", gcal_id, event_id);
                    spawn_blocking(move || cal_sync.gcal.delete_gcal_event(&gcal_id, &event_id))
                        .await?
                }?;
            }
            CalendarActions::ListCalendars => {
                for calendar in cal_sync.list_calendars().await? {
                    println!("{}", calendar);
                }
            }
            CalendarActions::List {
                gcal_id,
                min_date,
                max_date,
            } => {
                for event in cal_sync.list_events(&gcal_id, min_date, max_date).await? {
                    println!("{}", event);
                }
            }
        }

        Ok(())
    }
}
