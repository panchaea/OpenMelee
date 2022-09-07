use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Queryable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub play_key: String,
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: Option<String>,
}
