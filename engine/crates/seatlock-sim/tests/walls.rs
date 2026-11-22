//! Vibrating cabinet walls in the simulator (README section 5.11).

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_sim::walls::{cabinet_walls, chamber_walls};
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
    // 40 Hz by 2 %, which moves the level 0.012 dB. Printed PLA's sourced loss factor
    // (0.075) also adds a small resistive part, so 20 Hz is held to 0.02 dB and 10 Hz to 0.01.
    for (freq, bound) in [(10.0, 0.01), (20.0, 0.02)] {
        let got = level_at(&value, &with_walls, freq);
        let want = level_at(&larger, &rigid, freq) + give_back;
        assert!(
            (got - want).abs() < bound,
            "{freq} Hz: {got} against {want} (bound {bound})"
        );
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

/// The low-frequency spring identity of the check above for any cabinet: with `point`
/// radiation, walls equal the rigid cabinet whose air is larger by the walls' compliance,
/// less the share the walls give back. `enlarge` makes the rigid cabinet larger by a volume.
fn walls_are_a_spring(value: &Value, enlarge: impl Fn(&mut Value, f64)) {
    let file = design(value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let walls = cabinet_walls(&file, outer.as_ref()).unwrap();
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
    let mut larger = value.clone();
    enlarge(&mut larger, c_walls * RHO0 * C0 * C0);
    let give_back = 20.0 * (1.0 - c_walls / (c_air + c_walls)).log10();
    let first = walls.first_resonance_hz();
    println!(
        "{}: {} walls, lowest resonance {first:.1} Hz, compliance {:.3} % of the air's, \
         give-back {give_back:.4} dB",
        file.name,
        walls.walls.len(),
        100.0 * c_walls / c_air
    );
    // Well below the walls' lowest resonance, where their own mass hardly counts: the
    // difference left is that mass, and it falls toward zero with the frequency.
    let off = |freq: f64| {
        level_at(value, &with_walls, freq) - level_at(&larger, &rigid, freq) - give_back
    };
    for freq in [10.0, 20.0].map(|f: f64| f.min(first / 30.0)) {
        let (here, lower) = (off(freq), off(freq / 2.0));
        println!("  {freq:.2} Hz: off by {here:.5} dB, at half that {lower:.5} dB");
        assert!(here.abs() < 0.01, "{freq} Hz: off by {here} dB");
        assert!(lower.abs() <= here.abs() + 1e-6);
    }
}

/// A PVC tube's two printed end caps vibrate; the pipe is held still.
#[test]
fn a_tubes_end_caps_are_its_walls() {
    let value = example("pvc-tube");
    let file = design(&value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let walls = cabinet_walls(&file, outer.as_ref()).unwrap();
    assert_eq!(walls.walls.len(), 2);
    assert!(walls
        .notes
        .iter()
        .any(|n| n.contains("curved pipe is held still")));
    // Each cap spans the pipe's inside area.
    let inside = 0.1022604_f64 / 2.0;
    for wall in &walls.walls {
        let area = wall.panel.width_m * wall.panel.height_m;
        assert!((area - PI * inside * inside).abs() < 1e-9, "{area}");
    }
    walls_are_a_spring(&value, |larger, volume| {
        let inside_area = PI * inside * inside;
        let length = larger["speaker"]["enclosure"]["outer"]["length_m"]
            .as_f64()
            .unwrap();
        larger["speaker"]["enclosure"]["outer"]["length_m"] = json!(length + volume / inside_area);
    });
}

/// A box cut from foam board, whose maker publishes stiffness and density as ranges.
#[test]
fn a_sheet_box_vibrates_when_its_sheet_publishes_stiffness_and_density() {
    let mut value = example("mdf-box");
    value["speaker"]["enclosure"]["outer"]["stock_id"] = json!("komatex-foam-board");
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
    assert!(walls
        .notes
        .iter()
        .any(|n| n.contains("published as a range")));
    let t = 0.002997;
    walls_are_a_spring(&value, |larger, volume| {
        let depth = larger["speaker"]["enclosure"]["outer"]["depth_m"]
            .as_f64()
            .unwrap();
        larger["speaker"]["enclosure"]["outer"]["depth_m"] =
            json!(depth + volume / ((0.16 - 2.0 * t) * (0.25 - 2.0 * t)));
    });
}

/// MDF and plywood sheet boxes vibrate once the catalog carries density and stiffness
/// (Wood Handbook Table 12-1 ranges, 2026-11-15).
#[test]
fn mdf_and_plywood_sheet_boxes_vibrate() {
    let registry = builtin();
    let request = SimulateRequest {
        walls: true,
        ..SimulateRequest::default()
    };
    for (stock, thickness) in [
        ("ultrastock-mdf-3-4", 0.01905_f64),
        ("sandeply-18mm", 0.0180086_f64),
    ] {
        let mut value = example("mdf-box");
        value["speaker"]["enclosure"]["outer"]["stock_id"] = json!(stock);
        let file = design(&value);
        let outer = registry
            .create_outer(
                &file.speaker.enclosure.outer.kind,
                &file.speaker.enclosure.outer.fields,
            )
            .unwrap();
        let walls = cabinet_walls(&file, outer.as_ref()).unwrap();
        assert_eq!(walls.walls.len(), 6, "{stock}");
        assert!(
            walls
                .notes
                .iter()
                .any(|n| n.contains("published as a range")),
            "{stock}: {:?}",
            walls.notes
        );
        let result = simulate(&file, &request, &registry);
        assert!(
            result
                .fidelity
                .ignored
                .iter()
                .all(|l| !l.contains("no published stiffness or density")),
            "{stock}: {:?}",
            result.fidelity.ignored
        );
        let t = thickness;
        walls_are_a_spring(&value, |larger, volume| {
            let depth = larger["speaker"]["enclosure"]["outer"]["depth_m"]
                .as_f64()
                .unwrap();
            larger["speaker"]["enclosure"]["outer"]["depth_m"] =
                json!(depth + volume / ((0.16 - 2.0 * t) * (0.25 - 2.0 * t)));
        });
    }
}

fn everyday_walls() -> SimulateRequest {
    SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        listening_window: true,
        losses: true,
        walls: true,
        ..SimulateRequest::default()
    }
}

/// A chamber that fills the whole inside is the undivided cabinet: the same walls, the same
/// spans, the same sound, through the network's own path (README 5.11).
#[test]
fn a_chamber_that_fills_the_inside_has_the_undivided_cabinets_walls() {
    let plain = example("sealed-test");
    let mut divided = plain.clone();
    divided["speaker"]["enclosure"]["chambers"] = json!([
        { "id": "main", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242] }
    ]);
    for build in [
        json!({ "kind": "solid" }),
        json!({ "kind": "filled", "skin_m": 0.0016, "fill": "quikrete-play-sand" }),
    ] {
        let (mut a, mut b) = (plain.clone(), divided.clone());
        a["build"]["wall"] = build.clone();
        b["build"]["wall"] = build.clone();
        for request in [
            SimulateRequest {
                walls: true,
                ..SimulateRequest::default()
            },
            everyday_walls(),
        ] {
            let ra = simulate(&design(&a), &request, &builtin());
            let rb = simulate(&design(&b), &request, &builtin());
            assert!(rb
                .fidelity
                .ignored
                .iter()
                .any(|l| l.contains("over 1 chambers")));
            let rigid = simulate(
                &design(&a),
                &SimulateRequest {
                    walls: false,
                    ..request.clone()
                },
                &builtin(),
            );
            let mut worst = 0.0f64;
            let mut moved = 0.0f64;
            for (bin, f) in ra.frequencies_hz.iter().enumerate() {
                if *f < 20.0 || *f > 3000.0 {
                    continue;
                }
                worst = worst.max((ra.summed_spl_db[bin] - rb.summed_spl_db[bin]).abs());
                moved = moved.max((ra.summed_spl_db[bin] - rigid.summed_spl_db[bin]).abs());
            }
            println!("{build}: largest difference {worst:.2e} dB (walls move it {moved:.2} dB)");
            assert!(moved > 0.5, "the walls must be heard ({moved} dB)");
            assert!(worst < 1e-6, "{build}: {worst} dB");
        }
    }
}

/// Two sealed chambers one above the other, each with its own driver: below the outer walls'
/// resonances each chamber's outer walls are a spring on that chamber alone. The partition
/// between them also vibrates (a solid plate); at 10–20 Hz a stiff PLA partition couples the
/// chambers only slightly, so the outer-wall spring identity still holds within 0.05 dB.
#[test]
fn each_chamber_pushes_on_its_own_walls() {
    let base = example("sealed-test");
    let mut value = base.clone();
    let enclosure = &mut value["speaker"]["enclosure"];
    enclosure["outer"]["height_m"] = json!(0.492);
    enclosure["chambers"] = json!([
        { "id": "low", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242] },
        { "id": "high", "min_m": [0.008, 0.008, 0.250], "max_m": [0.152, 0.172, 0.484] }
    ]);
    enclosure["mounts"] = json!([
        { "id": "a", "surface": "front", "at_m": [0.08, 0.125], "rear_into": "low", "recess_m": 0.0 },
        { "id": "b", "surface": "front", "at_m": [0.08, 0.367], "rear_into": "high", "recess_m": 0.0 }
    ]);
    value["speaker"]["radiators"] = json!([
        { "kind": "cone", "driver_id": "test-woofer-a", "mount": "a" },
        { "kind": "cone", "driver_id": "test-woofer-a", "mount": "b" }
    ]);
    let chain = value["dsp"]["per_radiator"][0].clone();
    value["dsp"]["per_radiator"] = json!([chain.clone(), chain]);
    value["electronics"]["amplifiers"] = json!(["test-amp-a", "test-amp-a"]);
    let file = design(&value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let chambers = seatlock_sim::chambers::chambers(&file).unwrap();
    let walls = chamber_walls(&file, outer.as_ref(), &chambers).unwrap();
    // Five outer sides each; the shared face is one vibrating partition.
    assert_eq!(walls.walls.len(), 10);
    assert_eq!(walls.partitions.len(), 1);
    assert!(walls
        .notes
        .iter()
        .any(|n| n.contains("partition") && n.contains("vibrate")));
    let omega = 2.0 * PI * 0.5;
    let c_low = (walls.chamber_admittance(0, 0.5) / Complex64::new(0.0, omega)).re;
    let c_high = (walls.chamber_admittance(1, 0.5) / Complex64::new(0.0, omega)).re;
    assert!((c_low - c_high).abs() < 1e-9 * c_low, "{c_low} {c_high}");
    let rigid = SimulateRequest::default();
    let with_walls = SimulateRequest {
        walls: true,
        ..SimulateRequest::default()
    };
    let result = simulate(&file, &rigid, &registry);
    assert!(result
        .issues
        .iter()
        .all(|i| i.severity != seatlock_core::issue::Severity::Error));
    let c_air = result.interior_volume_m3.unwrap() / 2.0 / (RHO0 * C0 * C0);
    let delta = c_low * RHO0 * C0 * C0 / (0.144 * 0.234);
    let mut larger = value.clone();
    larger["speaker"]["enclosure"]["outer"]["depth_m"] = json!(0.18 + delta);
    larger["speaker"]["enclosure"]["chambers"][0]["max_m"][1] = json!(0.172 + delta);
    larger["speaker"]["enclosure"]["chambers"][1]["max_m"][1] = json!(0.172 + delta);
    let give_back = 20.0 * (1.0 - c_low / (c_air + c_low)).log10();
    println!(
        "each chamber's walls: {:.3} % of its air, give-back {give_back:.4} dB, partition \
         {:.0} Hz",
        100.0 * c_low / c_air,
        walls.partitions[0].panel.first_resonance_hz()
    );
    for freq in [10.0, 20.0] {
        let got = level_at(&value, &with_walls, freq);
        let want = level_at(&larger, &rigid, freq) + give_back;
        assert!((got - want).abs() < 0.05, "{freq} Hz: {got} against {want}");
    }
}

/// The partition between two sealed chambers is heard: near its resonance the level moves
/// relative to outer walls alone (checked by the partition's own compliance changing Z_ab).
#[test]
fn a_partition_between_chambers_vibrates() {
    let mut value = example("sealed-test");
    let enclosure = &mut value["speaker"]["enclosure"];
    enclosure["outer"]["height_m"] = json!(0.492);
    enclosure["chambers"] = json!([
        { "id": "low", "min_m": [0.008, 0.008, 0.008], "max_m": [0.152, 0.172, 0.242] },
        { "id": "high", "min_m": [0.008, 0.008, 0.250], "max_m": [0.152, 0.172, 0.484] }
    ]);
    enclosure["mounts"] = json!([
        { "id": "a", "surface": "front", "at_m": [0.08, 0.125], "rear_into": "low", "recess_m": 0.0 }
    ]);
    value["speaker"]["radiators"] =
        json!([{ "kind": "cone", "driver_id": "test-woofer-a", "mount": "a" }]);
    let file = design(&value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let chambers = seatlock_sim::chambers::chambers(&file).unwrap();
    let walls = chamber_walls(&file, outer.as_ref(), &chambers).unwrap();
    assert_eq!(walls.partitions.len(), 1);
    let part = &walls.partitions[0];
    let first = part.panel.first_resonance_hz();
    assert!(
        first > 50.0 && first < 5000.0,
        "partition resonance {first}"
    );
    // Soften the partition by asking for a thin solid: rebuild with wall_m still 8 mm gap
    // but compare Z via simulate — with walls on, fidelity names the partition.
    let with_walls = SimulateRequest {
        walls: true,
        radiation: "point".into(),
        frequencies_hz: Some(vec![first, first * 0.5, 20.0]),
        ..SimulateRequest::default()
    };
    let result = simulate(&file, &with_walls, &registry);
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|l| l.contains("partition") && l.contains("vibrate")));
    // At the partition resonance the two chambers share flow; Z_ab differs from the
    // outer-wall-only spring (rigid partition) enough to move the on-axis level.
    let rigid = SimulateRequest {
        radiation: "point".into(),
        frequencies_hz: Some(vec![first]),
        ..SimulateRequest::default()
    };
    let soft = level_at(&value, &with_walls, first);
    let hard = level_at(&value, &rigid, first);
    println!("partition {first:.0} Hz: walls {soft:.2} dB, rigid {hard:.2} dB");
    assert!(
        (soft - hard).abs() > 0.05,
        "partition should be heard near its resonance: {soft} against {hard}"
    );
}

