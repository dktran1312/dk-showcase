//! Điều phối một phiên ghi âm: mỗi nguồn (track) có 1 thread ghi file riêng,
//! cộng 1 thread "ticker" đẩy đồng hồ thời gian + mức âm lên UI.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError};
use parking_lot::Mutex;
use serde::Serialize;

use super::level::{downmix, measure};
use super::wav_writer::{ChunkEvent, WavChunkWriter};
use super::{AudioSource, CaptureHandle, ErrorFn, SourceKind, Track};
use crate::error::AppResult;
use crate::storage::Db;

/// Độ dài mỗi file WAV.
const CHUNK_SECS: u32 = 5 * 60;
/// Tần suất đẩy mức âm lên UI.
const TICK: Duration = Duration::from_millis(80);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackLevel {
    pub track: Track,
    pub rms: f32,
    pub peak: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsEvent {
    pub meeting_id: String,
    pub elapsed_ms: u64,
    pub paused: bool,
    pub levels: Vec<TrackLevel>,
}

pub type LevelsFn = Arc<dyn Fn(LevelsEvent) + Send + Sync>;

#[derive(Default)]
struct Clock {
    accumulated: Duration,
    running_since: Option<Instant>,
}

impl Clock {
    fn elapsed(&self) -> Duration {
        self.accumulated + self.running_since.map(|t| t.elapsed()).unwrap_or_default()
    }
    fn pause(&mut self) {
        if let Some(t) = self.running_since.take() {
            self.accumulated += t.elapsed();
        }
    }
    fn resume(&mut self) {
        if self.running_since.is_none() {
            self.running_since = Some(Instant::now());
        }
    }
}

struct Meter {
    track: Track,
    rms: AtomicU32,
    peak: AtomicU32,
}

impl Meter {
    fn set(&self, rms: f32, peak: f32) {
        self.rms.store(rms.to_bits(), Ordering::Relaxed);
        self.peak.store(peak.to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> TrackLevel {
        TrackLevel {
            track: self.track,
            rms: f32::from_bits(self.rms.load(Ordering::Relaxed)),
            peak: f32::from_bits(self.peak.load(Ordering::Relaxed)),
        }
    }
}

struct Shared {
    paused: AtomicBool,
    stopping: AtomicBool,
    clock: Mutex<Clock>,
}

struct TrackRunner {
    capture: Option<Box<dyn CaptureHandle>>,
    writer: Option<JoinHandle<()>>,
}

pub struct Recorder {
    meeting_id: String,
    sources: Vec<SourceKind>,
    shared: Arc<Shared>,
    tracks: Vec<TrackRunner>,
    ticker: Option<JoinHandle<()>>,
}

impl Recorder {
    pub fn start(
        meeting_id: String,
        dir: PathBuf,
        sources: Vec<Box<dyn AudioSource>>,
        db: Arc<Db>,
        on_levels: LevelsFn,
        on_error: ErrorFn,
    ) -> AppResult<Self> {
        std::fs::create_dir_all(&dir)?;
        let shared = Arc::new(Shared {
            paused: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            clock: Mutex::new(Clock::default()),
        });

        let mut meters = Vec::new();
        let mut tracks = Vec::new();
        let mut kinds = Vec::new();
        for source in sources {
            let track = source.track();
            let fmt = source.format();
            kinds.push(source.kind());
            let meter = Arc::new(Meter {
                track,
                rms: AtomicU32::new(0),
                peak: AtomicU32::new(0),
            });
            meters.push(meter.clone());

            let (tx, rx) = unbounded::<Vec<f32>>();
            let ctx = WriterCtx {
                meeting_id: meeting_id.clone(),
                track,
                channels: fmt.channels,
                writer: WavChunkWriter::new(&dir, track, fmt.sample_rate, CHUNK_SECS),
                db: db.clone(),
                shared: shared.clone(),
                meter,
                on_error: on_error.clone(),
            };
            let writer = std::thread::Builder::new()
                .name(format!("writer-{}", track.as_str()))
                .spawn(move || writer_loop(rx, ctx))?;

            match source.start(tx, on_error.clone()) {
                Ok(capture) => tracks.push(TrackRunner {
                    capture: Some(capture),
                    writer: Some(writer),
                }),
                Err(e) => {
                    // `tx` đã bị drop → writer tự kết thúc. Dừng các track đã chạy trước đó.
                    let _ = writer.join();
                    shared.stopping.store(true, Ordering::SeqCst);
                    for t in tracks {
                        shutdown_track(t);
                    }
                    return Err(e);
                }
            }
        }

        shared.clock.lock().resume();

        let ticker = {
            let shared = shared.clone();
            let meeting_id = meeting_id.clone();
            std::thread::Builder::new()
                .name("rec-ticker".into())
                .spawn(move || {
                    while !shared.stopping.load(Ordering::SeqCst) {
                        std::thread::sleep(TICK);
                        on_levels(LevelsEvent {
                            meeting_id: meeting_id.clone(),
                            elapsed_ms: shared.clock.lock().elapsed().as_millis() as u64,
                            paused: shared.paused.load(Ordering::Relaxed),
                            levels: meters.iter().map(|m| m.get()).collect(),
                        });
                    }
                })?
        };

        Ok(Self {
            meeting_id,
            sources: kinds,
            shared,
            tracks,
            ticker: Some(ticker),
        })
    }

    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }

    pub fn sources(&self) -> &[SourceKind] {
        &self.sources
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.shared.clock.lock().elapsed().as_millis() as u64
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::SeqCst)
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::SeqCst);
        self.shared.clock.lock().pause();
    }

    pub fn resume(&self) {
        self.shared.clock.lock().resume();
        self.shared.paused.store(false, Ordering::SeqCst);
    }

    /// Dừng thu, ghi nốt dữ liệu còn trong hàng đợi, đóng file. Trả về thời lượng (ms).
    pub fn stop(mut self) -> u64 {
        self.shared.clock.lock().pause();
        let elapsed = self.elapsed_ms();
        self.shared.stopping.store(true, Ordering::SeqCst);
        for t in self.tracks.drain(..) {
            shutdown_track(t);
        }
        if let Some(t) = self.ticker.take() {
            let _ = t.join();
        }
        elapsed
    }
}

