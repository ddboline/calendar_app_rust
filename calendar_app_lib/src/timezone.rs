use anyhow::{format_err, Error};
use chrono_tz::Tz;
use derive_more::Into;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::Deref;
use std::str::FromStr;

/// Direction in degrees
#[derive(Into, Debug, PartialEq, Copy, Clone, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "&str")]
pub struct TimeZone(Tz);

impl Deref for TimeZone {
    type Target = Tz;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::convert::Into<String> for TimeZone {
    fn into(self) -> String {
        self.0.name().to_string()
    }
}

impl FromStr for TimeZone {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse()
            .map(Self)
            .map_err(|e| format_err!("{} is not a valid timezone", e))
    }
}

impl TryFrom<&str> for TimeZone {
    type Error = Error;
    fn try_from(item: &str) -> Result<Self, Self::Error> {
        item.parse()
    }
}
