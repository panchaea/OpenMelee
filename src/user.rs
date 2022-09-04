use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub display_name: String,
    pub connect_code: String,
    pub latest_version: String,
}
