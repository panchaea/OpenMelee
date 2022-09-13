use diesel::prelude::*;
use bson::{oid::ObjectId, Uuid};
use serde::{Deserialize, Serialize};
use regex::Regex;

const CONNECT_CODE_VALIDATION_REGEX: &str = r"^[A-Z]{4}#\d{3}$";

#[derive(Debug, PartialEq, Eq, Clone, Queryable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub play_key: String,
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: Option<String>,
}

impl User {
    pub fn connect_code_is_valid(connect_code: String) -> bool {
        let re = Regex::new(CONNECT_CODE_VALIDATION_REGEX).unwrap();
        re.is_match(&connect_code)
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
        let user_1 = User::new("test".to_string(), "TES_#000".to_string());
        let user_2 = User::new("test".to_string(), "TES#000".to_string());
        let user_3 = User::new("test".to_string(), "TESTZ#000".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
        assert!(user_3.is_none());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_discriminant() {
        let user_1 = User::new("test".to_string(), "TEST#00A".to_string());
        let user_2 = User::new("test".to_string(), "TEST#00".to_string());
        let user_3 = User::new("test".to_string(), "TEST#0001".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
        assert!(user_3.is_none());
    }

    #[test]
    fn cannot_instantiate_user_with_invalid_connect_code_split_marker() {
        let user_1 = User::new("test".to_string(), "TEST/001".to_string());
        let user_2 = User::new("test".to_string(), "TEST?001".to_string());
        let user_3 = User::new("test".to_string(), "TEST'001".to_string());
        assert!(user_1.is_none());
        assert!(user_2.is_none());
        assert!(user_3.is_none());
    }
}
