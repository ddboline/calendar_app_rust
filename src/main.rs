use anyhow::Error;

use calendar_app_lib::calendar_cli_opts::CalendarCliOpts;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    CalendarCliOpts::parse_opts().await
}
