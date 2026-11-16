//! CTA-2034 curves and Olive's preference rating (README sections 2.6 and 11.1).
//!
//! Predicted in-room response: 12 % listening window + 44 % early reflections + 44 % sound
//! power (energy mean). Preference rating from Olive, AES 6190 / US 2005/0195982 A1:
//! `12.69 − 2.49·NBD_ON − 2.99·NBD_PIR − 4.31·LFX + 2.32·SM_PIR`.

use serde::Serialize;

/// Olive / CTA-2034 preference breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct PreferenceScore {
    /// Predicted preference rating (higher is better; typical hi-fi range about 2 to 7).
    pub preference: f64,
    pub nbd_on: f64,
    pub nbd_pir: f64,
    pub sm_pir: f64,
    /// Low-frequency extension in Hz (sound power −6 dB vs the listening-window midband).
    pub lfx_hz: f64,
    /// `log10(lfx_hz)`, the term used in the regression.
    pub lfx: f64,
}

/// CTA-2034 style curves the lab forms from the baffle model (and from kit measurements).
#[derive(Debug, Clone, Serialize)]
pub struct Cta2034Report {
    pub on_axis_db: Vec<f64>,
    pub listening_window_db: Vec<f64>,
    pub early_reflections_db: Vec<f64>,
    pub sound_power_db: Vec<f64>,
    pub predicted_in_room_db: Vec<f64>,
    pub score: PreferenceScore,
}

/// Energy-mean predicted in-room response (CTA-2034): 12 % LW, 44 % ER, 44 % SP.
pub fn predicted_in_room_db(lw: &[f64], er: &[f64], sp: &[f64]) -> Vec<f64> {
    let n = lw.len().min(er.len()).min(sp.len());
    (0..n)
        .map(|i| {
            let energy =
                0.12 * db_to_lin(lw[i]) + 0.44 * db_to_lin(er[i]) + 0.44 * db_to_lin(sp[i]);
            lin_to_db(energy)
        })
        .collect()
}

fn db_to_lin(db: f64) -> f64 {
    10f64.powf(db / 10.0)
}

fn lin_to_db(lin: f64) -> f64 {
    10.0 * lin.max(1e-30).log10()
}

/// Half-octave band centers from 100 Hz through 12 kHz (ratio √2).
fn half_octave_centers() -> Vec<f64> {
    let mut out = Vec::new();
    let mut f = 100.0;
    while f <= 12_000.0 + 1e-9 {
        out.push(f);
        f *= std::f64::consts::SQRT_2;
    }
    out
}

fn interpolate_db(freqs_hz: &[f64], db: &[f64], at_hz: f64) -> Option<f64> {
    if freqs_hz.len() < 2 || db.len() != freqs_hz.len() || at_hz <= 0.0 {
        return None;
    }
    if at_hz <= freqs_hz[0] {
        return Some(db[0]);
    }
    if at_hz >= *freqs_hz.last().unwrap() {
        return Some(*db.last().unwrap());
    }
    let log_at = at_hz.ln();
    for i in 0..freqs_hz.len() - 1 {
        let a = freqs_hz[i];
        let b = freqs_hz[i + 1];
        if at_hz >= a && at_hz <= b {
            let t = (log_at - a.ln()) / (b.ln() - a.ln());
            return Some(db[i] + t * (db[i + 1] - db[i]));
        }
    }
    None
}

fn mean_abs_deviation(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return f64::NAN;
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    samples.iter().map(|v| (v - mean).abs()).sum::<f64>() / samples.len() as f64
}

/// Narrow-band deviation (Olive): mean of per-½-octave mean absolute deviations, each from
/// ten log-spaced samples between 100 Hz and 12 kHz.
pub fn narrow_band_deviation(freqs_hz: &[f64], db: &[f64]) -> f64 {
    let mut band_mads = Vec::new();
    for center in half_octave_centers() {
        let low = center / std::f64::consts::SQRT_2;
        let high = center * std::f64::consts::SQRT_2;
        let mut samples = Vec::with_capacity(10);
        for k in 0..10 {
            let t = k as f64 / 9.0;
            let hz = low * (high / low).powf(t);
            if let Some(value) = interpolate_db(freqs_hz, db, hz) {
                samples.push(value);
            }
        }
        if samples.len() >= 2 {
            band_mads.push(mean_abs_deviation(&samples));
        }
    }
    if band_mads.is_empty() {
        return f64::NAN;
    }
    band_mads.iter().sum::<f64>() / band_mads.len() as f64
}

