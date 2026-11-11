//! The `simple` room in `simulate` (README sections 5.10 and 11.1), and sources that face away
//! from the listener under the `baffle` model (section 5.6).

use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::{json, Value};
use std::f64::consts::PI;

fn sealed_test() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(value: Value, radiation: &str, space: &str) -> seatlock_sim::SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    simulate(
        &design,
        &SimulateRequest {
            radiation: radiation.into(),
            space: space.into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

fn with_room(mut value: Value, room: Value) -> Value {
    value["speaker"]["room"] = room;
    value
}

fn bin(result: &seatlock_sim::SimulateResult, freq_hz: f64) -> usize {
    result
        .frequencies_hz
        .iter()
        .position(|f| *f >= freq_hz)
        .unwrap()
}

#[test]
fn a_design_without_a_room_has_no_in_room_response() {
    let result = run(sealed_test(), "baffle", "half");
    assert!(result.in_room.is_none());
    assert!(result.sound_power_db.is_none());
    assert_eq!(result.fidelity.room, "free");
}

#[test]
fn the_simple_room_adds_the_wall_and_the_diffuse_sound_and_sets_the_score() {
    let room = json!({
        "kind": "simple", "wall_behind_m": 0.5, "wall_reflection": 0.9,
        "volume_m3": 50.0, "rt60_s": 0.4
    });
    let free = run(sealed_test(), "baffle", "half");
    let result = run(with_room(sealed_test(), room), "baffle", "half");
    assert_eq!(result.fidelity.room, "simple");
    let in_room = result.in_room.as_ref().expect("an in-room response");
    // The direct sound is unchanged; the in-room response differs from it.
    assert_eq!(result.summed_spl_db, free.summed_spl_db);
    let at_1k = bin(&result, 1000.0);
    assert!((in_room.spl_db[at_1k] - result.summed_spl_db[at_1k]).abs() > 0.5);
    // The diffuse level follows from the sound power: p^2 = 4 W rho c / A.
    let power_db = result.sound_power_db.as_ref().unwrap()[at_1k];
    let watts = 1e-12 * 10f64.powf(power_db / 10.0);
    let area = 0.161 * 50.0 / 0.4;
    let diffuse = 10.0 * (4.0 * watts * RHO0 * C0 / area / (P_REF * P_REF)).log10();
    assert!((in_room.diffuse_spl_db[at_1k] - diffuse).abs() < 1e-6);
    // In-room level is early sound plus diffuse sound, added as energy.
    let energy = 10f64.powf(in_room.early_spl_db[at_1k] / 10.0)
        + 10f64.powf(in_room.diffuse_spl_db[at_1k] / 10.0);
    assert!((in_room.spl_db[at_1k] - 10.0 * energy.log10()).abs() < 1e-6);
    // The room estimate says what it leaves out.
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|line| line.contains("The room is an estimate")));
    // A room changes the score: it is taken from the in-room response.
    assert!(
        (result.flatness_db - free.flatness_db).abs() > 0.01
            || (result.f6_hz - free.f6_hz).abs() > 0.01
    );
}

#[test]
fn sound_power_of_a_small_sealed_box_is_that_of_a_simple_source_at_low_frequency() {
    // 100 Hz: the 0.16 m wide baffle is far smaller than the wavelength, so the box radiates
    // like a simple source in full space, and on-axis level equals the mean (index 0 dB).
    let design: seatlock_core::DesignFile = serde_json::from_value(sealed_test()).unwrap();
    let result = simulate(
        &design,
        &SimulateRequest {
            radiation: "baffle".into(),
            sound_power: true,
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    assert!(result.in_room.is_none());
    let at = bin(&result, 100.0);
    let pressure = P_REF * 10f64.powf(result.summed_spl_db[at] / 20.0);
    let watts = pressure * pressure * 4.0 * PI / (RHO0 * C0);
    let expected = 10.0 * (watts / 1e-12).log10();
    let got = result.sound_power_db.as_ref().unwrap()[at];
    assert!(
        (got - expected).abs() < 0.3,
        "{got} dB against {expected} dB"
    );
    let index = result.directivity_index_db.as_ref().unwrap();
    assert!(index[at].abs() < 0.3, "{} dB", index[at]);
    // At 5 kHz the baffle sends the sound forward: several dB above the mean.
    assert!(index[bin(&result, 5000.0)] > 3.0);
}

#[test]
fn a_room_with_a_missing_number_is_left_out_with_a_warning() {
    let room = json!({ "kind": "simple", "wall_behind_m": 0.5, "wall_reflection": 0.9 });
    let result = run(with_room(sealed_test(), room), "baffle", "half");
    assert!(result.in_room.is_none());
    assert_eq!(result.fidelity.room, "free");
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.message.contains("without a room")));
    assert!(!result.summed_spl_db.is_empty());
}

#[test]
fn a_driver_on_the_back_reaches_the_listener_around_its_panel_at_low_frequency_only() {
    let mut back = sealed_test();
    back["speaker"]["enclosure"]["mounts"][0]["surface"] = json!("back");
    let front = run(sealed_test(), "baffle", "half");
    let result = run(back, "baffle", "half");
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|line| line.contains("bends around")));
    // 25 Hz: the same all around, so only the 0.18 m longer way to the listener is lost:
    // 20*log10(1.18) = 1.44 dB.
    let low = bin(&result, 25.0);
    let lost = front.summed_spl_db[low] - result.summed_spl_db[low];
    assert!((lost - 1.44).abs() < 0.2, "{lost} dB");
    // 6 to 12 kHz: the cabinet's back panel shadows the listener. On average at least 10 dB down.
    let (from, to) = (bin(&result, 6000.0), bin(&result, 12000.0));
    let mean = |levels: &[f64]| levels[from..to].iter().sum::<f64>() / (to - from) as f64;
    let drop = mean(&front.summed_spl_db) - mean(&result.summed_spl_db);
    assert!(drop > 10.0, "{drop} dB");
    // The quick point model cannot tell, and says so.
    let mut back = sealed_test();
    back["speaker"]["enclosure"]["mounts"][0]["surface"] = json!("back");
    let point = run(back, "point", "half");
    assert!(point
        .issues
        .iter()
        .any(|issue| issue.message.contains("does not face the listener")));
}
