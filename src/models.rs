use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_sqlx_tx::Tx;
use bson::{oid::ObjectId, Uuid};
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, FromRow, Row, Sqlite, SqliteExecutor};
use validator::{Validate, ValidationError, ValidationErrors};
use wana_kana::utils::{is_char_hiragana, is_char_katakana};

const CONNECT_CODE_SEPARATOR: &str = "#";
const CONNECT_CODE_MAX_LENGTH: usize = 8;
const NAME_ENTRY_SELECTABLE_PUNCTUATION: &'static [&'static char] = &[
    &'+', &'-', &'=', &'!', &'?', &'@', &'%', &'&', &'#', &'$', &'.', &' ', &'｡', &'､',
];
const OTHER_DISPLAYABLE_PUNCTUATION: &[&char] = &[&'/'];

#[derive(Debug, PartialEq, Eq, FromRow, Clone, Validate, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub play_key: String,
    #[validate(
        length(min = 1, message = "Display name is too short"),
        custom(
            function = "is_displayable_in_game",
            message = "Only uppercase English letters, numbers, hiragana and katakana characters, \
                       spaces, and the following punctuation are allowed: \
                       +, -, /, =, !, ?, @, %, &, #, $, ., ｡, ､"
        )
    )]
    pub display_name: String,
    #[validate(
        length(
            min = 1,
            max = "CONNECT_CODE_MAX_LENGTH",
            message = "Must be at least 1 and at most 8 characters long"
        ),
        custom(
            function = "connect_code_contains_separator",
            message = "Must consist of: \n\
                         * English uppercase letters, numbers, hiragana characters, \
                           or katakana characters, \n\
                         * followed by a # symbol, \n\
                         * followed by at least one number"
        ),
        custom(
            function = "connect_code_prefix_is_not_empty",
            message = "At least one English uppercase letter, number, hiragana character, \
                       or katakana character must be present before the # symbol"
        ),
        custom(
            function = "connect_code_prefix_contains_only_valid_characters",
            message = "Only English uppercase letters, numbers, hiragana characters, and \
                       katakana characters may be present before the # symbol"
        ),
        custom(
            function = "connect_code_discriminant_is_not_empty",
            message = "At least one number must be present after the # symbol"
        ),
        custom(
            function = "connect_code_discriminant_contains_only_numeric_characters",
            message = "Only numbers may be present after the # symbol"
        )
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
    pub fn is_valid_connect_code(connect_code: &str) -> bool {
        connect_code_contains_separator(connect_code).is_ok()
            && connect_code_prefix_is_not_empty(connect_code).is_ok()
            && connect_code_prefix_contains_only_valid_characters(connect_code).is_ok()
            && connect_code_discriminant_is_not_empty(connect_code).is_ok()
            && connect_code_discriminant_contains_only_numeric_characters(connect_code).is_ok()
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

    pub async fn check_constraints_and_create(
        mut tx: Tx<Sqlite>,
        display_name: String,
        connect_code: String,
    ) -> Result<User, ValidationErrors> {
        let mut conn = tx.acquire().await.unwrap();
        let mut errors = ValidationErrors::new();

        if let Some(in_use) = Self::is_connect_code_in_use(conn, connect_code.clone()).await {
            if in_use {
                let mut error = ValidationError::new("duplicated");
                error.message = Some(std::borrow::Cow::Borrowed("Connect code is already in use"));
                errors.add("connect_code", error);
            }
        }

        conn = tx.acquire().await.unwrap();

        if !errors.is_empty() {
            return Err(errors);
        }

        Self::create(conn, display_name, connect_code).await
    }

    pub async fn is_connect_code_in_use<'a, T: SqliteExecutor<'a>>(
        executor: T,
        connect_code: String,
    ) -> Option<bool> {
        match sqlx::query("select count(uid) from users where connect_code = $1")
            .bind(connect_code)
            .fetch_one(executor)
            .await
        {
            Ok(row) => Some(row.get::<i64, usize>(0) > 0),
            _ => None,
        }
    }

    async fn create<'a, T: SqliteExecutor<'a>>(
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
                        let error = ValidationError::new("unknown");
                        errors.add("database", error);
                        Err(errors.clone())
                    }
                }
            }
            Err(errors) => Err(errors),
        }
    }
}

fn is_selectable_in_name_entry(s: &str) -> Result<(), ValidationError> {
    if s.chars().all(|c| {
        is_char_hiragana(c)
            || is_char_katakana(c)
            || char::is_numeric(c)
            || char::is_ascii_uppercase(&c)
            || NAME_ENTRY_SELECTABLE_PUNCTUATION.contains(&&c)
    }) {
        return Ok(());
    }

    Err(ValidationError::new("not_selectable_in_name_entry"))
}

