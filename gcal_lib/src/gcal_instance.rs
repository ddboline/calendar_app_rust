use anyhow::{format_err, Error};
use async_google_apis_common as common;
use chrono::{DateTime, Utc, MAX_DATETIME, MIN_DATETIME};
use common::{
    yup_oauth2::{self, InstalledFlowAuthenticator},
    TlsClient,
};
use log::debug;
use stack_string::{format_sstr, StackString};
use std::{fmt::Write, fs::create_dir_all, path::Path, sync::Arc};

use stdout_channel::rate_limiter::RateLimiter;

pub use crate::calendar_v3_types::{CalendarListEntry, Event, EventDateTime};
use crate::{
    calendar_v3_types::{
        CalendarList, CalendarListListParams, CalendarListService, CalendarScopes, Events,
        EventsDeleteParams, EventsGetParams, EventsInsertParams, EventsListParams, EventsService,
        EventsUpdateParams,
    },
    exponential_retry,
};

fn https_client() -> TlsClient {
    let conn = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_only()
        .enable_http1()
        .build();
    hyper::Client::builder().build(conn)
}

#[derive(Clone)]
pub struct GCalendarInstance {
    cal_list: Arc<CalendarListService>,
    cal_events: Arc<EventsService>,
    rate_limit: RateLimiter,
}

