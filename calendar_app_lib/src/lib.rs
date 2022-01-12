#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::default_trait_access)]

pub mod calendar;
pub mod calendar_cli_opts;
pub mod calendar_sync;
pub mod config;
pub mod latitude;
pub mod longitude;
pub mod models;
pub mod parse_hashnyc;
pub mod parse_nycruns;
pub mod pgpool;
pub mod timezone;

use chrono::{DateTime, Local, Utc};
use chrono_tz::Tz;
use stack_string::StackString;

use crate::config::Config;

pub fn get_default_or_local_time(dt: DateTime<Utc>, config: &Config) -> StackString {
    match config.default_time_zone {
        Some(tz) => {
            let tz: Tz = tz.into();
            StackString::from_display(dt.with_timezone(&tz))
        }
        None => StackString::from_display(dt.with_timezone(&Local)),
    }
}
