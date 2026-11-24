//! Page path: sealed-test with walls on and bem radiation (the e2e button).

use seatlock_sim::{builtin, simulate, SimulateRequest};

#[test]
fn sealed_test_bem_with_walls_like_the_page() {
    let path = format!(
        "{}/../../../designs/examples/sealed-test.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut freqs = Vec::new();
    let mut f = 20.0_f64;
    while f <= 20000.0 {
        freqs.push(f);
        f *= 2f64.powf(1.0 / 24.0);
    }
    let result = simulate(
        &design,
        &SimulateRequest {
            radiation: "bem".into(),
            bem_subdivisions: Some(4),
            listening_window: true,
            walls: true,
            frequencies_hz: Some(freqs),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let errors: Vec<_> = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, seatlock_core::Severity::Error))
        .map(|i| format!("{}: {}", i.code, i.message))
        .collect();
    assert!(
        errors.is_empty(),
        "page-like bem with walls should not error: {errors:?}"
    );
    assert_eq!(result.fidelity.radiation, "bem");
    assert!(!result.summed_spl_db.is_empty());
}
