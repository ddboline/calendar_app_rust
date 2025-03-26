use anyhow::{Error, format_err};
use derive_more::Into;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, ops::Deref, str::FromStr};
use time_tz::{
    TimeZone as TzTimeZone, Tz,
    timezones::{db::UTC, get_by_name},
};

use stack_string::StackString;

/// Direction in degrees
#[derive(Into, Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
#[serde(into = "StackString", try_from = "StackString")]
pub struct TimeZone(&'static Tz);

impl TimeZone {
    #[must_use]
    pub fn utc() -> Self {
        Self(UTC)
    }

    #[must_use]
    pub fn local() -> Self {
        Self(time_tz::system::get_timezone().unwrap_or(UTC))
    }
}

impl Deref for TimeZone {
    type Target = Tz;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl fmt::Display for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name())
    }
}

impl From<TimeZone> for String {
    fn from(item: TimeZone) -> Self {
        item.0.name().to_string()
    }
}

impl From<TimeZone> for StackString {
    fn from(item: TimeZone) -> Self {
        item.0.name().into()
    }
}

impl FromStr for TimeZone {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        get_by_name(s)
            .map(Self)
            .ok_or_else(|| format_err!("{s} is not a valid timezone"))
    }
}

impl TryFrom<&str> for TimeZone {
    type Error = Error;
    fn try_from(item: &str) -> Result<Self, Self::Error> {
        item.parse()
    }
}

impl TryFrom<StackString> for TimeZone {
    type Error = Error;
    fn try_from(item: StackString) -> Result<Self, Self::Error> {
        item.as_str().parse()
    }
}
