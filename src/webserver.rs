use std::net::SocketAddr;

use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use axum_sqlx_tx::Tx;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool};

use slippi_re::{models::*, Config, LATEST_SLIPPI_CLIENT_VERSION};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserNotFound {
    latest_version: String,
}

impl UserNotFound {
    fn new() -> UserNotFound {
        UserNotFound {
            latest_version: LATEST_SLIPPI_CLIENT_VERSION.to_string(),
        }
    }
}

impl IntoResponse for UserNotFound {
    fn into_response(self: Self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    uid: String,
    display_name: String,
    connect_code: String,
    latest_version: String,
}

impl IntoResponse for PublicUser {
    fn into_response(self: Self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserForm {
    pub display_name: String,
    pub connect_code: String,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> PublicUser {
        PublicUser {
            uid: user.uid.to_string(),
            display_name: user.display_name.to_string(),
            connect_code: user.connect_code.to_string(),
            latest_version: match &user.latest_version {
                Some(str) => str.to_string(),
                _ => LATEST_SLIPPI_CLIENT_VERSION.to_string(),
            },
        }
    }
}

async fn get_user(mut tx: Tx<Sqlite>, Path(uid): Path<String>) -> Result<PublicUser, UserNotFound> {
    User::get(&mut tx, uid)
        .await
        .map(|user| PublicUser::from(&user))
        .map_err(|_| UserNotFound::new())
}

async fn create_user(
    mut tx: Tx<Sqlite>,
    Form(user_form): Form<UserForm>,
) -> Result<PublicUser, ()> {
    println!("Received create_user request");
    User::create(
        &mut tx,
        user_form.display_name.to_string(),
        user_form.connect_code.to_string(),
    )
    .await
    .map(|user| PublicUser::from(&user))
    .map_err(|err| {
        println!("{:?}", err);
        ()
    })
}

pub async fn start_server(config: Config, pool: SqlitePool) -> Result<(), ()> {
    let app = Router::new()
        .route("/user", post(create_user))
        .route("/user/:uid", get(get_user))
        .layer(axum_sqlx_tx::Layer::new(pool));

    let server = axum::Server::bind(&SocketAddr::from((
        config.webserver_address,
        config.webserver_port,
    )))
    .serve(app.into_make_service());

    println!(
        "Web server listening on http://{}:{}",
        config.webserver_address, config.webserver_port
    );

    server.await.map_err(|_| ())
}

#[cfg(test)]
mod test {
    use crate::webserver::*;

    #[test]
    fn test_can_create_public_user_from_user() {
        let user = User {
            uid: "1234".to_string(),
            play_key: "5678".to_string(),
            display_name: "test".to_string(),
            connect_code: "TEST#001".to_string(),
            latest_version: None,
        };

        let public_user = PublicUser::from(&user);

        assert_eq!(public_user.uid, user.uid);
        assert_eq!(public_user.display_name, user.display_name);
        assert_eq!(public_user.connect_code, user.connect_code);
        assert_eq!(
            public_user.latest_version,
            LATEST_SLIPPI_CLIENT_VERSION.to_string()
        );
    }
}
