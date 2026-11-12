//! Phiên nhận dạng của một cuộc họp: mỗi track một thread cắt câu, một thread whisper chung.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;

use super::resample::StreamResampler;
use super::segmenter::{Segmenter, SegmenterConfig, Utterance};
use super::transcriber::WhisperEngine;
use super::vad::make_scorer;
use super::{NewSegment, SegmentSink, SttStatus};
use crate::audio::Track;

/// Giữ model whisper đã nạp giữa các cuộc họp.
#[derive(Default)]
pub struct EngineCache {
    inner: Mutex<Option<Arc<WhisperEngine>>>,
}

impl EngineCache {
    pub fn get_or_load(
        &self,
        path: &std::path::Path,
        model_id: &str,
    ) -> crate::error::AppResult<Arc<WhisperEngine>> {
        let mut g = self.inner.lock();
        if let Some(e) = g.as_ref().filter(|e| e.model_id == model_id) {
            return Ok(e.clone());
        }
        *g = None; // giải phóng model cũ trước khi nạp model mới
        let e = WhisperEngine::load(path, model_id)?;
        *g = Some(e.clone());
        Ok(e)
    }

    pub fn clear(&self) {
        *self.inner.lock() = None;
    }
}

pub struct SttConfig {
    pub model_id: String,
    pub model_path: PathBuf,
    pub vad_path: Option<PathBuf>,
    /// "auto" | "vi" | "en"
    pub language: String,
    pub prompt: String,
}

pub struct SttSession {
    threads: Vec<JoinHandle<()>>,
}

impl SttSession {
    /// `tracks`: (track, sample rate gốc). Trả về phiên + đầu vào (tap) cho từng track:
    /// gửi mẫu mono ở sample rate gốc vào đó. Drop tất cả tap = kết thúc phiên.
    pub fn start(
        cfg: SttConfig,
        cache: Arc<EngineCache>,
        tracks: &[(Track, u32)],
        sink: Arc<dyn SegmentSink>,
    ) -> (Self, HashMap<Track, Sender<Vec<f32>>>) {
        let (utt_tx, utt_rx) = unbounded::<Utterance>();
        let mut taps = HashMap::new();
        let mut threads = Vec::new();

        for &(track, rate) in tracks {
            let (tx, rx) = unbounded::<Vec<f32>>();
            taps.insert(track, tx);
            let utt_tx = utt_tx.clone();
            let vad_path = cfg.vad_path.clone();
            let t = std::thread::Builder::new()
                .name(format!("stt-seg-{}", track.as_str()))
                .spawn(move || segment_loop(track, rate, rx, vad_path, utt_tx))
                .expect("spawn segmenter");
            threads.push(t);
        }
        drop(utt_tx); // thread whisper kết thúc khi mọi segmenter đã xong

        let t = std::thread::Builder::new()
            .name("stt-whisper".into())
            .spawn(move || whisper_loop(cfg, cache, utt_rx, sink))
            .expect("spawn whisper");
        threads.push(t);

        (Self { threads }, taps)
    }

    /// Chờ xử lý xong mọi câu còn trong hàng đợi (gọi sau khi đã dừng ghi âm).
    pub fn finish(self) {
        for t in self.threads {
            let _ = t.join();
        }
    }
}

fn segment_loop(
    track: Track,
    rate: u32,
    rx: Receiver<Vec<f32>>,
    vad: Option<PathBuf>,
    out: Sender<Utterance>,
) {
    let mut resampler = StreamResampler::new(rate);
    let mut seg = Segmenter::new(
        track,
        make_scorer(vad.as_deref()),
        SegmenterConfig::default(),
    );
    for buf in rx {
        let s16 = resampler.push(&buf);
        for u in seg.push(&s16) {
            let _ = out.send(u);
        }
    }
    let tail = resampler.flush();
    let mut rest = seg.push(&tail);
    rest.extend(seg.finish());
    for u in rest {
        let _ = out.send(u);
    }
}

