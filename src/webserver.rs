use std::net::{IpAddr, SocketAddr};

use diesel::prelude::*;
use serde::Serialize;
use warp::Filter;

use slippi_re::{
    establish_connection, models::*, schema::users::dsl::*, LATEST_SLIPPI_CLIENT_VERSION,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserNotFound {
    latest_version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    uid: String,
    display_name: String,
    connect_code: String,
    latest_version: Option<String>,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> PublicUser {
        PublicUser {
            uid: user.uid.to_string(),
            display_name: user.display_name.to_string(),
            connect_code: user.connect_code.to_string(),
            latest_version: match &user.latest_version {
                Some(str) => Some(str.to_string()),
                _ => Some(LATEST_SLIPPI_CLIENT_VERSION.to_string()),
            },
        }
    }
}

pub async fn start_server(host: IpAddr, port: u16) {
    let socket_address = SocketAddr::new(host, port);

    let get_user = warp::get()
        .and(warp::path("user"))
        .and(warp::path::param::<String>())
        .map(|_uid: String| {
            println!("Received get_user request");

            let connection = &mut establish_connection();

            let _users = users
                .filter(uid.eq(_uid))
                .limit(1)
                .load::<User>(connection)
                .expect("Error connecting to database");

            match _users.get(0) {
                Some(user) => warp::reply::json(&PublicUser::from(user)),
                _ => warp::reply::json(&UserNotFound {
                    latest_version: LATEST_SLIPPI_CLIENT_VERSION.to_string(),
                }),
            }
        });

    tokio::spawn(warp::serve(get_user).run(socket_address))
        .await
        .unwrap();
}
