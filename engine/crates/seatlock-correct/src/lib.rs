//! Target curves, auto EQ, and correction features that respect hardware limits.

mod dynamic_bass;
mod indirect;
mod resonance;

pub use dynamic_bass::{dynamic_bass_plan, DynamicBassPlan};
pub use indirect::{indirect_tuning, midband_mean_db, IndirectTuning};
pub use resonance::{find_cabinet_resonances, resonance_cuts, ResonancePeak};

use seatlock_dsp::{biquad_block, response_db};

#[derive(Clone, Debug)]
pub struct CorrectionFilter {
    pub filter_type: String,
    pub f_hz: f64,
    pub q: f64,
    pub gain_db: f64,
    pub from_hz: f64,
    pub from_q: f64,
    pub reason: String,
}

/// One FIR correction block (C7): linear-phase taps plus latency in samples.
#[derive(Clone, Debug)]
pub struct CorrectionFir {
    pub taps: Vec<f64>,
    pub latency_samples: usize,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct CorrectionOutcome {
    pub filters: Vec<CorrectionFilter>,
    /// When set, midband peaking (C1) was replaced by this linear-phase FIR (C7).
    pub fir: Option<CorrectionFir>,
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

#[allow(clippy::too_many_arguments)]
pub fn correct(
    freqs: &[f64],
    response_db: &[f64],
    max_spl_db: &[f64],
    design_level_db: f64,
    protected_hz: [f64; 2],
    max_filters: usize,
    sample_rate_hz: f64,
    box_fc_hz: f64,
    box_qtc: f64,
    vented: bool,
    tuning_hz: f64,
    fixed_db: &[f64],
) -> CorrectionOutcome {
    let (natural_f6, _, _) = metrics(freqs, response_db, max_spl_db);
    let (bass, f_low, bass_note) = c2(
        freqs,
        response_db,
        max_spl_db,
        design_level_db,
        box_fc_hz,
        box_qtc,
        natural_f6,
        sample_rate_hz,
        vented,
        tuning_hz,
    );
    let mut with_bass = response_db.to_vec();
    for filter in &bass {
        add_filter(&mut with_bass, freqs, filter, sample_rate_hz);
    }
    let anchor = mean_db(freqs, &with_bass);
    let (f6_after, _, _) = metrics(freqs, &with_bass, max_spl_db);
    let bass_message = if !bass_note.is_empty() {
        format!("{bass_note} Bass limit {f6_after:.0} Hz.")
    } else if f6_after + 1.0 < natural_f6 {
        format!("Bass extended from {natural_f6:.0} Hz to {f6_after:.0} Hz, staying inside the driver's limits.")
    } else {
        format!(
            "Bass now starts at {f6_after:.0} Hz instead of {natural_f6:.0} Hz, so a {design_level_db:.0} dB note never exceeds the cone's travel."
        )
    };
    let target: Vec<f64> = freqs
        .iter()
        .enumerate()
        .map(|(index, freq)| {
            anchor
                + target_shape_db(*freq, f_low, vented, tuning_hz)
                + fixed_db.get(index).copied().unwrap_or(0.0)
        })
        .collect();
    let headroom: Vec<f64> = max_spl_db
        .iter()
        .map(|level| level - design_level_db)
        .collect();
    let before = metrics(freqs, &with_bass, max_spl_db);
    let (mut fit, fit_message) = c1(
        freqs,
        &with_bass,
        &target,
        &headroom,
        protected_hz,
        max_filters,
        sample_rate_hz,
        f_low,
    );
    trim_to_headroom(&mut fit, freqs, &bass, &headroom, sample_rate_hz);
    let mut after_curve = with_bass.clone();
    for filter in &fit {
        add_filter(&mut after_curve, freqs, filter, sample_rate_hz);
    }
    let boosted_max: Vec<f64> = max_spl_db
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let boost: f64 = fit
                .iter()
                .map(|filter| filter_db(filter, freqs[index], sample_rate_hz))
                .sum();
            level - boost.max(0.0)
        })
        .collect();
    let after = metrics(freqs, &after_curve, &boosted_max);
    let mut message = bass_message;
    if !fit.is_empty()
        && (before.1 - after.1 < 0.2 || after.0 > before.0 + 0.5 || after.2 + 0.2 < before.2)
    {
        fit.clear();
        message = format!(
            "{message} The extra filters were dropped because they did not help the scores."
        );
    } else if !fit_message.is_empty() {
        message = format!("{message} {fit_message}");
    }
    let mut filters = bass;
    filters.extend(fit);
    CorrectionOutcome {
        filters,
        fir: None,
        message,
    }
}

