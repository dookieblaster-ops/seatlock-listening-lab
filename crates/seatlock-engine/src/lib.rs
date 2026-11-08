//! The native listening engine.
//!
//! Step 1 fills an output buffer with silence and counts the frames. It does not
//! render the lab. Filters come after this, and only after they match the browser reference.

use std::sync::atomic::{AtomicU64, Ordering};

static FRAMES_FILLED: AtomicU64 = AtomicU64::new(0);

/// The lab is not in this callback. Silence is the proof the bridge ran.
pub fn renders_audio() -> bool {
    let _ = (
        seatlock_dsp::parity_proven(),
        seatlock_box::alignment_proven(),
        seatlock_geom::solid_matches_box(),
        seatlock_catalog::drivers().len(),
    );
    false
}

/// Write silence and count the frames. Returns how many samples were written.
pub fn fill_silence(out: &mut [f32]) -> usize {
    for sample in out.iter_mut() {
        *sample = 0.0;
    }
    FRAMES_FILLED.fetch_add(out.len() as u64, Ordering::Relaxed);
    out.len()
}

/// Samples written by [`fill_silence`] since the process started.
pub fn frames_filled() -> u64 {
    FRAMES_FILLED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_does_not_render_the_lab() {
        assert!(!renders_audio());
    }

    #[test]
    fn silence_crosses_the_engine_and_is_counted() {
        let before = frames_filled();
        let mut buf = [1.0_f32; 8];
        let n = fill_silence(&mut buf);
        assert_eq!(n, 8);
        assert!(buf.iter().all(|sample| *sample == 0.0));
        assert_eq!(frames_filled(), before + 8);
    }
}
