use serde::{Serialize, Serializer};

/// Lỗi chung của app. Khi trả về UI sẽ được chuyển thành chuỗi tiếng Việt.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Lỗi cơ sở dữ liệu: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Lỗi đọc/ghi file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Lỗi ghi file âm thanh: {0}")]
    Wav(#[from] hound::Error),
    #[error("Lỗi âm thanh: {0}")]
    Audio(String),
    #[error("Lỗi dữ liệu: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