/// Same as [`correct`], then replaces midband peaking filters with one linear-phase FIR
/// that matches their magnitude (C7). Bass extension / protective high-pass stay as IIR.
#[allow(clippy::too_many_arguments)]
pub fn correct_linear_phase(
    freqs: &[f64],
    response_db: &[f64],
    max_spl_db: &[f64],
    design_level_db: f64,
    protected_hz: [f64; 2],
    max_filters: usize,
    sample_rate_hz: f64,
    box_fc_hz: f64,
    box_qtc: f64,
    vented: bool,
    tuning_hz: f64,
    fixed_db: &[f64],
) -> CorrectionOutcome {
    let mut outcome = correct(
        freqs,
        response_db,
        max_spl_db,
        design_level_db,
        protected_hz,
        max_filters,
        sample_rate_hz,
        box_fc_hz,
        box_qtc,
        vented,
        tuning_hz,
        fixed_db,
    );
    let mid: Vec<CorrectionFilter> = outcome
        .filters
        .iter()
        .filter(|f| {
            // C2 bass uses linkwitz / highpass; C1 peaking and shelves stay for FIR.
            f.filter_type == "peaking"
                || f.filter_type == "lowshelf"
                || f.filter_type == "highshelf"
        })
        .cloned()
        .collect();
    if mid.is_empty() {
        outcome.message = format!(
            "{} Linear-phase correction had no midband filters to turn into an FIR.",
            outcome.message
        );
        return outcome;
    }
    let gain_db: Vec<f64> = freqs
        .iter()
        .map(|freq| {
            mid.iter()
                .map(|f| filter_db(f, *freq, sample_rate_hz))
                .sum()
        })
        .collect();
    match linear_phase_fir_from_gain_db(freqs, &gain_db, sample_rate_hz, 257) {
        Ok(fir) => {
            let latency = fir.latency_samples;
            outcome.filters.retain(|f| {
                f.filter_type != "peaking"
                    && f.filter_type != "lowshelf"
                    && f.filter_type != "highshelf"
            });
            outcome.fir = Some(fir);
            outcome.message = format!(
                "{} Midband correction is one linear-phase FIR (257 taps, latency {latency} \
                 samples / {:.1} ms at {sample_rate_hz:.0} Hz).",
                outcome.message,
                latency as f64 / sample_rate_hz * 1000.0
            );
        }
        Err(err) => {
            outcome.message = format!(
                "{} Linear-phase FIR was not built ({err}); peaking filters were kept.",
                outcome.message
            );
        }
    }
    outcome
}

