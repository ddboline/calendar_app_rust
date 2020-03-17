table! {
    calendar_cache (id) {
        id -> Int4,
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

table! {
    calendar_list (id) {
        id -> Int4,
        calendar_name -> Text,
        gcal_id -> Text,
        gcal_name -> Nullable<Text>,
        gcal_description -> Nullable<Text>,
        gcal_location -> Nullable<Text>,
        gcal_timezone -> Nullable<Text>,
        sync -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(calendar_cache, calendar_list,);
