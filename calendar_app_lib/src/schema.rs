table! {
    calendar_cache (id) {
        id -> Int4,
        calendar_name -> Text,
        gcal_id -> Text,
        event_id -> Text,
        event_start_time -> Timestamptz,
        event_end_time -> Timestamptz,
        event_url -> Nullable<Text>,
        event_name -> Text,
        event_description -> Nullable<Text>,
        event_location_name -> Nullable<Text>,
        event_location_lat -> Nullable<Float8>,
        event_location_lon -> Nullable<Float8>,
    }
}
