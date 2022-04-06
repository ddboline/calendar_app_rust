use anyhow::Error;
use core::marker::PhantomData;
use derive_more::{Deref, From, Into};
use serde::{
    de::{Error as DeError, Unexpected, Visitor},
    ser, Deserialize, Deserializer, Serialize, Serializer,
};
use stack_string::StackString;
use std::fmt;
use time::{format_description::well_known::Rfc3339, macros::datetime, OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Copy, Deref, Into, From, PartialEq)]
pub struct DateTimeType(OffsetDateTime);

impl fmt::Display for DateTimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if let Ok(s) = convert_datetime_to_str(self.0) {
            write!(f, "{s}")?;
        }
        Ok(())
    }
}

#[must_use]
pub fn sentinel_datetime() -> OffsetDateTime {
    datetime!(0001-01-01 00:00:00).assume_utc()
}

impl Serialize for DateTimeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&convert_datetime_to_str(self.0).map_err(ser::Error::custom)?)
    }
}

impl<'de> Deserialize<'de> for DateTimeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(DateTimeTypeVisitor(PhantomData))
    }
}

struct DateTimeTypeVisitor(PhantomData<*const DateTimeType>);

impl<'de> Visitor<'de> for DateTimeTypeVisitor {
    type Value = DateTimeType;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("datetime")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        convert_str_to_datetime(&v)
            .map_err(DeError::custom)
            .map(DateTimeType)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        convert_str_to_datetime(v)
            .map_err(DeError::custom)
            .map(DateTimeType)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match std::str::from_utf8(v) {
            Ok(s) => convert_str_to_datetime(s)
                .map_err(DeError::custom)
                .map(DateTimeType),
            Err(_) => Err(DeError::invalid_value(Unexpected::Bytes(v), &self)),
        }
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match String::from_utf8(v) {
            Ok(s) => convert_str_to_datetime(&s)
                .map_err(DeError::custom)
                .map(DateTimeType),
            Err(e) => Err(DeError::invalid_value(
                Unexpected::Bytes(&e.into_bytes()),
                &self,
            )),
        }
    }
}

/// # Errors
/// Returns error if formatting fails (which can only happen if formatting
/// string is non-utf8)
pub fn convert_datetime_to_str(datetime: OffsetDateTime) -> Result<StackString, Error> {
    datetime
        .format(&Rfc3339)
        .map_err(Into::into)
        .map(|s| s.replace('Z', "+00:00"))
        .map(Into::into)
}

/// # Errors
/// Return error if `parse_from_rfc3339` fails
pub fn convert_str_to_datetime(s: &str) -> Result<OffsetDateTime, Error> {
    OffsetDateTime::parse(&s.replace('Z', "+00:00"), &Rfc3339)
        .map(|x| x.to_offset(UtcOffset::UTC))
        .map_err(Into::into)
}
