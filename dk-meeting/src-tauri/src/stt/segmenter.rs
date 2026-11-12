//! Cắt luồng âm thanh 16 kHz thành từng câu (utterance) dựa trên VAD.
//!
//! - Cứ ~0,5 s xem lại kết quả VAD (VAD chạy tăng dần, không tính lại từ đầu).
//! - Hết câu khi có ≥ 0,7 s im lặng sau đoạn nói cuối → gửi đi nhận dạng (độ trễ ~1–3 s).
//! - Nói liền quá 12 s thì cắt ép ở khoảng nghỉ dài nhất để phụ đề không bị trễ.

use super::vad::{FrameScorer, FrameVad, VadConfig};
use super::SAMPLE_RATE;
use crate::audio::Track;

const MS: u64 = SAMPLE_RATE as u64 / 1000;

#[derive(Clone, Debug)]
pub struct Utterance {
    pub track: Track,
    pub start_ms: u64,
    pub end_ms: u64,
    pub samples: Vec<f32>,
}

pub struct SegmenterConfig {
    pub check_every: u64,
    pub end_silence: u64,
    pub max_utterance: u64,
    pub min_utterance: u64,
    pub pad: u64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            check_every: 500 * MS,
            end_silence: 700 * MS,
            max_utterance: 12_000 * MS,
            min_utterance: 400 * MS,
            pad: 200 * MS,
        }
    }
}

pub struct Segmenter {
    track: Track,
    vad: FrameVad,
    cfg: SegmenterConfig,
    buf: Vec<f32>,
    /// Vị trí tuyệt đối (mẫu 16 kHz tính từ đầu cuộc họp) của `buf[0]`.
    buf_start: u64,
    since_check: u64,
}

impl Segmenter {
    pub fn new(track: Track, scorer: Box<dyn FrameScorer>, cfg: SegmenterConfig) -> Self {
        Self {
            track,
            vad: FrameVad::new(scorer, VadConfig::default()),
            cfg,
            buf: Vec::new(),
            buf_start: 0,
            since_check: 0,
        }
    }

