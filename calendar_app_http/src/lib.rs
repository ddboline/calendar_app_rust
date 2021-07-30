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

pub mod app;
pub mod datetime_wrapper;
pub mod errors;
pub mod logged_user;
pub mod naivedate_wrapper;
pub mod routes;

use rweb::Schema;
use serde::{Deserialize, Serialize};
use stack_string::StackString;

use calendar_app_lib::models::{
    CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList,
};

use crate::datetime_wrapper::DateTimeWrapper;

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
pub struct CalendarListWrapper {
    pub id: i32,
    pub calendar_name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub gcal_description: Option<StackString>,
    pub gcal_location: Option<StackString>,
    pub gcal_timezone: Option<StackString>,
    pub sync: bool,
    pub last_modified: DateTimeWrapper,
    pub edit: bool,
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
            last_modified: item.last_modified.into(),
            edit: item.edit,
            display: item.display,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
pub struct InsertCalendarListWrapper {
    pub calendar_name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub gcal_description: Option<StackString>,
    pub gcal_location: Option<StackString>,
    pub gcal_timezone: Option<StackString>,
    pub sync: bool,
    pub last_modified: DateTimeWrapper,
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
            last_modified: item.last_modified.into(),
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
            last_modified: item.last_modified.into(),
            edit: item.edit,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
pub struct CalendarCacheWrapper {
    pub id: i32,
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_time: DateTimeWrapper,
    pub event_end_time: DateTimeWrapper,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
    pub last_modified: DateTimeWrapper,
}

impl From<CalendarCache> for CalendarCacheWrapper {
    fn from(item: CalendarCache) -> Self {
        Self {
            id: item.id,
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time.into(),
            event_end_time: item.event_end_time.into(),
            event_url: item.event_url,
            event_name: item.event_name,
            event_description: item.event_description,
            event_location_name: item.event_location_name,
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: item.last_modified.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
pub struct InsertCalendarCacheWrapper {
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_time: DateTimeWrapper,
    pub event_end_time: DateTimeWrapper,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
    pub last_modified: DateTimeWrapper,
}

impl From<InsertCalendarCache> for InsertCalendarCacheWrapper {
    fn from(item: InsertCalendarCache) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time.into(),
            event_end_time: item.event_end_time.into(),
            event_url: item.event_url,
            event_name: item.event_name,
            event_description: item.event_description,
            event_location_name: item.event_location_name,
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: item.last_modified.into(),
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
