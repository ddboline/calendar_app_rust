use anyhow::{format_err, Error};
use chrono::{DateTime, Utc};
use google_calendar3::{CalendarHub, CalendarList, Events};
pub use google_calendar3::{CalendarListEntry, Event, EventDateTime};
use hyper::{net::HttpsConnector, Client};
use hyper_native_tls::NativeTlsClient;
use oauth2::{
    Authenticator, ConsoleApplicationSecret, DefaultAuthenticatorDelegate, DiskTokenStorage,
    FlowType,
};
use parking_lot::Mutex;
use stack_string::StackString;
use std::{
    fs::{create_dir_all, File},
    path::Path,
    sync::Arc,
};
use yup_oauth2 as oauth2;

use crate::exponential_retry;

type GCClient = Client;
type GCAuthenticator = Authenticator<DefaultAuthenticatorDelegate, DiskTokenStorage, Client>;
type GCCalendar = CalendarHub<GCClient, GCAuthenticator>;

#[derive(Clone)]
pub struct GCalendarInstance {
    pub gcal: Arc<Mutex<GCCalendar>>,
}

impl GCalendarInstance {
    pub fn new(gcal_token_path: &Path, gcal_secret_file: &Path, session_name: &str) -> Self {
        Self {
            gcal: Arc::new(Mutex::new(
                Self::create_gcal(gcal_token_path, gcal_secret_file, session_name).unwrap(),
            )),
        }
    }

    /// Creates a cal hub.
    fn create_gcal(
        gcal_token_path: &Path,
        gcal_secret_file: &Path,
        session_name: &str,
    ) -> Result<GCCalendar, Error> {
        let auth = Self::create_drive_auth(gcal_token_path, gcal_secret_file, session_name)?;
        Ok(CalendarHub::new(
            Client::with_connector(HttpsConnector::new(NativeTlsClient::new()?)),
            auth,
        ))
    }

    fn create_drive_auth(
        gcal_token_path: &Path,
        gcal_secret_file: &Path,
        session_name: &str,
    ) -> Result<GCAuthenticator, Error> {
        let secret_file = File::open(gcal_secret_file)?;
        let secret: ConsoleApplicationSecret = serde_json::from_reader(secret_file)?;
        let secret = secret
            .installed
            .ok_or_else(|| format_err!("ConsoleApplicationSecret.installed is None"))?;
        let token_file = gcal_token_path.join(format!("{}.json", session_name));
        let token_file = token_file.to_string_lossy().to_string();

        let parent = gcal_token_path;

        if !parent.exists() {
            create_dir_all(parent)?;
        }

        let auth = Authenticator::new(
            &secret,
            DefaultAuthenticatorDelegate,
            Client::with_connector(HttpsConnector::new(NativeTlsClient::new()?)),
            DiskTokenStorage::new(&token_file)?,
            // Some(FlowType::InstalledInteractive),
            Some(FlowType::InstalledRedirect(8081)),
        );

        Ok(auth)
    }

    fn gcal_calendars(&self, next_page_token: Option<&str>) -> Result<CalendarList, Error> {
        exponential_retry(|| {
            let gcal = self.gcal.lock();
            let req = gcal
                .calendar_list()
                .list()
                .show_deleted(false)
                .show_hidden(true);
            let req = if let Some(next_page_token) = next_page_token {
                req.page_token(next_page_token)
            } else {
                req
            };
            let (_, cal_list) = req.doit().map_err(|e| format_err!("{:#?}", e))?;
            Ok(cal_list)
        })
    }

    pub fn list_gcal_calendars(&self) -> Result<Vec<CalendarListEntry>, Error> {
        let mut output = Vec::new();
        let mut next_page_token: Option<StackString> = None;
        loop {
            let cal_list =
                self.gcal_calendars(next_page_token.as_ref().map(StackString::as_str))?;
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

    fn gcal_events(
        &self,
        gcal_id: &str,
        min_time: Option<DateTime<Utc>>,
        max_time: Option<DateTime<Utc>>,
        next_page_token: Option<&str>,
    ) -> Result<Events, Error> {
        exponential_retry(|| {
            let gcal = self.gcal.lock();
            let req = gcal.events().list(gcal_id);
            let req = if let Some(min_time) = min_time {
                req.time_min(&min_time.to_rfc3339())
            } else {
                req
            };
            let req = if let Some(max_time) = max_time {
                req.time_min(&max_time.to_rfc3339())
            } else {
                req
            };
            let req = if let Some(next_page_token) = next_page_token {
                req.page_token(next_page_token)
            } else {
                req
            };
            let (_, result) = req.doit().map_err(|e| format_err!("{:#?}", e))?;
            Ok(result)
        })
    }

    pub fn get_gcal_events(
        &self,
        gcal_id: &str,
        min_time: Option<DateTime<Utc>>,
        max_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<Event>, Error> {
        let mut output = Vec::new();
        let mut next_page_token: Option<StackString> = None;
        loop {
            let cal_list = self.gcal_events(
                gcal_id,
                min_time,
                max_time,
                next_page_token.as_ref().map(StackString::as_str),
            )?;
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

    pub fn insert_gcal_event(&self, gcal_id: &str, gcal_event: Event) -> Result<Event, Error> {
        let gcal = self.gcal.lock();
        let (_, result) = gcal
            .events()
            .insert(gcal_event, gcal_id)
            .supports_attachments(true)
            .doit()
            .map_err(|e| format_err!("{:#?}", e))?;
        Ok(result)
    }

    pub fn update_gcal_event(&self, gcal_id: &str, gcal_event: Event) -> Result<Event, Error> {
        let event_id = gcal_event
            .id
            .clone()
            .ok_or_else(|| format_err!("No event id"))?;
        let gcal = self.gcal.lock();
        let (_, result) = gcal
            .events()
            .update(gcal_event, gcal_id, &event_id)
            .doit()
            .map_err(|e| format_err!("{:#?}", e))?;
        Ok(result)
    }

    pub fn delete_gcal_event(&self, gcal_id: &str, gcal_event_id: &str) -> Result<(), Error> {
        let gcal = self.gcal.lock();
        gcal.events()
            .delete(gcal_id, gcal_event_id)
            .doit()
            .map_err(|e| format_err!("{:#?}", e))?;
        Ok(())
    }
}
