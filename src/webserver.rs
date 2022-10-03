use std::net::SocketAddr;

use axum::{
    body::{boxed, Full},
    extract::Path,
    handler::Handler,
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Form, Json, Router,
};
use axum_sqlx_tx::Tx;
use secrecy::SecretString;
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

#[derive(Debug, Deserialize)]
pub struct UserForm {
    pub username: String,
    pub password: SecretString,
    pub display_name: String,
    pub connect_code: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PublicUserForm {
    pub username: String,
    pub display_name: String,
    pub connect_code: String,
}

impl From<&UserForm> for PublicUserForm {
    fn from(user_form: &UserForm) -> PublicUserForm {
        PublicUserForm {
            username: user_form.username.clone(),
            display_name: user_form.display_name.clone(),
            connect_code: user_form.connect_code.clone(),
        }
    }
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

async fn register(Extension(tera): Extension<Tera>) -> Html<String> {
    let mut context = Context::new();
    context.insert("field_errors", &false);
    context.insert("field_values", &false);
    let content = tera.render("register.html.tera", &context).unwrap();
    Html(content)
}

async fn register_form(
    tx: Tx<Sqlite>,
    Form(user_form): Form<UserForm>,
    Extension(tera): Extension<Tera>,
) -> impl IntoResponse {
    User::check_constraints_and_create(
        tx,
        user_form.username.to_string(),
        user_form.password.clone(),
        user_form.display_name.to_string(),
        user_form.connect_code.to_string(),
    )
    .await
    .map(|_| Redirect::to("/"))
    .map_err(|errors| {
        let mut context = Context::new();
        context.insert("field_errors", &errors.field_errors());
        context.insert("field_values", &PublicUserForm::from(&user_form));
        let content = tera.render("register.html.tera", &context).unwrap();
        (StatusCode::BAD_REQUEST, Html(content))
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

}

async fn app(pool: SqlitePool) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/register", get(register))
        .route("/register", post(register_form))
        .route("/user/:uid", get(get_user))
        .route("/static/*file", static_handler.into_service())
        .fallback(get(not_found))
        .layer(axum_sqlx_tx::Layer::new(pool))
        .layer(Extension(slippi_re::TEMPLATES.clone()))
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
    use std::str::FromStr;

    use rand::Rng;
    use serde_json::json;
    use sqlx::Pool;

    use crate::webserver::*;

    const TEST_USER_PASSWORD: &str = "5~}Eau&b5C1df.LI_|mOXnl0";

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

    async fn test_register_form(
        tx: Tx<Sqlite>,
        Form(user_form): Form<PublicUserForm>,
    ) -> impl IntoResponse {
        let password: SecretString = SecretString::from_str(TEST_USER_PASSWORD).unwrap();

        User::check_constraints_and_create(
            tx,
            user_form.username.to_string(),
            password,
            user_form.display_name.to_string(),
            user_form.connect_code.to_string(),
        )
        .await
        .map(|user| PublicUser::from(&user))
        .map_err(|errors| {
            let body = json!({ "errors": errors });
            (StatusCode::BAD_REQUEST, body.to_string())
        })
    }

    async fn start_test_server(pool: Pool<Sqlite>) -> (String, reqwest::Client) {
        let mut rng = rand::thread_rng();
        let config = Config::default();
        let port: u16 = rng.gen_range(config.webserver_port..10000);
        let addr = format!("{}:{}", config.webserver_address, port);
        let listener = TcpListener::bind(addr.parse::<SocketAddr>().unwrap()).unwrap();

        // Create a custom router which serves normal routes,
        // except for POST /register, which creates users with
        // a constant password and returns JSON
        let test_app: Router = Router::new()
            .route("/", get(index))
            .route("/register", get(register))
            .route("/register", post(test_register_form))
            .route("/user/:uid", get(get_user))
            .route("/static/*file", static_handler.into_service())
            .fallback(get(not_found))
            .layer(axum_sqlx_tx::Layer::new(pool))
            .layer(Extension(slippi_re::TEMPLATES.clone()));

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(test_app.into_make_service())
                .await
                .unwrap();
        });

        (addr, reqwest::Client::new())
    }

    fn extract_errors<'a>(res: &'a serde_json::Value, field: &str) -> Vec<&'a str> {
        let error_codes = res
            .get("errors")
            .unwrap()
            .get(field)
            .unwrap()
            .as_array()
            .unwrap()
            .into_iter()
            .map(|err| err.get("code").unwrap().as_str().unwrap());

        error_codes.collect::<Vec<&str>>()
    }

    #[sqlx::test]
    async fn can_register(pool: Pool<Sqlite>) {
        let (addr, client) = start_test_server(pool.clone()).await;

        let register_response = client
            .post(format!("http://{}/register", addr))
            .form(&PublicUserForm {
                username: "test".to_string(),
                display_name: "test".to_string(),
                connect_code: "TEST#001".to_string(),
            })
            .send()
            .await;

        assert!(register_response.is_ok());

        let created_user = register_response
            .unwrap()
            .json::<PublicUser>()
            .await
            .expect("Could not convert register_response to JSON");

        assert_eq!(created_user.display_name, "test".to_string());
        assert_eq!(created_user.connect_code, "TEST#001".to_string());

        assert!(User::get(&pool, created_user.uid).await.is_ok());
    }

    #[sqlx::test]
    async fn cannot_register_with_errors(pool: Pool<Sqlite>) {
        let (addr, client) = start_test_server(pool).await;

        let register_response = client
            .post(format!("http://{}/register", addr))
            .form(&PublicUserForm {
                username: "test".to_string(),
                display_name: "".to_string(),
                connect_code: "TEST#".to_string(),
            })
            .send()
            .await;

        let res: serde_json::Value =
            serde_json::from_str(&register_response.unwrap().text().await.unwrap()).unwrap();

        assert_eq!(
            extract_errors(&res.clone(), "connect_code"),
            vec!["discriminant_length"]
        );

        assert_eq!(extract_errors(&res.clone(), "display_name"), vec!["length"]);
    }

    #[sqlx::test]
    async fn cannot_register_with_existing_connect_code_or_username(pool: Pool<Sqlite>) {
        let (addr, client) = start_test_server(pool).await;

        client
            .post(format!("http://{}/register", addr))
            .form(&PublicUserForm {
                username: "test".to_string(),
                display_name: "test".to_string(),
                connect_code: "TEST#001".to_string(),
            })
            .send()
            .await
            .expect("First registration attempt failed");

        let register_response = client
            .post(format!("http://{}/register", addr))
            .form(&PublicUserForm {
                username: "test".to_string(),
                display_name: "test".to_string(),
                connect_code: "TEST#001".to_string(),
            })
            .send()
            .await;

        let res: serde_json::Value =
            serde_json::from_str(&register_response.unwrap().text().await.unwrap()).unwrap();

        assert_eq!(
            extract_errors(&res.clone(), "connect_code"),
            vec!["duplicated"]
        );

        assert_eq!(extract_errors(&res.clone(), "username"), vec!["duplicated"]);
    }

    #[test]
    fn can_render_index() {
        assert!(slippi_re::TEMPLATES
            .render("index.html.tera", &Context::new())
            .is_ok());
    }
}
