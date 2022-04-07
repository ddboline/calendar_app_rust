use itertools::Itertools;
use serde::{Deserialize, Serialize};
use stack_string::{format_sstr, StackString};
use std::{convert::TryInto, fmt};
use time::{Date, OffsetDateTime};
use time_tz::{OffsetDateTimeExt, PrimitiveDateTimeExt, TimeZone as TzTimeZone};
use url::Url;
use uuid::Uuid;

use gcal_lib::gcal_instance::{CalendarListEntry, Event as GCalEvent, EventDateTime};

use crate::{
    config::Config,
    get_default_or_local_time,
    latitude::Latitude,
    longitude::Longitude,
    models::{CalendarCache, CalendarList, ShortenedLinks},
    pgpool::PgPool,
    timezone::TimeZone,
    DateType,
};

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Location {
    pub name: StackString,
    pub lat_lon: Option<(Latitude, Longitude)>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Calendar {
    pub name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub description: Option<StackString>,
    pub location: Option<Location>,
    pub timezone: Option<TimeZone>,
    pub sync: bool,
    pub edit: bool,
    pub display: bool,
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "name: {}\tid: {}\n{}{}{}{}",
            self.name,
            self.gcal_id,
            self.gcal_name.as_ref().map_or("", StackString::as_str),
            self.description.as_ref().map_or("", StackString::as_str),
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
            edit: item.edit,
            display: item.display,
        }
    }
}

impl From<Calendar> for CalendarList {
    fn from(item: Calendar) -> Self {
        Self {
            calendar_name: item.name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.description,
            gcal_location: item.location.map(|l| l.name),
            gcal_timezone: item.timezone.map(Into::into),
            sync: false,
            last_modified: OffsetDateTime::now_utc(),
            edit: false,
            display: false,
        }
    }
}

impl Calendar {
    #[must_use]
    pub fn from_gcal_entry(item: &CalendarListEntry) -> Option<Self> {
        if item.deleted.unwrap_or(false) {
            None
        } else {
            Some(Self {
                name: item.summary.clone().map_or_else(|| "".into(), Into::into),
                gcal_id: item.id.clone().expect("No gcal_id").into(),
                gcal_name: item.summary.clone().map(Into::into),
                description: item.description.clone().map(Into::into),
                location: item.location.as_ref().map(|l| Location {
                    name: l.into(),
                    ..Location::default()
                }),
                timezone: item.time_zone.as_ref().and_then(|z| z.parse().ok()),
                sync: false,
                edit: false,
                display: false,
            })
        }
    }
}

#[derive(Debug)]
pub struct Event {
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub start_time: OffsetDateTime,
    pub end_time: OffsetDateTime,
    pub url: Option<Url>,
    pub name: StackString,
    pub description: Option<StackString>,
    pub location: Option<Location>,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\t{}:", self.name)?;
        if let Some(description) = &self.description {
            let description = description
                .split('\n')
                .map(|x| format_sstr!("\t\t{x}"))
                .join("\n");
            f.write_str(&description)?;
        }
        if let Some(url) = &self.url {
            writeln!(f, "\t\t{url}")?;
        }
        if let Some(location) = &self.location {
            writeln!(f, "\t\t{}", location.name)?;
            if let Some((lat, lon)) = &location.lat_lon {
                writeln!(f, "\t\t{lat} {lon}")?;
            }
        }
        let local = TimeZone::local().into();
        writeln!(
            f,
            "\t\t{} - {}",
            self.start_time.to_timezone(local),
            self.end_time.to_timezone(local),
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

impl From<Event> for CalendarCache {
    fn from(item: Event) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.start_time,
            event_end_time: item.end_time,
            event_url: item.url.map(Into::<String>::into).map(Into::into),
            event_name: item.name,
            event_description: item.description.map(Into::into),
            event_location_lat: item
                .location
                .as_ref()
                .and_then(|l| l.lat_lon.map(|(lat, _)| lat.into())),
            event_location_lon: item
                .location
                .as_ref()
                .and_then(|l| l.lat_lon.map(|(_, lon)| lon.into())),
            event_location_name: item.location.map(|l| l.name),
            last_modified: OffsetDateTime::now_utc(),
        }
    }
}

fn from_gcal_eventdatetime(dt: &EventDateTime) -> Option<OffsetDateTime> {
    dt.date_time.as_ref().map_or_else(
        || {
            dt.date.as_ref().and_then(|date| {
                let date: Option<DateType> = date.parse().ok();
                let date: Option<Date> = date.map(Into::into);
                let local = TimeZone::local();
                dt.time_zone
                    .as_ref()
                    .and_then(|tz| tz.parse::<TimeZone>().ok())
                    .map_or_else(
                        || {
                            date.and_then(|d| {
                                d.with_hms(0, 0, 0)
                                    .ok()
                                    .map(|dt| dt.assume_timezone(local.into()))
                            })
                            .map(|d| d.to_timezone(TimeZone::utc().into()))
                        },
                        |tz| {
                            date.and_then(|d| {
                                d.with_hms(0, 0, 0)
                                    .ok()
                                    .map(|dt| dt.assume_timezone(tz.into()))
                            })
                        },
                    )
            })
        },
        |date_time| Some((*date_time).into()),
    )
}

