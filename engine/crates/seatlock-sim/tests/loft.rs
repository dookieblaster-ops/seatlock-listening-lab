//! Outer shape `loft` (README §6.2): rectangular cross-sections along a depth spine.

use seatlock_sim::{builtin, geometry_report, simulate, SimulateRequest};

#[test]
fn the_tapered_loft_example_simulates_and_prints() {
    let path = format!(
        "{}/../../../designs/examples/tapered-loft.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let design: seatlock_core::DesignFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let registry = builtin();
    let result = simulate(
        &design,
        &SimulateRequest {
            enclosure: "lumped".into(),
            radiation: "baffle".into(),
            ..SimulateRequest::default()
        },
        &registry,
    );
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert!(result
        .summed_spl_db
        .iter()
        .any(|v| v.is_finite() && *v > 40.0));
    let report = geometry_report(&design).expect("geometry");
    assert!(report.can_export_print, "{:?}", report.warnings);
    assert!(
        report
            .warnings
            .iter()
            .any(|l| l.contains("loft of") && l.contains("cross-sections")),
        "{:?}",
        report.warnings
    );
    assert!(report.interior_volume_m3 > 0.0);
    assert_eq!(report.print_tree["op"], "difference");
}
