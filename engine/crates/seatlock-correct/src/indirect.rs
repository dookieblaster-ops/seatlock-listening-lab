//! C10: Indirect-radiator tuning (README §8).
//!
//! Radiators that do not face the listener are heard mostly by the wall behind the cabinet
//! and the room's diffuse sound (README §5.10). This module picks a drive level, band limits,
//! and delay for those radiators from the `simple` room's geometry — not a second EQ pass.

use seatlock_core::constants::C0;

/// Suggested drive settings for one radiator that faces away from the listener.
#[derive(Clone, Debug)]
pub struct IndirectTuning {
    pub delay_s: f64,
    pub gain_db: f64,
    pub highpass_hz: f64,
    pub lowpass_hz: Option<f64>,
    /// Extra path of the wall reflection against the front's direct path, seconds.
    pub reflection_lag_s: f64,
    pub message: String,
}

/// Midband mean level (100 Hz to 500 Hz) of a response curve.
pub fn midband_mean_db(freqs: &[f64], spl_db: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for (freq, level) in freqs.iter().zip(spl_db) {
        if (100.0..=500.0).contains(freq) {
            sum += level;
            n += 1.0;
        }
    }
    if n > 0.0 {
        sum / n
    } else {
        0.0
    }
}

/// C10 settings from the `simple` room and midband levels (direct sound of each radiator).
///
/// - **Delay** stays 0: the wall reflection path is longer than the front's direct path by
///   about `depth + 2·wall_behind`, so delaying the rear radiator cannot align them.
/// - **Gain** aims about 6 dB below the front midband, clamped to ±12 / +6 dB (same boost
///   ceiling as C1), scaled gently when the wall reflection is weak.
/// - **Highpass** sits near λ/4 of that reflection path length, clamped to 80–250 Hz.
/// - **Lowpass** is the panel's modal limit when known, else 3 kHz for a panel and 500 Hz
///   for a rear-facing cone (bass filler via the wall).
pub fn indirect_tuning(
    wall_behind_m: f64,
    wall_reflection: f64,
    cabinet_depth_m: f64,
    front_midband_db: f64,
    this_midband_db: f64,
    is_panel: bool,
    panel_limit_hz: Option<f64>,
) -> IndirectTuning {
    let path = cabinet_depth_m.max(0.0) + 2.0 * wall_behind_m.max(0.0);
    let reflection_lag_s = path / C0;
    let highpass_hz = (C0 / (4.0 * path.max(0.05))).clamp(80.0, 250.0);
    let lowpass_hz = if is_panel {
        Some(panel_limit_hz.unwrap_or(3_000.0).clamp(1_000.0, 6_000.0))
    } else {
        Some(500.0)
    };
    let wall = wall_reflection.clamp(0.0, 1.0);
    // A weak wall means less reflected drive is useful; ease the target toward unity gain.
    let target = front_midband_db - 6.0 * wall;
    let gain_db = (target - this_midband_db).clamp(-12.0, 6.0);
    let lp_note = lowpass_hz
        .map(|f| format!(" lowpass at {f:.0} Hz,"))
        .unwrap_or_default();
    let message = format!(
        "Indirect radiator (C10): highpass at {highpass_hz:.0} Hz,{lp_note} gain {gain_db:+.1} dB, \
         delay 0 ms (wall reflection arrives ~{:.1} ms after the front's direct sound).",
        reflection_lag_s * 1000.0
    );
    IndirectTuning {
        delay_s: 0.0,
        gain_db,
        highpass_hz,
        lowpass_hz,
        reflection_lag_s,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflection_lag_grows_with_wall_distance() {
        let near = indirect_tuning(0.2, 0.9, 0.2, 90.0, 80.0, true, Some(5_000.0));
        let far = indirect_tuning(1.0, 0.9, 0.2, 90.0, 80.0, true, Some(5_000.0));
        assert!(far.reflection_lag_s > near.reflection_lag_s);
        assert_eq!(near.delay_s, 0.0);
        assert!(near.highpass_hz >= 80.0 && near.highpass_hz <= 250.0);
    }

    #[test]
    fn gain_is_clamped_and_eased_for_a_weak_wall() {
        let strong = indirect_tuning(0.5, 1.0, 0.2, 90.0, 86.0, false, None);
        let weak = indirect_tuning(0.5, 0.1, 0.2, 90.0, 86.0, false, None);
        assert!((strong.gain_db - (-2.0)).abs() < 1e-9, "{}", strong.gain_db);
        assert!(
            weak.gain_db > strong.gain_db,
            "{} vs {}",
            weak.gain_db,
            strong.gain_db
        );
        assert_eq!(strong.lowpass_hz, Some(500.0));
    }
}
