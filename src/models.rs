use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bson::{oid::ObjectId, Uuid};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteExecutor};
use validator::{Validate, ValidationError, ValidationErrors};
use wana_kana::utils::{is_char_hiragana, is_char_katakana};

const CONNECT_CODE_SEPARATOR: &str = "#";
const CONNECT_CODE_MAX_LENGTH: usize = 8;
const CONNECT_CODE_TAG_VALID_PUNCTUATION: &[&char] = &[
    &'+', &'-', &'=', &'!', &'?', &'@', &'%', &'&', &'$', &'.', &' ', &'｡', &'､',
];

#[derive(Debug, PartialEq, Eq, FromRow, Clone, Validate, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub play_key: String,
    #[validate(length(min = 1), custom = "is_displayable_in_game")]
    pub display_name: String,
    #[validate(
        length(min = 1, max = "CONNECT_CODE_MAX_LENGTH"),
        custom = "User::is_valid_connect_code"
    )]
    pub connect_code: String,
    pub latest_version: Option<String>,
}

impl IntoResponse for User {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl User {
    pub fn is_valid_connect_code(connect_code: &str) -> Result<(), ValidationError> {
        return match connect_code.split_once(CONNECT_CODE_SEPARATOR) {
            Some((tag, discriminant)) => {
                if tag.is_empty() {
                    return Err(ValidationError::new("empty_prefix"));
                }

                if is_displayable_in_game(tag).is_err() {
                    return Err(ValidationError::new("prefix_invalid_characters"));
                }

                if discriminant.is_empty() {
                    return Err(ValidationError::new("empty_discriminant"));
                }

                if !discriminant.chars().all(char::is_numeric) {
                    return Err(ValidationError::new("discriminant_invalid_characters"));
                }

                Ok(())
            }
            _ => Err(ValidationError::new("missing_separator")),
        };
    }

    pub fn new(display_name: String, connect_code: String) -> Result<User, ValidationErrors> {
        let user = User {
            uid: format!("{}", Uuid::new()),
            play_key: ObjectId::new().to_hex(),
            display_name,
            connect_code,
            latest_version: None,
        };

        user.validate().map(|_| user)
    }

    pub async fn get<'a, T: SqliteExecutor<'a>>(
        executor: T,
        uid: String,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>("select * from users where uid = $1")
            .bind(uid)
            .fetch_one(executor)
            .await
    }

    pub async fn create<'a, T: SqliteExecutor<'a>>(
        executor: T,
        display_name: String,
        connect_code: String,
    ) -> Result<User, ValidationErrors> {
        match Self::new(display_name, connect_code) {
            Ok(user) => {
                let _user = user.clone();
                let query_result = sqlx::query("insert into users (uid, play_key, display_name, connect_code, latest_version) values ($1, $2, $3, $4, $5)")
                    .bind(user.uid)
                    .bind(user.play_key)
                    .bind(user.display_name)
                    .bind(user.connect_code)
                    .bind(user.latest_version)
                    .execute(executor)
                    .await;

                match query_result {
                    Ok(_) => Ok(_user),
                    Err(_) => {
                        let mut errors = ValidationErrors::new();
                        let error = ValidationError::new("constraint_validation");
                        errors.add("database", error);
                        Err(errors.clone())
                    }
                }
            }
            Err(errors) => Err(errors),
        }
    }
}

fn is_displayable_in_game(s: &str) -> Result<(), ValidationError> {
    if s.chars().all(|c| {
        is_char_hiragana(c)
            || is_char_katakana(c)
            || char::is_ascii_alphanumeric(&c)
            || CONNECT_CODE_TAG_VALID_PUNCTUATION.contains(&&c)
    }) {
        return Ok(());
    }

    Err(ValidationError::new(
        "Not displayable with Melee's character set",
    ))
}

#[cfg(test)]
mod test {
    use bson::{oid::ObjectId, Uuid};
    use sqlx::{Pool, Row, Sqlite};

    use crate::models::*;

    #[test]
    fn connect_code_with_katakana_is_valid() {
        assert!(User::is_valid_connect_code("リッピー#0").is_ok());
    }

    #[test]
    fn connect_code_with_hiragana_is_valid() {
        assert!(User::is_valid_connect_code("やまと#99").is_ok());
    }

