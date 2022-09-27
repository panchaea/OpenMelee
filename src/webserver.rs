use std::net::SocketAddr;

use axum::{
    body::{boxed, Full},
    extract::Path,
    handler::Handler,
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Extension, Form, Json, Router,
};
use axum_sqlx_tx::Tx;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool};
use tera::{Context, Tera};

use slippi_re::{models::*, Asset, Config, LATEST_SLIPPI_CLIENT_VERSION};

#[derive(Serialize, Deserialize)]
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
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    uid: String,
    display_name: String,
    connect_code: String,
    latest_version: String,
}

impl IntoResponse for PublicUser {
    fn into_response(self) -> Response {
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

async fn index(Extension(tera): Extension<Tera>) -> Html<String> {
    let content = tera.render("index.html.tera", &Context::new()).unwrap();
    Html(content)
}

async fn not_found(Extension(tera): Extension<Tera>) -> Html<String> {
    let content = tera.render("404.html.tera", &Context::new()).unwrap();
    Html(content)
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
    })
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();

    match Asset::get(path.as_str()) {
        Some(content) => {
            let body = boxed(Full::from(content.data));
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(body)
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(boxed(Full::from("Not Found")))
            .unwrap(),
    }
}

fn load_templates() -> Tera {
    let templates = Asset::iter()
        .into_iter()
        .filter(|asset_path| asset_path.ends_with(".tera"))
        .map(move |asset_path| {
            let _asset_path = asset_path.clone();
            let asset = Asset::get(&_asset_path).unwrap();
            let contents = std::str::from_utf8(asset.data.as_ref()).unwrap();

            (
                std::path::Path::new(&asset_path.to_string())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                contents.to_string(),
            )
        });

    let mut tera = Tera::new("assets/templates/*.tera").expect("Failed to read templates");

    tera.add_raw_templates(templates)
        .expect("Failed to parse templates");

    tera
}

async fn app(pool: SqlitePool) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/user", post(create_user))
        .route("/user/:uid", get(get_user))
        .route("/static/*file", static_handler.into_service())
        .fallback(get(not_found))
        .layer(axum_sqlx_tx::Layer::new(pool))
        .layer(Extension(load_templates()))
}

pub async fn start_server(config: Config, pool: SqlitePool) -> Result<(), ()> {
    let server = axum::Server::bind(&SocketAddr::from((
        config.webserver_address,
        config.webserver_port,
    )))
    .serve(app(pool).await.into_make_service());

    println!(
        "Web server listening on http://{}:{}",
        config.webserver_address, config.webserver_port
    );

    server.await.map_err(|_| ())
}

#[cfg(test)]
mod test {
    use std::net::TcpListener;

    use sqlx::Pool;

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

    #[sqlx::test]
    async fn can_create_user(pool: Pool<Sqlite>) {
        let config = Config::default();
        let addr = format!("{}:{}", config.webserver_address, config.webserver_port);
        let listener = TcpListener::bind(addr.parse::<SocketAddr>().unwrap()).unwrap();

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app(pool).await.into_make_service())
                .await
                .unwrap();
        });

        let client = reqwest::Client::new();

        let create_user_response = client
            .post(format!("http://{}/user", addr))
            .form(&UserForm {
                display_name: "test".to_string(),
                connect_code: "TEST#001".to_string(),
            })
            .send()
            .await;

        assert!(create_user_response.is_ok());

        let created_user = create_user_response
            .unwrap()
            .json::<PublicUser>()
            .await
            .expect("Could not convert create_user response to JSON");

        assert_eq!(created_user.display_name, "test".to_string());
        assert_eq!(created_user.connect_code, "TEST#001".to_string());
    }

    #[test]
    fn can_render_index() {
        let tera = load_templates();
        assert!(tera.render("index.html.tera", &Context::new()).is_ok());
    }
}