fn shutdown_track(mut t: TrackRunner) {
    if let Some(c) = t.capture.take() {
        c.stop(); // drop sink → writer nhận Disconnected sau khi đọc hết hàng đợi
    }
    if let Some(w) = t.writer.take() {
        let _ = w.join();
    }
}

struct WriterCtx {
    meeting_id: String,
    track: Track,
    channels: u16,
    writer: WavChunkWriter,
    db: Arc<Db>,
    shared: Arc<Shared>,
    meter: Arc<Meter>,
    on_error: ErrorFn,
}

impl WriterCtx {
    fn apply(&self, ev: ChunkEvent) {
        let res = match ev {
            ChunkEvent::Opened {
                idx,
                file_name,
                start_ms,
                sample_rate,
            } => self.db.insert_chunk(
                &self.meeting_id,
                self.track,
                idx,
                &file_name,
                sample_rate,
                start_ms,
            ),
            ChunkEvent::Closed { idx, duration_ms } => {
                self.db
                    .set_chunk_duration(&self.meeting_id, self.track, idx, duration_ms)
            }
        };
        if let Err(e) = res {
            log::error!("Không lưu được thông tin đoạn âm thanh: {e}");
        }
    }
}

fn writer_loop(rx: Receiver<Vec<f32>>, mut ctx: WriterCtx) {
    let mut reported_error = false;
    let mut report = |ctx: &WriterCtx, msg: String| {
        log::error!("{msg}");
        if !reported_error {
            reported_error = true;
            (ctx.on_error)(msg);
        }
    };

    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(buf) => {
                if ctx.shared.paused.load(Ordering::Relaxed) {
                    ctx.meter.set(0.0, 0.0);
                    continue;
                }
                let mono = downmix(&buf, ctx.channels);
                let (rms, peak) = measure(&mono);
                ctx.meter.set(rms, peak);
                match ctx.writer.write(&mono) {
                    Ok(events) => events.into_iter().for_each(|ev| ctx.apply(ev)),
                    Err(e) => report(
                        &ctx,
                        format!("Lỗi ghi file âm thanh ({}): {e}", ctx.track.as_str()),
                    ),
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Err(e) = ctx.writer.flush() {
                    report(&ctx, format!("Lỗi ghi file âm thanh: {e}"));
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    match ctx.writer.finish() {
        Ok(Some(ev)) => ctx.apply(ev),
        Ok(None) => {}
        Err(e) => report(&ctx, format!("Lỗi đóng file âm thanh: {e}")),
    }
    ctx.meter.set(0.0, 0.0);
    log::info!(
        "Track {} đã ghi {} ms",
        ctx.track.as_str(),
        ctx.writer.total_ms()
    );
}

/// Thư mục chứa file âm thanh của một cuộc họp.
pub fn meeting_dir(recordings_dir: &Path, meeting_id: &str) -> PathBuf {
    recordings_dir.join(meeting_id)
}
