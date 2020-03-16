use anyhow::{format_err, Error};
use chrono::{DateTime, Local, NaiveDate, Utc};
use std::convert::TryInto;
use url::Url;

use gcal_lib::gcal_instance::{CalendarListEntry, Event as GCalEvent, EventDateTime};

use crate::latitude::Latitude;
use crate::longitude::Longitude;
use crate::models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList};
use crate::timezone::TimeZone;

#[derive(Default, Debug)]
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
                ..Default::default()
            }),
            timezone: item.gcal_timezone.and_then(|z| z.parse().ok()),
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
        }
    }
}

impl From<CalendarListEntry> for Calendar {
    fn from(item: CalendarListEntry) -> Self {
        Self {
            name: item.summary.clone().unwrap_or_else(|| "".to_string()),
            gcal_id: item.id.expect("No gcal_id"),
            gcal_name: item.summary,
            description: item.description,
            location: item.location.map(|l| Location {
                name: l,
                ..Default::default()
            }),
            timezone: item.time_zone.and_then(|z| z.parse().ok()),
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

impl From<CalendarCache> for Event {
    fn from(item: CalendarCache) -> Self {
        let mut loc = None;
        if let Some(name) = item.event_location_name {
            let mut location = Location {
                name,
                ..Default::default()
            };
            let lat = item.event_location_lat.and_then(|l| l.try_into().ok());
            let lon = item.event_location_lon.and_then(|l| l.try_into().ok());
            if lat.is_some() && lon.is_some() {
                let lat = lat.unwrap();
                let lon = lon.unwrap();
                location.lat_lon.replace((lat, lon));
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
            event_url: self.url.map(|u| u.into_string()),
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
    pub fn from_gcal_event(item: &GCalEvent, gcal_id: &str) -> Result<Self, Error> {
        let mut loc = None;
        if let Some(name) = &item.location {
            let location = Location {
                name: name.to_string(),
                ..Default::default()
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
}
