use anyhow::{format_err, Error};
use chrono::{DateTime, Local, NaiveDate, Utc};
use chrono_tz::Tz;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use std::{convert::TryInto, fmt};
use url::Url;
use uuid::Uuid;

use gcal_lib::gcal_instance::{CalendarListEntry, Event as GCalEvent, EventDateTime};

use crate::{
    latitude::Latitude,
    longitude::Longitude,
    models::{
        CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList, InsertShortenedLinks,
        ShortenedLinks,
    },
    pgpool::PgPool,
    timezone::TimeZone,
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
        write!(
            f,
            "name: {}\tid: {}\n{}{}{}{}\n",
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

impl From<Calendar> for InsertCalendarList {
    fn from(item: Calendar) -> Self {
        Self {
            calendar_name: item.name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.description,
            gcal_location: item.location.map(|l| l.name),
            gcal_timezone: item.timezone.map(|z| z.into()),
            sync: false,
            last_modified: Utc::now().into(),
            edit: false,
        }
    }
}

impl Calendar {
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
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
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
                .map(|x| format!("\t\t{}", x))
                .join("\n");
            writeln!(f, "{}", description)?;
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
            start_time: item.event_start_time.into(),
            end_time: item.event_end_time.into(),
            url: item.event_url.and_then(|u| u.parse().ok()),
            name: item.event_name,
            description: item.event_description,
            location: loc,
        }
    }
}

impl From<Event> for InsertCalendarCache {
    fn from(item: Event) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.start_time.into(),
            event_end_time: item.end_time.into(),
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
            last_modified: Utc::now().into(),
        }
    }
}

fn from_gcal_eventdatetime(dt: &EventDateTime) -> Option<DateTime<Utc>> {
    dt.date_time.as_ref().map_or_else(
        || {
            dt.date.as_ref().and_then(|date| {
                let date: Option<NaiveDate> = date.parse().ok();
                dt.time_zone
                    .as_ref()
                    .and_then(|tz| tz.parse::<TimeZone>().ok())
                    .map_or_else(
                        || {
                            use chrono::TimeZone;
                            date.and_then(|d| {
                                Local.from_local_datetime(&d.and_hms(0, 0, 0)).single()
                            })
                            .map(|d| d.with_timezone(&Utc))
                        },
                        |tz| {
                            use chrono::TimeZone;
                            date.and_then(|d| tz.from_local_datetime(&d.and_hms(0, 0, 0)).single())
                                .map(|d| d.with_timezone(&Utc))
                        },
                    )
            })
        },
        |date_time| Some(*date_time),
    )
}

impl Event {
    pub fn new(
        gcal_id: &str,
        name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Self {
        Self {
            gcal_id: gcal_id.into(),
            event_id: Uuid::new_v4().to_string().replace("-", "").into(),
            start_time,
            end_time,
            url: None,
            name: name.into(),
            description: None,
            location: None,
        }
    }

    pub fn from_gcal_event(item: &GCalEvent, gcal_id: &str) -> Result<Self, Error> {
        let mut loc = None;
        if let Some(name) = &item.location {
            let location = Location {
                name: name.into(),
                ..Location::default()
            };
            loc.replace(location);
        }
        Ok(Self {
            gcal_id: gcal_id.into(),
            event_id: item
                .id
                .as_ref()
                .ok_or_else(|| format_err!("No event id"))?
                .into(),
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
                .as_ref()
                .ok_or_else(|| format_err!("No name for event"))?
                .into(),
            description: item.description.as_ref().map(Into::into),
            location: loc,
        })
    }

    pub fn to_gcal_event(&self) -> Result<(StackString, GCalEvent), Error> {
        let event = GCalEvent {
            id: Some(self.event_id.to_string()),
            start: Some(EventDateTime {
                date_time: Some(self.start_time),
                ..EventDateTime::default()
            }),
            end: Some(EventDateTime {
                date_time: Some(self.end_time),
                ..EventDateTime::default()
            }),
            summary: Some(self.name.to_string()),
            description: self.description.as_ref().map(ToString::to_string),
            location: self.location.as_ref().map(|l| l.name.to_string()),
            ..GCalEvent::default()
        };
        Ok((self.gcal_id.as_str().into(), event))
    }

    pub async fn get_summary(
        &self,
        domain: &str,
        pool: &PgPool,
        timezone: Option<TimeZone>,
    ) -> StackString {
        let mut short_url = None;
        let original_url = self.url.as_ref();

        if let Some(original_url) = original_url {
            if let Ok(mut result) =
                ShortenedLinks::get_by_original_url(original_url.as_str(), pool).await
            {
                if let Some(result) = result.pop() {
                    short_url.replace(format!(
                        "https://{}/calendar/link/{}",
                        domain, &result.shortened_url
                    ));
                }
            }
            if short_url.is_none() {
                if let Ok(result) =
                    InsertShortenedLinks::get_or_create(original_url.as_str(), pool).await
                {
                    if let Ok(result) = result.insert_shortened_link(pool).await {
                        short_url.replace(format!(
                            "https://{}/calendar/link/{}",
                            domain, &result.shortened_url
                        ));
                    }
                }
            }
        }

        let url = short_url.as_ref().map_or_else(
            || original_url.map_or_else(|| self.event_id.as_str(), Url::as_str),
            |short_url| short_url.as_str(),
        );

        let start_time = match timezone {
            Some(tz) => {
                let tz: Tz = tz.into();
                self.start_time.with_timezone(&tz).to_string()
            }
            None => self.start_time.with_timezone(&Local).to_string(),
        };

        format!(
            "{} {} {} {} {}",
            start_time, self.name, self.gcal_id, self.event_id, url,
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::{Duration, Utc};
    use log::debug;

    use gcal_lib::gcal_instance::GCalendarInstance;

    use crate::{calendar::Event, config::Config};

    #[test]
    fn test_new_evet() {
        let event = Event::new(
            "ddboline@gmail.com",
            "Test event",
            Utc::now(),
            Utc::now() + Duration::hours(1),
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
            Utc::now() + Duration::days(1),
            Utc::now() + Duration::days(1) + Duration::hours(1),
        );
        let (cal_id, event) = event.to_gcal_event()?;
        let event = gcal.insert_gcal_event(cal_id.as_str(), event).await?;
        let event_id = event.id.clone().unwrap();
        let event = Event::from_gcal_event(&event, cal_id.as_str())?;
        assert_eq!(event.name.as_str(), "Test Event");
        gcal.delete_gcal_event(cal_id.as_str(), event_id.as_str())
            .await?;
        Ok(())
    }
}
