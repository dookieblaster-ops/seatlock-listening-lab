//! Shaping the inside with the 3D model's gradient (README 5.5, "Shaping the inside").

use seatlock_sim::builtin;
use seatlock_sim::shape::{apply, shape_inside, ShapeRequest, Shaper};
use seatlock_sim::{geometry_report, simulate, SimulateRequest};

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// `sealed-test` made 0.5 m tall, so its first standing wave (about 355 Hz, top to bottom)
/// is well inside the band.
fn tall_box() -> seatlock_core::DesignFile {
    let mut design = example("sealed-test");
    design
        .speaker
        .enclosure
        .outer
        .fields
        .insert("height_m".into(), serde_json::json!(0.5));
    design
}

#[test]
fn the_objective_gradient_matches_finite_differences_through_the_driver() {
    let request = ShapeRequest {
        dx_m: 0.02,
        band_hz: [150.0, 1200.0],
        ..ShapeRequest::default()
    };
    let mut shaper = Shaper::new(&tall_box(), &request, &builtin()).unwrap();
    let n = shaper.variables();
    assert!(n > 100, "{n} cells to shape");
    for state in [1.0, 0.6] {
        let density = vec![state; n];
        let here = shaper.evaluate(&density, true);
        let largest = here.gradient.iter().map(|g| g.abs()).fold(0.0, f64::max);
        assert!(largest > 0.0);
        let mut worst = 0.0_f64;
        for pick in [n / 7, n / 3, n / 2, (2 * n) / 3, n - 5] {
            // At 1 the step goes inward only (second order); inside the range it is central.
            let h = 0.005;
            let moved = |shaper: &mut Shaper, by: f64| {
                let mut d = density.clone();
                d[pick] += by;
                shaper.evaluate(&d, false).objective
            };
            let fd = if state >= 1.0 {
                (3.0 * here.objective - 4.0 * moved(&mut shaper, -h) + moved(&mut shaper, -2.0 * h))
                    / (2.0 * h)
            } else {
                (moved(&mut shaper, h) - moved(&mut shaper, -h)) / (2.0 * h)
            };
            let error = (here.gradient[pick] - fd).abs() / largest;
            eprintln!(
                "density {state}, cell {pick}: gradient {:.5e}, finite difference {fd:.5e}",
                here.gradient[pick]
            );
            worst = worst.max(error);
            assert!(
                (here.gradient[pick] - fd).abs() <= 0.02 * fd.abs() + 1e-4 * largest,
                "cell {pick}: {} against {fd}",
                here.gradient[pick]
            );
        }
        eprintln!(
            "density {state}: worst error {:.4} % of the largest gradient",
            worst * 100.0
        );
    }
}

fn shape_and_report(dx: f64) -> seatlock_sim::shape::ShapeResult {
    let request = ShapeRequest {
        dx_m: dx,
        band_hz: [150.0, 1200.0],
        ..ShapeRequest::default()
    };
    let result = shape_inside(&tall_box(), &request, &builtin(), &mut |_, _| true).unwrap();
    eprintln!(
        "cell {:.0} mm: uneven {:.3} dB plain, {:.3} dB shaped (part-air cells), {:.3} dB as \
         printable plastic run from scratch; plastic {:.3} L of {:.3} L; {} runs of {} steps",
        dx * 1000.0,
        result.uneven_before_db,
        result.uneven_shaped_db,
        result.uneven_after_db,
        result.plastic_m3 * 1000.0,
        result.region_m3 * 1000.0,
        result.runs,
        result.steps_per_run
    );
    eprintln!("history {:.3?}", result.history_db);
    for ((f, a), b) in result
        .frequencies_hz
        .iter()
        .zip(&result.level_before_db)
        .zip(&result.level_after_db)
    {
        eprintln!("  {f:7.1} Hz  {a:+6.2} dB -> {b:+6.2} dB");
    }
    result
}