    #[test]
    fn connect_code_with_valid_punctuation_is_valid() {
        assert!(User::is_valid_connect_code("&-.%#123").is_ok());
        assert!(User::is_valid_connect_code("+?A!#524").is_ok());
        assert!(User::is_valid_connect_code("｡  9#558").is_ok());
    }

    #[test]
    fn connect_code_with_invalid_punctuation_is_not_valid() {
        assert_eq!(
            User::is_valid_connect_code("!!!*#958").unwrap_err().code,
            "prefix_invalid_characters"
        );
        assert_eq!(
            User::is_valid_connect_code("()''#88").unwrap_err().code,
            "prefix_invalid_characters"
        );
        assert_eq!(
            User::is_valid_connect_code("AAAA#AA").unwrap_err().code,
            "discriminant_invalid_characters"
        );
    }

    #[test]
    fn connect_code_with_empty_tag_or_discriminant_is_not_valid() {
        assert_eq!(
            User::is_valid_connect_code("TEST#").unwrap_err().code,
            "empty_discriminant"
        );
        assert_eq!(
            User::is_valid_connect_code("#0001").unwrap_err().code,
            "empty_prefix"
        );
    }

    #[test]
    fn connect_code_without_separator_is_not_valid() {
        assert_eq!(
            User::is_valid_connect_code("TEST001").unwrap_err().code,
            "missing_separator"
        );
    }

    #[test]
    fn can_instantiate_user_with_display_name_and_valid_connect_code() {
        let user = User::new("test".to_string(), "TEST#001".to_string());
        assert!(user.is_ok());
        assert!(ObjectId::parse_str(user.clone().unwrap().play_key).is_ok());
        assert!(Uuid::parse_str(user.clone().unwrap().uid).is_ok());
        assert_eq!(user.clone().unwrap().display_name, "test");
        assert_eq!(user.clone().unwrap().connect_code, "TEST#001");
    }

    #[test]
    fn cannot_instantiate_user_with_empty_display_name() {
        let user = User::new("".to_string(), "TEST#001".to_string());
        assert!(user.is_err());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_id() {
        let user_1 = User::new("test".to_string(), "TE❤T#000".to_string());
        let user_2 = User::new("test".to_string(), "TESTZ#000".to_string());
        assert!(user_1.is_err());
        assert!(user_2
            .unwrap_err()
            .field_errors()
            .into_iter()
            .any(|(field, errs)| field == "connect_code"
                && errs
                    .iter()
                    .map(|err| err.code.to_string())
                    .collect::<Vec<String>>()
                    == vec!["length".to_string()]));
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_discriminant() {
        let user_1 = User::new("test".to_string(), "TEST#00A".to_string());
        let user_2 = User::new("test".to_string(), "TEST##00".to_string());
        let user_3 = User::new("test".to_string(), "TEST#0001".to_string());
        assert!(user_1.is_err());
        assert!(user_2.is_err());
        assert!(user_3.is_err());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_separator() {
        let user_1 = User::new("test".to_string(), "TEST/001".to_string());
        let user_2 = User::new("test".to_string(), "TEST?001".to_string());
        let user_3 = User::new("test".to_string(), "TEST'001".to_string());
        assert!(user_1.is_err());
        assert!(user_2.is_err());
        assert!(user_3.is_err());
    }

    #[sqlx::test]
    async fn can_create_user_and_get_by_uid(pool: Pool<Sqlite>) {
        let user = User::create(&pool, "test".to_string(), "TEST#001".to_string())
            .await
            .expect("Could not create user");

        let user_from_db = User::get(&pool, user.uid.clone())
            .await
            .expect("Could not get user");

        assert_eq!(user, user_from_db);
    }

    #[sqlx::test]
    async fn cannot_create_two_users_with_same_connect_code(pool: Pool<Sqlite>) {
        User::create(&pool, "test".to_string(), "TEST#001".to_string())
            .await
            .expect("Could not create user");

        let user_with_same_connect_code =
            User::create(&pool, "test".to_string(), "TEST#001".to_string()).await;

        assert!(user_with_same_connect_code.is_err());
        assert!(user_with_same_connect_code
            .unwrap_err()
            .field_errors()
            .contains_key("database"));
        assert_eq!(
            sqlx::query("select count(uid) from users")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<i64, usize>(0),
            1
        )
    }
}
