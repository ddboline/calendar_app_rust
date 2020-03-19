use anyhow::Error;

use calendar_app_lib::calendar_cli_opts::CalendarCliOpts;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opts = CalendarCliOpts::parse_opts().await?;
    Ok(())
}
