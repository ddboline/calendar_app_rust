use anyhow::Error;
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::America::New_York;
use futures::future::try_join_all;
use select::{document::Document, predicate::Name};
use smallvec::SmallVec;
use stack_string::{format_sstr, StackString};
use std::{collections::HashMap, fmt::Write, sync::Arc};

use crate::{
    calendar::{Event, Location},
    models::CalendarCache,
    pgpool::PgPool,
};

const CALID: &str = "8hfjg0d8ls2od3s9bd1k1v9jtc@group.calendar.google.com";
const URL: &str = "https://hashnyc.com/?days=all";

/// # Errors
/// Return error if parsing datetime string fails
pub fn parse_hashnyc_text(body: &str) -> Result<Vec<Event>, Error> {
    let mut events = Vec::new();
    for table in Document::from(body).find(Name("table")) {
        if table.attr("class") != Some("future_hashes") {
            continue;
        }
        for tr in table.find(Name("tr")) {
            let mut start_time = None;
            let mut name = None;
            let mut description = None;
            let mut location = None;
            for td in tr.find(Name("td")) {
                let mut year = None;
                for a in td.find(Name("a")) {
                    if let Some(id) = a.attr("id") {
                        let s = &id[..4];
                        if let Ok(y) = s.parse::<u16>() {
                            year.replace(y);
                        }
                    }
                }
                if td.attr("class") == Some("deeplink_container") {
                    let text: SmallVec<[String; 5]> = td.children().map(|c| c.text()).collect();
                    let date = text.join(" ");
                    let date = date.trim();
                    // Local::parse_from_str(&date, "%A %B %d ")
                    if let Some(year) = year {
                        let date = format_sstr!("{date} {year}");
                        let dt = New_York.datetime_from_str(&date, "%A %B %d %l:%M %P %Y")?;
                        let dt = dt.with_timezone(&Utc);
                        start_time.replace(dt);
                    }
                } else {
                    for b in td.find(Name("b")) {
                        name.replace(b.text());
                    }
                    if description.is_none() {
                        let text: Vec<_> = td.children().map(|c| c.text()).collect();
                        description.replace(text.join(" "));
                        for line in text {
                            if line.contains("Start:") {
                                let loc: StackString = line.replace("Start:", "").trim().into();
                                if !loc.is_empty() {
                                    location.replace(loc);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(name) = name {
                if let Some(start_time) = start_time {
                    let end_time = start_time + Duration::hours(1);
                    let mut event = Event::new(CALID, &name, start_time, end_time);
                    if let Some(description) = description {
                        event.description.replace(description.into());
                    }
                    if let Some(location) = location {
                        event.location.replace(Location {
                            name: location,
                            ..Location::default()
                        });
                    }
                    events.push(event);
                }
            }
        }
    }

    Ok(events)
}

/// # Errors
/// Return error if `get_by_gcal_id` fails, reqwest call fals, `parse_nycruns_text` fails, or any db update fails.
pub async fn parse_hashnyc(pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
    let current_event_map: HashMap<_, _> = CalendarCache::get_by_gcal_id(CALID, pool)
        .await?
        .into_iter()
        .map(|event| {
            let start_time = event.event_start_time.with_timezone(&New_York);
            (start_time, event)
        })
        .collect();
    let current_event_map = Arc::new(current_event_map);

    let body = reqwest::get(URL).await?.text().await?;

    let futures = parse_hashnyc_text(&body)?.into_iter().map(|event| {
        let current_event_map = current_event_map.clone();
        async move {
            let mut event: CalendarCache = event.into();
            let start_time = event.event_start_time.with_timezone(&New_York);

            if let Some(existing_event) = current_event_map.get(&start_time) {
                if event.event_name != existing_event.event_name
                    || event.event_description != existing_event.event_description
                    || event.event_location_name != existing_event.event_location_name
                {
                    event.event_id = existing_event.event_id.as_str().into();
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

    use crate::parse_hashnyc::parse_hashnyc_text;

    #[test]
    fn test_parse_hashnyc_text() -> Result<(), Error> {
        let text = include_str!("../../tests/data/hashnyc.html");
        let result = parse_hashnyc_text(&text)?;
        assert_eq!(result.len(), 12);
        Ok(())
    }
}
