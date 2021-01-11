use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_web::{middleware::Compress, web, App, HttpServer};
use anyhow::Error;
use lazy_static::lazy_static;
use stack_string::StackString;
use std::time::Duration;
use tokio::time::interval;

use calendar_app_lib::{calendar_sync::CalendarSync, config::Config, pgpool::PgPool};

use crate::{
    logged_user::{fill_from_db, get_secrets, KEY_LENGTH, SECRET_KEY, TRIGGER_DB_UPDATE},
    routes::{
        agenda, build_calendar_event, calendar_cache, calendar_cache_update, calendar_index,
        calendar_list, calendar_list_update, create_calendar_event, delete_event, edit_calendar,
        event_detail, link_shortener, list_calendars, list_events, sync_calendars,
        sync_calendars_full, user,
    },
};

lazy_static! {
    pub static ref CONFIG: Config = Config::init_config().expect("Failed to load config");
}

pub struct AppState {
    pub cal_sync: CalendarSync,
}

pub async fn start_app() -> Result<(), Error> {
    let config = CONFIG.clone();
    get_secrets(&config.secret_path, &config.jwt_secret_path).await?;
    run_app(
        &config,
        config.port,
        SECRET_KEY.get(),
        config.domain.clone(),
    )
    .await
}

async fn run_app(
    config: &Config,
    port: u32,
    cookie_secret: [u8; KEY_LENGTH],
    domain: StackString,
) -> Result<(), Error> {
    async fn _update_db(pool: PgPool) {
        let mut i = interval(Duration::from_secs(60));
        loop {
            fill_from_db(&pool).await.unwrap_or(());
            i.tick().await;
        }
    }
    TRIGGER_DB_UPDATE.set();
    let pool = PgPool::new(&config.database_url);
    let cal_sync = CalendarSync::new(config.clone(), pool);

    actix_rt::spawn(_update_db(cal_sync.pool.clone()));

    HttpServer::new(move || {
        App::new()
            .data(AppState {
                cal_sync: cal_sync.clone(),
            })
            .wrap(Compress::default())
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&cookie_secret)
                    .name("auth")
                    .path("/")
                    .domain(domain.as_str())
                    .max_age(24 * 3600)
                    .secure(false),
            ))
            .service(
                web::scope("/calendar")
                    .service(web::resource("/index.html").route(web::get().to(calendar_index)))
                    .service(web::resource("/agenda").route(web::get().to(agenda)))
                    .service(web::resource("/sync_calendars").route(web::get().to(sync_calendars)))
                    .service(
                        web::resource("/sync_calendars_full")
                            .route(web::get().to(sync_calendars_full)),
                    )
                    .service(web::resource("/delete_event").route(web::delete().to(delete_event)))
                    .service(web::resource("/list_calendars").route(web::get().to(list_calendars)))
                    .service(web::resource("/list_events").route(web::get().to(list_events)))
                    .service(web::resource("/event_detail").route(web::post().to(event_detail)))
                    .service(
                        web::resource("/calendar_list")
                            .route(web::get().to(calendar_list))
                            .route(web::post().to(calendar_list_update)),
                    )
                    .service(
                        web::resource("/calendar_cache")
                            .route(web::get().to(calendar_cache))
                            .route(web::post().to(calendar_cache_update)),
                    )
                    .service(web::resource("/user").route(web::get().to(user)))
                    .service(web::resource("/link/{link}").route(web::get().to(link_shortener)))
                    .service(
                        web::resource("/create_calendar_event")
                            .route(web::get().to(build_calendar_event))
                            .route(web::post().to(create_calendar_event)),
                    )
                    .service(web::resource("/edit_calendar").route(web::get().to(edit_calendar))),
            )
    })
    .bind(&format!("127.0.0.1:{}", port))
    .unwrap_or_else(|_| panic!("Failed to bind to port {}", port))
    .run()
    .await
    .map_err(Into::into)
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

    #[actix_rt::test]
    async fn test_run_app() -> Result<(), Error> {
        set_var("TESTENV", "true");

        let email = format!("{}@localhost", get_random_string(32));
        let password = get_random_string(32);

        let config = Config::init_config()?;

        let mut secret_key = [0u8; KEY_LENGTH];
        secret_key.copy_from_slice(&get_random_key());

        JWT_SECRET.set(secret_key);
        SECRET_KEY.set(secret_key);

        let auth_port: u32 = 54321;
        actix_rt::spawn(async move {
            run_test_app(auth_port, secret_key, "localhost".into())
                .await
                .unwrap()
        });

        let test_port: u32 = 12345;
        actix_rt::spawn(async move {
            run_app(&config, test_port, secret_key, "localhost".into())
                .await
                .unwrap()
        });
        actix_rt::time::sleep(std::time::Duration::from_secs(10)).await;

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
