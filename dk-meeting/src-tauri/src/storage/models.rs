use serde::{Deserialize, Serialize};

pub mod status {
    pub const RECORDING: &str = "recording";
    pub const PAUSED: &str = "paused";
    pub const DONE: &str = "done";
    pub const INTERRUPTED: &str = "interrupted";
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub project: String,
    pub client: String,
    pub language: String,
    pub status: String,
    pub created_at: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub notes: String,
    pub bookmark_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioChunk {
    pub track: String,
    pub idx: i64,
    pub file_name: String,
    /// Đường dẫn tuyệt đối — UI dùng `convertFileSrc` để phát.
    pub path: String,
    pub sample_rate: i64,
    pub start_ms: i64,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: i64,
    pub meeting_id: String,
    pub at_ms: i64,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    pub meeting: Meeting,
    pub chunks: Vec<AudioChunk>,
    pub bookmarks: Vec<Bookmark>,
    pub folder: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingPatch {
    pub title: Option<String>,
    pub project: Option<String>,
    pub client: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Tên micro; None = micro mặc định của Windows.
    pub mic_device: Option<String>,
    /// "auto" | "vi" | "en"
    pub language: String,
    /// Chỉ có "dark" ở M1.
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mic_device: None,
            language: "auto".into(),
            theme: "dark".into(),
        }
    }
}
