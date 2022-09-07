#[macro_use]
extern crate diesel;

use std::env;

use diesel::prelude::*;

pub mod models;
pub mod schema;

pub const LATEST_SLIPPI_CLIENT_VERSION: &str = "2.5.1";

pub fn establish_connection() -> SqliteConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
