-- Your SQL goes here
CREATE TABLE shortened_links (
    shortened_url TEXT NOT NULL UNIQUE PRIMARY KEY,
    original_url TEXT NOT NULL UNIQUE,
    last_modified TIMESTAMP WITH TIME ZONE NOT NULL
)
