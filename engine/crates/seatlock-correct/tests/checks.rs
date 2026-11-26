use seatlock_core::grid::log_grid;
use seatlock_correct::{filter_db, metrics, CorrectionFilter};
use seatlock_dsp::{biquad_block, response_db};

fn peak_response() -> (Vec<f64>, Vec<f64>) {
    let freqs = log_grid(20.0, 20_000.0, 48.0);
    let block = biquad_block("peaking", 1_000.0, 2.0, 5.0, 48_000.0);
    let response = freqs
        .iter()
        .map(|freq| response_db(block.as_ref(), *freq, 48_000.0))
        .collect();
    (freqs, response)
}

#[test]
fn chk_eq_1_rolloff_does_not_hide_the_peak() {
    let freqs = log_grid(20.0, 20_000.0, 48.0);
    let peak = biquad_block("peaking", 1_000.0, 2.0, 5.0, 48_000.0);
    let response: Vec<f64> = freqs
        .iter()
        .map(|freq| {
            let ratio = freq / 80.0;
            let hp = 20.0 * (ratio * ratio / (1.0 + ratio.powi(4)).sqrt()).log10();
            hp + response_db(peak.as_ref(), *freq, 48_000.0)
        })
        .collect();
    let mut headroom = Vec::new();
    let mut max_spl = Vec::new();
    for freq in &freqs {
        if *freq < 200.0 {
            headroom.push(-20.0);
            max_spl.push(70.0);
        } else {
            headroom.push(20.0);
            max_spl.push(110.0);
        }
    }
    let target = vec![0.0; freqs.len()];
    let (filters, message) = seatlock_correct::c1(
        &freqs,
        &response,
        &target,
        &headroom,
        [3000.0, 5000.0],
        6,
        48_000.0,
        20.0,
    );
    let cut = filters
        .iter()
        .find(|filter| filter.filter_type == "peaking" && filter.gain_db < 0.0);
    let cut = cut.expect(&message);
    assert!((cut.gain_db + 5.0).abs() <= 0.5, "gain {}", cut.gain_db);
    assert!((cut.f_hz - 1000.0).abs() / 1000.0 <= 0.05, "f {}", cut.f_hz);
    assert!((cut.q - 2.0).abs() / 2.0 <= 0.2, "q {}", cut.q);
    let _ = max_spl;
}

#[test]
fn chk_eq_1_one_cut_removes_the_peak() {
    let (freqs, response) = peak_response();
    let headroom = vec![20.0; freqs.len()];
    let target = vec![0.0; freqs.len()];
    let (filters, _message) = seatlock_correct::c1(
        &freqs,
        &response,
        &target,
        &headroom,
        [3000.0, 5000.0],
        4,
        48_000.0,
        20.0,
    );
    let peaking: Vec<_> = filters
        .iter()
        .filter(|filter| filter.filter_type == "peaking")
        .collect();
    assert_eq!(
        peaking.len(),
        1,
        "{:?}",
        filters
            .iter()
            .map(|f| (&f.filter_type, f.f_hz, f.gain_db, f.q))
            .collect::<Vec<_>>()
    );
    let filter = peaking[0];
    assert!(
        (filter.gain_db + 5.0).abs() <= 0.5,
        "gain {}",
        filter.gain_db
    );
    assert!(
        (filter.f_hz - 1000.0).abs() / 1000.0 <= 0.05,
        "f {}",
        filter.f_hz
    );
    assert!((filter.q - 2.0).abs() / 2.0 <= 0.20, "q {}", filter.q);
    let mut flattened = response.clone();
    for filter in &filters {
        for (level, freq) in flattened.iter_mut().zip(&freqs) {
            *level += filter_db(filter, *freq, 48_000.0);
        }
    }
    let (_f6, flatness, _) = metrics(&freqs, &flattened, &vec![120.0; freqs.len()]);
    assert!(flatness < 0.2, "flatness {flatness}");
}

#[test]
fn chk_eq_3_flat_response_adds_nothing() {
    let freqs = log_grid(20.0, 20_000.0, 24.0);
    let response = vec![0.0; freqs.len()];
    let target = vec![0.0; freqs.len()];
    let headroom = vec![20.0; freqs.len()];
    let (filters, message) = seatlock_correct::c1(
        &freqs,
        &response,
        &target,
        &headroom,
        [3000.0, 5000.0],
        8,
        48_000.0,
        20.0,
    );
    assert!(filters.is_empty(), "{message}");
    assert!(!message.is_empty(), "{message}");
}

#[test]
fn linkwitz_is_near_flat_when_the_move_is_small() {
    let filter = CorrectionFilter {
        filter_type: "linkwitz".into(),
        f_hz: 80.0,
        q: 0.707,
        gain_db: 0.0,
        from_hz: 86.6,
        from_q: 0.7006,
        reason: String::new(),
    };
    let db = filter_db(&filter, 1000.0, 48_000.0);
    assert!(db.abs() < 1.0, "linkwitz at 1 kHz is {db} dB");
}

#[test]
fn filter_db_is_used() {
    let filter = CorrectionFilter {
        filter_type: "highpass".into(),
        f_hz: 80.0,
        q: 0.707,
        gain_db: 0.0,
        from_hz: 0.0,
        from_q: 0.0,
        reason: String::new(),
    };
    assert!(filter_db(&filter, 10.0, 48_000.0) < -10.0);
}

#[test]
fn c7_linear_phase_fir_matches_a_peaking_cut_magnitude() {
    let freqs = log_grid(20.0, 20_000.0, 48.0);
    let peak = biquad_block("peaking", 1_000.0, 2.0, 5.0, 48_000.0);
    let gain: Vec<f64> = freqs
        .iter()
        .map(|f| -response_db(peak.as_ref(), *f, 48_000.0))
        .collect();
    let fir =
        seatlock_correct::linear_phase_fir_from_gain_db(&freqs, &gain, 48_000.0, 257).unwrap();
    assert_eq!(fir.taps.len(), 257);
    assert_eq!(fir.latency_samples, 128);
    let block = seatlock_dsp::fir_block(fir.taps.clone()).unwrap();
    let mut err = 0.0;
    let mut n = 0.0;
    for (freq, want) in freqs.iter().zip(&gain) {
        if *freq < 100.0 || *freq > 10_000.0 {
            continue;
        }
        let got = response_db(block.as_ref(), *freq, 48_000.0);
        err += (got - want).powi(2);
        n += 1.0;
    }
    let rms = (err / n).sqrt();
    assert!(rms < 1.5, "FIR vs peaking magnitude RMS {rms} dB");
}

#[test]
fn correct_linear_phase_keeps_bass_iir_and_adds_fir() {
    // Flat midband with a clear +5 dB peak at 1 kHz (same shape as CHK-EQ-1).
    let (freqs, response) = peak_response();
    let max_spl = vec![120.0; freqs.len()];
    let outcome = seatlock_correct::correct_linear_phase(
        &freqs,
        &response,
        &max_spl,
        95.0,
        [3000.0, 5000.0],
        4,
        48_000.0,
        50.0,
        0.7,
        false,
        0.0,
        &[],
    );
    assert!(outcome.fir.is_some(), "expected FIR: {}", outcome.message);
    assert!(
        outcome.filters.iter().all(|f| f.filter_type != "peaking"),
        "peaking should be folded into FIR: {:?}",
        outcome.filters
    );
    assert!(
        outcome.message.contains("linear-phase FIR"),
        "{}",
        outcome.message
    );
}
