use std::net::{IpAddr, Ipv4Addr};

use figment::{providers::Env, providers::Serialized, Figment};
use serde::{Deserialize, Serialize};

mod matchmaking;
mod webserver;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    webserver_address: IpAddr,
    webserver_port: u16,
    matchmaking_server_address: Ipv4Addr,
    matchmaking_port: u16,
    matchmaking_max_peers: u64,
    database_url: String,
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

#[tokio::main]
async fn main() {
    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("SLIPPI_RE_"))
        .extract()
        .unwrap();

    let webserver_thread = tokio::spawn(async move {
        webserver::start_server(
            config.webserver_address,
            config.webserver_port,
            config.database_url,
        )
        .await;
    });

    println!("Started webserver");

    let enet_server_thread = tokio::task::spawn_blocking(move || {
        matchmaking::start_server(
            config.matchmaking_server_address,
            config.matchmaking_port,
            config.matchmaking_max_peers,
        );
    });

    println!("Started matchmaking server");

    if (webserver_thread.await).is_err() {
        println!("webserver thread exited abnormally")
    }
    if (enet_server_thread.await).is_err() {
        println!("ENet server thread exited abnormally")
    }
}
