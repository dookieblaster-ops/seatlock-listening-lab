//! Published on-axis curves, blended in above the piston limit.

use num_complex::Complex64;
use seatlock_core::constants::C0;
use std::f64::consts::PI;

/// Above `k*a = 1`, replace the piston shape with the measured curve's shape.
/// Returns the fidelity label for that region.
pub fn blend_measured_curve(
    pressure: &mut [Complex64],
    freqs_hz: &[f64],
    piston_radius_m: f64,
    measured_db: &[(f64, f64)],
) -> String {
    if piston_radius_m <= 0.0 {
        return "no measured cone break-up; the piston model continues above k*a = 1".to_string();
    }
    let blend_hz = C0 / (2.0 * PI * piston_radius_m);
    if measured_db.is_empty() {
        return format!(
            "no measured cone break-up; the piston model continues above {blend_hz:.0} Hz"
        );
    }
    let Some(anchor) = freqs_hz
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (*left - blend_hz)
                .abs()
                .partial_cmp(&(*right - blend_hz).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
    else {
        return format!("measured above {blend_hz:.0} Hz");
    };
    let anchor_hz = freqs_hz[anchor];
    let anchor_db = measured_at(measured_db, anchor_hz);
    let anchor_mag = pressure[anchor].norm().max(1e-30);
    for (index, freq) in freqs_hz.iter().enumerate() {
        if *freq + 1e-9 < anchor_hz {
            continue;
        }
        let delta = measured_at(measured_db, *freq) - anchor_db;
        let target = anchor_mag * 10_f64.powf(delta / 20.0);
        let magnitude = pressure[index].norm();
        if magnitude > 1e-30 {
            pressure[index] *= target / magnitude;
        }
    }
    format!("measured above {blend_hz:.0} Hz")
}

fn measured_at(points: &[(f64, f64)], freq_hz: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if freq_hz <= points[0].0 {
        return points[0].1;
    }
    if let Some(last) = points.last() {
        if freq_hz >= last.0 {
            return last.1;
        }
    }
    for pair in points.windows(2) {
        let (low_hz, low_db) = pair[0];
        let (high_hz, high_db) = pair[1];
        if freq_hz >= low_hz && freq_hz <= high_hz {
            let span = (high_hz / low_hz).ln().max(1e-12);
            let place = (freq_hz / low_hz).ln() / span;
            return low_db + place * (high_db - low_db);
        }
    }
    points.last().map(|point| point.1).unwrap_or(0.0)
}
