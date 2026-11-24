//! 3D wave-field capture for the page views (inside / outside / room).

use seatlock_sim::{builtin, wave_field, WaveFieldRequest};

fn load_example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

#[test]
fn inside_standing_wave_has_air_cells_and_a_peak() {
    let design = load_example("sealed-test");
    let report = wave_field(
        &design,
        &WaveFieldRequest {
            domain: "inside".into(),
            frequency_hz: Some(400.0),
            dx_m: Some(0.015),
            pulse: true,
        },
        &builtin(),
        &mut |_, _| true,
    )
    .expect("inside field");
    assert_eq!(report.domain, "inside");
    assert!(report.field.cells.len() > 20, "expected air cells");
    assert_eq!(report.field.standing.len(), 1);
    assert!(
        report.field.standing[0].max_abs > 0.0,
        "standing wave should be non-zero"
    );
    let pulse = report.field.pulse.expect("pulse frames");
    assert!(pulse.frames.len() >= 4, "expected several pulse frames");
}

#[test]
fn outside_bem_field_has_probe_points() {
    let design = load_example("sealed-test");
    let report = wave_field(
        &design,
        &WaveFieldRequest {
            domain: "outside".into(),
            frequency_hz: Some(400.0),
            dx_m: Some(0.025),
            pulse: false,
        },
        &builtin(),
        &mut |_, _| true,
    )
    .expect("outside field");
    assert_eq!(report.domain, "outside");
    assert!(report.field.cells.len() > 50, "expected exterior probes");
    assert!(report.field.standing[0].max_abs > 0.0);
}

#[test]
fn room_field_needs_the_room_and_then_fills_the_grid() {
    let mut design = load_example("sealed-test");
    design.speaker.room = Some(
        serde_json::from_value(serde_json::json!({
            "kind": "simple",
            "wall_behind_m": 0.5,
            "wall_reflection": 0.9,
            "volume_m3": 40.0,
            "rt60_s": 0.4
        }))
        .expect("room part"),
    );
    let report = wave_field(
        &design,
        &WaveFieldRequest {
            domain: "room".into(),
            frequency_hz: Some(60.0),
            dx_m: Some(0.25),
            pulse: true,
        },
        &builtin(),
        &mut |_, _| true,
    )
    .expect("room field");
    assert_eq!(report.domain, "room");
    assert!(report.field.cells.len() > 20);
    assert!(report.field.standing[0].max_abs > 0.0);
    assert_eq!(report.marker_labels.len(), 2);
}
