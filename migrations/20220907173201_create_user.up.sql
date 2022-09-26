CREATE TABLE users (
    uid VARCHAR PRIMARY KEY NOT NULL,
    play_key VARCHAR NOT NULL,
    display_name VARCHAR NOT NULL,
    connect_code VARCHAR UNIQUE NOT NULL,
    latest_version VARCHAR
)
