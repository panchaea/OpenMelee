use bson::{oid::ObjectId, Uuid};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use wana_kana::utils::{is_char_hiragana, is_char_katakana};

use crate::schema::users;

const CONNECT_CODE_SEPARATOR: &str = "#";
const CONNECT_CODE_MAX_LENGTH: usize = 8;
const CONNECT_CODE_TAG_VALID_PUNCTUATION: &[&char] = &[
    &'+', &'-', &'=', &'!', &'?', &'@', &'%', &'&', &'$', &'.', &' ', &'｡', &'､',
];

#[derive(Debug, PartialEq, Eq, Clone, Queryable, Insertable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[diesel(table_name = users)]
pub struct User {
    pub uid: String,
    pub play_key: String,
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: Option<String>,
}

impl User {
    fn tag_is_valid(tag: &str) -> bool {
        tag.len() > 0
            && tag.chars().all(|c| {
                is_char_hiragana(c)
                    || is_char_katakana(c)
                    || char::is_ascii_alphanumeric(&c)
                    || CONNECT_CODE_TAG_VALID_PUNCTUATION.contains(&&c)
            })
    }

    fn discriminant_is_valid(discriminant: &str) -> bool {
        discriminant.len() > 0 && discriminant.chars().all(char::is_numeric)
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

    pub fn new(display_name: String, connect_code: String) -> Option<User> {
        if Self::connect_code_is_valid(connect_code.clone()) && !display_name.is_empty() {
            Some(User {
                uid: format!("{}", Uuid::new()),
                play_key: ObjectId::new().to_hex(),
                display_name,
                connect_code,
                latest_version: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use bson::{oid::ObjectId, Uuid};

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
        assert!(user.is_some());
        assert!(ObjectId::parse_str(user.clone().unwrap().play_key).is_ok());
        assert!(Uuid::parse_str(user.clone().unwrap().uid).is_ok());
        assert_eq!(user.clone().unwrap().display_name, "test");
        assert_eq!(user.clone().unwrap().connect_code, "TEST#001");
    }

    #[test]
    fn cannot_instantiate_user_with_empty_display_name() {
        let user = User::new("".to_string(), "TEST#001".to_string());
        assert!(user.is_none());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_id() {
        let user_1 = User::new("test".to_string(), "TE❤T#000".to_string());
        let user_2 = User::new("test".to_string(), "TESTZ#000".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_discriminant() {
        let user_1 = User::new("test".to_string(), "TEST#00A".to_string());
        let user_2 = User::new("test".to_string(), "TEST##00".to_string());
        let user_3 = User::new("test".to_string(), "TEST#0001".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
        assert!(user_3.is_none());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_separator() {
        let user_1 = User::new("test".to_string(), "TEST/001".to_string());
        let user_2 = User::new("test".to_string(), "TEST?001".to_string());
        let user_3 = User::new("test".to_string(), "TEST'001".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
        assert!(user_3.is_none());
    }
}
