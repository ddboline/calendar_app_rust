-- Your SQL goes here
CREATE TABLE calendar_list (
    calendar_name TEXT UNIQUE NOT NULL PRIMARY KEY,
    gcal_id TEXT UNIQUE NOT NULL,
    gcal_name TEXT,
    gcal_description TEXT,
    gcal_location TEXT,
    gcal_timezone TEXT,
    sync BOOL NOT NULL DEFAULT FALSE,
    last_modified TIMESTAMP WITH TIME ZONE NOT NULL,
    edit BOOL NOT NULL DEFAULT FALSE,
    display BOOL NOT NULL DEFAULT FALSE
)
