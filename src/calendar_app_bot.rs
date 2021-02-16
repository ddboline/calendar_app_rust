use anyhow::Error;

use calendar_app_bot::telegram_bot::TelegramBot;
use calendar_app_lib::{config::Config, pgpool::PgPool};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    let config = Config::init_config()?;
    let pool = PgPool::new(&config.database_url);
    if let Some(telegram_bot_token) = config.telegram_bot_token.as_ref() {
        let bot = TelegramBot::new(telegram_bot_token, &pool, &config).await;
        bot.run().await?;
    }
    Ok(())
}