/// Frequency-sampled linear-phase FIR whose magnitude matches `gain_db` on `freqs`.
pub fn linear_phase_fir_from_gain_db(
    freqs: &[f64],
    gain_db: &[f64],
    sample_rate_hz: f64,
    tap_count: usize,
) -> Result<CorrectionFir, String> {
    if tap_count < 3 || tap_count.is_multiple_of(2) {
        return Err("FIR length must be odd and at least 3.".into());
    }
    if freqs.len() != gain_db.len() || freqs.is_empty() {
        return Err("gain curve length mismatch.".into());
    }
    let n = tap_count;
    let delay = (n - 1) / 2;
    let nyquist = sample_rate_hz * 0.5;
    // Desired real amplitude on FFT bins 0..N/2 (even symmetry).
    let mut amp = vec![1.0; n / 2 + 1];
    for (k, a) in amp.iter_mut().enumerate() {
        let f = k as f64 * sample_rate_hz / n as f64;
        if f > nyquist {
            *a = 0.0;
            continue;
        }
        let db = interp_db(freqs, gain_db, f);
        *a = 10_f64.powf(db / 20.0);
        if !a.is_finite() || *a < 0.0 {
            *a = 0.0;
        }
    }
    // IFFT of linear-phase spectrum: h[n] = (1/N) * (A0 + 2 Σ A_k cos(2π k (n−D)/N) + A_{N/2} cos(π(n−D)))
    let mut taps = vec![0.0; n];
    for (i, tap) in taps.iter_mut().enumerate() {
        let m = i as f64 - delay as f64;
        let mut sum = amp[0];
        for (k, a_k) in amp.iter().enumerate().take(n / 2).skip(1) {
            let phase = 2.0 * std::f64::consts::PI * k as f64 * m / n as f64;
            sum += 2.0 * a_k * phase.cos();
        }
        // Odd N: include the Nyquist bin.
        if !n.is_multiple_of(2) {
            let phase = std::f64::consts::PI * m;
            sum += amp[n / 2] * phase.cos();
        }
        *tap = sum / n as f64;
    }
    // Hann window to reduce ringing.
    for (i, tap) in taps.iter_mut().enumerate() {
        let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos());
        *tap *= w;
    }
    // Restore DC gain after windowing.
    let dc: f64 = taps.iter().sum();
    let want_dc = amp[0];
    if dc.abs() > 1e-18 && want_dc.is_finite() {
        let scale = want_dc / dc;
        for tap in &mut taps {
            *tap *= scale;
        }
    }
    if taps.iter().any(|t| !t.is_finite()) {
        return Err("FIR taps were not finite.".into());
    }
    Ok(CorrectionFir {
        taps,
        latency_samples: delay,
        reason: "Linear-phase correction matching the midband auto-EQ magnitude (C7).".into(),
    })
}

fn interp_db(freqs: &[f64], gain_db: &[f64], f: f64) -> f64 {
    if f <= freqs[0] {
        return gain_db[0];
    }
    if f >= *freqs.last().unwrap() {
        return *gain_db.last().unwrap();
    }
    let i = freqs.partition_point(|x| *x < f).saturating_sub(1);
    let j = (i + 1).min(freqs.len() - 1);
    if i == j {
        return gain_db[i];
    }
    let t = (f.ln() - freqs[i].ln()) / (freqs[j].ln() - freqs[i].ln()).max(1e-18);
    gain_db[i] + t * (gain_db[j] - gain_db[i])
}

