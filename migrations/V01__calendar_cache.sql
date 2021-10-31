-- Your SQL goes here
CREATE TABLE calendar_cache (
    event_id TEXT UNIQUE NOT NULL PRIMARY KEY,
    gcal_id TEXT NOT NULL,
    event_start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    event_end_time TIMESTAMP WITH TIME ZONE NOT NULL,
    event_url TEXT,
    event_name TEXT NOT NULL,
    event_description TEXT,
    event_location_name TEXT,
    event_location_lat DOUBLE PRECISION,
    event_location_lon DOUBLE PRECISION,
    last_modified TIMESTAMP WITH TIME ZONE NOT NULL
)
