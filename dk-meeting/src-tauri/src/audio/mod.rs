//! Thu âm thanh. Đây là module DUY NHẤT được gọi API riêng của từng hệ điều hành.
//!
//! - `MicSource` (cpal) chạy trên mọi OS.
//! - M2: `windows::ProcessLoopback` / `windows::SystemLoopback` (WASAPI).
//! - M7: `macos::ProcessTap` (Core Audio process tap, macOS 14.4+).
//!
//! Mọi nguồn đều implement `AudioSource`, nên `Recorder` và UI không cần biết nguồn đến từ đâu.

pub mod level;
pub mod mic;
pub mod recorder;
pub mod wav_writer;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// Track ghi âm. Mỗi track = 1 file riêng, đồng thời là nhãn người nói thô.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Track {
    /// Micro — "Phía mình".
    Mic,
    /// Âm thanh hệ thống / app họp — "Phía online".
    System,
}

impl Track {
    pub fn as_str(self) -> &'static str {
        match self {
            Track::Mic => "mic",
            Track::System => "system",
        }
    }
}

/// Định dạng gốc của thiết bị (trước khi resample).
#[derive(Clone, Copy, Debug)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Loại nguồn thu, dùng để hiển thị và lưu cấu hình cuộc họp.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SourceKind {
    Mic { device: Option<String> },
    SystemLoopback,
    ProcessLoopback { pid: u32, name: String },
}

/// Nơi nhận các khối mẫu f32 (interleaved theo `AudioFormat::channels`).
pub type FrameSink = crossbeam_channel::Sender<Vec<f32>>;
/// Callback báo lỗi từ thread thu âm (vd. rút micro giữa chừng).
pub type ErrorFn = Arc<dyn Fn(String) + Send + Sync>;

/// Handle của một phiên thu đang chạy. Gọi `stop` (hoặc drop) để dừng thu.
pub trait CaptureHandle: Send {
    fn stop(self: Box<Self>);
}

pub trait AudioSource: Send {
    fn kind(&self) -> SourceKind;
    fn track(&self) -> Track;
    fn format(&self) -> AudioFormat;
    /// Bắt đầu thu. Mỗi khối mẫu được gửi qua `sink`. Khi dừng, `sink` bị drop
    /// để phía nhận biết là đã hết dữ liệu.
    fn start(
        self: Box<Self>,
        sink: FrameSink,
        on_error: ErrorFn,
    ) -> AppResult<Box<dyn CaptureHandle>>;
}

/// Thông tin thiết bị thu để hiện trong UI.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    pub name: String,
    pub is_default: bool,
}
