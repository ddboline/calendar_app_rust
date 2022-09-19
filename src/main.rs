use anyhow::Error;

use calendar_app_lib::calendar_cli_opts::CalendarCliOpts;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    tokio::spawn(async move { CalendarCliOpts::parse_opts().await })
        .await
        .unwrap()
}
