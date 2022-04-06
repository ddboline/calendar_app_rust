use anyhow::Error;
use futures::future::try_join_all;
use log::debug;
use select::{document::Document, predicate::Class};
use smallvec::SmallVec;
use stack_string::{format_sstr, StackString};
use std::{collections::HashMap, sync::Arc};
use time::{macros::format_description, Date, Duration, PrimitiveDateTime, Time};
use time_tz::{timezones::db::america::NEW_YORK, OffsetDateTimeExt, PrimitiveDateTimeExt};
use url::Url;

use crate::{
    calendar::{Event, Location},
    models::CalendarCache,
    pgpool::PgPool,
};

const CALID: &str = "ufdpqtvophgg2qn643rducu1a4@group.calendar.google.com";
const BASE_URL: &str = "https://nycruns.com";
const URL: &str = "https://nycruns.com/races/?show=registerable";

/// # Errors
/// Return error if parsing datetime fails
pub fn parse_nycruns_text(body: &str) -> Result<Vec<Event>, Error> {
    let mut events = Vec::new();
    for race in Document::from(body).find(Class("_race")) {
        let mut current_date = None;
        let mut current_time = None;
        let mut location = None;
        let mut name: Option<StackString> = None;
        let mut event_url = None;
        for a in race.find(Class("_title")) {
            if let Some(url) = a.attr("href") {
                if let Ok(url) = format_sstr!("{BASE_URL}{url}").parse::<Url>() {
                    event_url.replace(url);
                }
            }
            if let Some(text) = a.text().trim().split('\n').next() {
                name.replace(text.trim().into());
            }
        }
        for date in race.find(Class("_date")) {
            let dt = Date::parse(
                &date.text(),
                format_description!(
                    "[weekday repr:long], [month repr:long] [day padding:none], [year]"
                ),
            )?;
            current_date.replace(dt);
        }
        for loc in race.find(Class("_subtitle")) {
            if let Some(class) = loc.attr("class") {
                let text = loc.text();
                if class.contains("_start-time") {
                    let items: SmallVec<[&str; 4]> = text.split_whitespace().collect();
                    let time_str = items[(items.len() - 2)..].join(" ");
                    if let Ok(time) = Time::parse(
                        &time_str,
                        format_description!("[hour padding:none]:[minute] [period case:upper]"),
                    ) {
                        current_time.replace(time);
                    } else if let Ok(time) = Time::parse(
                        items[items.len() - 1],
                        format_description!("[hour padding:none]:[minute][period case:upper]"),
                    ) {
                        current_time.replace(time);
                    } else {
                        debug!("{:?}", items);
                    }
                } else {
                    location.replace(text.trim().into());
                }
            }
        }
        if let Some(name) = name {
            if let Some(current_date) = current_date {
                if let Some(current_time) = current_time {
                    let current_datetime = PrimitiveDateTime::new(current_date, current_time);
                    let start_time = current_datetime.assume_timezone(NEW_YORK);
                    let end_time = start_time + Duration::hours(1);
                    let mut event = Event::new(CALID, &name, start_time, end_time);
                    if let Some(location) = location {
                        event.location.replace(Location {
                            name: location,
                            ..Location::default()
                        });
                    }
                    event.url = event_url;
                    events.push(event);
                }
            }
        }
    }
    Ok(events)
}

/// # Errors
/// Return error if `get_by_gcal_id` fails, reqwest call fals,
/// `parse_nycruns_text` fails, or any db update fails.
pub async fn parse_nycruns(pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
    let current_event_map: HashMap<_, _> = CalendarCache::get_by_gcal_id(CALID, pool)
        .await?
        .into_iter()
        .map(|event| {
            let start_time = event.event_start_time.to_timezone(NEW_YORK);
            (start_time, event)
        })
        .collect();
    let current_event_map = Arc::new(current_event_map);
    let body = reqwest::get(URL).await?.text().await?;

    let futures = parse_nycruns_text(&body)?.into_iter().map(|event| {
        let current_event_map = current_event_map.clone();
        async move {
            let mut event: CalendarCache = event.into();
            let start_time = event.event_start_time.to_timezone(NEW_YORK);
            if let Some(existing_event) = current_event_map.get(&start_time) {
                if event.event_name != existing_event.event_name
                    || event.event_description != existing_event.event_description
                    || event.event_location_name != existing_event.event_location_name
                {
                    event.event_id = existing_event.event_id.as_str().into();
                    debug!("modifying event {:#?} {:#?}", event, existing_event);
                    event.upsert(pool).await?;
                    Ok(Some(event))
                } else {
                    Ok(None)
                }
            } else {
                event.insert(pool).await?;
                Ok(Some(event))
            }
        }
    });
    let new_events: Result<Vec<_>, Error> = try_join_all(futures).await;
    Ok(new_events?.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use anyhow::Error;

    use crate::parse_nycruns::parse_nycruns_text;

    #[test]
    fn test_parse_nycruns_text() -> Result<(), Error> {
        let text = include_str!("../../tests/data/nycruns.html");
        let result = parse_nycruns_text(&text)?;
        assert_eq!(result.len(), 20);
        Ok(())
    }
}
