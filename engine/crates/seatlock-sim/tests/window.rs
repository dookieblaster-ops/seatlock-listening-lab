//! The listening window and the wider evenness score (README section 11.1).

use seatlock_sim::{builtin, simulate, SimulateRequest};
use std::time::Instant;

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(design: &seatlock_core::DesignFile, window: bool) -> seatlock_sim::SimulateResult {
    simulate(
        design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: "baffle".into(),
            listening_window: window,
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

#[test]
fn the_window_is_the_axis_where_sound_spreads_evenly_and_below_it_where_it_beams() {
    let design = example("sealed-test");
    let plain = run(&design, false);
    assert!(plain.listening_window_db.is_none());
    assert_eq!(plain.flatness_db, plain.axis_flatness_db);
    let started = Instant::now();
    let wide = run(&design, true);
    eprintln!(
        "with the window: {:.0} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let window = wide.listening_window_db.as_ref().unwrap();
    let at = |hz: f64| wide.frequencies_hz.iter().position(|f| *f >= hz).unwrap();
    // Bass spreads evenly: every direction of the window hears the level straight ahead.
    for hz in [50.0, 100.0, 200.0] {
        let i = at(hz);
        assert!((window[i] - wide.summed_spl_db[i]).abs() < 0.3, "{hz} Hz");
    }
    // A 104 mm cone beams at 10 kHz: off to the sides is quieter, so the window is below the
    // axis, and by more at 10 kHz than at 3 kHz.
    let (mid, high) = (at(3000.0), at(10_000.0));
    let drop = |i: usize| wide.summed_spl_db[i] - window[i];
    eprintln!(
        "window below axis: {:.2} dB at 3 kHz, {:.2} dB at 10 kHz",
        drop(mid),
        drop(high)
    );
    assert!(drop(high) > 1.0 && drop(high) > drop(mid) + 0.5);
    // The score takes the worse of the two.
    let w = wide.window_flatness_db.unwrap();
    eprintln!(
        "evenness: axis {:.2} dB, window {w:.2} dB",
        wide.axis_flatness_db
    );
    assert_eq!(wide.axis_flatness_db, plain.flatness_db);
    assert_eq!(wide.flatness_db, w.max(wide.axis_flatness_db));
}

#[test]
fn a_rigid_piston_far_away_gives_the_closed_form_window() {
    // CHK-RAD-1's piston on a large surface: the window's level against the axis is the
    // energy mean of the piston's directivity over the nine directions. The surface's far
    // edge still sends a little back, as a ripple along frequency of about a tenth of a
    // decibel, so the comparison is the mean over a band of frequencies.
    let mut design = example("sealed-test");
    let outer = &mut design.speaker.enclosure.outer.fields;
    for (key, value) in [("width_m", 40.0), ("height_m", 40.0)] {
        outer.insert(key.into(), serde_json::json!(value));
    }
    design.speaker.enclosure.mounts[0].at_m = [20.0, 20.0];
    design.speaker.listening.distance_m = 100.0;
    let freqs: Vec<f64> = (0..41).map(|i| 3000.0 + 15.0 * i as f64).collect();
    let result = simulate(
        &design,
        &SimulateRequest {
            radiation: "baffle".into(),
            listening_window: true,
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let a = (0.0085f64 / std::f64::consts::PI).sqrt();
    // J1 by its series, enough terms for x up to 3.
    let j1 = |x: f64| {
        let mut term = x / 2.0;
        let mut sum = term;
        for m in 1..30 {
            term *= -(x * x / 4.0) / (m as f64 * (m as f64 + 1.0));
            sum += term;
        }
        sum
    };
    let window = result.listening_window_db.unwrap();
    let (mut got, mut expected) = (0.0, 0.0);
    for (i, hz) in freqs.iter().enumerate() {
        let ka = 2.0 * std::f64::consts::PI * hz / seatlock_core::constants::C0 * a;
        let d = |deg: f64| {
            let x = ka * deg.to_radians().sin();
            2.0 * j1(x) / x
        };
        let mean =
            (1.0 + 4.0 * d(10.0).powi(2) + 2.0 * d(20.0).powi(2) + 2.0 * d(30.0).powi(2)) / 9.0;
        expected += 10.0 * mean.log10() / freqs.len() as f64;
        got += (window[i] - result.summed_spl_db[i]) / freqs.len() as f64;
    }
    eprintln!("window against axis: {got:.3} dB, closed form {expected:.3} dB");
    assert!((got - expected).abs() < 0.05);
}

#[test]
fn sources_at_one_point_sound_the_same_from_every_direction_of_the_window() {
    // The `point` model has no directions, so its window is its axis.
    let result = simulate(
        &example("sealed-test"),
        &SimulateRequest {
            listening_window: true,
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let window = result.listening_window_db.unwrap();
    for (w, axis) in window.iter().zip(&result.summed_spl_db) {
        assert!((w - axis).abs() < 0.02, "{w} against {axis}");
    }
}
