#[macro_use]
extern crate diesel;

use std::net::{IpAddr, Ipv4Addr};

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

pub mod models;
pub mod schema;

pub const LATEST_SLIPPI_CLIENT_VERSION: &str = "2.5.1";

pub fn establish_connection(database_url: String) -> SqliteConnection {
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub webserver_address: IpAddr,
    pub webserver_port: u16,
    pub matchmaking_server_address: Ipv4Addr,
    pub matchmaking_port: u16,
    pub matchmaking_max_peers: u64,
    pub database_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            webserver_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            webserver_port: 5000,
            matchmaking_server_address: Ipv4Addr::LOCALHOST,
            matchmaking_port: 43113,
            matchmaking_max_peers: 1024,
            database_url: "slippi-re.sqlite".to_string(),
        }
    }
}