impl GCalendarInstance {
    pub async fn new(
        gcal_token_path: &Path,
        gcal_secret_file: &Path,
        session_name: &str,
    ) -> Result<Self, Error> {
        debug!("{:?}", gcal_secret_file);
        let https = https_client();
        let sec = yup_oauth2::read_application_secret(gcal_secret_file).await?;

        let token_file = gcal_token_path.join(format_sstr!("{}.json", session_name));

        let parent = gcal_token_path;

        if !parent.exists() {
            create_dir_all(parent)?;
        }

        debug!("{:?}", token_file);
        let auth = InstalledFlowAuthenticator::builder(
            sec,
            common::yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk(token_file)
        .hyper_client(https.clone())
        .build()
        .await?;
        let auth = Arc::new(auth);

        let scopes = vec![
            CalendarScopes::CalendarReadonly,
            CalendarScopes::CalendarEventsReadonly,
            CalendarScopes::Calendar,
            CalendarScopes::CalendarEvents,
        ];

        let mut cal_list = CalendarListService::new(https.clone(), auth.clone());
        cal_list.set_scopes(scopes.clone());

        let mut cal_events = EventsService::new(https, auth);
        cal_events.set_scopes(scopes);

        Ok(Self {
            cal_list: Arc::new(cal_list),
            cal_events: Arc::new(cal_events),
            rate_limit: RateLimiter::new(600, 60000),
        })
    }

    async fn gcal_calendars(&self, next_page_token: Option<&str>) -> Result<CalendarList, Error> {
        let params = CalendarListListParams {
            show_deleted: Some(true),
            show_hidden: Some(true),
            page_token: next_page_token.map(Into::into),
            ..CalendarListListParams::default()
        };
        exponential_retry(|| async {
            self.rate_limit.acquire().await;
            self.cal_list.list(&params).await
        })
        .await
    }

    pub async fn list_gcal_calendars(&self) -> Result<Vec<CalendarListEntry>, Error> {
        let mut output = Vec::new();
        let mut next_page_token: Option<StackString> = None;
        loop {
            let cal_list = self
                .gcal_calendars(next_page_token.as_ref().map(StackString::as_str))
                .await?;
            if let Some(cal_list) = cal_list.items {
                output.extend_from_slice(&cal_list);
            }
            if let Some(token) = cal_list.next_page_token {
                next_page_token.replace(token.into());
            } else {
                break;
            }
        }
        Ok(output)
    }

    async fn gcal_events(
        &self,
        gcal_id: &str,
        min_time: Option<DateTime<Utc>>,
        max_time: Option<DateTime<Utc>>,
        next_page_token: Option<&str>,
    ) -> Result<Events, Error> {
        let params = EventsListParams {
            calendar_id: gcal_id.into(),
            time_min: Some(min_time.unwrap_or(MIN_DATETIME)),
            time_max: Some(max_time.unwrap_or(MAX_DATETIME)),
            page_token: next_page_token.map(Into::into),
            ..EventsListParams::default()
        };
        exponential_retry(|| async {
            self.rate_limit.acquire().await;
            self.cal_events.list(&params).await
        })
        .await
    }

    pub async fn get_gcal_events(
        &self,
        gcal_id: &str,
        min_time: Option<DateTime<Utc>>,
        max_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<Event>, Error> {
        let mut output = Vec::new();
        let mut next_page_token: Option<StackString> = None;
        loop {
            let cal_list = self
                .gcal_events(
                    gcal_id,
                    min_time,
                    max_time,
                    next_page_token.as_ref().map(StackString::as_str),
                )
                .await?;
            if let Some(cal_list) = cal_list.items {
                output.extend_from_slice(&cal_list);
            }
            if let Some(token) = cal_list.next_page_token {
                next_page_token.replace(token.into());
            } else {
                break;
            }
        }
        Ok(output)
    }

    pub async fn get_event(&self, gcal_id: &str, gcal_event_id: &str) -> Result<Event, Error> {
        let params = EventsGetParams {
            calendar_id: gcal_id.into(),
            event_id: gcal_event_id.into(),
            ..EventsGetParams::default()
        };
        exponential_retry(|| async {
            self.rate_limit.acquire().await;
            self.cal_events.get(&params).await
        })
        .await
    }

    pub async fn insert_gcal_event(
        &self,
        gcal_id: &str,
        gcal_event: Event,
    ) -> Result<Event, Error> {
        let params = EventsInsertParams {
            calendar_id: gcal_id.into(),
            supports_attachments: Some(true),
            ..EventsInsertParams::default()
        };
        self.rate_limit.acquire().await;
        self.cal_events.insert(&params, &gcal_event).await
    }

    pub async fn update_gcal_event(
        &self,
        gcal_id: &str,
        gcal_event: Event,
    ) -> Result<Event, Error> {
        let event_id = gcal_event
            .id
            .clone()
            .ok_or_else(|| format_err!("No event id"))?;
        let params = EventsUpdateParams {
            calendar_id: gcal_id.into(),
            event_id,
            ..EventsUpdateParams::default()
        };
        self.rate_limit.acquire().await;
        self.cal_events.update(&params, &gcal_event).await
    }

    pub async fn delete_gcal_event(&self, gcal_id: &str, gcal_event_id: &str) -> Result<(), Error> {
        let params = EventsDeleteParams {
            calendar_id: gcal_id.into(),
            event_id: gcal_event_id.into(),
            ..EventsDeleteParams::default()
        };
        self.rate_limit.acquire().await;
        self.cal_events.delete(&params).await
    }
}

pub fn compare_gcal_events(event0: &Event, event1: &Event) -> bool {
    (event0.id == event1.id)
        && (event0.start.as_ref().map(|s| s.date.as_ref())
            == event1.start.as_ref().map(|s| s.date.as_ref()))
        && (event0.start.as_ref().map(|s| s.time_zone.as_ref())
            == event1.start.as_ref().map(|s| s.time_zone.as_ref()))
        && (event0.start.as_ref().map(|s| s.date_time.as_ref())
            == event1.start.as_ref().map(|s| s.date_time.as_ref()))
        && (event0.end.as_ref().map(|s| s.date.as_ref())
            == event1.end.as_ref().map(|s| s.date.as_ref()))
        && (event0.end.as_ref().map(|s| s.time_zone.as_ref())
            == event1.end.as_ref().map(|s| s.time_zone.as_ref()))
        && (event0.end.as_ref().map(|s| s.date_time.as_ref())
            == event1.end.as_ref().map(|s| s.date_time.as_ref()))
        && (event0.summary == event1.summary)
        && (event0.description == event1.description)
        && (event0.location == event1.location)
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::{Duration, Utc};
    use log::debug;

    use calendar_app_lib::config::Config;

    use crate::gcal_instance::GCalendarInstance;

    #[tokio::test]
    async fn test_list_calendars() -> Result<(), Error> {
        let config = Config::init_config()?;
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        )
        .await?;
        let cal_list = gcal.list_gcal_calendars().await?;
        assert_eq!(cal_list.len(), 19);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_gcal_events() -> Result<(), Error> {
        let config = Config::init_config()?;
        let gcal = GCalendarInstance::new(
            &config.gcal_token_path,
            &config.gcal_secret_file,
            "ddboline@gmail.com",
        )
        .await?;
        let events = gcal
            .get_gcal_events(
                "ddboline@gmail.com",
                Some(Utc::now() - Duration::days(10)),
                Some(Utc::now() + Duration::days(10)),
            )
            .await?;
        debug!("{:#?}", events);
        debug!("{}", events.len());
        assert!(events.len() > 0);
        Ok(())
    }
}
