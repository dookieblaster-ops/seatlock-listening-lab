//! The `exterior` radiation model inside `simulate` (README section 5.6).

use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::{json, Value};

fn sealed_test() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(value: &Value, radiation: &str, freqs: &[f64]) -> SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value.clone()).unwrap();
    simulate(
        &design,
        &SimulateRequest {
            radiation: radiation.into(),
            frequencies_hz: Some(freqs.to_vec()),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

const FREQS: [f64; 9] = [
    30.0, 100.0, 300.0, 600.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0,
];

fn moved_to(surface: &str) -> Value {
    let mut value = sealed_test();
    value["speaker"]["enclosure"]["mounts"][0]["surface"] = json!(surface);
    if surface == "left" || surface == "top" {
        value["speaker"]["enclosure"]["mounts"][0]["at_m"] = json!([0.09, 0.08]);
    }
    value
}

#[test]
fn a_front_driver_agrees_with_the_baffle_model_and_hands_over_above_the_grid_limit() {
    let baffle = run(&sealed_test(), "baffle", &FREQS);
    let exterior = run(&sealed_test(), "exterior", &FREQS);
    assert_eq!(exterior.fidelity.radiation, "exterior");
    for (bin, freq) in FREQS.iter().enumerate() {
        let apart = exterior.summed_spl_db[bin] - baffle.summed_spl_db[bin];
        if *freq <= 3432.0 {
            // Two unrelated models of a small box with its driver in front: within 1.5 dB.
            assert!(apart.abs() < 1.5, "{freq} Hz: {apart:+.2} dB");
            assert!(apart != 0.0, "{freq} Hz was not taken from the 3D model");
        } else {
            // Above the 10 mm grid's limit the baffle model's value is used as it is.
            assert_eq!(apart, 0.0, "{freq} Hz");
        }
    }
    let label = exterior.fidelity.ignored.join(" ");
    assert!(label.contains("Full 3D outside the cabinet at 10 mm cells up to 3432 Hz"));
    assert!(label.contains("stair-stepped"));
}

#[test]
fn drivers_on_the_back_the_side_and_the_top_are_heard_around_the_cabinet() {
    let front = run(&sealed_test(), "exterior", &FREQS);
    for surface in ["back", "left", "top"] {
        let result = run(&moved_to(surface), "exterior", &FREQS);
        assert!(
            !result.summed_spl_db.is_empty(),
            "{surface}: {:?}",
            result.issues
        );
        // 30 and 100 Hz: the box is small against the wavelength, so the same level all
        // around, less the longer way for the driver on the back (at most 1.5 dB at 1 m).
        for (bin, freq) in FREQS.iter().enumerate().take(2) {
            let lost = front.summed_spl_db[bin] - result.summed_spl_db[bin];
            assert!(
                (-0.5..3.0).contains(&lost),
                "{surface} {freq} Hz: {lost:+.2} dB"
            );
        }
        // 3 kHz: the cabinet is in the way.
        let lost = front.summed_spl_db[7] - result.summed_spl_db[7];
        assert!(lost > 12.0, "{surface} 3 kHz: {lost:+.2} dB");
    }
    // A driver on the side of a 0.16 m wide box still reaches the listener at 1 kHz: within
    // 9 dB of the same driver in front. (The baffle model's edge-only estimate is far lower
    // there, which is why it sends such designs to this model.)
    let side = run(&moved_to("left"), "exterior", &FREQS);
    let lost = front.summed_spl_db[4] - side.summed_spl_db[4];
    assert!((3.0..9.0).contains(&lost), "left 1 kHz: {lost:+.2} dB");
    let rough = run(&moved_to("left"), "baffle", &FREQS);
    assert!(rough
        .issues
        .iter()
        .any(|issue| issue.message.contains("full 3D outside model")));
}

#[test]
fn the_room_takes_its_reflection_and_sound_power_from_the_3d_model() {
    let mut value = moved_to("back");
    value["speaker"]["room"] = json!({
        "kind": "simple", "wall_behind_m": 0.5, "wall_reflection": 0.9,
        "volume_m3": 50.0, "rt60_s": 0.4
    });
    let result = run(&value, "exterior", &FREQS);
    let in_room = result.in_room.as_ref().expect("an in-room response");
    let label = result.fidelity.ignored.join(" ");
    assert!(label.contains("14 directions"), "{label}");
    // 100 Hz: a small box radiates the same all around, so the 3D model's sound power is that
    // of the baffle model, which is a simple source's there.
    let simple = run(&value, "baffle", &FREQS);
    let apart =
        result.sound_power_db.as_ref().unwrap()[1] - simple.sound_power_db.as_ref().unwrap()[1];
    assert!(apart.abs() < 0.5, "{apart:+.2} dB");
    // The listener is 1.18 m from a driver on the back, and the index is taken at 1 m, so it
    // reads a little under 0 dB: between -3 and 0.
    let index = result.directivity_index_db.as_ref().unwrap();
    assert!((-3.0..0.0).contains(&index[1]), "{} dB", index[1]);
    // 3 kHz, driver on the back: the listener is in the cabinet's shadow, so far less arrives
    // directly than the average over all directions, and the wall behind returns more than
    // the direct sound.
    assert!(index[7] < -6.0, "{} dB", index[7]);
    assert!(in_room.reflection_spl_db[7] > result.summed_spl_db[7] + 6.0);
    assert!(in_room.spl_db[7] > result.summed_spl_db[7] + 6.0);
}

#[test]
fn shapes_that_are_not_boxes_run_in_the_3d_model() {
    for name in ["hex-prism", "pvc-tube", "sloped-baffle", "rounded-box"] {
        let path = format!(
            "{}/../../../designs/examples/{name}.seatlock.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let baffle = run(&value, "baffle", &FREQS);
        let exterior = run(&value, "exterior", &FREQS);
        assert!(
            !exterior
                .issues
                .iter()
                .any(|issue| issue.severity == seatlock_core::issue::Severity::Error),
            "{name}: {:?}",
            exterior.issues
        );
        // The driver is in front on all four: within 3 dB of the baffle model up to 1 kHz.
        for (bin, freq) in FREQS.iter().enumerate().take(5) {
            let apart = exterior.summed_spl_db[bin] - baffle.summed_spl_db[bin];
            assert!(apart.abs() < 3.0, "{name} {freq} Hz: {apart:+.2} dB");
            assert!(apart != 0.0, "{name} {freq} Hz");
        }
    }
}
