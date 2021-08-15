#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::unused_async)]

pub mod app;
pub mod errors;
pub mod logged_user;
pub mod routes;

use chrono::{DateTime, Utc};
use rweb::Schema;
use serde::{Deserialize, Serialize};
use stack_string::StackString;

use calendar_app_lib::models::{
    CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList,
};

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
pub struct CalendarListWrapper {
    #[schema(description = "Calendar ID")]
    pub id: i32,
    #[schema(description = "Calendar Name")]
    pub calendar_name: StackString,
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "GCal Calendar Name")]
    pub gcal_name: Option<StackString>,
    #[schema(description = "GCal Calendar Description")]
    pub gcal_description: Option<StackString>,
    #[schema(description = "GCal Calendar Location")]
    pub gcal_location: Option<StackString>,
    #[schema(description = "GCal Calendar Timezone")]
    pub gcal_timezone: Option<StackString>,
    #[schema(description = "Sync Flag")]
    pub sync: bool,
    #[schema(description = "Last Modified")]
    pub last_modified: DateTime<Utc>,
    #[schema(description = "Edit Flag")]
    pub edit: bool,
    #[schema(description = "Display Flag")]
    pub display: bool,
}

impl From<CalendarList> for CalendarListWrapper {
    fn from(item: CalendarList) -> Self {
        Self {
            id: item.id,
            calendar_name: item.calendar_name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.gcal_description,
            gcal_location: item.gcal_location,
            gcal_timezone: item.gcal_timezone,
            sync: item.sync,
            last_modified: item.last_modified,
            edit: item.edit,
            display: item.display,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
pub struct InsertCalendarListWrapper {
    #[schema(description = "Calendar Name")]
    pub calendar_name: StackString,
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "GCal Calendar Name")]
    pub gcal_name: Option<StackString>,
    #[schema(description = "GCal Calendar Description")]
    pub gcal_description: Option<StackString>,
    #[schema(description = "GCal Calendar Location")]
    pub gcal_location: Option<StackString>,
    #[schema(description = "GCal Calendar Timezone")]
    pub gcal_timezone: Option<StackString>,
    #[schema(description = "Sync Flag")]
    pub sync: bool,
    #[schema(description = "Last Modified")]
    pub last_modified: DateTime<Utc>,
    #[schema(description = "Edit Flag")]
    pub edit: bool,
}

impl From<InsertCalendarList> for InsertCalendarListWrapper {
    fn from(item: InsertCalendarList) -> Self {
        Self {
            calendar_name: item.calendar_name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.gcal_description,
            gcal_location: item.gcal_location,
            gcal_timezone: item.gcal_timezone,
            sync: item.sync,
            last_modified: item.last_modified,
            edit: item.edit,
        }
    }
}

impl From<CalendarListWrapper> for InsertCalendarList {
    fn from(item: CalendarListWrapper) -> Self {
        Self {
            calendar_name: item.calendar_name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.gcal_description,
            gcal_location: item.gcal_location,
            gcal_timezone: item.gcal_timezone,
            sync: item.sync,
            last_modified: item.last_modified,
            edit: item.edit,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
pub struct CalendarCacheWrapper {
    #[schema(description = "Calendar ID")]
    pub id: i32,
    #[schema(description = "Gcal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Calendar Event ID")]
    pub event_id: StackString,
    #[schema(description = "Event Start Time")]
    pub event_start_time: DateTime<Utc>,
    #[schema(description = "Event End Time")]
    pub event_end_time: DateTime<Utc>,
    #[schema(description = "Event URL")]
    pub event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    pub event_name: StackString,
    #[schema(description = "Event Description")]
    pub event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    pub event_location_name: Option<StackString>,
    #[schema(description = "Event Location Latitude")]
    pub event_location_lat: Option<f64>,
    #[schema(description = "Event Location Longitude")]
    pub event_location_lon: Option<f64>,
    #[schema(description = "Last Modified")]
    pub last_modified: DateTime<Utc>,
}

impl From<CalendarCache> for CalendarCacheWrapper {
    fn from(item: CalendarCache) -> Self {
        Self {
            id: item.id,
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time,
            event_end_time: item.event_end_time,
            event_url: item.event_url,
            event_name: item.event_name,
            event_description: item.event_description,
            event_location_name: item.event_location_name,
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: item.last_modified,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
pub struct InsertCalendarCacheWrapper {
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Event ID")]
    pub event_id: StackString,
    #[schema(description = "Event Start Time")]
    pub event_start_time: DateTime<Utc>,
    #[schema(description = "Event End Time")]
    pub event_end_time: DateTime<Utc>,
    #[schema(description = "Event URL")]
    pub event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    pub event_name: StackString,
    #[schema(description = "Event Description")]
    pub event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    pub event_location_name: Option<StackString>,
    #[schema(description = "Event Location Longitude")]
    pub event_location_lat: Option<f64>,
    #[schema(description = "Event Location Latitude")]
    pub event_location_lon: Option<f64>,
    #[schema(description = "Last Modified")]
    pub last_modified: DateTime<Utc>,
}

impl From<InsertCalendarCache> for InsertCalendarCacheWrapper {
    fn from(item: InsertCalendarCache) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time,
            event_end_time: item.event_end_time,
            event_url: item.event_url,
            event_name: item.event_name,
            event_description: item.event_description,
            event_location_name: item.event_location_name,
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: item.last_modified,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
