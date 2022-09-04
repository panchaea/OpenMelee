use std::net::{IpAddr, SocketAddr};

use warp::Filter;

use crate::user::User;

pub async fn start_server(host: IpAddr, port: u16) {
    let socket_address = SocketAddr::new(host, port);

    let get_user = warp::get()
        .and(warp::path("user"))
        .and(warp::path::param::<String>())
        .map(|_| {
            println!("Received web request");

            let test_user: User = User {
                display_name: "test".to_string(),
                connect_code: "TEST#001".to_string(),
                latest_version: "2.5.1".to_string(),
            };

            warp::reply::json(&test_user)
        });

    tokio::spawn(warp::serve(get_user).run(socket_address))
        .await
        .unwrap();
}
