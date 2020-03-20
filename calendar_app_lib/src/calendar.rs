use anyhow::{format_err, Error};
use chrono::{DateTime, Local, NaiveDate, Utc};
use std::convert::TryInto;
use std::fmt;
use url::Url;
use uuid::Uuid;

use gcal_lib::gcal_instance::{CalendarListEntry, Event as GCalEvent, EventDateTime};

use crate::latitude::Latitude;
use crate::longitude::Longitude;
use crate::models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList};
use crate::timezone::TimeZone;

#[derive(Default, Debug, PartialEq)]
pub struct Location {
    pub name: String,
    pub lat_lon: Option<(Latitude, Longitude)>,
}

pub struct Calendar {
    pub name: String,
    pub gcal_id: String,
    pub gcal_name: Option<String>,
    pub description: Option<String>,
    pub location: Option<Location>,
    pub timezone: Option<TimeZone>,
    pub sync: bool,
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "name: {}\tid: {}\n{}{}{}{}\n",
            self.name,
            self.gcal_id,
            self.gcal_name.as_ref().map_or("", String::as_str),
            self.description.as_ref().map_or("", String::as_str),
            self.location.as_ref().map_or("", |l| l.name.as_str()),
            self.timezone.as_ref().map_or("", |t| t.name()),
        )
    }
}

impl From<CalendarList> for Calendar {
    fn from(item: CalendarList) -> Self {
        Self {
            name: item.calendar_name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name.map(Into::into),
            description: item.gcal_description.map(Into::into),
            location: item.gcal_location.map(|l| Location {
                name: l,
                ..Location::default()
            }),
            timezone: item.gcal_timezone.and_then(|z| z.parse().ok()),
            sync: item.sync,
        }
    }
}

impl Into<InsertCalendarList> for Calendar {
    fn into(self) -> InsertCalendarList {
        InsertCalendarList {
            calendar_name: self.name,
            gcal_id: self.gcal_id,
            gcal_name: self.gcal_name,
            gcal_description: self.description,
            gcal_location: self.location.map(|l| l.name),
            gcal_timezone: self.timezone.map(|z| z.into()),
            sync: true,
            last_modified: Utc::now(),
        }
    }
}

impl Calendar {
    pub fn from_gcal_entry(item: &CalendarListEntry) -> Option<Self> {
        if item.deleted.unwrap_or(false) {
            None
        } else {
            Some(Self {
                name: item.summary.clone().unwrap_or_else(|| "".to_string()),
                gcal_id: item.id.clone().expect("No gcal_id"),
                gcal_name: item.summary.clone(),
                description: item.description.clone(),
                location: item.location.as_ref().map(|l| Location {
                    name: l.to_string(),
                    ..Location::default()
                }),
                timezone: item.time_zone.as_ref().and_then(|z| z.parse().ok()),
                sync: true,
            })
        }
    }
}

#[derive(Debug)]
pub struct Event {
    pub gcal_id: String,
    pub event_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub url: Option<Url>,
    pub name: String,
    pub description: Option<String>,
    pub location: Option<Location>,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\t{}:", self.name)?;
        if let Some(description) = &self.description {
            let description: Vec<_> = description
                .split('\n')
                .map(|x| format!("\t\t{}", x))
                .collect();
            writeln!(f, "{}", description.join("\n"))?;
        }
        if let Some(url) = &self.url {
            writeln!(f, "\t\t{}", url.as_str())?;
        }
        if let Some(location) = &self.location {
            writeln!(f, "\t\t{}", location.name)?;
            if let Some((lat, lon)) = &location.lat_lon {
                writeln!(f, "\t\t{} {}", lat, lon)?;
            }
        }
        writeln!(
            f,
            "\t\t{} - {}\n",
            self.start_time.with_timezone(&Local),
            self.end_time.with_timezone(&Local)
        )
    }
}

impl From<CalendarCache> for Event {
    fn from(item: CalendarCache) -> Self {
        let mut loc = None;
        if let Some(name) = item.event_location_name {
            let mut location = Location {
                name,
                ..Location::default()
            };
            let latitude = item.event_location_lat.and_then(|l| l.try_into().ok());
            let longitude = item.event_location_lon.and_then(|l| l.try_into().ok());
            if let Some(latitude) = latitude {
                if let Some(longitude) = longitude {
                    location.lat_lon.replace((latitude, longitude));
                }
            }
            loc.replace(location);
        }
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            start_time: item.event_start_time,
            end_time: item.event_end_time,
            url: item.event_url.and_then(|u| u.parse().ok()),
            name: item.event_name,
            description: item.event_description,
            location: loc,
        }
    }
}

