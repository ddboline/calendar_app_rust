use anyhow::{format_err, Error};
use serde::Deserialize;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use stack_string::StackString;

#[derive(Default, Debug, Deserialize)]
pub struct ConfigInner {
    pub database_url: StackString,
    #[serde(default = "default_gcal_secret")]
    pub gcal_secret_file: PathBuf,
    #[serde(default = "default_gcal_token_path")]
    pub gcal_token_path: PathBuf,
    #[serde(default = "default_secret_key")]
    pub secret_key: StackString,
    #[serde(default = "default_domain")]
    pub domain: StackString,
    #[serde(default = "default_port")]
    pub port: u32,
    #[serde(default = "default_n_db_workers")]
    pub n_db_workers: usize,
    pub telegram_bot_token: Option<StackString>,
}

#[derive(Default, Debug, Clone)]
pub struct Config(Arc<ConfigInner>);

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
fn default_port() -> u32 {
    4042
}
fn default_secret_key() -> StackString {
    "0123".repeat(8).into()
}
fn default_domain() -> StackString {
    "localhost".into()
}
fn default_n_db_workers() -> usize {
    2
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
