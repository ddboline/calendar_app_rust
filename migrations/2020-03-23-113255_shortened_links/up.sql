-- Your SQL goes here
CREATE TABLE shortened_links (
    id SERIAL PRIMARY KEY,
    original_url TEXT NOT NULL UNIQUE,
    shortened_url TEXT NOT NULL UNIQUE,
    last_modified TIMESTAMP WITH TIME ZONE NOT NULL
)
