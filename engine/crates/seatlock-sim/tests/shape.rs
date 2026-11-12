//! Shaping the inside with the 3D model's gradient (README 5.5, "Shaping the inside").

use seatlock_sim::builtin;
use seatlock_sim::shape::{shape_inside, ShapeRequest, Shaper};

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
}

#[test]
#[ignore = "minutes: the same at 10 mm cells; last result in STATUS"]
fn shaping_the_inside_at_10_mm() {
    let result = shape_and_report(0.01);
    assert!(result.uneven_after_db < 0.9 * result.uneven_before_db);
}