#[allow(clippy::too_many_arguments)]
fn c2(
    freqs: &[f64],
    response_db: &[f64],
    max_spl_db: &[f64],
    design_level_db: f64,
    box_fc_hz: f64,
    box_qtc: f64,
    natural_f6_hz: f64,
    sample_rate_hz: f64,
    vented: bool,
    tuning_hz: f64,
) -> (Vec<CorrectionFilter>, f64, String) {
    // The analog shape can fit while the Linkwitz biquad still boosts the bass.
    // Keep walking up until the filters that will actually be used stay inside the limits.
    let floor = if vented { tuning_hz.max(20.0) } else { 20.0 };
    let search = |level: f64| -> Option<f64> {
        freqs.iter().copied().find(|freq| {
            *freq >= floor
                && *freq <= BASS_SEARCH_TOP_HZ
                && shape_fits(freqs, max_spl_db, level, *freq, vented, tuning_hz)
                && filters_stay_inside(
                    freqs,
                    response_db,
                    max_spl_db,
                    &bass_filters(*freq, box_fc_hz, box_qtc, vented, tuning_hz),
                    level,
                    sample_rate_hz,
                )
        })
    };
    if let Some(f_low) = search(design_level_db) {
        return (
            bass_filters(f_low, box_fc_hz, box_qtc, vented, tuning_hz),
            f_low,
            String::new(),
        );
    }
    // The speaker cannot play the design level even in its midrange. Shape the bass for the
    // level it can reach there instead of pushing the bass limit up past the midrange.
    // The level at 1 kHz at which the response first touches a limit somewhere from 300 Hz to
    // 3 kHz, less a small margin.
    let at_1k = freqs
        .iter()
        .position(|freq| *freq >= 1_000.0)
        .map(|index| response_db[index])
        .unwrap_or(0.0);
    let reachable = freqs
        .iter()
        .enumerate()
        .filter(|(_, freq)| (300.0..=3_000.0).contains(*freq))
        .map(|(index, _)| max_spl_db[index] - (response_db[index] - at_1k))
        .fold(f64::INFINITY, f64::min)
        - 0.5;
    if reachable.is_finite() && reachable < design_level_db {
        if let Some(f_low) = search(reachable) {
            return (
                bass_filters(f_low, box_fc_hz, box_qtc, vented, tuning_hz),
                f_low,
                format!(
                    "This speaker reaches only {reachable:.0} dB between 300 Hz and 3 kHz, so the bass is shaped for that level instead of {design_level_db:.0} dB."
                ),
            );
        }
    }
    // Nothing fits: leave the natural bass, keeping only the protection a port needs.
    let protect: Vec<CorrectionFilter> = if vented {
        bass_filters(tuning_hz, box_fc_hz, box_qtc, true, tuning_hz)
            .into_iter()
            .take(1)
            .collect()
    } else {
        Vec::new()
    };
    (
        protect,
        natural_f6_hz,
        "No bass shape fits inside this speaker's limits, so the natural bass is kept.".into(),
    )
}

/// The bass corner is never moved above this: a higher corner is no longer bass correction.
const BASS_SEARCH_TOP_HZ: f64 = 300.0;

fn bass_filters(
    f_low: f64,
    box_fc_hz: f64,
    box_qtc: f64,
    vented: bool,
    tuning_hz: f64,
) -> Vec<CorrectionFilter> {
    if vented {
        let mut filters = vec![CorrectionFilter {
            filter_type: "highpass".into(),
            f_hz: tuning_hz.max(15.0),
            q: 0.707,
            gain_db: 0.0,
            from_hz: 0.0,
            from_q: 0.0,
            reason: format!(
                "Protective high-pass at {:.0} Hz, because below the port tuning the cone is unloaded.",
                tuning_hz.max(15.0)
            ),
        }];
        if f_low > tuning_hz * 1.2 {
            for q in [0.5412_f64, 1.3066] {
                filters.push(CorrectionFilter {
                    filter_type: "highpass".into(),
                    f_hz: f_low,
                    q,
                    gain_db: 0.0,
                    from_hz: 0.0,
                    from_q: 0.0,
                    reason: format!(
                        "Raises the bass corner to {f_low:.0} Hz with part of a fourth-order slope."
                    ),
                });
            }
        }
        return filters;
    }
    let protect = (f_low / 2.0).max(15.0);
    vec![
        CorrectionFilter {
            filter_type: "linkwitz".into(),
            f_hz: f_low,
            q: 0.707,
            gain_db: 0.0,
            from_hz: box_fc_hz.max(20.0),
            from_q: box_qtc.max(0.3),
            reason: format!(
                "Moves the box resonance from {box_fc_hz:.0} Hz to {f_low:.0} Hz so the bass follows a clean shape."
            ),
        },
        CorrectionFilter {
            filter_type: "highpass".into(),
            f_hz: protect,
            q: 0.707,
            gain_db: 0.0,
            from_hz: 0.0,
            from_q: 0.0,
            reason: format!("Protective high-pass at {protect:.0} Hz below the new bass limit."),
        },
    ]
}

