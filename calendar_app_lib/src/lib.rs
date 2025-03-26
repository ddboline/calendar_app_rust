#![allow(clippy::too_many_lines)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]

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

use anyhow::Error;
use derive_more::{From, Into};
use stack_string::StackString;
use std::str::FromStr;
use time::{
    Date, OffsetDateTime, format_description::well_known::Rfc3339, macros::format_description,
};
use time_tz::OffsetDateTimeExt;

use crate::{config::Config, timezone::TimeZone};

#[must_use]
pub fn get_default_or_local_time(dt: OffsetDateTime, config: &Config) -> StackString {
    let tz = config.default_time_zone.unwrap_or_else(TimeZone::local);
    match dt.to_timezone(tz.into()).format(&Rfc3339) {
        Ok(s) => s.into(),
        Err(_) => unreachable!(),
    }
}

#[derive(Into, From, Debug, Clone, Copy)]
pub struct DateType(Date);

impl FromStr for DateType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Date::parse(s, format_description!("[year]-[month]-[day]"))
            .map(Self)
            .map_err(Into::into)
    }
}

impl DateType {
    /// # Errors
    /// Returns error if parse fails
    pub fn parse_from_str(s: &str) -> Result<Self, String> {
        s.parse().map_err(|e| format!("{e}"))
    }
}