fn is_displayable_in_game(s: &str) -> Result<(), ValidationError> {
    if is_selectable_in_name_entry(s).is_ok()
        || s.chars().all(|c| {
            char::is_ascii_alphanumeric(&c)
                || NAME_ENTRY_SELECTABLE_PUNCTUATION.contains(&&c)
                || OTHER_DISPLAYABLE_PUNCTUATION.contains(&&c)
        })
    {
        return Ok(());
    }

    Err(ValidationError::new("not_displayable_in_game"))
}

fn connect_code_contains_separator(s: &str) -> Result<(), ValidationError> {
    if !s.contains(CONNECT_CODE_SEPARATOR) {
        return Err(ValidationError::new("missing_separator"));
    }

    Ok(())
}

fn connect_code_prefix_is_not_empty(s: &str) -> Result<(), ValidationError> {
    if let Some((prefix, _)) = s.split_once(CONNECT_CODE_SEPARATOR) {
        if prefix.is_empty() {
            return Err(ValidationError::new("empty_prefix"));
        }
    }

    Ok(())
}

fn connect_code_prefix_contains_only_valid_characters(s: &str) -> Result<(), ValidationError> {
    if let Some((prefix, _)) = s.split_once(CONNECT_CODE_SEPARATOR) {
        if !is_selectable_in_name_entry(prefix).is_ok()
            || prefix
                .chars()
                .any(|c| NAME_ENTRY_SELECTABLE_PUNCTUATION.contains(&&c))
        {
            return Err(ValidationError::new("invalid_characters_in_prefix"));
        }
    }

    Ok(())
}

fn connect_code_discriminant_is_not_empty(s: &str) -> Result<(), ValidationError> {
    if let Some((_, discriminant)) = s.split_once(CONNECT_CODE_SEPARATOR) {
        if discriminant.is_empty() {
            return Err(ValidationError::new("empty_discriminant"));
        }
    }

    Ok(())
}

fn connect_code_discriminant_contains_only_numeric_characters(
    s: &str,
) -> Result<(), ValidationError> {
    if let Some((_, discriminant)) = s.split_once(CONNECT_CODE_SEPARATOR) {
        if !discriminant.chars().all(char::is_numeric) {
            return Err(ValidationError::new(
                "non_numeric_characters_in_discriminant",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use bson::{oid::ObjectId, Uuid};
    use sqlx::{Pool, Row, Sqlite};

    use crate::models::*;

    #[test]
    fn connect_code_with_letters_is_valid() {
        assert!(User::is_valid_connect_code("FOO#999"));
    }

    #[test]
    fn connect_code_with_numbers_is_valid() {
        assert!(User::is_valid_connect_code("TEST9#03"));
    }

    #[test]
    fn connect_code_with_katakana_is_valid() {
        assert!(User::is_valid_connect_code("リッピー#0"));
    }

    #[test]
    fn connect_code_with_hiragana_is_valid() {
        assert!(User::is_valid_connect_code("やまと#99"));
    }

    #[test]
    fn connect_code_with_punctuation_is_not_valid() {
        assert!(!User::is_valid_connect_code("&-.%#123"));
        assert!(!User::is_valid_connect_code("+?A!#524"));
        assert!(!User::is_valid_connect_code("｡  9#558"));
        assert!(!User::is_valid_connect_code("!!!*#958"));
        assert!(!User::is_valid_connect_code("()''#88"));
        assert!(!User::is_valid_connect_code("AAAA#AA"));
    }

    #[test]
    fn connect_code_with_lower_case_is_not_valid() {
        assert!(!User::is_valid_connect_code("test#001"));
    }

    #[test]
    fn connect_code_with_empty_tag_or_discriminant_is_not_valid() {
        assert!(!User::is_valid_connect_code("TEST#"));
        assert!(!User::is_valid_connect_code("#0001"));
    }

    #[test]
    fn connect_code_without_separator_is_not_valid() {
        assert!(!User::is_valid_connect_code("TEST001"));
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
    fn can_include_slashes_in_display_name() {
        let user = User::new("site/user".to_string(), "TEST#001".to_string());
        assert!(user.is_ok());
        assert!(ObjectId::parse_str(user.clone().unwrap().play_key).is_ok());
        assert!(Uuid::parse_str(user.clone().unwrap().uid).is_ok());
        assert_eq!(user.clone().unwrap().display_name, "site/user");
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

        // NOTE: we are missing a detailed error code here, since we used
        // User::create instead of User::check_constraints_and_create
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