impl Event {
    pub fn new(
        gcal_id: impl Into<StackString>,
        name: impl Into<StackString>,
        start_time: OffsetDateTime,
        end_time: OffsetDateTime,
    ) -> Self {
        Self {
            gcal_id: gcal_id.into(),
            event_id: Uuid::new_v4().to_string().replace('-', "").into(),
            start_time,
            end_time,
            url: None,
            name: name.into(),
            description: None,
            location: None,
        }
    }

    pub fn from_gcal_event(item: &GCalEvent, gcal_id: impl Into<StackString>) -> Option<Self> {
        let mut loc = None;
        if let Some(name) = &item.location {
            let location = Location {
                name: name.into(),
                ..Location::default()
            };
            loc.replace(location);
        }
        Some(Self {
            gcal_id: gcal_id.into(),
            event_id: item.id.as_ref()?.into(),
            start_time: item.start.as_ref().and_then(from_gcal_eventdatetime)?,
            end_time: item.end.as_ref().and_then(from_gcal_eventdatetime)?,
            url: item.html_link.as_ref().and_then(|u| u.parse().ok()),
            name: item.summary.as_ref()?.into(),
            description: item.description.as_ref().map(Into::into),
            location: loc,
        })
    }

    #[must_use]
    pub fn to_gcal_event(&self) -> (StackString, GCalEvent) {
        let event = GCalEvent {
            id: Some(self.event_id.to_string()),
            start: Some(EventDateTime {
                date_time: Some(self.start_time.into()),
                ..EventDateTime::default()
            }),
            end: Some(EventDateTime {
                date_time: Some(self.end_time.into()),
                ..EventDateTime::default()
            }),
            summary: Some(self.name.to_string()),
            description: self.description.as_ref().map(ToString::to_string),
            location: self.location.as_ref().map(|l| l.name.to_string()),
            ..GCalEvent::default()
        };
        (self.gcal_id.clone(), event)
    }

    pub async fn get_summary(
        &self,
        domain: impl AsRef<str>,
        pool: &PgPool,
        config: &Config,
    ) -> StackString {
        let mut short_url = None;
        let original_url = self.url.as_ref();
        let domain = domain.as_ref();
        if let Some(original_url) = original_url {
            if let Ok(mut result) =
                ShortenedLinks::get_by_original_url(original_url.as_str(), pool).await
            {
                if let Some(result) = result.pop() {
                    short_url.replace(format_sstr!(
                        "https://{domain}/calendar/link/{url}",
                        url = result.shortened_url
                    ));
                }
            }
            if short_url.is_none() {
                if let Ok(result) = ShortenedLinks::get_or_create(original_url.as_str(), pool).await
                {
                    if result.insert_shortened_link(pool).await.is_ok() {
                        short_url.replace(format_sstr!(
                            "https://{domain}/calendar/link/{url}",
                            url = result.shortened_url
                        ));
                    }
                }
            }
        }

        let url = short_url.as_ref().map_or_else(
            || original_url.map_or_else(|| self.event_id.as_str(), Url::as_str),
            StackString::as_str,
        );
        let start_time = get_default_or_local_time(self.start_time, config);

        format_sstr!(
            "{start_time} {n} {i} {e} {url}",
            n = self.name,
            i = self.gcal_id,
            e = self.event_id,
        )
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use log::debug;
    use time::{Duration, OffsetDateTime};

    use gcal_lib::gcal_instance::GCalendarInstance;

    use crate::{calendar::Event, config::Config};

    #[test]
    fn test_new_event() {
        let event = Event::new(
            "ddboline@gmail.com",
            "Test event",
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc() + Duration::hours(1),
        );
        debug!("{:#?}", event);
        assert_eq!(&event.name, "Test event");
    }

    #[tokio::test]
    async fn test_insert_delete_gcal_event() -> Result<(), Error> {
        let config = Config::init_config()?;
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        )
        .await?;

        let event = Event::new(
            "ddboline@gmail.com",
            "Test Event",
            OffsetDateTime::now_utc() + Duration::days(1),
            OffsetDateTime::now_utc() + Duration::days(1) + Duration::hours(1),
        );
        let (cal_id, event) = event.to_gcal_event();
        let event = gcal.insert_gcal_event(&cal_id, event).await?;
        let event_id = event.id.as_ref().unwrap();
        let event = Event::from_gcal_event(&event, &cal_id).unwrap();
        assert_eq!(&event.name, "Test Event");
        gcal.delete_gcal_event(&cal_id, &event_id).await?;
        Ok(())
    }
}
