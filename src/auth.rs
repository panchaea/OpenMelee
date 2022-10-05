// based on:
// https://github.com/tokio-rs/axum/blob/0.5.x/examples/jwt/src/main.rs

use std::io::prelude::Read;

use async_trait::async_trait;
use axum::{
    extract::{FromRequest, RequestParts},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::PrivateCookieJar;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use sqlx::SqliteExecutor;
use tera::Context;

use crate::models::User;

pub const JWT_COOKIE_NAME: &str = "token";
pub const JWT_COOKIE_DURATION_HOURS: i64 = 1;

static JWT_KEYS: Lazy<Keys> = Lazy::new(|| {
    let jwt_secret_file_path = crate::CONFIG.jwt_secret_path.as_ref().unwrap();
    let mut buffer = String::new();
    let mut file = std::fs::File::open(jwt_secret_file_path.clone())
        .expect(format!("Unable to open {}", jwt_secret_file_path).as_str());

    file.read_to_string(&mut buffer)
        .expect(format!("Unable to read {}", jwt_secret_file_path).as_str());

    return Keys::new(buffer.trim().as_bytes());
});

pub async fn create_token<'a, T: SqliteExecutor<'a>>(
    executor: T,
    payload: &AuthPayload,
) -> Result<String, AuthError> {
    match User::get_user_from_credentials(
        executor,
        payload.username.clone(),
        payload.password.clone(),
    )
    .await
    {
        Some(user) => {
            let claims = Claims {
                uid: user.uid,
                exp: usize::try_from(
                    (Utc::now() + Duration::hours(JWT_COOKIE_DURATION_HOURS)).timestamp(),
                )
                .unwrap(),
            };

            encode(&Header::default(), &claims, &JWT_KEYS.encoding)
                .map_err(|_| AuthError::TokenCreation)
        }
        None => Err(AuthError::WrongCredentials),
    }
}

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub uid: String,
    pub exp: usize,
}

#[async_trait]
impl<B> FromRequest<B> for Claims
where
    B: Send,
{
    type Rejection = AuthError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        PrivateCookieJar::<cookie::Key>::from_request(req)
            .await
            .map_err(|_| AuthError::InvalidToken)
            .and_then(|jar| {
                let cookie = jar.get(JWT_COOKIE_NAME);

                if cookie.is_none() {
                    return Err(AuthError::InvalidToken);
                }

                let claim = Claims::try_from(cookie.as_ref().unwrap().value());

                if claim.is_err() {
                    let _result = jar.remove(cookie.unwrap());
                }

                return Ok(claim);
            })
            .unwrap_or(Err(AuthError::InvalidToken))
    }
}

impl TryFrom<&str> for Claims {
    type Error = AuthError;
    fn try_from(token: &str) -> Result<Claims, AuthError> {
        let token_data = decode::<Claims>(token, &JWT_KEYS.decoding, &Validation::default())
            .map_err(|_| AuthError::InvalidToken)?;
        Ok(token_data.claims)
    }
}

#[derive(Deserialize)]
pub struct AuthPayload {
    pub username: String,
    pub password: SecretString,
}

#[derive(Deserialize, Serialize)]
pub struct PublicAuthPayload {
    pub username: String,
}

impl From<&AuthPayload> for PublicAuthPayload {
    fn from(auth_payload: &AuthPayload) -> PublicAuthPayload {
        PublicAuthPayload {
            username: auth_payload.username.to_string(),
        }
    }
}

pub enum AuthError {
    WrongCredentials,
    TokenCreation,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match self {
            AuthError::WrongCredentials => StatusCode::UNAUTHORIZED,
            AuthError::TokenCreation => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::InvalidToken => StatusCode::BAD_REQUEST,
        };

        let content = crate::TEMPLATES
            .render("unauthorized.html.tera", &Context::new())
            .unwrap();

        (status_code, Html(content)).into_response()
    }
}