/// Smoothness (Olive): r² of a straight line fit to dB vs log10(Hz) from 100 Hz to 16 kHz.
pub fn smoothness(freqs_hz: &[f64], db: &[f64]) -> f64 {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (freq, value) in freqs_hz.iter().zip(db) {
        if *freq >= 100.0 && *freq <= 16_000.0 && value.is_finite() {
            xs.push(freq.log10());
            ys.push(*value);
        }
    }
    let n = xs.len() as f64;
    if n < 3.0 {
        return f64::NAN;
    }
    let sx: f64 = xs.iter().sum();
    let sy: f64 = ys.iter().sum();
    let sxx: f64 = xs.iter().map(|x| x * x).sum();
    let syy: f64 = ys.iter().map(|y| y * y).sum();
    let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| x * y).sum();
    let num = n * sxy - sx * sy;
    let var_y = n * syy - sy * sy;
    // A perfectly flat curve has no y variance: the fit is exact.
    if var_y.abs() <= 1e-18 {
        return 1.0;
    }
    let den = ((n * sxx - sx * sx) * var_y).sqrt();
    if den <= 0.0 {
        return f64::NAN;
    }
    let r = num / den;
    r * r
}

/// Low-frequency extension in Hz: first sound-power frequency below 300 Hz that is 6 dB
/// below the listening-window mean from 300 Hz to 10 kHz (Olive LFX).
pub fn low_frequency_extension_hz(
    freqs_hz: &[f64],
    listening_window_db: &[f64],
    sound_power_db: &[f64],
) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (freq, value) in freqs_hz.iter().zip(listening_window_db) {
        if *freq >= 300.0 && *freq <= 10_000.0 && value.is_finite() {
            sum += *value;
            count += 1.0;
        }
    }
    if count < 1.0 {
        return freqs_hz.first().copied().unwrap_or(300.0);
    }
    let threshold = sum / count - 6.0;
    // Indices where SP is at or below the threshold and f ≤ 300 Hz.
    let mut below: Vec<usize> = freqs_hz
        .iter()
        .zip(sound_power_db)
        .enumerate()
        .filter(|(_, (f, v))| **f <= 300.0 && v.is_finite() && **v <= threshold)
        .map(|(i, _)| i)
        .collect();
    if below.is_empty() {
        return freqs_hz.first().copied().unwrap_or(300.0);
    }
    // Contiguous groups; take the last group's last index, then the next sample
    // (spinorama / Olive practice for oscillating bass).
    below.sort_unstable();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for index in below {
        if current.is_empty() || index == *current.last().unwrap() + 1 {
            current.push(index);
        } else {
            groups.push(std::mem::take(&mut current));
            current.push(index);
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    let last = *groups.last().unwrap().last().unwrap();
    let next = (last + 1).min(freqs_hz.len() - 1);
    freqs_hz[next]
}

/// Olive preference rating from on-axis, listening window, early reflections, and sound power.
pub fn preference_rating(
    freqs_hz: &[f64],
    on_axis_db: &[f64],
    listening_window_db: &[f64],
    early_reflections_db: &[f64],
    sound_power_db: &[f64],
) -> PreferenceScore {
    let pir = predicted_in_room_db(listening_window_db, early_reflections_db, sound_power_db);
    let nbd_on = narrow_band_deviation(freqs_hz, on_axis_db);
    let nbd_pir = narrow_band_deviation(freqs_hz, &pir);
    let sm_pir = smoothness(freqs_hz, &pir);
    let lfx_hz = low_frequency_extension_hz(freqs_hz, listening_window_db, sound_power_db);
    let lfx = lfx_hz.max(1e-3).log10();
    let preference = 12.69 - 2.49 * nbd_on - 2.99 * nbd_pir - 4.31 * lfx + 2.32 * sm_pir;
    PreferenceScore {
        preference,
        nbd_on,
        nbd_pir,
        sm_pir,
        lfx_hz,
        lfx,
    }
}

/// Build a full CTA-2034 report from the four curves.
pub fn cta2034_report(
    freqs_hz: &[f64],
    on_axis_db: &[f64],
    listening_window_db: &[f64],
    early_reflections_db: &[f64],
    sound_power_curve_db: &[f64],
) -> Cta2034Report {
    let predicted_in_room_db = predicted_in_room_db(
        listening_window_db,
        early_reflections_db,
        sound_power_curve_db,
    );
    let score = preference_rating(
        freqs_hz,
        on_axis_db,
        listening_window_db,
        early_reflections_db,
        sound_power_curve_db,
    );
    Cta2034Report {
        on_axis_db: on_axis_db.to_vec(),
        listening_window_db: listening_window_db.to_vec(),
        early_reflections_db: early_reflections_db.to_vec(),
        sound_power_db: sound_power_curve_db.to_vec(),
        predicted_in_room_db,
        score,
    }
}

/// Horizontal and vertical angles (degrees) for the CTA-2034 / CEA-2034-A early-reflections
/// average (WinMF / Audiomatica APPNOTE-022 bounce groups):
/// floor 20°/30°/40° down, ceiling 40°/50°/60° up, front wall 0° and ±10°/20°/30°,
/// side walls ±40°…±80°, rear wall 180° and ±90°.
pub fn early_reflection_angles_deg() -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for deg in [20.0, 30.0, 40.0] {
        out.push((0.0, -deg));
    }
    for deg in [40.0, 50.0, 60.0] {
        out.push((0.0, deg));
    }
    for deg in [-30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0] {
        out.push((deg, 0.0));
    }
    for deg in [
        -80.0, -70.0, -60.0, -50.0, -40.0, 40.0, 50.0, 60.0, 70.0, 80.0,
    ] {
        out.push((deg, 0.0));
    }
    for deg in [-90.0, 90.0, 180.0] {
        out.push((deg, 0.0));
    }
    out
}

