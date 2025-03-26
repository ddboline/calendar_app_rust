#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unused_async)]
#![allow(clippy::implicit_hasher)]
#![allow(clippy::ignored_unit_patterns)]

pub mod app;
pub mod elements;
pub mod errors;
pub mod logged_user;
pub mod routes;

use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_helper::derive_utoipa_schema;

use gcal_lib::date_time_wrapper::DateTimeWrapper;

use calendar_app_lib::models::{CalendarCache, CalendarList};

#[derive(Clone, Debug, Serialize, Deserialize, Into, From)]
pub struct CalendarListWrapper(CalendarList);

derive_utoipa_schema!(CalendarListWrapper, _CalendarListWrapper);

#[allow(dead_code)]
#[derive(ToSchema)]
// CalendarList
struct _CalendarListWrapper {
    // Calendar Name
    calendar_name: StackString,
    // GCal Calendar ID
    gcal_id: StackString,
    // GCal Calendar Name
    gcal_name: Option<StackString>,
    // GCal Calendar Description
    gcal_description: Option<StackString>,
    // GCal Calendar Location
    gcal_location: Option<StackString>,
    // GCal Calendar Timezone
    gcal_timezone: Option<StackString>,
    // Sync Flag
    sync: bool,
    // Last Modified
    last_modified: OffsetDateTime,
    // Edit Flag
    edit: bool,
    // Display Flag
    display: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Into, From)]
pub struct CalendarCacheWrapper(CalendarCache);

derive_utoipa_schema!(CalendarCacheWrapper, _CalendarCacheWrapper);

#[allow(dead_code)]
#[derive(ToSchema)]
// CalendarCache
#[schema(as = CalendarCache)]
struct _CalendarCacheWrapper {
    // Gcal Calendar ID
    gcal_id: StackString,
    // Calendar Event ID
    event_id: StackString,
    // Event Start Time
    event_start_time: OffsetDateTime,
    // Event End Time
    event_end_time: OffsetDateTime,
    // Event URL
    event_url: Option<StackString>,
    // Event Name
    event_name: StackString,
    // Event Description
    event_description: Option<StackString>,
    // Event Location Name
    event_location_name: Option<StackString>,
    // Event Location Latitude
    event_location_lat: Option<f64>,
    // Event Location Longitude
    event_location_lon: Option<f64>,
    // Last Modified
    last_modified: OffsetDateTime,
}

#[derive(Serialize, Deserialize)]
pub struct MinModifiedQuery {
    pub min_modified: Option<DateTimeWrapper>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

derive_utoipa_schema!(MinModifiedQuery, _MinModifiedQuery);

#[allow(dead_code)]
#[derive(ToSchema)]
struct _MinModifiedQuery {
    // Min Modified Date
    min_modified: Option<OffsetDateTime>,
    // Offset
    offset: Option<usize>,
    // Limit
    limit: Option<usize>,
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

derive_utoipa_schema!(CalendarCacheRequest, _CalendarCacheRequest);

#[allow(dead_code)]
#[derive(ToSchema)]
// CalendarCacheRequest
struct _CalendarCacheRequest {
    // GCal Calendar ID
    gcal_id: StackString,
    // Calendar Event ID
    event_id: StackString,
    // Event Start Time
    event_start_time: OffsetDateTime,
    // Event End Time
    event_end_time: OffsetDateTime,
    // Event URL
    event_url: Option<StackString>,
    // Event Name
    event_name: StackString,
    // Event Description
    event_description: Option<StackString>,
    // Event Location Name
    event_location_name: Option<StackString>,
    // Event Location Latitude
    event_location_lat: Option<f64>,
    // Event Location Longitude
    event_location_lon: Option<f64>,
    // Last Modified
    last_modified: OffsetDateTime,
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

derive_utoipa_schema!(CreateCalendarEventRequest, _CreateCalendarEventRequest);

#[allow(dead_code)]
#[derive(ToSchema)]
// CreateCalendarEventRequest
struct _CreateCalendarEventRequest {
    // GCal Calendar ID
    gcal_id: StackString,
    // Event ID
    event_id: StackString,
    // Event Start Time
    event_start_datetime: OffsetDateTime,
    // Event End Time
    event_end_datetime: OffsetDateTime,
    // Event URL
    event_url: Option<StackString>,
    // Event Name
    event_name: StackString,
    // Event Description
    event_description: Option<StackString>,
    // Event Location Name
    event_location_name: Option<StackString>,
}

#[cfg(test)]
mod test {
    use utoipa_helper::derive_utoipa_test;

    use crate::{
        CalendarCacheRequest, CalendarCacheWrapper, CalendarListWrapper,
        CreateCalendarEventRequest, MinModifiedQuery, _CalendarCacheRequest, _CalendarCacheWrapper,
        _CalendarListWrapper, _CreateCalendarEventRequest, _MinModifiedQuery,
    };

    #[test]
    fn test_types() {
        derive_utoipa_test!(CalendarListWrapper, _CalendarListWrapper);
        derive_utoipa_test!(CalendarCacheWrapper, _CalendarCacheWrapper);
        derive_utoipa_test!(MinModifiedQuery, _MinModifiedQuery);
        derive_utoipa_test!(CalendarCacheRequest, _CalendarCacheRequest);
        derive_utoipa_test!(CreateCalendarEventRequest, _CreateCalendarEventRequest);
    }
}
