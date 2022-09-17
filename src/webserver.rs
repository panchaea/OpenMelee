use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use diesel::{insert_into, prelude::*};
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUserRequest {
    display_name: String,
    connect_code: String,
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

pub async fn start_server(host: IpAddr, port: u16, _database_url: String) {
    let socket_address = SocketAddr::new(host, port);
    let database_url = _database_url.clone();

    let get_user = warp::get()
        .and(warp::path("user"))
        .and(warp::path::param::<String>())
        .map(move |_uid: String| {
            println!("Received get_user request");

            let connection = &mut establish_connection(database_url.clone());

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

    let create_user = warp::post()
        .and(warp::path("user"))
        .and(warp::body::content_length_limit(2048))
        .and(warp::body::json::<CreateUserRequest>())
        .map(move |req: CreateUserRequest| {
            println!("Received create_user request");

            let _user = User::new(req.display_name, req.connect_code);

            match _user {
                Some(user) => {
                    let connection = &mut establish_connection(_database_url.clone());
                    match insert_into(users).values(&user).execute(connection) {
                        Ok(_) => warp::reply::json(&PublicUser::from(&user)),
                        _ => {
                            let res = HashMap::from([("error", "Failed to create user")]);
                            warp::reply::json(&res)
                        }
                    }
                }
                _ => {
                    let res = HashMap::from([("error", "Could not create user")]);
                    warp::reply::json(&res)
                }
            }
        });

    tokio::spawn(warp::serve(get_user.or(create_user)).run(socket_address))
        .await
        .unwrap();
}
