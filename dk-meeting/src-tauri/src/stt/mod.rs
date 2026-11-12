//! Speech-to-text: resample 16 kHz mono → VAD cắt theo câu → whisper.cpp (whisper-rs).
//!
//! Luồng của một cuộc họp (`SttSession`):
//! ```text
//! Recorder (mỗi track) ──tap──► thread segmenter: resample → VAD → Utterance ──┐
//!                                                                              ├─► thread whisper ─► SegmentSink
//! Recorder (track khác) ─tap──► thread segmenter ...                         ──┘   (1 thread dùng GPU)
//! ```

pub mod models;
pub mod resample;
pub mod segmenter;
pub mod session;
pub mod transcriber;
pub mod vad;

use serde::Serialize;

use crate::audio::Track;

/// Tần số mẫu whisper yêu cầu.
pub const SAMPLE_RATE: u32 = 16_000;

/// Một câu đã nhận dạng, chưa lưu DB.
#[derive(Clone, Debug)]
pub struct NewSegment {
    pub track: Track,
    pub start_ms: u64,
    pub end_ms: u64,
    pub lang: String,
    pub text: String,
}

/// Trạng thái bộ nhận dạng, đẩy lên UI.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SttStatus {
    Loading { model: String },
    Ready { model: String, gpu: bool },
    Unavailable { reason: String },
    Error { message: String },
}

/// Nơi nhận kết quả của phiên nhận dạng (lưu DB, đẩy UI, gửi đi dịch...).
pub trait SegmentSink: Send + Sync {
    fn on_segment(&self, seg: NewSegment);
    fn on_status(&self, status: SttStatus);
}

/// Thông tin backend whisper.cpp được biên dịch kèm (CUDA / Vulkan / CPU).
pub fn backend_name() -> &'static str {
    if cfg!(feature = "cuda") {
        "CUDA"
    } else if cfg!(feature = "vulkan") {
        "Vulkan"
    } else if cfg!(feature = "metal") {
        "Metal"
    } else {
        "CPU"
    }
}

pub fn gpu_compiled() -> bool {
    backend_name() != "CPU"
}
