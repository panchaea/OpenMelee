#[macro_use]
extern crate diesel;

use diesel::prelude::*;

pub mod models;
pub mod schema;

pub const LATEST_SLIPPI_CLIENT_VERSION: &str = "2.5.1";

pub fn establish_connection(database_url: String) -> SqliteConnection {
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
