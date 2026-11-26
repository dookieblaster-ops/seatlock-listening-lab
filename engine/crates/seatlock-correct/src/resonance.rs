//! C5: Cabinet-resonance correction (README §8).
//!
//! Internal resonances show up in the full 3D (`fdtd`) result and are missing from the
//! everyday Cap/waveguide models. Peaks of `fdtd − everyday` SPL that reach the listener
//! through the cone get peaking cuts, with a plain-language note to add damping or reshape
//! the inside.

use crate::CorrectionFilter;

/// One standing-wave peak found by comparing full-3D SPL to the everyday model.
#[derive(Clone, Debug)]
pub struct ResonancePeak {
    pub f_hz: f64,
    pub excess_db: f64,
}

/// Local maxima of `fdtd_db − everyday_db` in `[lo_hz, hi_hz]` that exceed `min_excess_db`
/// and stand at least `min_prominence_db` above their octave neighbors.
pub fn find_cabinet_resonances(
    freqs: &[f64],
    everyday_db: &[f64],
    fdtd_db: &[f64],
    lo_hz: f64,
    hi_hz: f64,
    min_excess_db: f64,
    min_prominence_db: f64,
) -> Vec<ResonancePeak> {
    if freqs.len() < 3
        || everyday_db.len() != freqs.len()
        || fdtd_db.len() != freqs.len()
        || hi_hz <= lo_hz
    {
        return Vec::new();
    }
    let diff: Vec<f64> = fdtd_db
        .iter()
        .zip(everyday_db)
        .map(|(a, b)| a - b)
        .collect();
    let mut peaks = Vec::new();
    for i in 1..freqs.len() - 1 {
        let f = freqs[i];
        if f < lo_hz || f > hi_hz {
            continue;
        }
        let d = diff[i];
        if d < min_excess_db {
            continue;
        }
        if d < diff[i - 1] || d <= diff[i + 1] {
            continue;
        }
        // Prominence against the quieter of the neighboring octave floor.
        let mut floor = d;
        for (j, freq) in freqs.iter().enumerate() {
            if *freq < f / 2.0 || *freq > f * 2.0 || j == i {
                continue;
            }
            floor = floor.min(diff[j]);
        }
        if d - floor < min_prominence_db {
            continue;
        }
        peaks.push(ResonancePeak {
            f_hz: f,
            excess_db: d,
        });
    }
    peaks.sort_by(|a, b| b.excess_db.partial_cmp(&a.excess_db).unwrap());
    peaks
}

/// Peaking cuts for C5. Gain is the excess, capped at −12 dB; Q is 6 (a standing-wave width).
pub fn resonance_cuts(peaks: &[ResonancePeak], max_cuts: usize) -> Vec<CorrectionFilter> {
    peaks
        .iter()
        .take(max_cuts)
        .map(|peak| {
            let cut = peak.excess_db.clamp(1.0, 12.0);
            CorrectionFilter {
                filter_type: "peaking".into(),
                f_hz: peak.f_hz,
                q: 6.0,
                gain_db: -cut,
                from_hz: peak.f_hz,
                from_q: 6.0,
                reason: format!(
                    "Cuts {cut:.1} dB at {:.0} Hz: cabinet standing wave in the full 3D model. \
                     Add damping fill or Shape the inside to remove the cause.",
                    peak.f_hz
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_narrow_standing_wave_peak() {
        let freqs: Vec<f64> = (0..40).map(|i| 100.0 * 1.1f64.powi(i)).collect();
        let everyday = vec![80.0; freqs.len()];
        let mut fdtd = everyday.clone();
        // Peak near 700 Hz.
        let i = freqs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - 700.0).abs().total_cmp(&(*b - 700.0).abs()))
            .map(|(i, _)| i)
            .unwrap();
        fdtd[i] += 5.0;
        if i > 0 {
            fdtd[i - 1] += 1.5;
        }
        if i + 1 < fdtd.len() {
            fdtd[i + 1] += 1.5;
        }
        let peaks = find_cabinet_resonances(&freqs, &everyday, &fdtd, 150.0, 3_000.0, 1.5, 1.0);
        assert!(!peaks.is_empty(), "{peaks:?}");
        assert!((peaks[0].f_hz - freqs[i]).abs() < 1.0);
        let cuts = resonance_cuts(&peaks, 3);
        assert_eq!(cuts.len(), 1);
        assert!(cuts[0].gain_db < 0.0);
        assert!(cuts[0].reason.contains("standing wave"));
        assert!(cuts[0].reason.contains("damping"));
    }

    #[test]
    fn ignores_a_broad_tilt() {
        let freqs: Vec<f64> = (0..40).map(|i| 100.0 * 1.1f64.powi(i)).collect();
        let everyday = vec![80.0; freqs.len()];
        let fdtd: Vec<f64> = freqs.iter().map(|f| 80.0 + (f / 1000.0).log10()).collect();
        let peaks = find_cabinet_resonances(&freqs, &everyday, &fdtd, 150.0, 3_000.0, 1.5, 2.0);
        assert!(peaks.is_empty(), "{peaks:?}");
    }
}
