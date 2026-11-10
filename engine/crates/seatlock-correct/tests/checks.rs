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
    );
    assert!(filters.is_empty(), "{message}");
    assert!(!message.is_empty(), "{message}");
}

#[test]
fn filter_db_is_used() {
    let filter = CorrectionFilter {
        filter_type: "highpass".into(),
        f_hz: 80.0,
        q: 0.707,
        gain_db: 0.0,
        reason: String::new(),
    };
    assert!(filter_db(&filter, 10.0, 48_000.0) < -10.0);
}
