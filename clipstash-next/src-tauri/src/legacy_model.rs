use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageView {
    Normal,
    Archived,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Newest,
    Oldest,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyMessageImage {
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Serialize)]
pub struct LegacyMessage {
    pub id: i64,
    pub text_content: Option<String>,
    pub created_at: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub images: Vec<LegacyMessageImage>,
}

#[derive(Serialize)]
pub struct LegacyMessagePage {
    pub view: String,
    pub sort: String,
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub has_more: bool,
    pub messages: Vec<LegacyMessage>,
}
