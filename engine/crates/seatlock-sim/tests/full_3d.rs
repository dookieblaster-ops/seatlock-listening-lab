//! The full 3D model on cabinets as the builder makes them (README section 5.5).

use seatlock_core::grid::log_grid;
use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};

fn fixture(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/tests/fixtures/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(design: &seatlock_core::DesignFile, enclosure: &str, dx: Option<f64>) -> SimulateResult {
    simulate(
        design,
        &SimulateRequest {
            enclosure: enclosure.to_string(),
            frequencies_hz: Some(log_grid(10.0, 300.0, 96.0)),
            fdtd_dx_m: dx,
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

/// Tuning: where the cone moves least between 20 and 80 Hz.
fn tuning_hz(result: &SimulateResult) -> f64 {
    let travel = &result.radiators[0].cone_travel_m;
    let mut best = (f64::MAX, 0.0);
    for (index, freq) in result.frequencies_hz.iter().enumerate() {
        if (20.0..=80.0).contains(freq) && travel[index] < best.0 {
            best = (travel[index], *freq);
        }
    }
    best.1
}

fn compare(dx: f64) -> (f64, f64, SimulateResult) {
    let design = fixture("chk-fdtd-4");
    let wave = run(&design, "waveguide", None);
    let full = run(&design, "fdtd", Some(dx));
    let report = full.fdtd.as_ref().expect("the full 3D model ran");
    eprintln!(
        "FDTD4 dx {dx} cells {} steps {} decayed {} ms {:.0} grid {:.4} L net {:.4} L self-check {:?}",
        report.cells,
        report.steps,
        report.decayed,
        report.elapsed_ms,
        report.grid_volume_m3 * 1000.0,
        report.net_volume_m3 * 1000.0,
        report.self_check_error
    );
    for issue in &full.issues {
        eprintln!("ISSUE {} {}", issue.code, issue.message);
    }
    let (fw, ff) = (tuning_hz(&wave), tuning_hz(&full));
    let mut worst = 0.0_f64;
    for (index, freq) in full.frequencies_hz.iter().enumerate() {
        let d = full.summed_spl_db[index] - wave.summed_spl_db[index];
        if index % 12 == 0 {
            eprintln!(
                "ROW {freq:.1} Hz wave {:.2} full {:.2} diff {d:.2}",
                wave.summed_spl_db[index], full.summed_spl_db[index]
            );
        }
        if *freq < 300.0 {
            worst = worst.max(d.abs());
        }
    }
    eprintln!("FDTD4 tuning wave {fw:.2} full {ff:.2}; worst SPL difference {worst:.2} dB");
    ((ff - fw).abs() / fw, worst, full)
}

#[test]
fn chk_fdtd_4_full_3d_against_waveguide_at_5_mm() {
    let (tuning, spl, full) = compare(0.005);
    assert!(full.fdtd.as_ref().unwrap().decayed);
    assert_eq!(full.fidelity.enclosure, "fdtd");
    assert!(tuning <= 0.03, "tuning differs by {tuning}");
    assert!(spl <= 1.5, "SPL differs by {spl} dB");
}

#[test]
fn full_3d_at_10_mm_agrees_with_waveguide_too() {
    let (tuning, spl, full) = compare(0.01);
    assert!(full.fdtd.as_ref().unwrap().decayed);
    assert!(tuning <= 0.03, "tuning differs by {tuning}");
    assert!(spl <= 1.5, "SPL differs by {spl} dB");
}

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn sealed_box_passes_the_low_frequency_self_check() {
    // README 5.5: below C0 / (50 * longest dimension), Z_ab within 3 % of the quick model.
    // At 10 mm the 8 mm walls leave 14 x 16 x 23 cells of air, 7 % short: the warning says so.
    let result = run(&example("sealed-test"), "fdtd", Some(0.005));
    let report = result.fdtd.as_ref().expect("ran");
    eprintln!(
        "SEALED grid {:.4} L net {:.4} L self-check {:?} ms {:.0}",
        report.grid_volume_m3 * 1000.0,
        report.net_volume_m3 * 1000.0,
        report.self_check_error,
        report.elapsed_ms
    );
    let error = report.self_check_error.expect("checked");
    assert!(error < 0.03, "self-check {error}");
    assert!(result
        .issues
        .iter()
        .all(|issue| issue.code != "fdtd_self_check"));
}

#[test]
fn a_shape_that_is_not_a_box_runs_in_full_3d() {
    for name in ["hex-prism", "pvc-tube", "rounded-box"] {
        let result = run(&example(name), "fdtd", Some(0.01));
        let report = result.fdtd.as_ref().unwrap_or_else(|| {
            panic!(
                "{name}: {:?}",
                result.issues.iter().map(|i| &i.message).collect::<Vec<_>>()
            )
        });
        eprintln!(
            "SHAPE {name} grid {:.3} L net {:.3} L self-check {:?} ms {:.0}",
            report.grid_volume_m3 * 1000.0,
            report.net_volume_m3 * 1000.0,
            report.self_check_error,
            report.elapsed_ms
        );
        assert_eq!(result.fidelity.enclosure, "fdtd");
        assert!(
            !result
                .fidelity
                .ignored
                .iter()
                .any(|line| line.contains("sealed-box model")),
            "{name}: {:?}",
            result.fidelity.ignored
        );
        assert!(report.decayed, "{name}");
        // Stair-stepped curves at 10 mm: the volume within a tenth.
        assert!(
            (report.grid_volume_m3 / report.net_volume_m3 - 1.0).abs() < 0.1,
            "{name}"
        );
    }
}

#[test]
#[ignore = "timing only, about 2 minutes"]
fn two_way_vented_run_time() {
    let design = example("two-way-vented");
    for dx in [0.01, 0.005] {
        let r = run(&design, "fdtd", Some(dx));
        let f = r.fdtd.as_ref().unwrap();
        eprintln!(
            "TW dx {dx} cells {} steps {} decayed {} ms {:.0} sc {:?}",
            f.cells, f.steps, f.decayed, f.elapsed_ms, f.self_check_error
        );
    }
}
