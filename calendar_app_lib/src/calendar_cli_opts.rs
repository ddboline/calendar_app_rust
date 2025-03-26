use anyhow::{Error, format_err};
use clap::Parser;
use futures::{TryStreamExt, future::try_join_all};
use refinery::embed_migrations;
use stack_string::{StackString, format_sstr};
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};
use tokio::{
    fs::{File, read},
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, stdin, stdout},
};

use crate::{
    DateType,
    calendar::Event,
    calendar_sync::CalendarSync,
    config::Config,
    models::{CalendarCache, CalendarList},
    pgpool::PgPool,
};

embed_migrations!("../migrations");

#[derive(Parser, Debug)]
pub enum CalendarActions {
    /// Print today's Agenda
    PrintAgenda,
    /// Sync all Calendars for future events
    SyncCalendars,
    /// Sync all Calendars for All events
    SyncCalendarsFull,
    /// Delete Calendar Event
    Delete {
        #[clap(short, long)]
        /// Google Calendar Id
        gcal_id: StackString,
        #[clap(short, long)]
        /// Google Event Id
        event_id: StackString,
    },
    /// List All Calendars
    ListCalendars,
    /// List Events in a Single Calendar
    List {
        #[clap(short, long)]
        /// Google Calendar Id
        gcal_id: StackString,
        #[clap(long, value_parser=DateType::parse_from_str)]
        /// Earliest date to consider (defaults to 1 week in the past)
        min_date: Option<DateType>,
        #[clap(long, value_parser=DateType::parse_from_str)]
        /// Latest date to consider (default to 1 week from today)
        max_date: Option<DateType>,
    },
    /// Display full details of an event
    Detail {
        #[clap(short, long)]
        /// Google Calendar Id
        gcal_id: StackString,
        #[clap(short, long)]
        /// Google Event Id
        event_id: StackString,
    },
    /// Import into table
    Import {
        #[clap(short, long)]
        /// Table name
        table: StackString,
        #[clap(short, long)]
        /// Input file (if missinge will read from stdin)
        filepath: Option<PathBuf>,
    },
    Export {
        #[clap(short, long)]
        /// Table name
        table: StackString,
        #[clap(short, long)]
        /// Input file (if missinge will read from stdin)
        filepath: Option<PathBuf>,
    },
    RunMigrations,
}

#[derive(Parser, Debug)]
pub struct CalendarCliOpts {
    #[clap(subcommand)]
    action: Option<CalendarActions>,
}

impl CalendarCliOpts {
    /// # Errors
    /// Returns error if api calls fail
    pub async fn parse_opts() -> Result<(), Error> {
        let opts = Self::parse();
        let action = opts.action.unwrap_or(CalendarActions::PrintAgenda);

        let config = Config::init_config()?;
        let pool = PgPool::new(&config.database_url)?;
        let cal_sync = CalendarSync::new(config, pool).await;

        match action {
            CalendarActions::PrintAgenda => {
                for event in cal_sync.list_agenda(1, 2).await? {
                    cal_sync.stdout.send(
                        event
                            .get_summary(&cal_sync.config.domain, &cal_sync.pool, &cal_sync.config)
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
                        .send(format_sstr!("delete {gcal_id} {event_id}"));
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
                let mut stream = Box::pin(cal_sync.list_calendars().await?);
                while let Some(calendar) = stream.try_next().await? {
                    cal_sync.stdout.send(format_sstr!("{calendar}"));
                }
            }
            CalendarActions::List {
                gcal_id,
                min_date,
                max_date,
            } => {
                for event in cal_sync
                    .list_events(&gcal_id, min_date.map(Into::into), max_date.map(Into::into))
                    .await?
                {
                    cal_sync.stdout.send(
                        event
                            .get_summary(&cal_sync.config.domain, &cal_sync.pool, &cal_sync.config)
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
                    let event_str = StackString::from_display(&event);
                    cal_sync.stdout.send(event_str);
                }
            }
            CalendarActions::Import { table, filepath } => {
                let data = if let Some(filepath) = filepath {
                    read(&filepath).await?
                } else {
                    let mut stdin = stdin();
                    let mut buf = Vec::new();
                    stdin.read_to_end(&mut buf).await?;
                    buf
                };
                match table.as_str() {
                    "calendar_list" => {
                        let calendars: Vec<CalendarList> = serde_json::from_slice(&data)?;
                        let futures = calendars.into_iter().map(|calendar| {
                            let pool = cal_sync.pool.clone();
                            async move { calendar.upsert(&pool).await }
                        });
                        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
                        cal_sync
                            .stdout
                            .send(format_sstr!("calendar_list {}", results?.len()));
                    }
                    "calendar_cache" => {
                        let events: Vec<CalendarCache> = serde_json::from_slice(&data)?;
                        let futures = events.into_iter().map(|event| {
                            let pool = cal_sync.pool.clone();
                            async move { event.upsert(&pool).await }
                        });
                        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
                        cal_sync
                            .stdout
                            .send(format_sstr!("calendar_cache {}", results?.len()));
                    }
                    _ => {}
                }
            }
            CalendarActions::Export { table, filepath } => {
                let mut file: Box<dyn AsyncWrite + Unpin + Send + Sync> =
                    if let Some(filepath) = filepath {
                        Box::new(File::create(&filepath).await?)
                    } else {
                        Box::new(stdout())
                    };
                match table.as_str() {
                    "calendar_list" => {
                        let max_modified = OffsetDateTime::now_utc() - Duration::days(7);
                        let calendars: Vec<_> = CalendarList::get_recent(
                            &cal_sync.pool,
                            Some(max_modified),
                            None,
                            None,
                        )
                        .await?
                        .try_collect()
                        .await?;
                        file.write_all(&serde_json::to_vec(&calendars)?).await?;
                    }
                    "calendar_cache" => {
                        let max_modified = OffsetDateTime::now_utc() - Duration::days(7);
                        let events: Vec<_> = CalendarCache::get_recent(
                            &cal_sync.pool,
                            Some(max_modified),
                            None,
                            None,
                        )
                        .await?
                        .try_collect()
                        .await?;
                        file.write_all(&serde_json::to_vec(&events)?).await?;
                    }
                    _ => {}
                }
            }
            CalendarActions::RunMigrations => {
                let mut client = cal_sync.pool.get().await?;
                migrations::runner().run_async(&mut **client).await?;
            }
        }
        cal_sync.stdout.close().await?;
        Ok(())
    }
}
