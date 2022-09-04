use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: String,
}