fn filters_stay_inside(
    freqs: &[f64],
    response_db: &[f64],
    max_spl_db: &[f64],
    filters: &[CorrectionFilter],
    design_level_db: f64,
    sample_rate_hz: f64,
) -> bool {
    let Some(reference) = freqs.iter().position(|freq| *freq >= 1_000.0) else {
        return false;
    };
    let spl_ref = response_db.get(reference).copied().unwrap_or(0.0);
    let h_ref: f64 = filters
        .iter()
        .map(|filter| filter_db(filter, 1_000.0, sample_rate_hz))
        .sum();
    freqs.iter().enumerate().all(|(index, freq)| {
        if *freq < 10.0 {
            return true;
        }
        let gain: f64 = filters
            .iter()
            .map(|filter| filter_db(filter, *freq, sample_rate_hz))
            .sum();
        // Above 3 kHz the bass filters change nothing; the coil and the amplifier set the limit
        // there, which no bass shape can fix (see `shape_fits`).
        if *freq > 3_000.0 && (gain - h_ref).abs() < 0.05 {
            return true;
        }
        let room = (max_spl_db[index] - design_level_db).max(0.0);
        if gain > room + 0.05 {
            return false;
        }
        let played = response_db[index] + (design_level_db - spl_ref - h_ref) + gain;
        played <= max_spl_db[index] + 0.02
    })
}

fn target_shape_db(freq: f64, f_low: f64, vented: bool, tuning_hz: f64) -> f64 {
    if vented {
        let ratio = f_low / freq.max(1e-6);
        let fourth = 1.0 / (1.0 + ratio.powi(8)).sqrt();
        let fourth_db = 20.0 * fourth.max(1e-12).log10();
        let protect = tuning_hz.max(f_low / 2.0).max(10.0);
        return fourth_db + highpass_db(freq, protect, 0.707);
    }
    highpass_db(freq, f_low, 0.707) + highpass_db(freq, (f_low / 2.0).max(10.0), 0.707)
}

fn highpass_db(freq: f64, fc: f64, q: f64) -> f64 {
    let f2 = freq * freq;
    let fc2 = fc * fc;
    let mag = f2 / ((f2 - fc2).powi(2) + (freq * fc / q).powi(2)).sqrt();
    20.0 * mag.max(1e-12).log10()
}

fn shape_fits(
    freqs: &[f64],
    max_spl_db: &[f64],
    design_level_db: f64,
    f_low: f64,
    vented: bool,
    tuning_hz: f64,
) -> bool {
    freqs.iter().zip(max_spl_db).all(|(freq, limit)| {
        // Above a few kilohertz the coil inductance, not the bass shape, sets the clean level.
        *freq < 10.0
            || *freq > 3_000.0
            || design_level_db + target_shape_db(*freq, f_low, vented, tuning_hz) <= *limit + 0.2
    })
}