    fn buf_end(&self) -> u64 {
        self.buf_start + self.buf.len() as u64
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<Utterance> {
        self.buf.extend_from_slice(samples);
        self.vad.accept(samples);
        self.since_check += samples.len() as u64;
        if self.since_check < self.cfg.check_every {
            return Vec::new();
        }
        self.since_check = 0;
        self.process(false)
    }

    /// Gọi khi kết thúc luồng: trả nốt câu đang dở.
    pub fn finish(&mut self) -> Vec<Utterance> {
        self.vad.flush();
        self.process(true)
    }

    fn process(&mut self, final_: bool) -> Vec<Utterance> {
        let mut out = Vec::new();
        loop {
            // chỉ xét phần đã được VAD chấm điểm
            let scored = self.vad.scored_until().min(self.buf_end());
            let spans: Vec<(u64, u64)> = self
                .vad
                .spans()
                .into_iter()
                .map(|(s, e)| (s.max(self.buf_start), e.min(scored)))
                .filter(|(s, e)| e > s)
                .collect();

            let Some(&(first_start, _)) = spans.first() else {
                let keep_from = scored.saturating_sub(self.cfg.pad);
                self.drain_to(keep_from);
                return out;
            };
            let last_end = spans.last().unwrap().1;
            let silence_after = scored - last_end;
            let speech_len = scored - first_start;

            let cut = if final_ || silence_after >= self.cfg.end_silence {
                (last_end + self.cfg.pad).min(self.buf_end())
            } else if speech_len >= self.cfg.max_utterance {
                self.forced_cut(&spans, first_start)
            } else {
                // Câu chưa xong: bỏ phần im lặng phía trước cho gọn bộ đệm.
                self.drain_to(first_start.saturating_sub(self.cfg.pad));
                return out;
            };

            let start = first_start.saturating_sub(self.cfg.pad).max(self.buf_start);
            if cut > start && cut - start >= self.cfg.min_utterance {
                let a = (start - self.buf_start) as usize;
                let b = (cut - self.buf_start) as usize;
                out.push(Utterance {
                    track: self.track,
                    start_ms: start / MS,
                    end_ms: cut / MS,
                    samples: self.buf[a..b].to_vec(),
                });
            }
            self.drain_to(cut);
            let remaining = self
                .vad
                .scored_until()
                .min(self.buf_end())
                .saturating_sub(self.buf_start);
            if remaining < self.cfg.min_utterance || (!final_ && remaining < self.cfg.max_utterance)
            {
                if final_ && remaining > 0 {
                    continue;
                }
                return out;
            }
        }
    }

    /// Chọn điểm cắt ở khoảng nghỉ dài nhất trong cửa sổ `max_utterance`.
    fn forced_cut(&self, spans: &[(u64, u64)], first_start: u64) -> u64 {
        let limit = first_start + self.cfg.max_utterance;
        let best_gap = spans
            .windows(2)
            .filter(|w| w[0].1 <= limit && w[0].1 > first_start + self.cfg.min_utterance)
            .max_by_key(|w| w[1].0 - w[0].1);
        match best_gap {
            Some(w) => (w[0].1 + (w[1].0 - w[0].1) / 2).min(self.buf_end()),
            None => limit.min(self.buf_end()),
        }
    }

    fn drain_to(&mut self, abs: u64) {
        let n = abs
            .saturating_sub(self.buf_start)
            .min(self.buf.len() as u64) as usize;
        if n > 0 {
            self.buf.drain(..n);
            self.buf_start += n as u64;
        }
        self.vad.forget_before(self.buf_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::vad::tests::synth;
    use crate::stt::vad::EnergyScorer;

    fn run(parts: &[(bool, f32)]) -> Vec<Utterance> {
        let audio = synth(parts);
        let mut seg = Segmenter::new(
            Track::Mic,
            Box::new(EnergyScorer::default()),
            SegmenterConfig::default(),
        );
        let mut out = Vec::new();
        for chunk in audio.chunks(1600) {
            out.extend(seg.push(chunk));
        }
        out.extend(seg.finish());
        out
    }

    #[test]
    fn splits_on_silence_with_correct_timestamps() {
        let u = run(&[
            (false, 1.0),
            (true, 2.0),
            (false, 1.5),
            (true, 1.0),
            (false, 1.0),
        ]);
        assert_eq!(
            u.len(),
            2,
            "{:?}",
            u.iter().map(|x| (x.start_ms, x.end_ms)).collect::<Vec<_>>()
        );
        assert!(
            (u[0].start_ms as i64 - 800).abs() < 150,
            "{}",
            u[0].start_ms
        );
        assert!((u[0].end_ms as i64 - 3200).abs() < 150, "{}", u[0].end_ms);
        assert!(
            (u[1].start_ms as i64 - 4300).abs() < 150,
            "{}",
            u[1].start_ms
        );
        assert_eq!(
            u[0].samples.len() as u64,
            (u[0].end_ms - u[0].start_ms) * MS
        );
    }

    #[test]
    fn forces_cut_on_long_speech() {
        // 30 s nói liên tục có nghỉ ngắn 0,4 s mỗi 5 s → phải ra nhiều câu, mỗi câu ≤ 12 s
        let mut parts = vec![(false, 0.5)];
        for _ in 0..6 {
            parts.push((true, 4.6));
            parts.push((false, 0.4));
        }
        let u = run(&parts);
        assert!(u.len() >= 3, "chỉ có {} câu", u.len());
        assert!(u.iter().all(|x| x.end_ms - x.start_ms <= 12_500));
        assert!(u
            .windows(2)
            .all(|w| w[1].start_ms >= w[0].end_ms.saturating_sub(250)));
    }

    #[test]
    fn silence_only_produces_nothing() {
        assert!(run(&[(false, 5.0)]).is_empty());
    }
}
