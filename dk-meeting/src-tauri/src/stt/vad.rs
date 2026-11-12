//! Phát hiện đoạn có tiếng nói, chạy tăng dần (chỉ xử lý âm thanh mới).
//!
//! Âm thanh được chia khung 512 mẫu (32 ms @16 kHz). Mỗi khung có một xác suất "có tiếng nói":
//! - `SileroScorer`: Silero VAD có sẵn trong whisper.cpp (chính xác, cần file model ~1 MB).
//! - `EnergyScorer`: theo năng lượng, dùng khi chưa có model VAD.
//!
//! `FrameVad` giữ xác suất của các khung trong bộ đệm hiện tại và dựng ra các đoạn nói
//! (có trễ bật/tắt, bỏ đoạn quá ngắn, gộp khoảng nghỉ ngắn).

use std::collections::VecDeque;
use std::path::Path;

use whisper_rs::{WhisperVadContext, WhisperVadContextParams};

pub const FRAME: usize = 512;
/// Xử lý theo lô 8 khung (~256 ms) để giảm chi phí gọi.
const BATCH_FRAMES: usize = 8;
/// Ngữ cảnh đưa kèm cho Silero (mạng LSTM cần "khởi động").
const CONTEXT_FRAMES: usize = 16;

pub trait FrameScorer: Send {
    /// Trả về xác suất cho từng khung của `frames` (độ dài là bội của FRAME).
    /// `context` là âm thanh ngay trước đó (có thể rỗng).
    fn score(&mut self, context: &[f32], frames: &[f32]) -> Vec<f32>;
}

pub fn make_scorer(vad_model: Option<&Path>) -> Box<dyn FrameScorer> {
    if let Some(path) = vad_model.filter(|p| p.exists()) {
        match SileroScorer::load(path) {
            Ok(v) => return Box::new(v),
            Err(e) => log::warn!("Không nạp được Silero VAD ({e}), dùng VAD năng lượng"),
        }
    }
    Box::new(EnergyScorer::default())
}

pub struct SileroScorer {
    ctx: WhisperVadContext,
}

impl SileroScorer {
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut p = WhisperVadContextParams::new();
        p.set_n_threads(1);
        p.set_use_gpu(false); // model rất nhỏ; để GPU cho whisper
        let ctx = WhisperVadContext::new(&path.to_string_lossy(), p).map_err(|e| e.to_string())?;
        Ok(Self { ctx })
    }
}

impl FrameScorer for SileroScorer {
    fn score(&mut self, context: &[f32], frames: &[f32]) -> Vec<f32> {
        let n = frames.len() / FRAME;
        let mut input = Vec::with_capacity(context.len() + frames.len());
        input.extend_from_slice(context);
        input.extend_from_slice(frames);
        if self.ctx.detect_speech(&input).is_err() {
            return vec![0.0; n];
        }
        let probs = self.ctx.probabilities();
        let mut out: Vec<f32> = probs.iter().rev().take(n).rev().copied().collect();
        out.resize(n, 0.0);
        out
    }
}

pub struct EnergyScorer {
    noise_floor: f32,
}

impl Default for EnergyScorer {
    fn default() -> Self {
        Self { noise_floor: 0.003 }
    }
}

