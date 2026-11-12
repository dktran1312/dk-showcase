//! Gọi whisper.cpp cho từng câu.

use std::path::Path;
use std::sync::Arc;

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::error::{AppError, AppResult};

/// Model whisper đã nạp. Dùng chung giữa các cuộc họp (nạp model lớn mất vài giây).
pub struct WhisperEngine {
    ctx: WhisperContext,
    pub model_id: String,
    pub gpu: bool,
}

#[derive(Clone, Debug)]
pub struct RawSegment {
    /// ms, tính từ đầu đoạn âm thanh đưa vào.
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub struct Transcript {
    pub lang: String,
    pub segments: Vec<RawSegment>,
}

impl WhisperEngine {
    pub fn load(path: &Path, model_id: &str) -> AppResult<Arc<Self>> {
        let gpu = super::gpu_compiled();
        let params = WhisperContextParameters {
            use_gpu: gpu,
            flash_attn: gpu,
            ..Default::default()
        };
        let ctx = WhisperContext::new_with_params(path, params)
            .map_err(|e| AppError::msg(format!("Không nạp được model nhận giọng nói: {e}")))?;
        Ok(Arc::new(Self {
            ctx,
            model_id: model_id.to_string(),
            gpu,
        }))
    }

    pub fn new_state(&self) -> AppResult<WhisperState> {
        self.ctx
            .create_state()
            .map_err(|e| AppError::msg(format!("Lỗi khởi tạo whisper: {e}")))
    }

    /// `language`: "auto" | "vi" | "en". `prompt`: gợi ý thuật ngữ.
    pub fn transcribe(
        &self,
        state: &mut WhisperState,
        samples: &[f32],
        language: &str,
        prompt: &str,
    ) -> AppResult<Transcript> {
        let threads = num_threads(self.gpu);
        // Câu quá ngắn → đệm im lặng cho đủ 1 s (whisper xử lý câu < 1 s kém).
        let padded: Vec<f32>;
        let audio = if samples.len() < 16_000 {
            let mut v = samples.to_vec();
            v.resize(16_000, 0.0);
            padded = v;
            &padded[..]
        } else {
            samples
        };

        let lang = if language == "auto" {
            self.detect_language(state, audio, threads)
        } else {
            language.to_string()
        };

        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 2 });
        p.set_language(Some(&lang));
        p.set_translate(false);
        p.set_n_threads(threads as i32);
        p.set_no_context(true);
        p.set_suppress_blank(true);
        p.set_suppress_nst(true);
        p.set_no_speech_thold(0.6);
        p.set_print_special(false);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        if !prompt.is_empty() {
            p.set_initial_prompt(prompt);
        }
        state
            .full(p, audio)
            .map_err(|e| AppError::msg(format!("whisper lỗi: {e}")))?;

        let mut segments = Vec::new();
        for seg in state.as_iter() {
            // whisper.cpp đã tự bỏ câu khi no_speech cao VÀ độ tin cậy thấp; ở đây chỉ chặn trường hợp rất rõ.
            if seg.no_speech_probability() > 0.9 {
                log::debug!(
                    "bỏ câu (no_speech {:.2}): {:?}",
                    seg.no_speech_probability(),
                    seg.to_str_lossy()
                );
                continue;
            }
            let text = seg
                .to_str_lossy()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if text.is_empty() || is_hallucination(&text, prompt) {
                continue;
            }
            let dur = samples.len() as u64 / 16;
            segments.push(RawSegment {
                start_ms: (seg.start_timestamp().max(0) as u64 * 10).min(dur),
                end_ms: (seg.end_timestamp().max(0) as u64 * 10).min(dur),
                text,
            });
        }
        Ok(Transcript { lang, segments })
    }

    /// Nhận diện ngôn ngữ. Ưu tiên vi/en (cuộc họp ở VN); chỉ chọn ngôn ngữ khác khi model rất chắc chắn.
    fn detect_language(&self, state: &mut WhisperState, audio: &[f32], threads: usize) -> String {
        let probs = state
            .pcm_to_mel(audio, threads)
            .and_then(|_| state.lang_detect(0, threads))
            .map(|(_, p)| p);
        let Ok(probs) = probs else { return "vi".into() };
        let id = |l: &str| whisper_rs::get_lang_id(l).map(|i| i as usize);
        let (vi, en) = (id("vi").unwrap_or(0), id("en").unwrap_or(0));
        let (top, top_p) = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, p)| (i, *p))
            .unwrap_or((vi, 1.0));
        let chosen = if top == vi || top == en || top_p >= 0.7 {
            top
        } else if probs.get(en).copied().unwrap_or(0.0) > probs.get(vi).copied().unwrap_or(0.0) {
            en
        } else {
            vi
        };
        whisper_rs::get_lang_str(chosen as i32)
            .unwrap_or("vi")
            .to_string()
    }
}

/// Chừa lại CPU cho ghi âm, VAD và giao diện (ggml dùng luồng "spin" nên tranh CPU rất tệ).
fn num_threads(gpu: bool) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    if gpu {
        cores.clamp(1, 4)
    } else {
        cores.saturating_sub(2).clamp(1, 8)
    }
}

/// Các câu "ảo giác" kinh điển của whisper khi gặp im lặng / nhạc nền.
const HALLUCINATIONS: &[&str] = &[
    "subscribe",
    "đăng ký kênh",
    "ghiền mì gõ",
    "cảm ơn các bạn đã theo dõi",
    "cảm ơn các bạn đã xem",
    "hẹn gặp lại các bạn trong những video tiếp theo",
    "la la school",
    "thanks for watching",
    "thank you for watching",
    "please like and subscribe",
    "amara.org",
    "♪",
];

pub fn is_hallucination(text: &str, prompt: &str) -> bool {
    let lower = text.to_lowercase();
    if HALLUCINATIONS.iter().any(|h| lower.contains(h)) {
        return true;
    }
    let stripped: String = lower.chars().filter(|c| c.is_alphanumeric()).collect();
    if stripped.is_empty() {
        return true; // chỉ có dấu câu / ký hiệu, vd. "..." hoặc "[âm nhạc]" đã bị lọc chữ
    }
    if lower.starts_with('[') && lower.ends_with(']')
        || lower.starts_with('(') && lower.ends_with(')')
    {
        return true; // [Music], (cười), [âm nhạc]...
    }
    // Whisper lặp lại chính danh sách thuật ngữ trong prompt
    if !prompt.is_empty() && text.matches(',').count() >= 5 {
        let p = prompt.to_lowercase();
        let parts: Vec<&str> = lower
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let hits = parts.iter().filter(|s| p.contains(*s)).count();
        if hits * 10 >= parts.len() * 7 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_hallucinations() {
        let prompt = "Thuật ngữ: LED, màn LED, pixel map, pixel pitch, visual, VJ, Resolume";
        assert!(is_hallucination(
            "Hãy subscribe cho kênh Ghiền Mì Gõ để không bỏ lỡ",
            prompt
        ));
        assert!(is_hallucination("[Music]", prompt));
        assert!(is_hallucination("...", prompt));
        assert!(is_hallucination(
            "LED, màn LED, pixel map, pixel pitch, visual, VJ",
            prompt
        ));
        assert!(!is_hallucination(
            "Màn LED chính dùng pixel pitch 3.9, kích thước 12 x 6 mét.",
            prompt
        ));
        assert!(!is_hallucination("OK, chốt vậy nha.", prompt));
    }
}