#[test]
fn shaping_the_inside_of_a_tall_box_evens_out_its_standing_wave() {
    let result = shape_and_report(0.02);
    assert!(result.plastic_m3 > 0.0);
    assert!(result.plastic_m3 <= 0.15 * result.region_m3 * 1.3);
    // The printable shape, run again from scratch, is at least a tenth more even.
    assert!(
        result.uneven_after_db < 0.9 * result.uneven_before_db,
        "{} dB against {} dB",
        result.uneven_after_db,
        result.uneven_before_db
    );

    // As a design: the plastic is a `voxels` solid. The 3D model of that design is the
    // shaped cabinet (the same unevenness with nothing left to shape), the quick model has
    // that much less air, and the print holds that much more plastic.
    let plain = tall_box();
    let shaped = apply(&plain, &result);
    assert_eq!(shaped.speaker.enclosure.solids.len(), 1);
    let again = apply(&shaped, &result);
    assert_eq!(
        again.speaker.enclosure.solids.len(),
        1,
        "shaping again replaces"
    );
    let request = ShapeRequest {
        dx_m: 0.02,
        band_hz: [150.0, 1200.0],
        ..ShapeRequest::default()
    };
    let mut check = Shaper::new(&shaped, &request, &builtin()).unwrap();
    let n = check.variables();
    let rerun = check.evaluate(&vec![1.0; n], false).objective.sqrt();
    assert!(
        (rerun - result.uneven_after_db).abs() < 1e-6,
        "{rerun} dB against {} dB",
        result.uneven_after_db
    );
    let air = |design: &seatlock_core::DesignFile| {
        simulate(design, &SimulateRequest::default(), &builtin())
            .interior_volume_m3
            .unwrap()
    };
    let lost = air(&plain) - air(&shaped);
    eprintln!(
        "air lost {:.4} L, plastic {:.4} L (gap {:.4} L, {:.2} % of cells)",
        lost * 1e3,
        result.plastic_m3 * 1e3,
        (lost - result.plastic_m3) * 1e3,
        100.0 * (lost - result.plastic_m3) / result.plastic_m3
    );
    // `result.plastic_m3` is the count of air-region FDTD cells. The voxels element also
    // pads wall cells to close the grid/wall gap (STATUS 2026-11-12), so air lost is a
    // little larger. Relative gap grows when plastic lines more walls (Actions runners
    // take a different finishing path under float rounding: ~1.21 L cells / ~8 % gap vs
    // ~0.51 L / ~2 % here). Exact agreement is the geometry solid ↔ air checks below.
    // Bound: cells do not exceed air lost, and the wall-gap fill is less than half a cell
    // layer on the plastic (worst case every cell against a wall).
    assert!(
        lost + 1e-12 >= result.plastic_m3,
        "air lost {lost} less than cell plastic {}",
        result.plastic_m3
    );
    let gap = lost - result.plastic_m3;
    assert!(
        gap < 0.5 * result.plastic_m3,
        "wall-gap fill {gap} exceeds half the cell plastic {}",
        result.plastic_m3
    );
    let before = geometry_report(&plain).unwrap();
    let after = geometry_report(&shaped).unwrap();
    assert!(after.can_export_print, "{:?}", after.warnings);
    assert!((after.solid_volume_m3 - before.solid_volume_m3 - lost).abs() < 1e-9);
    assert!((before.interior_volume_m3 - after.interior_volume_m3 - lost).abs() < 1e-9);
    // The 5 mm cells of CHK-GEO-1 see the same air.
    assert!(
        (after.voxel_air_volume_m3 - after.interior_volume_m3).abs()
            < 0.02 * after.interior_volume_m3
    );
}

#[test]
fn placed_plastic_is_written_and_read_back() {
    use seatlock_sim::solids::{decode, encode};
    let cells = [false, false, true, true, true, false, true];
    assert_eq!(encode(&cells), "2 3 1 1");
    assert_eq!(decode("2 3 1 1", 7).unwrap(), cells);
    assert_eq!(encode(&[true, false]), "0 1 1");
    assert_eq!(decode("0 1 1", 2).unwrap(), [true, false]);
    assert!(decode("2 3", 7).is_err());
    assert!(decode("9", 7).is_err());
}

#[test]
#[ignore = "minutes: the same at 10 mm cells; last result in STATUS"]
fn shaping_the_inside_at_10_mm() {
    let result = shape_and_report(0.01);
    // WRITE_EXAMPLE=1 writes the result as the example design `shaped-box`.
    if std::env::var("WRITE_EXAMPLE").is_ok() {
        let mut design = apply(&tall_box(), &result);
        design.name = "shaped-box".into();
        let path = format!(
            "{}/../../../designs/examples/shaped-box.seatlock.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(path, serde_json::to_string_pretty(&design).unwrap() + "\n").unwrap();
    }
    assert!(result.uneven_after_db < 0.9 * result.uneven_before_db);
}
