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

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeDirection {
    Up,
    Down,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyMessageImage {
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub exists: bool,
}

/// 钉住位置：置顶 / 不钉 / 置底。库内用 1 / 0 / -1 存储，
/// 这样 Normal 视图沿用 `COALESCE(pinned, 0) DESC` 排序即可得到「置顶在最前、置底在最后」，
/// 旧库里的布尔值 1 正好是置顶。
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePin {
    None,
    Top,
    Bottom,
}

impl MessagePin {
    pub(crate) fn from_stored(value: i64) -> Self {
        match value {
            1 => Self::Top,
            -1 => Self::Bottom,
            _ => Self::None,
        }
    }

    pub(crate) fn to_stored(self) -> i64 {
        match self {
            Self::Top => 1,
            Self::None => 0,
            Self::Bottom => -1,
        }
    }
}

#[derive(Serialize)]
pub struct LegacyMessage {
    pub id: i64,
    pub text_content: Option<String>,
    pub created_at: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub pin: MessagePin,
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
