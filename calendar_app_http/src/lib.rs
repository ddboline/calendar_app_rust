#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unused_async)]

pub mod app;
pub mod errors;
pub mod logged_user;
pub mod routes;
pub mod elements;

use derive_more::{From, Into};
use rweb::Schema;
use rweb_helper::{derive_rweb_schema, DateTimeType};
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use time::OffsetDateTime;

use gcal_lib::date_time_wrapper::DateTimeWrapper;

use calendar_app_lib::models::{CalendarCache, CalendarList};

#[derive(Clone, Debug, Serialize, Deserialize, Into, From)]
pub struct CalendarListWrapper(CalendarList);

derive_rweb_schema!(CalendarListWrapper, _CalendarListWrapper);

#[allow(dead_code)]
#[derive(Schema)]
struct _CalendarListWrapper {
    #[schema(description = "Calendar Name")]
    calendar_name: StackString,
    #[schema(description = "GCal Calendar ID")]
    gcal_id: StackString,
    #[schema(description = "GCal Calendar Name")]
    gcal_name: Option<StackString>,
    #[schema(description = "GCal Calendar Description")]
    gcal_description: Option<StackString>,
    #[schema(description = "GCal Calendar Location")]
    gcal_location: Option<StackString>,
    #[schema(description = "GCal Calendar Timezone")]
    gcal_timezone: Option<StackString>,
    #[schema(description = "Sync Flag")]
    sync: bool,
    #[schema(description = "Last Modified")]
    last_modified: DateTimeType,
    #[schema(description = "Edit Flag")]
    edit: bool,
    #[schema(description = "Display Flag")]
    display: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Into, From)]
pub struct CalendarCacheWrapper(CalendarCache);

derive_rweb_schema!(CalendarCacheWrapper, _CalendarCacheWrapper);

#[allow(dead_code)]
#[derive(Schema)]
struct _CalendarCacheWrapper {
    #[schema(description = "Gcal Calendar ID")]
    gcal_id: StackString,
    #[schema(description = "Calendar Event ID")]
    event_id: StackString,
    #[schema(description = "Event Start Time")]
    event_start_time: DateTimeType,
    #[schema(description = "Event End Time")]
    event_end_time: DateTimeType,
    #[schema(description = "Event URL")]
    event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    event_name: StackString,
    #[schema(description = "Event Description")]
    event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    event_location_name: Option<StackString>,
    #[schema(description = "Event Location Latitude")]
    event_location_lat: Option<f64>,
    #[schema(description = "Event Location Longitude")]
    event_location_lon: Option<f64>,
    #[schema(description = "Last Modified")]
    last_modified: DateTimeType,
}

#[derive(Serialize, Deserialize)]
pub struct MinModifiedQuery {
    pub min_modified: Option<DateTimeWrapper>,
}

derive_rweb_schema!(MinModifiedQuery, _MinModifiedQuery);

#[allow(dead_code)]
#[derive(Schema)]
struct _MinModifiedQuery {
    #[schema(description = "Min Modified Date")]
    min_modified: Option<DateTimeType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarCacheRequest {
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

impl From<CalendarCacheRequest> for CalendarCache {
    fn from(item: CalendarCacheRequest) -> Self {
        let event_start_time: OffsetDateTime = item.event_start_time.into();
        let event_end_time: OffsetDateTime = item.event_end_time.into();
        let last_modified: OffsetDateTime = item.last_modified.into();
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: event_start_time.into(),
            event_end_time: event_end_time.into(),
            event_url: item.event_url.map(Into::into),
            event_name: item.event_name,
            event_description: item.event_description.map(Into::into),
            event_location_name: item.event_location_name.map(Into::into),
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: last_modified.into(),
        }
    }
}

derive_rweb_schema!(CalendarCacheRequest, _CalendarCacheRequest);

#[allow(dead_code)]
#[derive(Schema)]
struct _CalendarCacheRequest {
    #[schema(description = "GCal Calendar ID")]
    gcal_id: StackString,
    #[schema(description = "Calendar Event ID")]
    event_id: StackString,
    #[schema(description = "Event Start Time")]
    event_start_time: DateTimeType,
    #[schema(description = "Event End Time")]
    event_end_time: DateTimeType,
    #[schema(description = "Event URL")]
    event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    event_name: StackString,
    #[schema(description = "Event Description")]
    event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    event_location_name: Option<StackString>,
    #[schema(description = "Event Location Latitude")]
    event_location_lat: Option<f64>,
    #[schema(description = "Event Location Longitude")]
    event_location_lon: Option<f64>,
    #[schema(description = "Last Modified")]
    last_modified: DateTimeType,
}

#[derive(Serialize, Deserialize)]
pub struct CreateCalendarEventRequest {
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_datetime: DateTimeWrapper,
    pub event_end_datetime: DateTimeWrapper,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
}

derive_rweb_schema!(CreateCalendarEventRequest, _CreateCalendarEventRequest);

#[allow(dead_code)]
#[derive(Schema)]
struct _CreateCalendarEventRequest {
    #[schema(description = "GCal Calendar ID")]
    gcal_id: StackString,
    #[schema(description = "Event ID")]
    event_id: StackString,
    #[schema(description = "Event Start Time")]
    event_start_datetime: DateTimeType,
    #[schema(description = "Event End Time")]
    event_end_datetime: DateTimeType,
    #[schema(description = "Event URL")]
    event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    event_name: StackString,
    #[schema(description = "Event Description")]
    event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    event_location_name: Option<StackString>,
}

#[cfg(test)]
mod test {
    use rweb_helper::derive_rweb_test;

    use crate::{
        CalendarCacheRequest, CalendarCacheWrapper, CalendarListWrapper,
        CreateCalendarEventRequest, MinModifiedQuery, _CalendarCacheRequest, _CalendarCacheWrapper,
        _CalendarListWrapper, _CreateCalendarEventRequest, _MinModifiedQuery,
    };

    #[test]
    fn test_types() {
        derive_rweb_test!(CalendarListWrapper, _CalendarListWrapper);
        derive_rweb_test!(CalendarCacheWrapper, _CalendarCacheWrapper);
        derive_rweb_test!(MinModifiedQuery, _MinModifiedQuery);
        derive_rweb_test!(CalendarCacheRequest, _CalendarCacheRequest);
        derive_rweb_test!(CreateCalendarEventRequest, _CreateCalendarEventRequest);
    }
}
