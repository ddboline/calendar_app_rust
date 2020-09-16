use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_web::{web, App, HttpServer};
use anyhow::Error;
use lazy_static::lazy_static;
use std::time::Duration;
use tokio::time::interval;

use calendar_app_lib::{calendar_sync::CalendarSync, config::Config, pgpool::PgPool};

use crate::{
    logged_user::{fill_from_db, get_secrets, SECRET_KEY, TRIGGER_DB_UPDATE},
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
    async fn _update_db(pool: PgPool) {
        let mut i = interval(Duration::from_secs(60));
        loop {
            fill_from_db(&pool).await.unwrap_or(());
            i.tick().await;
        }
    }
    TRIGGER_DB_UPDATE.set();
    get_secrets(&CONFIG.secret_path, &CONFIG.jwt_secret_path).await?;
    let pool = PgPool::new(&CONFIG.database_url);
    let cal_sync = CalendarSync::new(CONFIG.clone(), pool);

    actix_rt::spawn(_update_db(cal_sync.pool.clone()));
    let port = cal_sync.config.port;

    HttpServer::new(move || {
        App::new()
            .data(AppState {
                cal_sync: cal_sync.clone(),
            })
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&SECRET_KEY.load())
                    .name("auth")
                    .path("/")
                    .domain(cal_sync.config.domain.as_str())
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
