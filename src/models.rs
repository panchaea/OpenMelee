use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bson::{oid::ObjectId, Uuid};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteExecutor};
use wana_kana::utils::{is_char_hiragana, is_char_katakana};

const CONNECT_CODE_SEPARATOR: &str = "#";
const CONNECT_CODE_MAX_LENGTH: usize = 8;
const CONNECT_CODE_TAG_VALID_PUNCTUATION: &[&char] = &[
    &'+', &'-', &'=', &'!', &'?', &'@', &'%', &'&', &'$', &'.', &' ', &'｡', &'､',
];

#[derive(Debug, PartialEq, Eq, FromRow, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub play_key: String,
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: Option<String>,
}

impl IntoResponse for User {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl User {
    fn is_displayable(s: &str) -> bool {
        s.chars().all(|c| {
            is_char_hiragana(c)
                || is_char_katakana(c)
                || char::is_ascii_alphanumeric(&c)
                || CONNECT_CODE_TAG_VALID_PUNCTUATION.contains(&&c)
        })
    }

    fn tag_is_valid(tag: &str) -> bool {
        !tag.is_empty() && Self::is_displayable(tag)
    }

    fn discriminant_is_valid(discriminant: &str) -> bool {
        !discriminant.is_empty() && discriminant.chars().all(char::is_numeric)
    }

    pub fn connect_code_is_valid(connect_code: String) -> bool {
        if connect_code.chars().count() > CONNECT_CODE_MAX_LENGTH {
            return false;
        }

        return match connect_code.split_once(CONNECT_CODE_SEPARATOR) {
            Some((tag, discriminant)) => {
                Self::tag_is_valid(tag) && Self::discriminant_is_valid(discriminant)
            }
            _ => false,
        };
    }

    pub fn display_name_is_valid(display_name: String) -> bool {
        !display_name.is_empty() && Self::is_displayable(&display_name)
    }

    pub fn new(display_name: String, connect_code: String) -> Result<User, Vec<CreateUserError>> {
        let connect_code_valid = Self::connect_code_is_valid(connect_code.clone());
        let display_name_valid = Self::display_name_is_valid(display_name.clone());
        let mut errors = vec![];

        if connect_code_valid && display_name_valid {
            return Ok(User {
                uid: format!("{}", Uuid::new()),
                play_key: ObjectId::new().to_hex(),
                display_name,
                connect_code,
                latest_version: None,
            });
        }

        if !connect_code_valid {
            errors.push(CreateUserError::InvalidConnectCode);
        }

        if !display_name_valid {
            errors.push(CreateUserError::InvalidDisplayName);
        }

        Err(errors)
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
    ) -> Result<User, Vec<CreateUserError>> {
        match Self::new(display_name, connect_code) {
            Ok(user) => {
                let _user = user.clone();
                sqlx::query("insert into users (uid, play_key, display_name, connect_code, latest_version) values ($1, $2, $3, $4, $5)")
                    .bind(user.uid)
                    .bind(user.play_key)
                    .bind(user.display_name)
                    .bind(user.connect_code)
                    .bind(user.latest_version)
                    .execute(executor)
                    .await
                    .map(|_| _user)
                    .map_err(|_| vec![CreateUserError::DuplicateConnectCode])
            }
            Err(errors) => Err(errors),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CreateUserError {
    DuplicateConnectCode,
    InvalidConnectCode,
    InvalidDisplayName,
}

impl ToString for CreateUserError {
    fn to_string(&self) -> String {
        match self {
            CreateUserError::DuplicateConnectCode => "Connect code is already in use",
            CreateUserError::InvalidConnectCode => "Connect code is not valid",
            CreateUserError::InvalidDisplayName => "Display name is not valid",
        }
        .to_string()
    }
}

#[cfg(test)]
mod test {
    use bson::{oid::ObjectId, Uuid};
    use sqlx::{Pool, Sqlite};

    use crate::models::*;

    #[test]
    fn connect_code_with_katakana_is_valid() {
        assert!(User::connect_code_is_valid("リッピー#0".to_string()));
    }

    #[test]
    fn connect_code_with_hiragana_is_valid() {
        assert!(User::connect_code_is_valid("やまと#99".to_string()));
    }

    #[test]
    fn connect_code_with_valid_punctuation_is_valid() {
        assert!(User::connect_code_is_valid("&-.%#123".to_string()));
        assert!(User::connect_code_is_valid("+?A!#524".to_string()));
        assert!(User::connect_code_is_valid("｡  9#558".to_string()));
    }

    #[test]
    fn connect_code_with_invalid_punctuation_is_not_valid() {
        assert!(!User::connect_code_is_valid("!!!*#958".to_string()));
        assert!(!User::connect_code_is_valid("()''#88".to_string()));
    }

    #[test]
    fn connect_code_with_empty_tag_or_discriminant_is_not_valid() {
        assert!(!User::connect_code_is_valid("TEST#".to_string()));
        assert!(!User::connect_code_is_valid("#0001".to_string()));
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
        assert!(user_2.is_err());
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
}
