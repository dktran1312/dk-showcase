//! Ghi WAV theo từng đoạn để không mất dữ liệu khi app crash.
//!
//! - Mỗi đoạn tối đa `chunk_secs` giây: `mic_000.wav`, `mic_001.wav`, ...
//! - Header WAV được cập nhật ~mỗi giây (`hound::WavWriter::flush`), nên file luôn mở được
//!   kể cả khi tiến trình bị tắt đột ngột; tối đa mất ~1 giây âm thanh cuối.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::Track;

const FLUSH_EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkEvent {
    Opened {
        idx: u32,
        file_name: String,
        start_ms: u64,
        sample_rate: u32,
    },
    Closed {
        idx: u32,
        duration_ms: u64,
    },
}

struct OpenChunk {
    writer: hound::WavWriter<BufWriter<File>>,
    idx: u32,
    frames: u64,
}

pub struct WavChunkWriter {
    dir: PathBuf,
    track: Track,
    sample_rate: u32,
    chunk_frames: u64,
    cur: Option<OpenChunk>,
    next_idx: u32,
    total_frames: u64,
    last_flush: Instant,
}

pub fn chunk_file_name(track: Track, idx: u32) -> String {
    format!("{}_{:03}.wav", track.as_str(), idx)
}

impl WavChunkWriter {
    pub fn new(dir: &Path, track: Track, sample_rate: u32, chunk_secs: u32) -> Self {
        Self {
            dir: dir.to_path_buf(),
            track,
            sample_rate,
            chunk_frames: sample_rate as u64 * chunk_secs.max(1) as u64,
            cur: None,
            next_idx: 0,
            total_frames: 0,
            last_flush: Instant::now(),
        }
    }

    fn frames_to_ms(&self, frames: u64) -> u64 {
        frames * 1000 / self.sample_rate as u64
    }

    /// Ghi các mẫu mono f32. Trả về các sự kiện mở/đóng đoạn để lưu vào DB.
    pub fn write(&mut self, mono: &[f32]) -> Result<Vec<ChunkEvent>, hound::Error> {
        let mut events = Vec::new();
        let mut rest = mono;
        while !rest.is_empty() {
            if self.cur.is_none() {
                events.push(self.open_next()?);
            }
            let chunk_frames = self.chunk_frames;
            let cur = self.cur.as_mut().expect("chunk vừa được mở");
            let room = (chunk_frames - cur.frames) as usize;
            let n = room.min(rest.len());
            for &s in &rest[..n] {
                cur.writer.write_sample(f32_to_i16(s))?;
            }
            cur.frames += n as u64;
            self.total_frames += n as u64;
            rest = &rest[n..];
            if cur.frames >= chunk_frames {
                if let Some(ev) = self.close_current()? {
                    events.push(ev);
                }
            }
        }
        if self.last_flush.elapsed() >= FLUSH_EVERY {
            self.flush()?;
        }
        Ok(events)
    }

    /// Cập nhật header + đẩy buffer xuống đĩa.
    pub fn flush(&mut self) -> Result<(), hound::Error> {
        self.last_flush = Instant::now();
        if let Some(cur) = self.cur.as_mut() {
            cur.writer.flush()?;
        }
        Ok(())
    }

    /// Đóng đoạn đang ghi (khi dừng họp).
    pub fn finish(&mut self) -> Result<Option<ChunkEvent>, hound::Error> {
        self.close_current()
    }

    pub fn total_ms(&self) -> u64 {
        self.frames_to_ms(self.total_frames)
    }

    fn open_next(&mut self) -> Result<ChunkEvent, hound::Error> {
        let idx = self.next_idx;
        self.next_idx += 1;
        let file_name = chunk_file_name(self.track, idx);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(self.dir.join(&file_name), spec)?;
        self.cur = Some(OpenChunk {
            writer,
            idx,
            frames: 0,
        });
        self.last_flush = Instant::now();
        Ok(ChunkEvent::Opened {
            idx,
            file_name,
            start_ms: self.frames_to_ms(self.total_frames),
            sample_rate: self.sample_rate,
        })
    }

    fn close_current(&mut self) -> Result<Option<ChunkEvent>, hound::Error> {
        let Some(cur) = self.cur.take() else {
            return Ok(None);
        };
        let duration_ms = self.frames_to_ms(cur.frames);
        cur.writer.finalize()?;
        Ok(Some(ChunkEvent::Closed {
            idx: cur.idx,
            duration_ms,
        }))
    }
}

fn f32_to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// Đọc thời lượng (ms) của một file WAV. Nếu header hỏng (crash trước lần flush đầu tiên),
/// sửa lại header dựa trên kích thước file rồi đọc lại.
pub fn recover_duration_ms(path: &Path) -> Option<u64> {
    if let Some(ms) = read_duration_ms(path) {
        if ms > 0 {
            return Some(ms);
        }
    }
    repair_header(path).ok()?;
    read_duration_ms(path)
}

fn read_duration_ms(path: &Path) -> Option<u64> {
    let r = hound::WavReader::open(path).ok()?;
    let spec = r.spec();
    Some(r.duration() as u64 * 1000 / spec.sample_rate as u64)
}

/// Header 44 byte chuẩn PCM do `hound` ghi cho file mono 16-bit: vá lại 2 trường kích thước.
fn repair_header(path: &Path) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let len = std::fs::metadata(path)?.len();
    if len < 44 {
        return Err(std::io::Error::other("file quá ngắn"));
    }
    let data_len = ((len - 44) & !1) as u32; // làm tròn xuống số mẫu 16-bit
    let mut f = std::fs::OpenOptions::new().write(true).open(path)?;
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.seek(SeekFrom::Start(40))?;
    f.write_all(&data_len.to_le_bytes())?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dkm-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn splits_into_chunks() {
        let dir = tmpdir("split");
        let mut w = WavChunkWriter::new(&dir, Track::Mic, 1000, 2); // 2000 frames / đoạn
        let ev = w.write(&vec![0.1; 4500]).unwrap();
        let ev2 = w.finish().unwrap();
        assert_eq!(ev.len(), 5); // open0, close0, open1, close1, open2
        assert_eq!(
            ev2,
            Some(ChunkEvent::Closed {
                idx: 2,
                duration_ms: 500
            })
        );
        assert_eq!(w.total_ms(), 4500);
        assert_eq!(read_duration_ms(&dir.join("mic_001.wav")), Some(2000));
        assert_eq!(read_duration_ms(&dir.join("mic_002.wav")), Some(500));
    }

    #[test]
    fn survives_crash_after_flush() {
        let dir = tmpdir("crash");
        let mut w = WavChunkWriter::new(&dir, Track::Mic, 1000, 60);
        w.write(&vec![0.2; 1500]).unwrap();
        w.flush().unwrap();
        std::mem::forget(w); // giả lập crash: không finalize
        assert_eq!(recover_duration_ms(&dir.join("mic_000.wav")), Some(1500));
    }
}
