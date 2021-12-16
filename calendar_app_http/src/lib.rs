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
use derive_more::{From, Into};
use rweb::Schema;
use rweb_helper::derive_rweb_schema;
use serde::{Deserialize, Serialize};
use stack_string::StackString;

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
    last_modified: DateTime<Utc>,
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
    #[schema(description = "Calendar ID")]
    id: i32,
    #[schema(description = "Gcal Calendar ID")]
    gcal_id: StackString,
    #[schema(description = "Calendar Event ID")]
    event_id: StackString,
    #[schema(description = "Event Start Time")]
    event_start_time: DateTime<Utc>,
    #[schema(description = "Event End Time")]
    event_end_time: DateTime<Utc>,
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
    last_modified: DateTime<Utc>,
}

#[cfg(test)]
mod test {
    use rweb_helper::derive_rweb_test;

    use crate::{
        CalendarCacheWrapper, CalendarListWrapper, _CalendarCacheWrapper, _CalendarListWrapper,
    };

    #[test]
    fn test_types() {
        derive_rweb_test!(CalendarListWrapper, _CalendarListWrapper);
        derive_rweb_test!(CalendarCacheWrapper, _CalendarCacheWrapper);
    }
}
