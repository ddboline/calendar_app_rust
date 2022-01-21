use anyhow::Error;
use rweb::{
    filters::BoxedFilter,
    http::header::CONTENT_TYPE,
    openapi::{self, Info},
    Filter, Reply,
};
use stack_string::{format_sstr, StackString};
use std::{collections::HashMap, fmt::Write, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::interval};

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

pub type UrlCache = RwLock<HashMap<StackString, StackString>>;

#[derive(Clone)]
pub struct AppState {
    pub cal_sync: CalendarSync,
    pub shortened_urls: Arc<UrlCache>,
}

pub async fn start_app() -> Result<(), Error> {
    let config = Config::init_config()?;
    get_secrets(&config.secret_path, &config.jwt_secret_path).await?;
    run_app(&config).await
}

fn get_calendar_path(app: &AppState) -> BoxedFilter<(impl Reply,)> {
    let calendar_index_path = calendar_index().boxed();
    let agenda_path = agenda(app.clone()).boxed();
    let sync_calendars_path = sync_calendars(app.clone()).boxed();

    let sync_calendars_full_path = sync_calendars_full(app.clone()).boxed();
    let delete_event_path = delete_event(app.clone()).boxed();
    let list_calendars_path = list_calendars(app.clone()).boxed();
    let list_events_path = list_events(app.clone()).boxed();
    let event_detail_path = event_detail(app.clone()).boxed();

    let calendar_list_get = calendar_list(app.clone()).boxed();
    let calendar_list_post = calendar_list_update(app.clone()).boxed();
    let calendar_list_path = calendar_list_get.or(calendar_list_post).boxed();

    let calendar_cache_get = calendar_cache(app.clone()).boxed();
    let calendar_cache_post = calendar_cache_update(app.clone()).boxed();
    let calendar_cache_path = calendar_cache_get.or(calendar_cache_post).boxed();

    let user_path = user().boxed();

    let link_path = link_shortener(app.clone()).boxed();

    let create_calendar_event_get = build_calendar_event(app.clone()).boxed();
    let create_calendar_event_post = create_calendar_event(app.clone()).boxed();
    let create_calendar_event_path = create_calendar_event_get
        .or(create_calendar_event_post)
        .boxed();

    let edit_calendar_path = edit_calendar(app.clone()).boxed();

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
        .or(edit_calendar_path)
        .boxed()
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
    let shortened_urls = Arc::new(RwLock::new(HashMap::new()));

    tokio::task::spawn(_update_db(cal_sync.pool.clone()));

    let app = AppState {
        cal_sync,
        shortened_urls,
    };

    let (spec, calendar_path) = openapi::spec()
        .info(Info {
            title: "Calendar Web App".into(),
            description: "Web App to Display Calendar, Sync with GCal".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Info::default()
        })
        .build(|| get_calendar_path(&app));

    let spec = Arc::new(spec);
    let spec_json_path = rweb::path!("calendar" / "openapi" / "json")
        .and(rweb::path::end())
        .map({
            let spec = spec.clone();
            move || rweb::reply::json(spec.as_ref())
        });

    let spec_yaml = serde_yaml::to_string(spec.as_ref())?;
    let spec_yaml_path = rweb::path!("calendar" / "openapi" / "yaml")
        .and(rweb::path::end())
        .map(move || {
            let reply = rweb::reply::html(spec_yaml.clone());
            rweb::reply::with_header(reply, CONTENT_TYPE, "text/yaml")
        });

    let routes = calendar_path
        .or(spec_json_path)
        .or(spec_yaml_path)
        .recover(error_response);
    let addr: SocketAddr = format_sstr!("{}:{}", config.host, config.port).parse()?;
    rweb::serve(routes).bind(addr).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use maplit::hashmap;
    use stack_string::format_sstr;
    use std::{
        env::{remove_var, set_var},
        fmt::Write,
    };

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

        let email = format_sstr!("{}@localhost", get_random_string(32));
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
        let url = format_sstr!("http://localhost:{auth_port}/api/auth");
        let data = hashmap! {
            "email" => &email,
            "password" => &password,
        };
        let result = client
            .post(url.as_str())
            .json(&data)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        println!("{}", result);

        let url = format_sstr!("http://localhost:{test_port}/calendar/index.html");
        let result = client
            .get(url.as_str())
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