/// Listening-window angles (CTA-2034): on-axis, ±10/20/30° horizontal, ±10° vertical.
pub fn listening_window_angles_deg() -> [(f64, f64); 9] {
    [
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 0.0),
        (20.0, 0.0),
        (-20.0, 0.0),
        (30.0, 0.0),
        (-30.0, 0.0),
        (0.0, 10.0),
        (0.0, -10.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn olive_formula_with_published_criton_terms_matches_spinorama() {
        // Spinorama page for CSS Criton 3TD-X (ASR): NBD ON 0.594, PIR 0.41; SM PIR 0.741; LFX 42 Hz; score 4.68.
        let nbd_on = 0.594;
        let nbd_pir = 0.41;
        let sm_pir = 0.741;
        let lfx = (42.0f64).log10();
        let preference = 12.69 - 2.49 * nbd_on - 2.99 * nbd_pir - 4.31 * lfx + 2.32 * sm_pir;
        assert!(
            (preference - 4.68).abs() < 0.05,
            "preference {preference} against published 4.68"
        );
    }

    #[test]
    fn a_flat_curve_is_smooth_with_no_narrow_band_deviation() {
        let freqs: Vec<f64> = (0..80)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / 79.0))
            .collect();
        let flat: Vec<f64> = freqs.iter().map(|_| 0.0).collect();
        assert!(narrow_band_deviation(&freqs, &flat) < 1e-9);
        assert!((smoothness(&freqs, &flat) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn early_reflection_angles_match_cta2034_bounce_groups() {
        let angles = early_reflection_angles_deg();
        // 3 floor + 3 ceiling + 7 front + 10 side + 3 rear = 26.
        assert_eq!(angles.len(), 26, "{angles:?}");
        assert!(angles.contains(&(0.0, -20.0)) && angles.contains(&(0.0, -40.0)));
        assert!(!angles.contains(&(0.0, -25.0)));
        assert!(angles.contains(&(0.0, 40.0)) && angles.contains(&(0.0, 60.0)));
        assert!(!angles.contains(&(0.0, 45.0)));
        assert!(angles.contains(&(180.0, 0.0)) && angles.contains(&(90.0, 0.0)));
        assert!(!angles.contains(&(100.0, 0.0)));
        assert_eq!(listening_window_angles_deg().len(), 9);
    }
}
