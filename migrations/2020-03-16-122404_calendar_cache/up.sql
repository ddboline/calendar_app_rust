-- Your SQL goes here
CREATE TABLE calendar_cache (
    id SERIAL PRIMARY KEY,
    calendar_name TEXT NOT NULL,
    gcal_id TEXT NOT NULL,
    event_id TEXT UNIQUE NOT NULL,
    event_start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    event_end_time TIMESTAMP WITH TIME ZONE NOT NULL,
    event_url TEXT,
    event_name TEXT NOT NULL,
    event_description TEXT,
    event_location_name TEXT,
    event_location_lat DOUBLE PRECISION,
    event_location_lon DOUBLE PRECISION
)
