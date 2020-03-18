use anyhow::Error;

use calendar_app_lib::calendar_sync::CalendarSync;
use calendar_app_lib::config::Config;
use calendar_app_lib::models::CalendarList;
use calendar_app_lib::parse_hashnyc::parse_hashnyc;
use calendar_app_lib::parse_nycruns::ParseNycRuns;
use calendar_app_lib::pgpool::PgPool;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::init_config().unwrap();
    let pool = PgPool::new(&config.database_url);
    let nycruns = ParseNycRuns::new(pool);
    nycruns.parse_nycruns().await?;
    Ok(())
}

async fn run_syncing() -> Result<(), Error> {
    let config = Config::init_config().unwrap();
    let pool = PgPool::new(&config.database_url);
    let cal_sync = CalendarSync::new(config, pool);
    let inserted = cal_sync.sync_calendar_list().await?;
    println!("inserted {} caledars", inserted.len());
    let calendar_list = CalendarList::get_calendars(&cal_sync.pool).await?;
    for calendar in calendar_list {
        if !calendar.sync {
            continue;
        }
        println!("starting calendar {}", calendar.calendar_name);
        let inserted = cal_sync.sync_future_events(&calendar.gcal_id).await?;
        println!("{} {}", calendar.calendar_name, inserted.len());
    }

    let pool = cal_sync.pool.clone();
    let events = parse_hashnyc(&pool).await?;
    println!("events {:#?}", events);
    println!("events {}", events.len());
    Ok(())
}
