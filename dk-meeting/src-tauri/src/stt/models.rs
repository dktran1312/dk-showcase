//! Danh mục model whisper + tải / xoá model trong thư mục dữ liệu của app.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::net;

const HF: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    /// RAM/VRAM tối thiểu nên có (GB).
    pub min_mem_gb: u32,
    pub description: &'static str,
}

pub const WHISPER_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "large-v3-turbo-q5_0",
        label: "Large v3 Turbo",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 574,
        min_mem_gb: 4,
        description: "Chính xác nhất cho tiếng Việt. Nên dùng khi có GPU.",
    },
    ModelInfo {
        id: "medium-q5_0",
        label: "Medium",
        file: "ggml-medium-q5_0.bin",
        size_mb: 539,
        min_mem_gb: 4,
        description: "Khá chính xác, chậm hơn Turbo trên CPU.",
    },
    ModelInfo {
        id: "small-q5_1",
        label: "Small",
        file: "ggml-small-q5_1.bin",
        size_mb: 190,
        min_mem_gb: 2,
        description: "Cho máy không có GPU. Nhanh, độ chính xác trung bình.",
    },
    ModelInfo {
        id: "base-q5_1",
        label: "Base",
        file: "ggml-base-q5_1.bin",
        size_mb: 60,
        min_mem_gb: 1,
        description: "Rất nhanh, kém chính xác. Chỉ để thử.",
    },
];

pub const DEFAULT_MODEL: &str = "large-v3-turbo-q5_0";

pub const VAD_FILE: &str = "ggml-silero-v5.1.2.bin";
const VAD_URL: &str =
    "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin";

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    WHISPER_MODELS.iter().find(|m| m.id == id)
}

pub fn model_path(dir: &Path, m: &ModelInfo) -> PathBuf {
    dir.join(m.file)
}

pub fn vad_path(dir: &Path) -> PathBuf {
    dir.join(VAD_FILE)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelState {
    #[serde(flatten)]
    pub info: ModelInfo,
    pub installed: bool,
}

pub fn list(dir: &Path) -> Vec<ModelState> {
    WHISPER_MODELS
        .iter()
        .map(|m| ModelState {
            info: *m,
            installed: model_path(dir, m).exists(),
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

/// Tải model (và model VAD nếu chưa có). Ghi ra file `.part` rồi đổi tên khi xong,
/// nên tải dở / mất mạng không để lại file hỏng.
pub fn download(
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(DownloadProgress),
) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    if !vad_path(dir).exists() {
        download_file(VAD_URL, &vad_path(dir), cancel, &|_, _| {})?;
    }
    let m = find(id).ok_or_else(|| AppError::msg("Model không hợp lệ"))?;
    let report = |downloaded, total| {
        progress(DownloadProgress {
            id: id.into(),
            downloaded,
            total,
            done: false,
            error: None,
        })
    };
    download_file(
        &format!("{HF}/{}", m.file),
        &model_path(dir, m),
        cancel,
        &report,
    )
}

pub fn delete(dir: &Path, id: &str) -> AppResult<()> {
    let m = find(id).ok_or_else(|| AppError::msg("Model không hợp lệ"))?;
    let p = model_path(dir, m);
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

fn download_file(
    url: &str,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64, u64),
) -> AppResult<()> {
    let part = dest.with_extension("part");
    let mut resp = net::agent(None)
        .get(url)
        .call()
        .map_err(|e| AppError::msg(format!("Không tải được model (kiểm tra mạng): {e}")))?;
    let total = resp.body().content_length().unwrap_or(0);
    let mut reader = resp.body_mut().as_reader();
    let mut file = std::io::BufWriter::new(std::fs::File::create(&part)?);
    let mut buf = vec![0u8; 256 * 1024];
    let mut downloaded = 0u64;
    let mut last = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(AppError::msg("Đã huỷ tải"));
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| AppError::msg(format!("Mất kết nối khi tải: {e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        if last.elapsed() > Duration::from_millis(250) {
            last = Instant::now();
            progress(downloaded, total);
        }
    }
    file.flush()?;
    drop(file);
    if total > 0 && downloaded != total {
        let _ = std::fs::remove_file(&part);
        return Err(AppError::msg("File tải về không đầy đủ, hãy thử lại."));
    }
    std::fs::rename(&part, dest)?;
    progress(downloaded, total.max(downloaded));
    Ok(())
}

/// Gợi ý model theo cấu hình máy.
pub fn recommend(gpu_vram_gb: Option<f32>, ram_gb: f32) -> &'static str {
    match gpu_vram_gb {
        Some(v) if v >= 4.0 && super::gpu_compiled() => "large-v3-turbo-q5_0",
        _ if ram_gb >= 12.0 => "small-q5_1",
        _ => "base-q5_1",
    }
}