impl FrameScorer for EnergyScorer {
    fn score(&mut self, _context: &[f32], frames: &[f32]) -> Vec<f32> {
        frames
            .chunks_exact(FRAME)
            .map(|f| {
                let rms = (f.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt();
                // nền nhiễu bám theo mức thấp, tăng chậm
                self.noise_floor = if rms < self.noise_floor {
                    0.9 * self.noise_floor + 0.1 * rms.max(0.0002)
                } else {
                    0.999 * self.noise_floor + 0.001 * rms
                };
                let thr = (self.noise_floor * 4.0).max(0.01);
                if rms > thr {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    }
}

pub struct VadConfig {
    pub on: f32,
    pub off: f32,
    pub min_speech_frames: usize,
    pub merge_gap_frames: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        // 250 ms / 300 ms
        Self {
            on: 0.5,
            off: 0.35,
            min_speech_frames: 8,
            merge_gap_frames: 9,
        }
    }
}

/// Theo dõi xác suất từng khung của bộ đệm hiện tại (đánh số theo mẫu tuyệt đối).
pub struct FrameVad {
    scorer: Box<dyn FrameScorer>,
    cfg: VadConfig,
    /// Mẫu chưa đủ một lô.
    pending: Vec<f32>,
    /// Âm thanh gần nhất làm ngữ cảnh.
    context: VecDeque<f32>,
    probs: VecDeque<f32>,
    /// Vị trí tuyệt đối (mẫu) của khung đầu tiên trong `probs`.
    probs_start: u64,
}

impl FrameVad {
    pub fn new(scorer: Box<dyn FrameScorer>, cfg: VadConfig) -> Self {
        Self {
            scorer,
            cfg,
            pending: Vec::new(),
            context: VecDeque::new(),
            probs: VecDeque::new(),
            probs_start: 0,
        }
    }

    pub fn accept(&mut self, samples: &[f32]) {
        self.pending.extend_from_slice(samples);
        let batch = BATCH_FRAMES * FRAME;
        while self.pending.len() >= batch {
            let chunk: Vec<f32> = self.pending.drain(..batch).collect();
            self.score_chunk(&chunk);
        }
    }

    /// Chấm nốt phần lẻ (khi kết thúc luồng) — đệm 0 cho tròn khung.
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut chunk = std::mem::take(&mut self.pending);
        let n = chunk.len().div_ceil(FRAME) * FRAME;
        chunk.resize(n, 0.0);
        self.score_chunk(&chunk);
    }

    fn score_chunk(&mut self, chunk: &[f32]) {
        let ctx: Vec<f32> = self.context.iter().copied().collect();
        let p = self.scorer.score(&ctx, chunk);
        self.probs.extend(p);
        self.context.extend(chunk.iter().copied());
        let max_ctx = CONTEXT_FRAMES * FRAME;
        while self.context.len() > max_ctx {
            self.context.pop_front();
        }
    }

    /// Số mẫu tuyệt đối đã được chấm điểm.
    pub fn scored_until(&self) -> u64 {
        self.probs_start + (self.probs.len() * FRAME) as u64
    }

    /// Bỏ các khung nằm hoàn toàn trước vị trí tuyệt đối `abs`.
    pub fn forget_before(&mut self, abs: u64) {
        while self.probs_start + FRAME as u64 <= abs && !self.probs.is_empty() {
            self.probs.pop_front();
            self.probs_start += FRAME as u64;
        }
    }

    /// Các đoạn nói, dạng (bắt đầu, kết thúc) theo mẫu tuyệt đối.
    pub fn spans(&self) -> Vec<(u64, u64)> {
        let mut raw: Vec<(usize, usize)> = Vec::new();
        let mut start: Option<usize> = None;
        for (i, &p) in self.probs.iter().enumerate() {
            match start {
                None if p >= self.cfg.on => start = Some(i),
                Some(s) if p < self.cfg.off => {
                    raw.push((s, i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            raw.push((s, self.probs.len()));
        }
        // gộp khoảng nghỉ ngắn
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for r in raw {
            match merged.last_mut() {
                Some(last) if r.0 - last.1 <= self.cfg.merge_gap_frames => last.1 = r.1,
                _ => merged.push(r),
            }
        }
        merged
            .into_iter()
            .filter(|(s, e)| e - s >= self.cfg.min_speech_frames)
            .map(|(s, e)| {
                (
                    self.probs_start + (s * FRAME) as u64,
                    self.probs_start + (e * FRAME) as u64,
                )
            })
            .collect()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::stt::SAMPLE_RATE;

    /// Tạo tín hiệu thử: các đoạn "tiếng nói" (sine 300 Hz) xen khoảng lặng, tính bằng giây.
    pub fn synth(parts: &[(bool, f32)]) -> Vec<f32> {
        let mut out = Vec::new();
        for &(voiced, secs) in parts {
            let n = (secs * SAMPLE_RATE as f32) as usize;
            for i in 0..n {
                let t = (out.len() + i) as f32 / SAMPLE_RATE as f32;
                out.push(if voiced {
                    0.3 * (2.0 * std::f32::consts::PI * 300.0 * t).sin()
                } else {
                    0.0005
                });
            }
        }
        out
    }

    #[test]
    fn energy_vad_finds_two_spans() {
        let s = synth(&[
            (false, 0.5),
            (true, 1.0),
            (false, 1.0),
            (true, 0.8),
            (false, 0.5),
        ]);
        let mut v = FrameVad::new(Box::new(EnergyScorer::default()), VadConfig::default());
        for c in s.chunks(700) {
            v.accept(c);
        }
        v.flush();
        let spans = v.spans();
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert!((spans[0].0 as i64 - 8000).abs() < 800);
        v.forget_before(spans[0].1);
        assert_eq!(v.spans().len(), 1);
    }
}
