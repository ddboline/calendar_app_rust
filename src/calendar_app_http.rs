#![allow(clippy::semicolon_if_nothing_returned)]

use anyhow::Error;

use calendar_app_http::app::start_app;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    start_app().await
}
