use anyhow::Error;
use stack_string::StackString;
use std::{net::SocketAddr, time::Duration};
use tokio::time::interval;
use warp::Filter;

use calendar_app_lib::{calendar_sync::CalendarSync, config::Config, pgpool::PgPool};

use crate::{
    errors::error_response,
    logged_user::{fill_from_db, get_secrets, TRIGGER_DB_UPDATE},
    routes::{
        agenda, build_calendar_event, calendar_cache, calendar_cache_update, calendar_index,
        calendar_list, calendar_list_update, create_calendar_event, delete_event, edit_calendar,
        event_detail, link_shortener, list_calendars, list_events, sync_calendars,
        sync_calendars_full, user,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub cal_sync: CalendarSync,
}

pub async fn start_app() -> Result<(), Error> {
    let config = Config::init_config()?;
    get_secrets(&config.secret_path, &config.jwt_secret_path).await?;
    run_app(&config).await
}

async fn run_app(config: &Config) -> Result<(), Error> {
    async fn _update_db(pool: PgPool) {
        let mut i = interval(Duration::from_secs(60));
        loop {
            fill_from_db(&pool).await.unwrap_or(());
            i.tick().await;
        }
    }
    TRIGGER_DB_UPDATE.set();
    let pool = PgPool::new(&config.database_url);
    let cal_sync = CalendarSync::new(config.clone(), pool).await;

    tokio::task::spawn(_update_db(cal_sync.pool.clone()));

    let app = AppState {
        cal_sync: cal_sync.clone(),
    };

    let data = warp::any().map(move || app.clone());

    let calendar_index_path = warp::path("index.html")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and_then(calendar_index);
    let agenda_path = warp::path("agenda")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(agenda);
    let sync_calendars_path = warp::path("sync_calendars")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(sync_calendars);
    let sync_calendars_full_path = warp::path("sync_calendars_full")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(sync_calendars_full);
    let delete_event_path = warp::path("delete_event")
        .and(warp::path::end())
        .and(warp::delete())
        .and(warp::body::json())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(delete_event);
    let list_calendars_path = warp::path("list_calendars")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(list_calendars);
    let list_events_path = warp::path("list_events")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(list_events);
    let event_detail_path = warp::path("event_detail")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(event_detail);

    let calendar_list_get = warp::get()
        .and(warp::path::end())
        .and(warp::query())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(calendar_list);
    let calendar_list_post = warp::post()
        .and(warp::path::end())
        .and(warp::body::json())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(calendar_list_update);
    let calendar_list_path =
        warp::path("calendar_list").and(calendar_list_get.or(calendar_list_post));

    let calendar_cache_get = warp::get()
        .and(warp::path::end())
        .and(warp::query())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(calendar_cache);
    let calendar_cache_post = warp::post()
        .and(warp::path::end())
        .and(warp::body::json())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(calendar_cache_update);
    let calendar_cache_path =
        warp::path("calendar_cache").and(calendar_cache_get.or(calendar_cache_post));

    let user_path = warp::path("user")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::cookie("jwt"))
        .and_then(user);

    let link_path = warp::path!("link" / StackString)
        .and(warp::get())
        .and(warp::path::end())
        .and(data.clone())
        .and_then(link_shortener);

    let create_calendar_event_get = warp::get()
        .and(warp::path::end())
        .and(warp::query())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(build_calendar_event);
    let create_calendar_event_post = warp::post()
        .and(warp::path::end())
        .and(warp::body::json())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(create_calendar_event);
    let create_calendar_event_path = warp::path("create_calendar_event")
        .and(create_calendar_event_get.or(create_calendar_event_post));

    let edit_calendar_path = warp::path("edit_calendar")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query())
        .and(warp::cookie("jwt"))
        .and(data.clone())
        .and_then(edit_calendar);

    let calendar_path = warp::path("calendar").and(
        calendar_index_path
            .or(agenda_path)
            .or(sync_calendars_path)
            .or(sync_calendars_full_path)
            .or(delete_event_path)
            .or(list_calendars_path)
            .or(list_events_path)
            .or(event_detail_path)
            .or(calendar_list_path)
            .or(calendar_cache_path)
            .or(user_path)
            .or(link_path)
            .or(create_calendar_event_path)
            .or(edit_calendar_path),
    );

    let routes = calendar_path.recover(error_response);
    let addr: SocketAddr = format!("127.0.0.1:{}", config.port).parse()?;
    warp::serve(routes).bind(addr).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use maplit::hashmap;
    use std::env::{remove_var, set_var};

    use auth_server_http::app::run_test_app;
    use auth_server_lib::get_random_string;

    use calendar_app_lib::config::Config;

    use crate::{
        app::run_app,
        logged_user::{get_random_key, JWT_SECRET, KEY_LENGTH, SECRET_KEY},
    };

    #[tokio::test]
    async fn test_run_app() -> Result<(), Error> {
        set_var("TESTENV", "true");

        let email = format!("{}@localhost", get_random_string(32));
        let password = get_random_string(32);

        let auth_port: u32 = 54321;
        set_var("PORT", auth_port.to_string());
        set_var("DOMAIN", "localhost");

        let config = auth_server_lib::config::Config::init_config()?;

        let mut secret_key = [0u8; KEY_LENGTH];
        secret_key.copy_from_slice(&get_random_key());

        JWT_SECRET.set(secret_key);
        SECRET_KEY.set(secret_key);

        println!("spawning auth");
        tokio::task::spawn(async move { run_test_app(config).await.unwrap() });

        let test_port: u32 = 12345;
        set_var("PORT", test_port.to_string());
        let config = Config::init_config()?;

        tokio::task::spawn(async move { run_app(&config).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        let client = reqwest::Client::builder().cookie_store(true).build()?;
        let url = format!("http://localhost:{}/api/auth", auth_port);
        let data = hashmap! {
            "email" => &email,
            "password" => &password,
        };
        let result = client
            .post(&url)
            .json(&data)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        println!("{}", result);

        let url = format!("http://localhost:{}/calendar/index.html", test_port);
        let result = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        println!("{}", result);
        assert!(result.len() > 0);
        assert!(result.contains("Calendar"));

        remove_var("TESTENV");
        Ok(())
    }
}
