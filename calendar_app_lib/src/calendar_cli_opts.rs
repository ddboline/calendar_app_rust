use anyhow::{format_err, Error};
use chrono::{Duration, NaiveDate, Utc};
use futures::future::try_join_all;
use stack_string::StackString;
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::{
    fs::{read_to_string, File},
    io::{stdin, stdout, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

use crate::{
    calendar::Event,
    calendar_sync::CalendarSync,
    config::Config,
    models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList},
    pgpool::PgPool,
};

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
        gcal_id: StackString,
        #[structopt(short, long)]
        /// Google Event Id
        event_id: StackString,
    },
    /// List All Calendars
    ListCalendars,
    /// List Events in a Single Calendar
    List {
        #[structopt(short, long)]
        /// Google Calendar Id
        gcal_id: StackString,
        #[structopt(long)]
        /// Earliest date to consider (defaults to 1 week in the past)
        min_date: Option<NaiveDate>,
        #[structopt(long)]
        /// Latest date to consider (default to 1 week from today)
        max_date: Option<NaiveDate>,
    },
    /// Display full details of an event
    Detail {
        #[structopt(short, long)]
        /// Google Calendar Id
        gcal_id: StackString,
        #[structopt(short, long)]
        /// Google Event Id
        event_id: StackString,
    },
    /// Import into table
    Import {
        #[structopt(short, long)]
        /// Table name
        table: StackString,
        #[structopt(short, long)]
        /// Input file (if missinge will read from stdin)
        filepath: Option<PathBuf>,
    },
    Export {
        #[structopt(short, long)]
        /// Table name
        table: StackString,
        #[structopt(short, long)]
        /// Input file (if missinge will read from stdin)
        filepath: Option<PathBuf>,
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

        let config = Config::init_config()?;
        let pool = PgPool::new(&config.database_url);
        let cal_sync = CalendarSync::new(config, pool).await;

        match action {
            CalendarActions::PrintAgenda => {
                for event in cal_sync.list_agenda(1, 2).await? {
                    cal_sync.stdout.send(
                        event
                            .get_summary(
                                &cal_sync.config.domain,
                                &cal_sync.pool,
                                cal_sync.config.default_time_zone,
                            )
                            .await,
                    );
                }
            }
            CalendarActions::SyncCalendars => {
                for line in cal_sync.run_syncing(false).await? {
                    cal_sync.stdout.send(line);
                }
            }
            CalendarActions::SyncCalendarsFull => {
                for line in cal_sync.run_syncing(true).await? {
                    cal_sync.stdout.send(line);
                }
            }
            CalendarActions::Delete { gcal_id, event_id } => {
                {
                    cal_sync
                        .stdout
                        .send(format!("delete {} {}", gcal_id, event_id));
                    if let Some(event) =
                        CalendarCache::get_by_gcal_id_event_id(&gcal_id, &event_id, &cal_sync.pool)
                            .await?
                    {
                        event.delete(&cal_sync.pool).await?;
                    }
                    cal_sync
                        .gcal
                        .as_ref()
                        .ok_or_else(|| format_err!("No gcal instance found"))?
                        .delete_gcal_event(&gcal_id, &event_id)
                        .await?;
                };
            }
            CalendarActions::ListCalendars => {
                for calendar in cal_sync.list_calendars().await? {
                    cal_sync.stdout.send(format!("{}", calendar));
                }
            }
            CalendarActions::List {
                gcal_id,
                min_date,
                max_date,
            } => {
                for event in cal_sync.list_events(&gcal_id, min_date, max_date).await? {
                    cal_sync.stdout.send(
                        event
                            .get_summary(
                                &cal_sync.config.domain,
                                &cal_sync.pool,
                                cal_sync.config.default_time_zone,
                            )
                            .await,
                    );
                }
            }
            CalendarActions::Detail { gcal_id, event_id } => {
                if let Some(event) =
                    CalendarCache::get_by_gcal_id_event_id(&gcal_id, &event_id, &cal_sync.pool)
                        .await?
                {
                    let event: Event = event.into();
                    cal_sync.stdout.send(event.to_string());
                }
            }
            CalendarActions::Import { table, filepath } => {
                let data = if let Some(filepath) = filepath {
                    read_to_string(&filepath).await?
                } else {
                    let mut stdin = stdin();
                    let mut buf = String::new();
                    stdin.read_to_string(&mut buf).await?;
                    buf
                };
                match table.as_str() {
                    "calendar_list" => {
                        let calendars: Vec<CalendarList> = serde_json::from_str(&data)?;
                        let futures = calendars.into_iter().map(|calendar| {
                            let pool = cal_sync.pool.clone();
                            let calendar: InsertCalendarList = calendar.into();
                            async move { calendar.upsert(&pool).await.map_err(Into::into) }
                        });
                        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
                        cal_sync
                            .stdout
                            .send(format!("calendar_list {}", results?.len()));
                    }
                    "calendar_cache" => {
                        let events: Vec<CalendarCache> = serde_json::from_str(&data)?;
                        let futures = events.into_iter().map(|event| {
                            let pool = cal_sync.pool.clone();
                            let event: InsertCalendarCache = event.into();
                            async move { event.upsert(&pool).await.map_err(Into::into) }
                        });
                        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
                        cal_sync
                            .stdout
                            .send(format!("calendar_cache {}", results?.len()));
                    }
                    _ => {}
                }
            }
            CalendarActions::Export { table, filepath } => {
                let mut file: Box<dyn AsyncWrite + Unpin> = if let Some(filepath) = filepath {
                    Box::new(File::create(&filepath).await?)
                } else {
                    Box::new(stdout())
                };
                match table.as_str() {
                    "calendar_list" => {
                        let max_modified = Utc::now() - Duration::days(7);
                        let calendars =
                            CalendarList::get_recent(max_modified, &cal_sync.pool).await?;
                        file.write_all(&serde_json::to_vec(&calendars)?).await?;
                    }
                    "calendar_cache" => {
                        let max_modified = Utc::now() - Duration::days(7);
                        let events =
                            CalendarCache::get_recent(max_modified, &cal_sync.pool).await?;
                        file.write_all(&serde_json::to_vec(&events)?).await?;
                    }
                    _ => {}
                }
            }
        }
        cal_sync.stdout.close().await?;
        Ok(())
    }
}
