use anyhow::{format_err, Error};
use serde::Deserialize;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use stack_string::StackString;

use crate::timezone::TimeZone;

#[derive(Default, Debug, Deserialize)]
pub struct ConfigInner {
    #[serde(default = "default_database_url")]
    pub database_url: StackString,
    #[serde(default = "default_gcal_secret")]
    pub gcal_secret_file: PathBuf,
    #[serde(default = "default_gcal_token_path")]
    pub gcal_token_path: PathBuf,
    #[serde(default = "default_domain")]
    pub domain: StackString,
    #[serde(default = "default_host")]
    pub host: StackString,
    #[serde(default = "default_port")]
    pub port: u32,
    #[serde(default = "default_n_db_workers")]
    pub n_db_workers: usize,
    pub telegram_bot_token: Option<StackString>,
    pub default_time_zone: Option<TimeZone>,
    #[serde(default = "default_secret_path")]
    pub secret_path: PathBuf,
    #[serde(default = "default_secret_path")]
    pub jwt_secret_path: PathBuf,
}

#[derive(Default, Debug, Clone)]
pub struct Config(Arc<ConfigInner>);

fn default_database_url() -> StackString {
    "postgresql://user:password@host:1234/test_db".into()
}
fn default_gcal_secret() -> PathBuf {
    let config_dir = dirs::config_dir().expect("No CONFIG directory");
    config_dir
        .join("calendar_app_rust")
        .join("client_secrets.json")
}
fn default_gcal_token_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("No HOME directory");
    home_dir.join(".gcal")
}
fn default_host() -> StackString {
    "0.0.0.0".into()
}
fn default_port() -> u32 {
    4042
}
fn default_domain() -> StackString {
    "localhost".into()
}
fn default_n_db_workers() -> usize {
    2
}
fn default_secret_path() -> PathBuf {
    dirs::config_dir()
        .expect("No CONFIG directory")
        .join("aws_app_rust")
        .join("secret.bin")
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init_config() -> Result<Self, Error> {
        let fname = Path::new("config.env");
        let config_dir = dirs::config_dir().ok_or_else(|| format_err!("No CONFIG directory"))?;
        let default_fname = config_dir.join("calendar_app_rust").join("config.env");

        let env_file = if fname.exists() {
            fname
        } else {
            &default_fname
        };

        dotenv::dotenv().ok();

        if env_file.exists() {
            dotenv::from_path(env_file).ok();
        }

        let conf: ConfigInner = envy::from_env()?;

        Ok(Self(Arc::new(conf)))
    }
}

impl Deref for Config {
    type Target = ConfigInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
