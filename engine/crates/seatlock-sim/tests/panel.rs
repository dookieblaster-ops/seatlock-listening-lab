//! The `dml_panel` radiator in `simulate`, and the `rear-panel` half of CHK-E2E-3 (README
//! sections 5.9 and 12).

use seatlock_core::constants::{C0, RHO0};
use seatlock_core::issue::Severity;
use seatlock_sim::{builtin, design_cost, simulate, SimulateRequest, SimulateResult};
use serde_json::{json, Value};
use std::f64::consts::PI;

fn example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(value: &Value, request: SimulateRequest) -> SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value.clone()).unwrap();
    simulate(&design, &request, &builtin())
}

fn log_grid(from_hz: f64, to_hz: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|index| from_hz * (to_hz / from_hz).powf(index as f64 / (count - 1) as f64))
        .collect()
}

#[test]
fn chk_e2e_3_rear_panel_runs_in_3d_with_the_room_and_the_room_changes_what_is_heard() {
    let value = example("rear-panel");
    let freqs = vec![100.0, 200.0, 400.0, 800.0, 1500.0, 3000.0];
    let result = run(
        &value,
        SimulateRequest {
            radiation: "exterior".into(),
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
    );
    assert!(
        !result
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.fidelity.radiation, "exterior");
    assert_eq!(result.fidelity.room, "simple");
    assert_eq!(result.radiators.len(), 2);
    assert_eq!(result.radiators[1].kind, "dml_panel");
    let label = result.fidelity.ignored.join(" ");
    for part in [
        "resonances up to",
        "Full 3D outside the cabinet",
        "The room is an estimate",
    ] {
        assert!(label.contains(part), "{part}: {label}");
    }
    let in_room = result.in_room.as_ref().expect("an in-room response");
    for (bin, freq) in freqs.iter().enumerate() {
        // The in-room response differs from the direct response at every frequency.
        let apart = in_room.spl_db[bin] - result.summed_spl_db[bin];
        assert!(apart > 1.0, "{freq} Hz: {apart:+.2} dB");
        // The panel faces the wall: what it sends the listener directly is far below what it
        // sends to the wall behind, which comes back as the reflection.
        if *freq >= 400.0 {
            let panel_direct = result.radiators[1].spl_db[bin];
            assert!(
                panel_direct < result.radiators[0].spl_db[bin] - 10.0,
                "{freq} Hz: panel {panel_direct:.1} dB"
            );
        }
    }
}

/// A design with only the test panel, on the front of a box, and nothing else.
fn panel_alone() -> Value {
    let mut value = example("rear-panel");
    value["speaker"]["enclosure"]["outer"] = json!({
        "kind": "box", "width_m": 0.34, "height_m": 0.26, "depth_m": 0.2, "wall_m": 0.008
    });
    value["speaker"]["enclosure"]["mounts"] = json!([
        { "id": "m2", "surface": "front", "at_m": [0.17, 0.13], "rear_into": "outside", "recess_m": 0.0 }
    ]);
    let panel = value["speaker"]["radiators"][1].clone();
    value["speaker"]["radiators"] = json!([panel]);
    value["dsp"]["per_radiator"] = json!([
        { "blocks": [], "delay_s": 0.0, "gain_db": 0.0, "invert": false, "limiter": null }
    ]);
    value["speaker"].as_object_mut().unwrap().remove("room");
    value
}

#[test]
fn a_panel_radiates_about_what_a_point_driven_plate_should() {
    // Cremer's result for a large plate driven at a point, well below the frequency where
    // bending waves catch up with sound in air (12 kHz for this panel): radiated power
    // W = RHO0 * F^2 / (2*pi*C0*m^2), with m the plate's mass per area and F the force.
    // A finite panel adds its edges and its resonances, so the band average is held to 5 dB.
    let freqs = log_grid(700.0, 2800.0, 49);
    let result = run(
        &panel_alone(),
        SimulateRequest {
            radiation: "baffle".into(),
            sound_power: true,
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
    );
    let power_db = result.sound_power_db.as_ref().expect("sound power");
    let impedance = &result.radiators[0].impedance_ohm;
    let mass_per_area = 1240.0 * 0.003;
    let (mut got, mut expected) = (0.0, 0.0);
    for bin in 0..freqs.len() {
        got += 1e-12 * 10f64.powf(power_db[bin] / 10.0);
        // Above the magnet's own resonance the force on the panel is bl * I.
        let force = 3.0 * 2.83 / impedance[bin];
        expected += RHO0 * force * force / (2.0 * PI * C0 * mass_per_area * mass_per_area);
    }
    let apart = 10.0 * (got / expected).log10();
    println!("panel power over the band: {apart:+.2} dB against the point-driven plate");
    assert!(apart.abs() < 5.0, "{apart:+.2} dB");
    // And it is heard in front at a sensible level for 2.83 V: 65 to 85 dB at 1 m.
    let mean = result.summed_spl_db.iter().sum::<f64>() / freqs.len() as f64;
    println!("panel level in front, band mean: {mean:.1} dB");
    assert!((65.0..85.0).contains(&mean), "{mean:.1} dB");
}

#[test]
fn a_panel_that_does_not_fit_or_cannot_be_built_says_so() {
    // A 0.30 m wide panel on the 0.16 m wide sealed-test box: part of it is off the surface.
    let mut value = example("sealed-test");
    let mut panel = example("rear-panel")["speaker"]["radiators"][1].clone();
    panel["mount"] = json!("m1");
    value["speaker"]["radiators"] = json!([panel.clone()]);
    let result = run(&value, SimulateRequest::default());
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.message.contains("not on its mounting surface")));
    // An open-backed panel is not built yet, and is left out with the reason.
    panel["backing"] = json!("open");
    value["speaker"]["radiators"] = json!([panel]);
    let result = run(&value, SimulateRequest::default());
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.message.contains("open-backed panel is not built yet")));
    assert!(result.radiators.is_empty());
}

