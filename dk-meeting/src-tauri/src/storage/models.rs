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
    pub segments: Vec<Segment>,
    pub minutes: Option<MinutesRecord>,
    pub action_items: Vec<ActionItemRow>,
    /// Đang nhận dạng / dịch nốt phần cuối sau khi dừng ghi.
    pub processing: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingPatch {
    pub title: Option<String>,
    pub project: Option<String>,
    pub client: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: i64,
    pub meeting_id: String,
    pub track: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub lang: String,
    pub text: String,
    pub translation: Option<String>,
    pub edited: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionItemRow {
    #[serde(default)]
    pub id: i64,
    pub task: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub deadline: String,
    /// "todo" | "doing" | "done"
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub source_ms: Option<i64>,
}

fn default_status() -> String {
    "todo".into()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinutesRecord {
    pub data: serde_json::Value,
    pub model: String,
    pub edited: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Tên micro; None = micro mặc định của Windows.
    pub mic_device: Option<String>,
    /// "auto" | "vi" | "en"
    pub language: String,
    /// Chỉ có "dark".
    pub theme: String,
    /// Bật phụ đề trực tiếp (whisper).
    pub stt_enabled: bool,
    /// id model whisper, xem `stt::models::WHISPER_MODELS`.
    pub whisper_model: String,
    /// Bật dịch trực tiếp sang tiếng Việt.
    pub translate_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Đã qua màn hình "Kiểm tra hệ thống" lần đầu.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mic_device: None,
            language: "auto".into(),
            theme: "dark".into(),
            stt_enabled: true,
            whisper_model: crate::stt::models::DEFAULT_MODEL.into(),
            translate_enabled: true,
            ollama_url: crate::llm::ollama::DEFAULT_URL.into(),
            ollama_model: crate::llm::ollama::DEFAULT_MODEL.into(),
            onboarded: false,
        }
    }
}
