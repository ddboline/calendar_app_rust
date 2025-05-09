#![allow(clippy::semicolon_if_nothing_returned)]

use calendar_app_http::{app::start_app, errors::ServiceError as Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    tokio::spawn(async move { start_app().await })
        .await
        .unwrap()
}
