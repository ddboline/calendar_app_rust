-- Your SQL goes here
CREATE TABLE calendar_list (
    id SERIAL PRIMARY KEY,
    calendar_name TEXT UNIQUE NOT NULL,
    gcal_id TEXT UNIQUE NOT NULL,
    gcal_name TEXT,
    gcal_description TEXT,
    gcal_location TEXT,
    gcal_timezone TEXT
)