/// A brace bar across the chamber reaches opposite walls, splits those plates, raises the
/// lowest wall resonance, takes air, and does not raise the "unknown solid" warning.
#[test]
fn a_brace_splits_the_walls_it_reaches_and_takes_air() {
    let mut value = example("sealed-test");
    let file = design(&value);
    let registry = builtin();
    let outer = registry
        .create_outer(
            &file.speaker.enclosure.outer.kind,
            &file.speaker.enclosure.outer.fields,
        )
        .unwrap();
    let plain = cabinet_walls(&file, outer.as_ref()).unwrap();
    let plain_first = plain.first_resonance_hz();
    let plain_air = simulate(&file, &SimulateRequest::default(), &registry)
        .interior_volume_m3
        .unwrap();

    // Left–right bar at mid height (README 6.2's brace shape), spanning the inside width.
    value["speaker"]["enclosure"]["solids"] = json!([{
        "id": "brace-1",
        "kind": "brace",
        "min_m": [0.008, 0.085, 0.120],
        "max_m": [0.152, 0.095, 0.130]
    }]);
    let braced_file = design(&value);
    let braced = cabinet_walls(&braced_file, outer.as_ref()).unwrap();
    let braced_first = braced.first_resonance_hz();
    let left_plain = plain
        .walls
        .iter()
        .filter(|w| w.surface == "left")
        .map(|w| w.panel.first_resonance_hz())
        .fold(f64::INFINITY, f64::min);
    let left_braced = braced
        .walls
        .iter()
        .filter(|w| w.surface == "left")
        .map(|w| w.panel.first_resonance_hz())
        .fold(f64::INFINITY, f64::min);
    println!(
        "sealed-test: cabinet {plain_first:.0} → {braced_first:.0} Hz, left wall {left_plain:.0} → \
         {left_braced:.0} Hz, {} plates against {}",
        braced.walls.len(),
        plain.walls.len()
    );
    // Halving the taller span raises that wall by about √2; front/back then set the cabinet
    // floor, so the whole-cabinet rise is smaller.
    assert!(
        left_braced > 1.3 * left_plain,
        "brace should raise the left wall: {left_braced} against {left_plain}"
    );
    assert!(
        braced.walls.len() > plain.walls.len(),
        "brace should split walls into more plates"
    );
    assert!(braced
        .notes
        .iter()
        .any(|n| n.contains("brace") && n.contains("split")));

    let result = simulate(
        &braced_file,
        &SimulateRequest {
            walls: true,
            ..SimulateRequest::default()
        },
        &registry,
    );
    assert!(
        result
            .issues
            .iter()
            .all(|i| !i.message.contains("does not make yet")),
        "brace must not be an unknown solid: {:?}",
        result.issues
    );
    let braced_air = result.interior_volume_m3.unwrap();
    let brace_m3 = 0.144 * 0.010 * 0.010;
    assert!(
        (plain_air - braced_air - brace_m3).abs() < 0.15 * brace_m3,
        "brace should take about {:.6} m3 of air: plain {plain_air}, braced {braced_air}",
        brace_m3
    );
}

/// The `two-chamber` bandpass: both chambers' outer walls are heard, and the partition vibrates.
#[test]
fn a_divided_cabinet_hears_its_wall_build() {
    let mut value = example("two-chamber");
    let request = everyday_walls();
    let solid = simulate(&design(&value), &request, &builtin());
    value["build"]["wall"] =
        json!({ "kind": "filled", "skin_m": 0.0016, "fill": "quikrete-play-sand" });
    let sand = simulate(&design(&value), &request, &builtin());
    for result in [&solid, &sand] {
        assert!(result
            .issues
            .iter()
            .all(|i| i.code != "wall_build_not_heard"
                && i.severity != seatlock_core::issue::Severity::Error));
        assert!(result
            .fidelity
            .ignored
            .iter()
            .any(|l| l.contains("over 2 chambers")));
        assert!(result
            .fidelity
            .ignored
            .iter()
            .any(|l| l.contains("partition") && l.contains("vibrate")));
    }
    let worst = solid
        .summed_spl_db
        .iter()
        .zip(&sand.summed_spl_db)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("two-chamber: sand-filled against solid walls, largest difference {worst:.2} dB");
    assert!(worst > 0.5);
}
