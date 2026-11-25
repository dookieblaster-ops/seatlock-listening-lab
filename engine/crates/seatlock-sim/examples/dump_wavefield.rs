use seatlock_sim::{builtin, wave_field, WaveFieldRequest};
use std::fs;
use std::path::PathBuf;

fn load_example(name: &str) -> seatlock_core::DesignFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../designs/examples")
        .join(format!("{name}.seatlock.json"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "/opt/cursor/artifacts".into()));
    fs::create_dir_all(&out).ok();
    let registry = builtin();

    // ~first height mode of sealed-test (~734 Hz) so antinodes read clearly.
    let inside = wave_field(
        &load_example("sealed-test"),
        &WaveFieldRequest {
            domain: "inside".into(),
            frequency_hz: Some(750.0),
            dx_m: Some(0.012),
            pulse: true,
        },
        &registry,
        &mut |_, _| true,
    )
    .expect("inside");
    fs::write(out.join("wave-inside.json"), serde_json::to_string(&inside).unwrap()).unwrap();
    eprintln!("inside cells {} @ {} Hz", inside.field.cells.len(), inside.frequency_hz);

    let outside = wave_field(
        &load_example("sealed-test"),
        &WaveFieldRequest {
            domain: "outside".into(),
            frequency_hz: Some(800.0),
            dx_m: Some(0.022),
            pulse: false,
        },
        &registry,
        &mut |_, _| true,
    )
    .expect("outside");
    fs::write(out.join("wave-outside.json"), serde_json::to_string(&outside).unwrap()).unwrap();
    eprintln!("outside cells {} @ {} Hz", outside.field.cells.len(), outside.frequency_hz);

    let mut design = load_example("sealed-test");
    design.speaker.room = Some(
        serde_json::from_value(serde_json::json!({
            "kind": "simple",
            "wall_behind_m": 0.5,
            "wall_reflection": 0.9,
            "volume_m3": 40.0,
            "rt60_s": 0.4
        }))
        .unwrap(),
    );
    // First axial room modes around 40–70 Hz; 90 Hz shows clearer structure.
    let room = wave_field(
        &design,
        &WaveFieldRequest {
            domain: "room".into(),
            frequency_hz: Some(90.0),
            dx_m: Some(0.2),
            pulse: true,
        },
        &registry,
        &mut |_, _| true,
    )
    .expect("room");
    fs::write(out.join("wave-room.json"), serde_json::to_string(&room).unwrap()).unwrap();
    eprintln!("room cells {} @ {} Hz", room.field.cells.len(), room.frequency_hz);
}