#[test]
fn the_cost_lists_the_exciter_and_the_panel() {
    let design: seatlock_core::DesignFile = serde_json::from_value(example("rear-panel")).unwrap();
    let cost = design_cost(&design);
    let exciter = cost
        .lines
        .iter()
        .find(|line| line.item.contains("DAEX25FHE-4") || line.item.contains("daex25fhe"))
        .expect("an exciter line");
    assert_eq!(exciter.quantity, 2.0);
    assert_eq!(exciter.unit_usd, Some(15.98));
    assert!((exciter.total_usd.unwrap() - 2.0 * 15.98).abs() < 1e-9);
    let panel = cost
        .lines
        .iter()
        .find(|line| line.item == "Panel, Test panel material A")
        .expect("a panel line");
    // Two panels of 0.30 x 0.20 x 0.003 m at 1240 kg/m^3: 0.4464 kg.
    assert!((panel.quantity - 0.4464).abs() < 1e-9, "{}", panel.quantity);
    assert_eq!(panel.unit, "kg");
}

#[test]
fn a_design_with_a_panel_still_gets_its_cabinet_its_sound_and_its_correction() {
    let value = example("rear-panel");
    let design: seatlock_core::DesignFile = serde_json::from_value(value).unwrap();
    let registry = builtin();

    // The cabinet: the woofer's hole is cut, nothing is cut for the panel, and the print file
    // is still offered, with a line that says the panel's frame is not in it.
    let report = seatlock_sim::geometry_report(&design).unwrap();
    assert_eq!(report.mounts.len(), 2);
    assert!(report.mounts[0].cutout_diameter_m > 0.05);
    assert_eq!(report.mounts[1].cutout_diameter_m, 0.0);
    assert!(report.can_export_print, "{:?}", report.warnings);
    let lines = report.warnings.join(" ");
    assert!(lines.contains("is not a driver"), "{lines}");
    assert!(!lines.contains("mounting size not published"), "{lines}");

    // Listening: one channel per sound source, each with a finite impulse that is not silent.
    let package = seatlock_sim::listen_package(&design, &registry, 48000.0, false, 4096);
    assert_eq!(package["channels"].as_array().unwrap().len(), 2);
    let impulses = package["impulses"].as_array().unwrap();
    assert_eq!(impulses.len(), 2);
    for impulse in impulses {
        let samples = impulse.as_array().unwrap();
        assert_eq!(samples.len(), 4096);
        let peak = samples
            .iter()
            .map(|sample| sample.as_f64().unwrap().abs())
            .fold(0.0, f64::max);
        assert!(peak.is_finite() && peak > 0.0, "{peak}");
    }

    // Correction: it runs, and what it returns still simulates without an error.
    let corrected = seatlock_sim::compare::apply_correction(&design, &registry);
    let result = simulate(
        &corrected,
        &SimulateRequest {
            radiation: "baffle".into(),
            frequencies_hz: Some(log_grid(30.0, 3000.0, 24)),
            ..SimulateRequest::default()
        },
        &registry,
    );
    assert!(
        !result
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.radiators.len(), 2);
}
