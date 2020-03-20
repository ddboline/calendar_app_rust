use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_web::{web, App, HttpServer};
use chrono::Duration;
use std::time;
use tokio::time::interval;

use calendar_app_lib::calendar_sync::CalendarSync;
use calendar_app_lib::config::Config;
use calendar_app_lib::pgpool::PgPool;

use crate::logged_user::{fill_from_db, TRIGGER_DB_UPDATE};
use crate::routes::{
    agenda, calendar_index, delete_event, event_detail, list_calendars, list_events,
    sync_calendars, sync_calendars_full,
};

pub struct AppState {
    pub cal_sync: CalendarSync,
}

pub async fn start_app() {
    async fn _update_db(pool: PgPool) {
        let mut i = interval(time::Duration::from_secs(60));
        loop {
            i.tick().await;
            fill_from_db(&pool).await.unwrap_or(());
        }
    }
    TRIGGER_DB_UPDATE.set();
    let config = Config::init_config().expect("Failed to load config");
    let pool = PgPool::new(&config.database_url);
    let cal_sync = CalendarSync::new(config, pool);

    actix_rt::spawn(_update_db(cal_sync.pool.clone()));
    let port = cal_sync.config.port;

    HttpServer::new(move || {
        App::new()
            .data(AppState {
                cal_sync: cal_sync.clone(),
            })
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(cal_sync.config.secret_key.as_bytes())
                    .name("auth")
                    .path("/")
                    .domain(cal_sync.config.domain.as_str())
                    .max_age_time(Duration::days(1))
                    .secure(false),
            ))
            .service(web::resource("/calendar/index.html").route(web::get().to(calendar_index)))
            .service(web::resource("/calendar/agenda").route(web::get().to(agenda)))
            .service(web::resource("/calendar/sync_calendars").route(web::get().to(sync_calendars)))
            .service(
                web::resource("/calendar/sync_calendars_full")
                    .route(web::get().to(sync_calendars_full)),
            )
            .service(web::resource("/calendar/delete_event").route(web::delete().to(delete_event)))
            .service(web::resource("/calendar/list_calendars").route(web::get().to(list_calendars)))
            .service(web::resource("/calendar/list_events").route(web::get().to(list_events)))
            .service(web::resource("/calendar/event_detail").route(web::post().to(event_detail)))
    })
    .bind(&format!("127.0.0.1:{}", port))
    .unwrap_or_else(|_| panic!("Failed to bind to port {}", port))
    .run()
    .await
    .expect("Failed to start app");
}