#[allow(clippy::too_many_arguments)]
pub fn c1(
    freqs: &[f64],
    response_db: &[f64],
    target_db: &[f64],
    headroom_db: &[f64],
    protected_hz: [f64; 2],
    max_filters: usize,
    sample_rate_hz: f64,
    f_low_hz: f64,
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
    let mut excluded = vec![false; freqs.len()];
    let mut previous = initial;
    let mut notes: Vec<(f64, String)> = Vec::new();
    for _ in 0..max_filters.max(8) * 2 {
        if filters.len() >= max_filters {
            break;
        }
        let smoothed_now = smooth(freqs, &current);
        let error: Vec<f64> = target_db
            .iter()
            .zip(&smoothed_now)
            .map(|(target, level)| target - level)
            .collect();
        let Some((index, peak)) = error
            .iter()
            .enumerate()
            .filter(|(i, _)| !excluded[*i])
            .max_by(|left, right| {
                left.1
                    .abs()
                    .partial_cmp(&right.1.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            break;
        };
        let peak = *peak;
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
        let mut gain = peak.clamp(-12.0, 6.0);
        if gain > 0.0 {
            if center >= protected_hz[0] && center <= protected_hz[1] {
                notes.push((
                    center,
                    format!(
                        "The region near {center:.0} Hz was left alone: no boost between 3 kHz and 5 kHz."
                    ),
                ));
                exclude_region(&mut excluded, left, right);
                previous = rms_allowed(freqs, &smooth(freqs, &current), target_db, &excluded);
                continue;
            }
            if q > 4.0 {
                notes.push((
                    center,
                    format!(
                        "A narrow dip near {center:.0} Hz was left alone. That is a cancellation, not something EQ can fill."
                    ),
                ));
                exclude_region(&mut excluded, left, right);
                previous = rms_allowed(freqs, &smooth(freqs, &current), target_db, &excluded);
                continue;
            }
            q = q.min(4.0);
            let room = headroom_db.get(index).copied().unwrap_or(0.0);
            if room <= 0.0 {
                notes.push((
                    center,
                    format!(
                        "The region near {center:.0} Hz was left alone: boosting it would exceed what the driver can do."
                    ),
                ));
                exclude_region(&mut excluded, left, right);
                previous = rms_allowed(freqs, &smooth(freqs, &current), target_db, &excluded);
                continue;
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
            exclude_region(&mut excluded, left, right);
            previous = rms_allowed(freqs, &smooth(freqs, &current), target_db, &excluded);
            continue;
        }
        let candidate = CorrectionFilter {
            filter_type: "peaking".into(),
            f_hz: center,
            q,
            gain_db: gain,
            from_hz: 0.0,
            from_q: 0.0,
            reason: if gain < 0.0 {
                format!("Cuts {:.1} dB at {center:.0} Hz to flatten a peak.", -gain)
            } else {
                format!("Boosts {gain:.1} dB at {center:.0} Hz where there is room to spare.")
            },
        };
        let mut trial = current.clone();
        add_filter(&mut trial, freqs, &candidate, sample_rate_hz);
        let improved = rms_allowed(freqs, &smooth(freqs, &trial), target_db, &excluded);
        if previous - improved < 0.1 {
            break;
        }
        current = trial;
        previous = improved;
        filters.push(candidate);
    }
    let after = rms_allowed(freqs, &smooth(freqs, &current), target_db, &excluded);
    let before_allowed = rms_allowed(freqs, &smooth(freqs, response_db), target_db, &excluded);
    if filters.is_empty() {
        let message = if notes.is_empty() {
            "No filter improved the response enough to keep.".into()
        } else {
            render_notes(&notes, f_low_hz)
        };
        return (Vec::new(), message);
    }
    if before_allowed - after < 0.2 {
        return (
            Vec::new(),
            "Correction was dropped because it did not make the response meaningfully more even."
                .into(),
        );
    }
    let mut parts: Vec<String> = filters.iter().map(|filter| filter.reason.clone()).collect();
    let notes_text = render_notes(&notes, f_low_hz);
    if !notes_text.is_empty() {
        parts.push(notes_text);
    }
    (filters, parts.join(" "))
}

fn render_notes(notes: &[(f64, String)], f_low_hz: f64) -> String {
    let eligible: Vec<&str> = notes
        .iter()
        .filter(|(hz, _)| *hz + 0.5 >= f_low_hz)
        .map(|(_, text)| text.as_str())
        .collect();
    let hidden = notes.len().saturating_sub(eligible.len());
    let extra = eligible.len().saturating_sub(3);
    let mut text = eligible
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    let rest = hidden + extra;
    if rest > 0 {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!("{rest} more regions were left alone."));
    }
    text
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
                from_hz: 0.0,
                from_q: 0.0,
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
    for (level, freq) in curve.iter_mut().zip(freqs) {
        *level += filter_db(filter, *freq, sample_rate_hz);
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

fn exclude_region(excluded: &mut [bool], left: usize, right: usize) {
    for slot in excluded.iter_mut().take(right.saturating_add(1)).skip(left) {
        *slot = true;
    }
}

fn rms_allowed(freqs: &[f64], response: &[f64], target: &[f64], excluded: &[bool]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for index in 0..freqs.len() {
        if excluded[index] || freqs[index] < 20.0 || freqs[index] > 20_000.0 {
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
    if filter.filter_type == "linkwitz" {
        let (b0, b1, b2, a1, a2) = linkwitz_coeffs(
            filter.from_hz,
            filter.from_q,
            filter.f_hz,
            filter.q,
            sample_rate_hz,
        );
        return biquad_db(b0, b1, b2, a1, a2, freq_hz, sample_rate_hz);
    }
    let block = biquad_block(
        &filter.filter_type,
        filter.f_hz,
        filter.q,
        filter.gain_db,
        sample_rate_hz,
    );
    response_db(block.as_ref(), freq_hz, sample_rate_hz)
}

fn linkwitz_coeffs(
    from_hz: f64,
    from_q: f64,
    to_hz: f64,
    to_q: f64,
    sample_rate_hz: f64,
) -> (f64, f64, f64, f64, f64) {
    let w0 = 2.0 * sample_rate_hz * (std::f64::consts::PI * from_hz / sample_rate_hz).tan();
    let w1 = 2.0 * sample_rate_hz * (std::f64::consts::PI * to_hz / sample_rate_hz).tan();
    let c = 2.0 * sample_rate_hz;
    let (b0, b1, b2) = analog_bilinear(w0, from_q, c);
    let (a0, a1, a2) = analog_bilinear(w1, to_q, c);
    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

fn analog_bilinear(omega: f64, q: f64, c: f64) -> (f64, f64, f64) {
    let b0 = c * c + (omega / q) * c + omega * omega;
    let b1 = -2.0 * c * c + 2.0 * omega * omega;
    let b2 = c * c - (omega / q) * c + omega * omega;
    (b0, b1, b2)
}

fn biquad_db(
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    freq_hz: f64,
    sample_rate_hz: f64,
) -> f64 {
    let zm1 = num_polar(freq_hz, sample_rate_hz);
    let zm2 = (zm1.0 * zm1.0 - zm1.1 * zm1.1, 2.0 * zm1.0 * zm1.1);
    let num = (b0 + b1 * zm1.0 + b2 * zm2.0, b1 * zm1.1 + b2 * zm2.1);
    let den = (1.0 + a1 * zm1.0 + a2 * zm2.0, a1 * zm1.1 + a2 * zm2.1);
    let den_mag = (den.0 * den.0 + den.1 * den.1).sqrt().max(1e-20);
    let num_mag = (num.0 * num.0 + num.1 * num.1).sqrt();
    20.0 * (num_mag / den_mag).log10()
}

fn num_polar(freq_hz: f64, sample_rate_hz: f64) -> (f64, f64) {
    let angle = -2.0 * std::f64::consts::PI * freq_hz / sample_rate_hz;
    (angle.cos(), angle.sin())
}

fn trim_to_headroom(
    fit: &mut [CorrectionFilter],
    freqs: &[f64],
    bass: &[CorrectionFilter],
    headroom_db: &[f64],
    sample_rate_hz: f64,
) {
    for _ in 0..8 {
        let mut worst_freq = 0.0;
        let mut worst_over = 0.0;
        for (index, freq) in freqs.iter().enumerate() {
            let total: f64 = bass
                .iter()
                .chain(fit.iter())
                .map(|filter| filter_db(filter, *freq, sample_rate_hz))
                .sum();
            let over = total.max(0.0) - headroom_db.get(index).copied().unwrap_or(0.0);
            if over > worst_over {
                worst_over = over;
                worst_freq = *freq;
            }
        }
        if worst_over <= 0.05 {
            break;
        }
        let Some(filter) =
            fit.iter_mut()
                .filter(|filter| filter.gain_db > 0.0)
                .max_by(|left, right| {
                    filter_db(left, worst_freq, sample_rate_hz)
                        .partial_cmp(&filter_db(right, worst_freq, sample_rate_hz))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        else {
            break;
        };
        filter.gain_db = (filter.gain_db - worst_over).max(0.0);
    }
}
