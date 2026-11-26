//! Mono IEEE float32 WAV for FIR impulse export (README §7).

/// Little-endian mono WAVE with IEEE float samples (`format` 3).
pub fn mono_f32_wav(sample_rate_hz: u32, samples: &[f64]) -> Vec<u8> {
    let n = samples.len();
    let data_bytes = n * 4;
    let mut out = Vec::with_capacity(44 + data_bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM/IEEE chunk size
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate_hz.to_le_bytes());
    let byte_rate = sample_rate_hz * 4;
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes()); // block align
    out.extend_from_slice(&32u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for sample in samples {
        let v = if sample.is_finite() {
            (*sample).clamp(-1.0, 1.0) as f32
        } else {
            0.0
        };
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Decode a mono IEEE float WAV written by [`mono_f32_wav`] (for tests).
pub fn read_mono_f32_wav(bytes: &[u8]) -> Result<(u32, Vec<f64>), String> {
    if bytes.len() < 44 {
        return Err("WAV too short".into());
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAVE file".into());
    }
    if &bytes[12..16] != b"fmt " {
        return Err("missing fmt chunk".into());
    }
    let format = u16::from_le_bytes([bytes[20], bytes[21]]);
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
    if format != 3 || channels != 1 || bits != 32 {
        return Err(format!(
            "expected mono IEEE float32, got format {format} ch {channels} bits {bits}"
        ));
    }
    if &bytes[36..40] != b"data" {
        return Err("missing data chunk".into());
    }
    let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
    let data = &bytes[44..44 + data_len.min(bytes.len().saturating_sub(44))];
    let mut samples = Vec::with_capacity(data.len() / 4);
    for chunk in data.as_chunks::<4>().0 {
        samples.push(f32::from_le_bytes(*chunk) as f64);
    }
    Ok((rate, samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_taps() {
        let taps = vec![0.0, 0.25, 0.5, 0.25, 0.0];
        let wav = mono_f32_wav(48_000, &taps);
        let (rate, got) = read_mono_f32_wav(&wav).unwrap();
        assert_eq!(rate, 48_000);
        assert_eq!(got.len(), taps.len());
        for (a, b) in taps.iter().zip(&got) {
            assert!((a - b).abs() < 1e-6, "{a} vs {b}");
        }
    }
}
