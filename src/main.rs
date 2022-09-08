use std::net::{IpAddr, Ipv4Addr};

mod matchmaking;
mod webserver;

const MATCHMAKING_PORT: u16 = 43113;
const WEBSERVER_PORT: u16 = 5000;

#[tokio::main]
async fn main() {
    let webserver_thread = tokio::spawn(async move {
        webserver::start_server(IpAddr::V4(Ipv4Addr::LOCALHOST), WEBSERVER_PORT).await;
    });

    println!("Started webserver");

    let enet_server_thread = tokio::task::spawn_blocking(move || {
        matchmaking::start_server(Ipv4Addr::LOCALHOST, MATCHMAKING_PORT);
    });

    println!("Started matchmaking server");

    match webserver_thread.await {
        _ => (),
    };
    match enet_server_thread.await {
        _ => (),
    };
}
