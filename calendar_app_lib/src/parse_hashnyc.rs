use anyhow::Error;
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::America::New_York;
use futures::future::try_join_all;
use select::{document::Document, predicate::Name};
use std::{collections::HashMap, sync::Arc};

use crate::{
    calendar::{Event, Location},
    models::{CalendarCache, InsertCalendarCache},
    pgpool::PgPool,
};

const CALID: &str = "8hfjg0d8ls2od3s9bd1k1v9jtc@group.calendar.google.com";
const URL: &str = "https://hashnyc.com/?days=all";

pub async fn parse_hashnyc_text(body: &str) -> Result<Vec<Event>, Error> {
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
                    let text: Vec<_> = td.children().map(|c| c.text()).collect();
                    let date = text.join(" ");
                    let date = date.trim();
                    // Local::parse_from_str(&date, "%A %B %d ")
                    if let Some(year) = year {
                        let date = format!("{} {}", date, year);
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
                                let loc = line.replace("Start:", "").trim().to_string();
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
                        event.description.replace(description);
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

pub async fn parse_hashnyc(pool: &PgPool) -> Result<Vec<InsertCalendarCache>, Error> {
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

    let futures = parse_hashnyc_text(&body).await?.into_iter().map(|event| {
        let current_event_map = current_event_map.clone();
        async move {
            let mut event: InsertCalendarCache = event.into();
            let start_time = event.event_start_time.with_timezone(&New_York);
            match current_event_map.get(&start_time) {
                Some(existing_event) => {
                    if event.event_name != existing_event.event_name
                        || event.event_description != existing_event.event_description
                        || event.event_location_name != existing_event.event_location_name
                    {
                        event.event_id = existing_event.event_id.to_string();
                        Ok(Some(event.upsert(&pool).await?))
                    } else {
                        Ok(None)
                    }
                }
                None => Ok(Some(event.insert(&pool).await?)),
            }
        }
    });
    let results: Result<Vec<_>, Error> = try_join_all(futures).await;
    let new_events: Vec<_> = results?.into_iter().filter_map(|x| x).collect();
    Ok(new_events)
}

#[cfg(test)]
mod tests {
    use anyhow::Error;

    use crate::parse_hashnyc::parse_hashnyc_text;

    #[tokio::test]
    async fn test_parse_hashnyc_text() -> Result<(), Error> {
        let text = include_str!("../../tests/data/hashnyc.html");
        let result = parse_hashnyc_text(&text).await?;
        assert_eq!(result.len(), 12);
        Ok(())
    }
}
