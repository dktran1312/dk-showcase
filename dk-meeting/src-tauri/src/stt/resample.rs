//! Resample luồng mono về 16 kHz cho whisper.

use rubato::{FftFixedIn, Resampler};

use super::SAMPLE_RATE;

const CHUNK_IN: usize = 1024;

pub struct StreamResampler {
    inner: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
    /// Số mẫu đầu ra cần bỏ để bù độ trễ của bộ lọc (giữ timestamp chính xác).
    skip: usize,
}

impl StreamResampler {
    pub fn new(rate_in: u32) -> Self {
        let inner = (rate_in != SAMPLE_RATE).then(|| {
            FftFixedIn::<f32>::new(rate_in as usize, SAMPLE_RATE as usize, CHUNK_IN, 2, 1)
                .expect("tham số resample hợp lệ")
        });
        let skip = inner.as_ref().map(|r| r.output_delay()).unwrap_or(0);
        Self {
            inner,
            pending: Vec::new(),
            skip,
        }
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else {
            return input.to_vec();
        };
        self.pending.extend_from_slice(input);
        let mut out = Vec::new();
        while self.pending.len() >= r.input_frames_next() {
            let n = r.input_frames_next();
            if let Ok(mut res) = r.process(&[&self.pending[..n]], None) {
                out.append(&mut res[0]);
            }
            self.pending.drain(..n);
        }
        self.trim_delay(out)
    }

    /// Xả phần còn lại khi kết thúc luồng.
    pub fn flush(&mut self) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if !self.pending.is_empty() {
            if let Ok(mut res) = r.process_partial(Some(&[&self.pending[..]]), None) {
                out.append(&mut res[0]);
            }
            self.pending.clear();
        }
        if let Ok(mut res) = r.process_partial::<&[f32]>(None, None) {
            out.append(&mut res[0]);
        }
        self.trim_delay(out)
    }

    fn trim_delay(&mut self, mut out: Vec<f32>) -> Vec<f32> {
        if self.skip > 0 {
            let n = self.skip.min(out.len());
            out.drain(..n);
            self.skip -= n;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_ratio_48k() {
        let mut r = StreamResampler::new(48_000);
        let mut total = 0;
        for _ in 0..50 {
            total += r.push(&vec![0.1; 4800]).len(); // 5 s
        }
        total += r.flush().len();
        // 5 s @16k = 80_000 mẫu (sai số nhỏ do bộ lọc)
        assert!((total as i64 - 80_000).abs() < 2000, "total = {total}");
    }

    #[test]
    fn passthrough_16k() {
        let mut r = StreamResampler::new(16_000);
        assert_eq!(r.push(&[0.5; 10]).len(), 10);
    }
}
