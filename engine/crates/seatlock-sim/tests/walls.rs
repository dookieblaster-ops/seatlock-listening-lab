//! Vibrating cabinet walls in the simulator (README section 5.11).

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_sim::walls::cabinet_walls;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::{json, Value};
use std::f64::consts::PI;
use std::time::Instant;

fn example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn design(value: &Value) -> seatlock_core::DesignFile {
    serde_json::from_value(value.clone()).unwrap()
}

fn level_at(value: &Value, request: &SimulateRequest, freq: f64) -> f64 {
    let request = SimulateRequest {
        frequencies_hz: Some(vec![freq]),
        ..request.clone()
    };
    let result = simulate(&design(value), &request, &builtin());
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    result.summed_spl_db[0]
}

/// Below the walls' resonances they are springs: the box acts larger by their compliance, and
/// they give back, in opposite phase, the share of the cone's flow that went into them. With
/// every source at one point (`point` radiation) that is exact:
/// `SPL(walls) = SPL(rigid box larger by rho*c^2*C_w) + 20*log10(1 - C_w/(C_air + C_w))`.
#[test]
fn below_their_resonance_the_walls_are_a_spring_that_gives_back_its_share() {
    let value = example("sealed-test");
    let file = design(&value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let walls = cabinet_walls(&file, outer.as_ref()).unwrap();
    assert_eq!(walls.walls.len(), 6);
    let c_walls = (walls.admittance(0.5) / Complex64::new(0.0, 2.0 * PI * 0.5)).re;
    let rigid = SimulateRequest::default();
    let with_walls = SimulateRequest {
        walls: true,
        ..SimulateRequest::default()
    };
    let net = simulate(&file, &rigid, &registry)
        .interior_volume_m3
        .unwrap();
    let c_air = net / (RHO0 * C0 * C0);
    // The same box, deeper by the walls' compliance, rigid.
    let (inside_w, inside_h) = (0.16 - 0.016, 0.25 - 0.016);
    let mut larger = value.clone();
    let depth = larger["speaker"]["enclosure"]["outer"]["depth_m"]
        .as_f64()
        .unwrap();
    larger["speaker"]["enclosure"]["outer"]["depth_m"] =
        json!(depth + c_walls * RHO0 * C0 * C0 / (inside_w * inside_h));
    let give_back = 20.0 * (1.0 - c_walls / (c_air + c_walls)).log10();
    println!(
        "walls' compliance {:.3} % of the air's, give-back {give_back:.4} dB",
        100.0 * c_walls / c_air
    );
    // At 20 Hz the walls' own mass changes their compliance by (20/284)^2, about 0.5 %; at
    // 40 Hz by 2 %, which moves the level 0.012 dB, beyond this check's 0.01 dB.
    for freq in [10.0, 20.0] {
        let got = level_at(&value, &with_walls, freq);
        let want = level_at(&larger, &rigid, freq) + give_back;
        assert!((got - want).abs() < 0.01, "{freq} Hz: {got} against {want}");
    }
}

#[test]
fn each_wall_build_is_heard_and_says_what_it_assumed() {
    let base = example("sealed-test");
    let freqs: Vec<f64> = seatlock_core::grid::log_grid(100.0, 3000.0, 48.0);
    let request = |walls: bool| SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        listening_window: true,
        losses: true,
        walls,
        frequencies_hz: Some(freqs.clone()),
        ..SimulateRequest::default()
    };
    let started = Instant::now();
    let rigid = simulate(&design(&base), &request(false), &builtin());
    let rigid_ms = started.elapsed().as_secs_f64() * 1000.0;
    let builds: Vec<(&str, Value)> = vec![
        ("solid PLA", json!({ "kind": "solid" })),
        (
            "sand",
            json!({ "kind": "filled", "skin_m": 0.0016, "fill": "quikrete-play-sand" }),
        ),
        (
            "sand, 3 mm skins",
            json!({ "kind": "filled", "skin_m": 0.003, "fill": "quikrete-play-sand" }),
        ),
        (
            "plaster",
            json!({ "kind": "filled", "skin_m": 0.0016, "fill": "usg-hydrocal-white" }),
        ),
        (
            "varioShore TPU",
            json!({ "kind": "two_material", "skin_m": 0.0016, "core": "varioshore_tpu" }),
        ),
        (
            "LW-PLA",
            json!({ "kind": "two_material", "skin_m": 0.0016, "core": "lw_pla" }),
        ),
        (
            "Roamr TPU Air",
            json!({ "kind": "two_material", "skin_m": 0.0016, "core": "roamr_tpu_air_85a" }),
        ),
    ];
    let mut curves: Vec<Vec<f64>> = Vec::new();
    for (name, wall) in &builds {
        let mut value = base.clone();
        value["build"]["wall"] = wall.clone();
        let file = design(&value);
        let started = Instant::now();
        let result = simulate(&file, &request(true), &builtin());
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.code == "wall_build_not_heard"),
            "{name}"
        );
        assert!(
            result
                .fidelity
                .ignored
                .iter()
                .any(|l| l.starts_with("Walls vibrate")),
            "{name}: {:?}",
            result.fidelity.ignored
        );
        let registry = builtin();
        let outer = registry
            .create_outer(
                &file.speaker.enclosure.outer.kind,
                &file.speaker.enclosure.outer.fields,
            )
            .unwrap();
        let first = cabinet_walls(&file, outer.as_ref())
            .unwrap()
            .first_resonance_hz();
        let differences: Vec<f64> = result
            .summed_spl_db
            .iter()
            .zip(&rigid.summed_spl_db)
            .map(|(a, b)| a - b)
            .collect();
        let worst = differences.iter().fold(0.0f64, |w, d| w.max(d.abs()));
        let at = freqs[differences
            .iter()
            .position(|d| d.abs() == worst)
            .unwrap_or(0)];
        let rms =
            (differences.iter().map(|d| d * d).sum::<f64>() / differences.len() as f64).sqrt();
        println!(
            "{name:>15}: lowest wall resonance {first:6.0} Hz, against rigid walls largest \
             {worst:5.2} dB at {at:.0} Hz, RMS {rms:5.2} dB (100 Hz to 3 kHz), evenness {:.2} dB against \
             {:.2} dB, {ms:.0} ms against {rigid_ms:.0} ms",
            result.flatness_db, rigid.flatness_db
        );
        assert!(worst > 0.05, "{name} is not heard: {worst} dB");
        curves.push(result.summed_spl_db.clone());
    }
    // Every build sounds different from every other.
    for a in 0..curves.len() {
        for b in a + 1..curves.len() {
            let worst = curves[a]
                .iter()
                .zip(&curves[b])
                .fold(0.0f64, |w, (x, y)| w.max((x - y).abs()));
            assert!(
                worst > 0.05,
                "{} and {} sound the same ({worst} dB)",
                builds[a].0,
                builds[b].0
            );
        }
    }
    // The engine's default keeps rigid walls, so the numbered checks do not move.
    let mut value = base.clone();
    value["build"]["wall"] = builds[1].1.clone();
    let default = simulate(&design(&value), &request(false), &builtin());
    assert_eq!(default.summed_spl_db, rigid.summed_spl_db);
    assert!(default
        .issues
        .iter()
        .any(|i| i.code == "wall_build_not_heard"));
}

#[test]
fn a_cabinet_whose_walls_are_not_modeled_says_so() {
    let mut value = example("two-chamber");
    value["build"]["wall"] =
        json!({ "kind": "filled", "skin_m": 0.0016, "fill": "quikrete-play-sand" });
    let request = SimulateRequest {
        walls: true,
        ..SimulateRequest::default()
    };
    let result = simulate(&design(&value), &request, &builtin());
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|l| l.starts_with("Walls are rigid")));
    assert!(result
        .issues
        .iter()
        .any(|i| i.code == "wall_build_not_heard"));
}
