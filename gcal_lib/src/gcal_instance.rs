use anyhow::{format_err, Error};
use google_calendar3::{CalendarHub, CalendarList, CalendarListEntry};
use hyper::{net::HttpsConnector, Client};
use hyper_native_tls::NativeTlsClient;
use oauth2::{
    Authenticator, ConsoleApplicationSecret, DefaultAuthenticatorDelegate, DiskTokenStorage,
    FlowType,
};
use parking_lot::Mutex;
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::Arc;
use yup_oauth2 as oauth2;

type GCClient = Client;
type GCAuthenticator = Authenticator<DefaultAuthenticatorDelegate, DiskTokenStorage, Client>;
type GCCalendar = CalendarHub<GCClient, GCAuthenticator>;

pub struct GCalendarInstance {
    pub gcal: Arc<Mutex<GCCalendar>>,
}

impl GCalendarInstance {
    pub fn new(gcal_token_path: &str, gcal_secret_file: &str, session_name: &str) -> Self {
        Self {
            gcal: Arc::new(Mutex::new(
                Self::create_gcal(gcal_token_path, gcal_secret_file, session_name).unwrap(),
            )),
        }
    }

    /// Creates a cal hub.
    fn create_gcal(
        gcal_token_path: &str,
        gcal_secret_file: &str,
        session_name: &str,
    ) -> Result<GCCalendar, Error> {
        let auth = Self::create_drive_auth(gcal_token_path, gcal_secret_file, session_name)?;
        Ok(CalendarHub::new(
            Client::with_connector(HttpsConnector::new(NativeTlsClient::new()?)),
            auth,
        ))
    }

    fn create_drive_auth(
        gcal_token_path: &str,
        gcal_secret_file: &str,
        session_name: &str,
    ) -> Result<GCAuthenticator, Error> {
        let secret_file = File::open(gcal_secret_file)?;
        let secret: ConsoleApplicationSecret = serde_json::from_reader(secret_file)?;
        let secret = secret
            .installed
            .ok_or_else(|| format_err!("ConsoleApplicationSecret.installed is None"))?;
        let token_file = format!("{}/{}.json", gcal_token_path, session_name);

        let parent = Path::new(gcal_token_path);

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
        let gcal = self.gcal.lock();
        let req = gcal
            .calendar_list()
            .list()
            .show_deleted(false)
            .show_hidden(false);
        let req = if let Some(next_page_token) = next_page_token {
            req.page_token(next_page_token)
        } else {
            req
        };
        let (_, cal_list) = req.doit().map_err(|e| format_err!("{:#?}", e))?;
        Ok(cal_list)
    }

    pub fn list_gcal_calendars(&self) -> Result<Vec<CalendarListEntry>, Error> {
        let mut output = Vec::new();
        let mut next_page_token: Option<String> = None;
        loop {
            let cal_list = self.gcal_calendars(next_page_token.as_ref().map(|x| x.as_str()))?;
            if let Some(cal_list) = cal_list.items {
                output.extend_from_slice(&cal_list);
            }
            if let Some(token) = cal_list.next_page_token {
                next_page_token.replace(token);
            } else {
                break;
            }
        }
        Ok(output)
    }
}
