use axum::http::{Method, StatusCode};
use log::debug;
use stack_string::{StackString, format_sstr};
use std::{collections::HashMap, convert::TryInto, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::RwLock, time::interval};
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use calendar_app_lib::{calendar_sync::CalendarSync, config::Config, pgpool::PgPool};

use crate::{
    errors::ServiceError as Error,
    logged_user::{fill_from_db, get_secrets},
    routes::{ApiDoc, get_calendar_path},
};

pub type UrlCache = RwLock<HashMap<StackString, StackString>>;

#[derive(Clone)]
pub struct AppState {
    pub cal_sync: CalendarSync,
    pub shortened_urls: Arc<UrlCache>,
}

/// # Errors
/// Returns error if `init_config` or `get_secrets` fail
pub async fn start_app() -> Result<(), Error> {
    let config = Config::init_config()?;
    get_secrets(&config.secret_path, &config.jwt_secret_path).await?;
    run_app(&config).await
}

async fn run_app(config: &Config) -> Result<(), Error> {
    async fn update_db(pool: PgPool) {
        let mut i = interval(Duration::from_secs(60));
        loop {
            fill_from_db(&pool).await.unwrap_or(());
            i.tick().await;
        }
    }
    let pool = PgPool::new(&config.database_url)?;
    let cal_sync = CalendarSync::new(config.clone(), pool).await;
    let shortened_urls = Arc::new(RwLock::new(HashMap::new()));

    tokio::task::spawn(update_db(cal_sync.pool.clone()));

    let app = AppState {
        cal_sync,
        shortened_urls,
    };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(["content-type".try_into()?, "jwt".try_into()?])
        .allow_origin(Any);

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(get_calendar_path(&app))
        .split_for_parts();

    let spec_json = serde_json::to_string_pretty(&api)?;
    let spec_yaml = serde_yml::to_string(&api)?;

    let router = router
        .route(
            "/calendar/openapi/json",
            axum::routing::get(|| async move {
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    spec_json,
                )
            }),
        )
        .route(
            "/calendar/openapi/yaml",
            axum::routing::get(|| async move {
                (StatusCode::OK, [("content-type", "text/yaml")], spec_yaml)
            }),
        )
        .layer(cors);

    let host = &config.host;
    let port = config.port;

    let addr: SocketAddr = format_sstr!("{host}:{port}").parse()?;
    debug!("{addr:?}");
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, router.into_make_service())
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use maplit::hashmap;
    use stack_string::format_sstr;
    use std::env::{remove_var, set_var};

    use auth_server_http::app::run_test_app;
    use auth_server_lib::get_random_string;

    use calendar_app_lib::config::Config;

    use crate::{
        app::run_app,
        logged_user::{JWT_SECRET, KEY_LENGTH, SECRET_KEY, get_random_key},
    };

    #[tokio::test]
    async fn test_run_app() -> Result<(), Error> {
        unsafe {
            set_var("TESTENV", "true");
        }

        let email = format_sstr!("{}@localhost", get_random_string(32));
        let password = get_random_string(32);

        let auth_port: u32 = 54321;
        unsafe {
            set_var("PORT", auth_port.to_string());
            set_var("DOMAIN", "localhost");
        }

        let config = auth_server_lib::config::Config::init_config()?;

        let mut secret_key = [0u8; KEY_LENGTH];
        secret_key.copy_from_slice(&get_random_key());

        JWT_SECRET.set(secret_key);
        SECRET_KEY.set(secret_key);

        println!("spawning auth");
        tokio::task::spawn(async move { run_test_app(config).await.unwrap() });

        let test_port: u32 = 12345;
        unsafe {
            set_var("PORT", test_port.to_string());
        }
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

        let url = format_sstr!("http://localhost:{test_port}/calendar/openapi/yaml");
        let spec_yaml = client
            .get(url.as_str())
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        std::fs::write("../scripts/openapi.yaml", &spec_yaml)?;

        unsafe {
            remove_var("TESTENV");
        }
        Ok(())
    }
}
