//! Folded and tapered lines (README sections 5.4, 6.2, and 12, Level 3 tracks C, D, and E),
//! and the `folded-line` half of CHK-E2E-3.

use seatlock_core::issue::Severity;
use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::{json, Value};

fn example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn design(value: &Value) -> seatlock_core::DesignFile {
    serde_json::from_value(value.clone()).unwrap()
}

fn run(value: &Value, enclosure: &str, freqs: &[f64]) -> SimulateResult {
    simulate(
        &design(value),
        &SimulateRequest {
            enclosure: enclosure.into(),
            frequencies_hz: Some(freqs.to_vec()),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

fn log_grid(from_hz: f64, to_hz: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|index| from_hz * (to_hz / from_hz).powf(index as f64 / (count - 1) as f64))
        .collect()
}

fn no_errors(result: &SimulateResult, what: &str) {
    assert!(
        !result
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Error),
        "{what}: {:?}",
        result.issues
    );
    assert!(!result.radiators.is_empty(), "{what}");
}

/// The lowest frequency at which the level reaches `level_db`, by linear interpolation.
fn crossing(freqs: &[f64], spl: &[f64], level_db: f64) -> f64 {
    for index in 1..freqs.len() {
        if spl[index - 1] < level_db && spl[index] >= level_db {
            let t = (level_db - spl[index - 1]) / (spl[index] - spl[index - 1]);
            return freqs[index - 1] * (freqs[index] / freqs[index - 1]).powf(t);
        }
    }
    f64::NAN
}

fn deepest_dip(freqs: &[f64], spl: &[f64], from: f64, to: f64) -> f64 {
    let mut best = (f64::MAX, f64::NAN);
    for (freq, level) in freqs.iter().zip(spl) {
        if *freq >= from && *freq <= to && *level < best.0 {
            best = (*level, *freq);
        }
    }
    best.1
}

fn grid() -> Vec<f64> {
    let mut freqs = log_grid(20.0, 150.0, 30);
    freqs.extend(log_grid(160.0, 300.0, 40));
    freqs
}

#[test]
fn chk_e2e_3_folded_line_quick_and_detailed_models_list_what_each_ignores() {
    let value = example("folded-line");
    let freqs = grid();
    let quick = run(&value, "lumped", &freqs);
    let detailed = run(&value, "waveguide", &freqs);
    for (result, what) in [(&quick, "lumped"), (&detailed, "waveguide")] {
        no_errors(result, what);
        assert!(
            !result
                .issues
                .iter()
                .any(|issue| issue.message.contains("Openings are not part")),
            "{what}"
        );
    }
    let label = |result: &SimulateResult| result.fidelity.ignored.join(" | ");
    assert!(
        label(&quick).contains("each duct one air mass"),
        "{}",
        label(&quick)
    );
    assert!(
        label(&detailed).contains("3 bends in the ducts treated as straight"),
        "{}",
        label(&detailed)
    );
}

/// The full 3D half of CHK-E2E-3 for `folded-line`. About 1 minute in the test build with the
/// early stop (8 before it), so it runs on request: `cargo test -p seatlock-sim --test folded --
/// --ignored`. Last run and its numbers are in STATUS.md.
#[test]
#[ignore]
fn chk_e2e_3_folded_line_runs_in_all_three_models_and_lists_what_each_ignores() {
    let value = example("folded-line");
    let freqs = grid();
    let quick = run(&value, "lumped", &freqs);
    let detailed = run(&value, "waveguide", &freqs);
    let full = run(&value, "fdtd", &freqs);
    for (result, what) in [
        (&quick, "lumped"),
        (&detailed, "waveguide"),
        (&full, "fdtd"),
    ] {
        no_errors(result, what);
        assert!(
            !result
                .issues
                .iter()
                .any(|issue| issue.message.contains("Openings are not part")),
            "{what}"
        );
    }
    let label = |result: &SimulateResult| result.fidelity.ignored.join(" | ");
    assert!(
        label(&quick).contains("each duct one air mass"),
        "{}",
        label(&quick)
    );
    assert!(
        label(&detailed).contains("3 bends in the ducts treated as straight"),
        "{}",
        label(&detailed)
    );
    assert!(
        label(&full).contains("Full 3D at 10 mm"),
        "{}",
        label(&full)
    );
    assert!(full.fdtd.is_some());

    // The detailed model and the full 3D model, which builds the folds cell by cell, put
    // the line's bass roll-off and its first deep dip (where the line's mouth cancels the
    // cone) in the same places. The bends are what the detailed model leaves out.
    let level = 80.0;
    let corner_wave = crossing(&freqs, &detailed.summed_spl_db, level);
    let corner_full = crossing(&freqs, &full.summed_spl_db, level);
    println!("{level} dB reached at {corner_wave:.1} Hz detailed, {corner_full:.1} Hz full 3D");
    assert!((corner_full / corner_wave - 1.0).abs() < 0.05);
    let dip_wave = deepest_dip(&freqs, &detailed.summed_spl_db, 160.0, 300.0);
    let dip_full = deepest_dip(&freqs, &full.summed_spl_db, 160.0, 300.0);
    println!("deepest dip {dip_wave:.1} Hz detailed, {dip_full:.1} Hz full 3D");
    assert!((dip_full / dip_wave - 1.0).abs() < 0.04);
    let mut worst: f64 = 0.0;
    for (bin, freq) in freqs.iter().enumerate() {
        if *freq > 50.0 && *freq < 150.0 {
            worst = worst.max((full.summed_spl_db[bin] - detailed.summed_spl_db[bin]).abs());
        }
    }
    println!("50 to 150 Hz: models within {worst:.2} dB");
    assert!(worst < 1.5, "{worst}");
}

#[test]
fn a_bend_and_the_direction_a_duct_is_written_change_nothing_in_the_detailed_model() {
    let value = example("folded-line");
    let freqs = log_grid(20.0, 1000.0, 60);
    let folded = run(&value, "waveguide", &freqs);
    // The same line written from outside back to the chamber.
    let mut reversed = value.clone();
    let duct = &mut reversed["speaker"]["enclosure"]["ducts"][0];
    let mut path = duct["path_m"].as_array().unwrap().clone();
    path.reverse();
    duct["path_m"] = json!(path);
    duct["from"] = json!("outside");
    duct["to"] = json!("main");
    let (start, end) = (
        duct["section"]["start_m"].clone(),
        duct["section"]["end_m"].clone(),
    );
    duct["section"]["start_m"] = end;
    duct["section"]["end_m"] = start;
    let back = run(&reversed, "waveguide", &freqs);
    for (a, b) in folded.summed_spl_db.iter().zip(&back.summed_spl_db) {
        assert!((a - b).abs() < 1e-9, "{a} {b}");
    }
    // A taper matters: the same line with one size throughout differs.
    let mut even = value.clone();
    even["speaker"]["enclosure"]["ducts"][0]["section"]["end_m"] = json!([0.10, 0.08]);
    let plain = run(&even, "waveguide", &freqs);
    let apart = folded
        .summed_spl_db
        .iter()
        .zip(&plain.summed_spl_db)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(apart > 0.5, "{apart}");
}

#[test]
fn the_folded_line_prints_with_its_bore_and_the_plastic_matches_a_hand_count() {
    let value = example("folded-line");
    let report = seatlock_sim::geometry_report(&design(&value)).unwrap();
    assert!(report.can_export_print, "{:?}", report.warnings);
    let mut plain = value.clone();
    plain["speaker"]["enclosure"]["ducts"] = json!([]);
    let without = seatlock_sim::geometry_report(&design(&plain)).unwrap();
    // The slot is 0.10 m wide; its height tapers from 0.08 to 0.06 m over the 1.02 m center
    // line. Its first 0.06 m run through the chamber's air, as a tube with a 3 mm wall; the
    // rest is bored through the solid around it and the 12 mm front wall.
    let length = 1.02;
    let height = |x: f64| 0.08 - 0.02 * x / length;
    let bore = |x: f64| 0.10 * height(x);
    let tube = |x: f64| 0.106 * (height(x) + 0.006);
    // Simpson's rule is exact for these quadratics.
    let integral = |f: &dyn Fn(f64) -> f64, a: f64, b: f64| {
        (b - a) / 6.0 * (f(a) + 4.0 * f(0.5 * (a + b)) + f(b))
    };
    let in_air = 0.06;
    let expected = integral(&tube, 0.0, in_air)
        - integral(&bore, 0.0, in_air)
        - integral(&bore, in_air, length);
    let added = report.solid_volume_m3 - without.solid_volume_m3;
    println!("plastic added {added:.6} m^3, hand count {expected:.6} m^3");
    assert!(
        (added - expected).abs() < 1e-9,
        "{added} against {expected}"
    );
}

#[test]
fn a_slot_that_bends_out_of_one_plane_is_simulated_but_not_printed() {
    let mut value = example("folded-line");
    value["speaker"]["enclosure"]["ducts"][0]["path_m"] = json!([
        [0.08, 0.19, 0.60],
        [0.08, 0.19, 0.30],
        [0.05, 0.19, 0.30],
        [0.05, 0.19, 0.06],
        [0.05, 0.06, 0.06],
        [0.05, 0.06, 0.35],
        [0.05, 0.0, 0.35]
    ]);
    let report = seatlock_sim::geometry_report(&design(&value)).unwrap();
    assert!(!report.can_export_print);
    assert!(
        report
            .warnings
            .iter()
            .any(|line| line.contains("one plane")),
        "{:?}",
        report.warnings
    );
    let result = run(&value, "waveguide", &log_grid(20.0, 500.0, 20));
    no_errors(&result, "waveguide");
}
