//! The air's losses through the simulator (README sections 5.4 and 5.5).

use seatlock_sim::{builtin, simulate, SimulateRequest};

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(
    design: &seatlock_core::DesignFile,
    enclosure: &str,
    losses: bool,
) -> seatlock_sim::SimulateResult {
    simulate(
        design,
        &SimulateRequest {
            enclosure: enclosure.into(),
            radiation: "baffle".into(),
            losses,
            frequencies_hz: Some(seatlock_core::grid::log_grid(20.0, 3000.0, 96.0)),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

#[test]
fn a_folded_lines_dips_are_shallower_with_wall_losses_and_its_bass_is_the_same() {
    let design = example("folded-line");
    let (dry, wet) = (
        run(&design, "waveguide", false),
        run(&design, "waveguide", true),
    );
    let at = |hz: f64| dry.frequencies_hz.iter().position(|f| *f >= hz).unwrap();
    // Where the line is short against the wavelength the loss changes almost nothing.
    for hz in [30.0, 50.0, 80.0] {
        let i = at(hz);
        assert!(
            (dry.summed_spl_db[i] - wet.summed_spl_db[i]).abs() < 0.15,
            "{hz} Hz"
        );
    }
    // The deepest dip between 150 and 1500 Hz, a cancellation along the line, fills in.
    let (low, high) = (at(150.0), at(1500.0));
    let deepest = |r: &seatlock_sim::SimulateResult| {
        r.summed_spl_db[low..high]
            .iter()
            .cloned()
            .fold(f64::MAX, f64::min)
    };
    eprintln!(
        "deepest dip: {:.2} dB without losses, {:.2} dB with",
        deepest(&dry),
        deepest(&wet)
    );
    assert!(deepest(&wet) > deepest(&dry) + 0.2);
    assert!(wet
        .fidelity
        .ignored
        .iter()
        .any(|note| note.contains("lose sound at their walls")));
    assert!(!dry
        .fidelity
        .ignored
        .iter()
        .any(|note| note.contains("lose sound at their walls")));
}

#[test]
fn the_3d_model_takes_the_wall_loss_as_one_average_and_says_so() {
    let design = example("sealed-test");
    let (dry, wet) = (run(&design, "fdtd", false), run(&design, "fdtd", true));
    assert!(
        wet.issues
            .iter()
            .all(|i| i.severity != seatlock_core::Severity::Error),
        "{:?}",
        wet.issues
    );
    let note = wet
        .fidelity
        .ignored
        .iter()
        .find(|note| note.contains("loss at the walls"))
        .expect("the label names the loss");
    eprintln!("{note}");
    // sealed-test's air is 0.144 x 0.164 x 0.234 m: 2*(ab + bc + ca)/abc = 34.6 per meter.
    // The grid's cells are 10 mm, so its count is near that, not equal to it.
    let per_m: f64 = note
        .split('(')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .expect("the wall area per volume is in the note");
    assert!((per_m - 34.6).abs() < 0.15 * 34.6, "{per_m}");
    // The box's first standing wave, along its 0.234 m height, is near 733 Hz: its peak in
    // the impedance the cone sees is lower with the loss, and the bass is unchanged.
    let z = |r: &seatlock_sim::SimulateResult| {
        let report = r.fdtd.as_ref().unwrap();
        report
            .frequencies_hz
            .iter()
            .zip(report.z_ab.re.iter().zip(&report.z_ab.im))
            .filter(|(f, _)| **f > 650.0 && **f < 820.0)
            .map(|(_, (re, im))| (re * re + im * im).sqrt())
            .fold(0.0, f64::max)
    };
    eprintln!(
        "peak |Z_ab| near 733 Hz: {:.3e} without losses, {:.3e} with",
        z(&dry),
        z(&wet)
    );
    // (How much lower depends on how finely the frequencies sample the lossless peak, so
    // only the direction is asserted; the width itself is checked in seatlock-fdtd.)
    assert!(z(&wet) < z(&dry));
    let at = |hz: f64| dry.frequencies_hz.iter().position(|f| *f >= hz).unwrap();
    for hz in [40.0, 100.0, 200.0] {
        let i = at(hz);
        assert!(
            (dry.summed_spl_db[i] - wet.summed_spl_db[i]).abs() < 0.05,
            "{hz} Hz"
        );
    }
}
