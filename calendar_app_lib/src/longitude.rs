use anyhow::{Error, format_err};
use derive_more::{Display, FromStr, Into};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// Latitude in degrees, required be within the range -180.0 to 180.0
#[derive(Into, Clone, Copy, Display, FromStr, Debug, Serialize, Deserialize, PartialEq)]
#[serde(into = "f64", try_from = "f64")]
pub struct Longitude(f64);

impl TryFrom<f64> for Longitude {
    type Error = Error;
    fn try_from(item: f64) -> Result<Self, Self::Error> {
        if (-180.0..=180.0).contains(&item) {
            Ok(Self(item))
        } else {
            Err(format_err!("{item} is not a valid latitude"))
        }
    }
}
