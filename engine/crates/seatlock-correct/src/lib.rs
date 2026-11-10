//! Target curves, auto EQ, and correction features that respect hardware limits.

use seatlock_dsp::{biquad_block, response_db};

#[derive(Clone, Debug)]
pub struct CorrectionFilter {
    pub filter_type: String,
    pub f_hz: f64,
    pub q: f64,
    pub gain_db: f64,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct CorrectionOutcome {
    pub filters: Vec<CorrectionFilter>,
    pub message: String,
}

pub fn metrics(freqs: &[f64], spl_db: &[f64], max_spl_db: &[f64]) -> (f64, f64, f64) {
    if freqs.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let smoothed = smooth(freqs, spl_db);
    let mut mean = 0.0;
    let mut count = 0.0;
    for (freq, level) in freqs.iter().zip(&smoothed) {
        if *freq >= 300.0 && *freq <= 3_000.0 {
            mean += level;
            count += 1.0;
        }
    }
    let mean = if count > 0.0 { mean / count } else { 0.0 };
    let normalized: Vec<f64> = smoothed.iter().map(|level| level - mean).collect();
    let mut f6 = freqs[freqs.len() - 1];
    for (freq, level) in freqs.iter().zip(&normalized) {
        if *level >= -6.0 {
            f6 = *freq;
            break;
        }
    }
    let mut sum = 0.0;
    let mut n = 0.0;
    for (freq, level) in freqs.iter().zip(&normalized) {
        if *freq >= 1.5 * f6 && *freq <= 16_000.0 {
            sum += level * level;
            n += 1.0;
        }
    }
    let flatness = if n > 0.0 { (sum / n).sqrt() } else { 0.0 };
    let mut loudest = f64::MAX;
    for (freq, level) in freqs.iter().zip(max_spl_db) {
        if *freq >= f6 && *freq <= 16_000.0 {
            loudest = loudest.min(*level);
        }
    }
    if !loudest.is_finite() {
        loudest = 0.0;
    }
    (f6, flatness, loudest)
}

pub fn correct(
    freqs: &[f64],
    response_db: &[f64],
    max_spl_db: &[f64],
    design_level_db: f64,
    protected_hz: [f64; 2],
    max_filters: usize,
    sample_rate_hz: f64,
) -> CorrectionOutcome {
    let bass = c2(freqs, max_spl_db, design_level_db);
    let mut working = response_db.to_vec();
    for filter in &bass {
        add_filter(&mut working, freqs, filter, sample_rate_hz);
    }
    let headroom: Vec<f64> = max_spl_db
        .iter()
        .map(|level| level - design_level_db)
        .collect();
    let target_level = mean_db(freqs, &working);
    let target = vec![target_level; freqs.len()];
    let (fit, message) = c1(
        freqs,
        &working,
        &target,
        &headroom,
        protected_hz,
        max_filters,
        sample_rate_hz,
    );
    let mut filters = bass;
    filters.extend(fit);
    CorrectionOutcome { filters, message }
}

fn c2(freqs: &[f64], max_spl_db: &[f64], design_level_db: f64) -> Vec<CorrectionFilter> {
    let mut f_low = *freqs.last().unwrap_or(&20.0);
    for (freq, level) in freqs.iter().zip(max_spl_db) {
        if *freq >= 10.0 && *level >= design_level_db {
            f_low = *freq;
            break;
        }
    }
    f_low = f_low.clamp(20.0, 250.0);
    vec![CorrectionFilter {
        filter_type: "highpass".into(),
        f_hz: f_low,
        q: 0.707,
        gain_db: 0.0,
        reason: format!(
            "Protective high-pass at {f_low:.0} Hz so a loud note stays within what the driver can do."
        ),
    }]
}

pub fn c1(
    freqs: &[f64],
    response_db: &[f64],
    target_db: &[f64],
    headroom_db: &[f64],
    protected_hz: [f64; 2],
    max_filters: usize,
    sample_rate_hz: f64,
) -> (Vec<CorrectionFilter>, String) {
    let initial = rms_error(freqs, &smooth(freqs, response_db), target_db);
    if initial < 0.2 {
        return (
            Vec::new(),
            "The response already follows the target, so no extra filters were added.".into(),
        );
    }
    let mut filters = Vec::new();
    let mut current = response_db.to_vec();
    let mut previous = initial;
    let mut notes = Vec::new();
    for _ in 0..max_filters {
        let smoothed_now = smooth(freqs, &current);
        let error: Vec<f64> = target_db
            .iter()
            .zip(&smoothed_now)
            .map(|(target, level)| target - level)
            .collect();
        let Some((index, peak)) = error.iter().enumerate().max_by(|left, right| {
            left.1
                .abs()
                .partial_cmp(&right.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            break;
        };
        if peak.abs() < 0.3 {
            break;
        }
        let center = freqs[index];
        let half = peak.abs() * 0.5;
        let mut left = index;
        let mut right = index;
        while left > 0 && error[left].signum() == peak.signum() && error[left].abs() >= half {
            left -= 1;
        }
        while right + 1 < error.len()
            && error[right].signum() == peak.signum()
            && error[right].abs() >= half
        {
            right += 1;
        }
        let bandwidth = (freqs[right] - freqs[left]).max(center * 0.05);
        let mut q = (center / bandwidth).clamp(0.4, 8.0);
        let mut gain = (*peak).clamp(-12.0, 6.0);
        if gain > 0.0 {
            if center >= protected_hz[0] && center <= protected_hz[1] {
                notes.push("A bump was left alone between 3 kHz and 5 kHz.".to_string());
                break;
            }
            if q > 4.0 {
                notes.push(format!(
                    "A narrow dip near {center:.0} Hz was left alone. That is a cancellation, not something EQ can fill."
                ));
                break;
            }
            q = q.min(4.0);
            let room = headroom_db.get(index).copied().unwrap_or(0.0);
            if room <= 0.0 {
                break;
            }
            gain = gain.min(room).min(6.0);
        }
        refine(
            freqs,
            &current,
            target_db,
            &mut q,
            &mut gain,
            center,
            sample_rate_hz,
            protected_hz,
            headroom_db,
        );
        if gain.abs() < 0.2 {
            break;
        }
        let candidate = CorrectionFilter {
            filter_type: "peaking".into(),
            f_hz: center,
            q,
            gain_db: gain,
            reason: if gain < 0.0 {
                format!("Cuts {:.1} dB at {center:.0} Hz to flatten a peak.", -gain)
            } else {
                format!("Boosts {gain:.1} dB at {center:.0} Hz where there is room to spare.")
            },
        };
        let mut trial = current.clone();
        add_filter(&mut trial, freqs, &candidate, sample_rate_hz);
        let improved = rms_error(freqs, &smooth(freqs, &trial), target_db);
        if previous - improved < 0.1 {
            break;
        }
        current = trial;
        previous = improved;
        filters.push(candidate);
    }
    let after = rms_error(freqs, &smooth(freqs, &current), target_db);
    if filters.is_empty() {
        let message = if notes.is_empty() {
            "No filter improved the response enough to keep.".into()
        } else {
            notes.join(" ")
        };
        return (Vec::new(), message);
    }
    if initial - after < 0.2 {
        return (
            Vec::new(),
            "Correction was dropped because it did not make the response meaningfully more even."
                .into(),
        );
    }
    let message = filters
        .iter()
        .map(|filter| filter.reason.clone())
        .chain(notes)
        .collect::<Vec<_>>()
        .join(" ");
    (filters, message)
}

#[allow(clippy::too_many_arguments)]
fn refine(
    freqs: &[f64],
    response: &[f64],
    target: &[f64],
    q: &mut f64,
    gain: &mut f64,
    center: f64,
    sample_rate_hz: f64,
    protected_hz: [f64; 2],
    headroom_db: &[f64],
) {
    let mut best_q = *q;
    let mut best_gain = *gain;
    let mut best = f64::MAX;
    for q_scale in [0.7, 0.85, 1.0, 1.15, 1.3] {
        for gain_delta in [-1.5, -0.75, -0.25, 0.0, 0.25, 0.75, 1.5] {
            let mut gain_value = (*gain + gain_delta).clamp(-12.0, 6.0);
            let q_cap = if gain_value > 0.0 { 4.0 } else { 8.0 };
            let q_value = (*q * q_scale).clamp(0.4, q_cap);
            if gain_value > 0.0 && center >= protected_hz[0] && center <= protected_hz[1] {
                continue;
            }
            if let Some(index) = freqs.iter().position(|freq| *freq >= center) {
                if gain_value > 0.0 {
                    let room = headroom_db.get(index).copied().unwrap_or(0.0).max(0.0);
                    gain_value = gain_value.min(room);
                }
            }
            let filter = CorrectionFilter {
                filter_type: "peaking".into(),
                f_hz: center,
                q: q_value,
                gain_db: gain_value,
                reason: String::new(),
            };
            let mut trial = response.to_vec();
            add_filter(&mut trial, freqs, &filter, sample_rate_hz);
            let score = rms_error(freqs, &smooth(freqs, &trial), target);
            if score < best {
                best = score;
                best_q = q_value;
                best_gain = gain_value;
            }
        }
    }
    *q = best_q;
    *gain = best_gain;
}

fn add_filter(curve: &mut [f64], freqs: &[f64], filter: &CorrectionFilter, sample_rate_hz: f64) {
    let block = biquad_block(
        &filter.filter_type,
        filter.f_hz,
        filter.q,
        filter.gain_db,
        sample_rate_hz,
    );
    for (level, freq) in curve.iter_mut().zip(freqs) {
        *level += response_db(block.as_ref(), *freq, sample_rate_hz);
    }
}

fn smooth(freqs: &[f64], db: &[f64]) -> Vec<f64> {
    freqs
        .iter()
        .enumerate()
        .map(|(index, freq)| {
            let mut sum = 0.0;
            let mut count = 0.0;
            for (other_index, other) in freqs.iter().enumerate() {
                if (*other / *freq).log2().abs() <= 1.0 / 12.0 {
                    sum += db[other_index];
                    count += 1.0;
                }
            }
            if count == 0.0 {
                db[index]
            } else {
                sum / count
            }
        })
        .collect()
}

fn rms_error(freqs: &[f64], response: &[f64], target: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for index in 0..freqs.len() {
        if freqs[index] < 20.0 || freqs[index] > 20_000.0 {
            continue;
        }
        let error = target[index] - response[index];
        sum += error * error;
        count += 1.0;
    }
    if count == 0.0 {
        0.0
    } else {
        (sum / count).sqrt()
    }
}

fn mean_db(freqs: &[f64], db: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (freq, level) in freqs.iter().zip(db) {
        if *freq >= 300.0 && *freq <= 3_000.0 {
            sum += level;
            count += 1.0;
        }
    }
    if count == 0.0 {
        0.0
    } else {
        sum / count
    }
}

pub fn filter_db(filter: &CorrectionFilter, freq_hz: f64, sample_rate_hz: f64) -> f64 {
    let block = biquad_block(
        &filter.filter_type,
        filter.f_hz,
        filter.q,
        filter.gain_db,
        sample_rate_hz,
    );
    response_db(block.as_ref(), freq_hz, sample_rate_hz)
}
