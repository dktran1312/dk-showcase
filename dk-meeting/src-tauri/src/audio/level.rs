//! Đo mức âm (RMS / peak) cho đồng hồ VU trên UI.

/// Trả về (rms, peak) đã quy về thang 0..1 theo dBFS (-60 dB → 0, 0 dB → 1).
pub fn measure(mono: &[f32]) -> (f32, f32) {
    if mono.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f64;
    let mut peak = 0.0f32;
    for &s in mono {
        sum += (s as f64) * (s as f64);
        peak = peak.max(s.abs());
    }
    let rms = (sum / mono.len() as f64).sqrt() as f32;
    (to_meter(rms), to_meter(peak))
}

fn to_meter(amplitude: f32) -> f32 {
    const FLOOR_DB: f32 = -60.0;
    if amplitude <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * amplitude.log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Gộp các kênh interleaved thành mono.
pub fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    if ch == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_zero() {
        assert_eq!(measure(&[0.0; 100]), (0.0, 0.0));
    }

    #[test]
    fn full_scale_is_one() {
        let (rms, peak) = measure(&[1.0, -1.0, 1.0, -1.0]);
        assert!((rms - 1.0).abs() < 1e-6);
        assert!((peak - 1.0).abs() < 1e-6);
    }

    #[test]
    fn downmix_stereo() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }
}
