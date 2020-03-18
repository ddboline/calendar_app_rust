use anyhow::Error;
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use futures::future::try_join_all;
use reqwest::Client;
use select::document::Document;
use select::predicate::Class;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

use crate::calendar::{Event, Location};
use crate::models::{CalendarCache, InsertCalendarCache};
use crate::pgpool::PgPool;

const CALID: &str = "ufdpqtvophgg2qn643rducu1a4@group.calendar.google.com";
const BASE_URL: &str = "https://nycruns.com";
const URL: &str = "https://nycruns.com/races/?show=registerable";

pub struct ParseNycRuns {
    client: Client,
    pool: PgPool,
}

impl ParseNycRuns {
    pub fn new(pool: PgPool) -> Self {
        Self {
            client: Client::new(),
            pool,
        }
    }

    pub async fn parse_nycruns_text(&self, body: &str) -> Result<Vec<Event>, Error> {
        let mut events = Vec::new();
        for race in Document::from(body).find(Class("_race")) {
            let mut current_date = None;
            let mut current_time = None;
            let mut location = None;
            let mut name = None;
            let mut event_url = None;
            for a in race.find(Class("_title")) {
                if let Some(url) = a.attr("href") {
                    if let Ok(url) = format!("{}{}", BASE_URL, url).parse::<Url>() {
                        event_url.replace(url);
                    }
                }
                if let Some(text) = a.text().trim().split("\n").nth(0) {
                    name.replace(text.trim().to_string());
                }
            }
            for date in race.find(Class("_date")) {
                let dt = NaiveDate::parse_from_str(&date.text(), "%A, %B %d, %Y")?;
                current_date.replace(dt);
            }
            for loc in race.find(Class("_subtitle")) {
                if let Some(class) = loc.attr("class") {
                    let text = loc.text();
                    if class.contains("_start-time") {
                        let items: Vec<_> = text.split_whitespace().collect();
                        let last_two = items[(items.len() - 2)..].join(" ");
                        let last_one = items[items.len() - 1];
                        if let Ok(time) = NaiveTime::parse_from_str(&last_two, "%l:%M %p") {
                            current_time.replace(time);
                        } else if let Ok(time) = NaiveTime::parse_from_str(&last_one, "%l:%M%p") {
                            current_time.replace(time);
                        } else {
                            println!("{:?}", items);
                        }
                    } else {
                        location.replace(text);
                    }
                }
            }
            if name.is_some() && current_date.is_some() && current_time.is_some() {
                let current_date = current_date.unwrap();
                let current_time = current_time.unwrap();
                let current_datetime = NaiveDateTime::new(current_date, current_time);
                let start_time = New_York
                    .from_local_datetime(&current_datetime)
                    .single()
                    .unwrap()
                    .with_timezone(&Utc);
                let end_time = start_time + Duration::hours(1);
                let name = name.unwrap();
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
        Ok(events)
    }

    pub async fn parse_nycruns(&self) -> Result<Vec<InsertCalendarCache>, Error> {
        let current_event_map: HashMap<_, _> = CalendarCache::get_by_gcal_id(CALID, &self.pool)
            .await?
            .into_iter()
            .map(|event| {
                let start_time = event.event_start_time.with_timezone(&New_York);
                (start_time, event)
            })
            .collect();
        let current_event_map = Arc::new(current_event_map);
        let body = self.client.get(URL).send().await?.text().await?;

        let futures = self
            .parse_nycruns_text(&body)
            .await?
            .into_iter()
            .map(|event| {
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
                                println!("modifying event {:#?} {:#?}", event, existing_event);
                                Ok(Some(event.upsert(&self.pool).await?))
                            } else {
                                Ok(None)
                            }
                        }
                        None => Ok(Some(event.insert(&self.pool).await?)),
                    }
                }
            });
        let results: Result<Vec<_>, Error> = try_join_all(futures).await;
        let new_events: Vec<_> = results?.into_iter().filter_map(|x| x).collect();
        Ok(new_events)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;

    use crate::config::Config;
    use crate::parse_nycruns::ParseNycRuns;
    use crate::pgpool::PgPool;

    #[tokio::test]
    async fn test_parse_nycruns_text() -> Result<(), Error> {
        let config = Config::init_config()?;
        let pool = PgPool::new(&config.database_url);
        let nycruns = ParseNycRuns::new(pool);
        let text = include_str!("../../tests/data/nycruns.html");
        let result = nycruns.parse_nycruns_text(&text).await?;
        assert_eq!(result.len(), 20);
        Ok(())
    }
}