fn whisper_loop(
    cfg: SttConfig,
    cache: Arc<EngineCache>,
    rx: Receiver<Utterance>,
    sink: Arc<dyn SegmentSink>,
) {
    sink.on_status(SttStatus::Loading {
        model: cfg.model_id.clone(),
    });
    let engine = match cache.get_or_load(&cfg.model_path, &cfg.model_id) {
        Ok(e) => e,
        Err(e) => {
            sink.on_status(SttStatus::Error {
                message: e.to_string(),
            });
            for _ in rx {} // vẫn rút hàng đợi để các thread khác không bị giữ
            return;
        }
    };
    let mut state = match engine.new_state() {
        Ok(s) => s,
        Err(e) => {
            sink.on_status(SttStatus::Error {
                message: e.to_string(),
            });
            for _ in rx {}
            return;
        }
    };
    sink.on_status(SttStatus::Ready {
        model: cfg.model_id.clone(),
        gpu: engine.gpu,
    });

    for u in rx {
        let started = std::time::Instant::now();
        match engine.transcribe(&mut state, &u.samples, &cfg.language, &cfg.prompt) {
            Ok(t) => {
                log::debug!(
                    "STT {} {}–{} ms ({} ms xử lý): {} câu",
                    u.track.as_str(),
                    u.start_ms,
                    u.end_ms,
                    started.elapsed().as_millis(),
                    t.segments.len()
                );
                for s in t.segments {
                    sink.on_segment(NewSegment {
                        track: u.track,
                        start_ms: u.start_ms + s.start_ms,
                        end_ms: (u.start_ms + s.end_ms).max(u.start_ms + s.start_ms),
                        lang: t.lang.clone(),
                        text: s.text,
                    });
                }
            }
            Err(e) => log::error!("{e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Collect(Mutex<Vec<NewSegment>>, Mutex<Vec<SttStatus>>);
    impl SegmentSink for Collect {
        fn on_segment(&self, seg: NewSegment) {
            self.0.lock().push(seg);
        }
        fn on_status(&self, status: SttStatus) {
            self.1.lock().push(status);
        }
    }

    /// Chạy với model thật:
    /// DKM_TEST_MODEL=.../ggml-base-q5_1.bin DKM_TEST_VAD=.../ggml-silero-v5.1.2.bin
    /// DKM_TEST_WAV=.../meeting.wav cargo test --release stt_real -- --ignored --nocapture
    #[test]
    #[ignore]
    fn stt_real_model_end_to_end() {
        whisper_rs::install_logging_hooks();
        let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("thiếu {k}"));
        let mut reader = hound::WavReader::open(env("DKM_TEST_WAV")).unwrap();
        let rate = reader.spec().sample_rate;
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        let sink = Arc::new(Collect(Mutex::new(vec![]), Mutex::new(vec![])));
        let cfg = SttConfig {
            model_id: "test".into(),
            model_path: env("DKM_TEST_MODEL").into(),
            vad_path: std::env::var("DKM_TEST_VAD").ok().map(PathBuf::from),
            language: std::env::var("DKM_TEST_LANG").unwrap_or_else(|_| "auto".into()),
            prompt: "Thuật ngữ: LED, sân khấu, ánh sáng, run-down, brief".into(),
        };
        let started = std::time::Instant::now();
        let (session, taps) = SttSession::start(
            cfg,
            Arc::new(EngineCache::default()),
            &[(Track::Mic, rate)],
            sink.clone(),
        );
        let tap = taps.get(&Track::Mic).unwrap().clone();
        drop(taps);
        for chunk in samples.chunks(rate as usize / 100) {
            tap.send(chunk.to_vec()).unwrap();
        }
        drop(tap);
        session.finish();
        println!("statuses: {:?}", sink.1.lock());
        println!(
            "xử lý {:.1}s audio trong {:?}",
            samples.len() as f32 / rate as f32,
            started.elapsed()
        );
        for s in sink.0.lock().iter() {
            println!(
                "[{:>6} → {:>6} ms] ({}) {}",
                s.start_ms, s.end_ms, s.lang, s.text
            );
        }
        assert!(!sink.0.lock().is_empty());
    }
}