impl Into<InsertCalendarCache> for Event {
    fn into(self) -> InsertCalendarCache {
        InsertCalendarCache {
            gcal_id: self.gcal_id,
            event_id: self.event_id,
            event_start_time: self.start_time,
            event_end_time: self.end_time,
            event_url: self.url.map(Url::into_string),
            event_name: self.name,
            event_description: self.description,
            event_location_lat: self
                .location
                .as_ref()
                .and_then(|l| l.lat_lon.map(|(lat, _)| lat.into())),
            event_location_lon: self
                .location
                .as_ref()
                .and_then(|l| l.lat_lon.map(|(_, lon)| lon.into())),
            event_location_name: self.location.map(|l| l.name),
            last_modified: Utc::now(),
        }
    }
}

fn from_gcal_eventdatetime(dt: &EventDateTime) -> Option<DateTime<Utc>> {
    if let Some(date_time) = dt.date_time.as_ref() {
        DateTime::parse_from_rfc3339(date_time)
            .ok()
            .map(|d| d.with_timezone(&Utc))
    } else if let Some(date) = dt.date.as_ref() {
        let date: Option<NaiveDate> = date.parse().ok();
        if let Some(tz) = dt
            .time_zone
            .as_ref()
            .and_then(|tz| tz.parse::<TimeZone>().ok())
        {
            use chrono::TimeZone;
            date.and_then(|d| tz.from_local_datetime(&d.and_hms(0, 0, 0)).single())
                .map(|d| d.with_timezone(&Utc))
        } else {
            use chrono::TimeZone;
            date.and_then(|d| Local.from_local_datetime(&d.and_hms(0, 0, 0)).single())
                .map(|d| d.with_timezone(&Utc))
        }
    } else {
        None
    }
}

impl Event {
    pub fn new(
        gcal_id: &str,
        name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Self {
        Self {
            gcal_id: gcal_id.to_string(),
            event_id: Uuid::new_v4().to_string().replace("-", ""),
            start_time,
            end_time,
            url: None,
            name: name.to_string(),
            description: None,
            location: None,
        }
    }

    pub fn from_gcal_event(item: &GCalEvent, gcal_id: &str) -> Result<Self, Error> {
        let mut loc = None;
        if let Some(name) = &item.location {
            let location = Location {
                name: name.to_string(),
                ..Location::default()
            };
            loc.replace(location);
        }
        Ok(Self {
            gcal_id: gcal_id.to_string(),
            event_id: item.id.clone().ok_or_else(|| format_err!("No event id"))?,
            start_time: item
                .start
                .as_ref()
                .and_then(|s| from_gcal_eventdatetime(s))
                .ok_or_else(|| format_err!("No start time"))?,
            end_time: item
                .end
                .as_ref()
                .and_then(|s| from_gcal_eventdatetime(s))
                .ok_or_else(|| format_err!("No start time"))?,
            url: item.html_link.as_ref().and_then(|u| u.parse().ok()),
            name: item
                .summary
                .clone()
                .ok_or_else(|| format_err!("No name for event"))?,
            description: item.description.clone(),
            location: loc,
        })
    }

    pub fn to_gcal_event(&self) -> Result<(String, GCalEvent), Error> {
        let event = GCalEvent {
            id: Some(self.event_id.to_string()),
            start: Some(EventDateTime {
                date_time: Some(self.start_time.to_rfc3339()),
                ..EventDateTime::default()
            }),
            end: Some(EventDateTime {
                date_time: Some(self.end_time.to_rfc3339()),
                ..EventDateTime::default()
            }),
            summary: Some(self.name.to_string()),
            description: self.description.clone(),
            location: self.location.as_ref().map(|l| l.name.to_string()),
            ..GCalEvent::default()
        };
        Ok((self.gcal_id.to_string(), event))
    }

    pub fn get_summary(&self) -> String {
        format!(
            "{} {} {} {}",
            self.start_time.with_timezone(&Local),
            self.name,
            self.gcal_id,
            self.url
                .as_ref()
                .map_or_else(|| self.event_id.as_str(), Url::as_str)
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::calendar::Event;
    use chrono::{Duration, Utc};

    #[test]
    fn test_new_evet() {
        let event = Event::new(
            "ddboline@gmail.com",
            "Test event",
            Utc::now(),
            Utc::now() + Duration::hours(1),
        );
        println!("{:#?}", event);
        assert_eq!(event.name, "Test event");
    }
}
