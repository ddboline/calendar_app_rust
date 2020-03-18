use anyhow::Error;
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use futures::future::try_join_all;
use reqwest::Client;
use select::document::Document;
use select::predicate::{Class, Name};
use std::collections::HashMap;
use std::sync::Arc;

use crate::calendar::{Event, Location};
use crate::models::{CalendarCache, InsertCalendarCache};
use crate::pgpool::PgPool;

const CALID: &str = "ufdpqtvophgg2qn643rducu1a4@group.calendar.google.com";
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
        for race in Document::from(body).find(Class("_race")) {
            for date in race.find(Class("_date")) {
                let dt = NaiveDate::parse_from_str(&date.text(), "%A, %B %d, %Y")?;
                println!("date {}", dt);
            }
            for time in race.find(Class("_start-time")) {
                println!("{:?}", time.text());
            }
        }
        Ok(Vec::new())
    }

    pub async fn parse_nycruns(&self) -> Result<Vec<Event>, Error> {
        let body = self.client.get(URL).send().await?.text().await?;
        self.parse_nycruns_text(&body).await
    }
}
