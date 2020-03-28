table! {
    authorized_users (email) {
        email -> Varchar,
    }
}

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
        last_modified -> Timestamptz,
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
        last_modified -> Timestamptz,
        edit -> Bool,
    }
}

table! {
    shortened_links (id) {
        id -> Int4,
        original_url -> Text,
        shortened_url -> Text,
        last_modified -> Timestamptz,
    }
}

allow_tables_to_appear_in_same_query!(
    authorized_users,
    calendar_cache,
    calendar_list,
    shortened_links,
);
