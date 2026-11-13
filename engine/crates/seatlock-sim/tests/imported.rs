//! An imported cabinet (README 6.2, outer shape `mesh`) behaves like the same cabinet built
//! the usual way.

use seatlock_geom::mesh::encode_base64;
use seatlock_sim::{builtin, geometry_report, simulate, SimulateRequest};
use serde_json::{json, Value};

type Mesh = (Vec<[f64; 3]>, Vec<[u32; 3]>);

fn box_mesh(lo: [f64; 3], hi: [f64; 3], out: bool) -> Mesh {
    let v: Vec<[f64; 3]> = (0..8)
        .map(|i| {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        })
        .collect();
    let mut t = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    if !out {
        for tri in &mut t {
            tri.swap(1, 2);
        }
    }
    (v, t)
}

fn part(name: &str, role: &str, meshes: &[Mesh]) -> Value {
    let (mut v, mut t) = (Vec::new(), Vec::new());
    for (bv, bt) in meshes {
        let off = v.len() as u32;
        v.extend(bv.iter().copied());
        t.extend(bt.iter().map(|x| x.map(|i| i + off)));
    }
    let fb: Vec<u8> = v
        .iter()
        .flatten()
        .flat_map(|x: &f64| (*x as f32).to_le_bytes())
        .collect();
    let ib: Vec<u8> = t
        .iter()
        .flatten()
        .flat_map(|x: &u32| x.to_le_bytes())
        .collect();
    json!({ "name": name, "role": role, "material": "pla",
            "vertices_m": encode_base64(&fb), "triangles": encode_base64(&ib) })
}

fn example() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/sealed-test.seatlock.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// `sealed-test` (0.16 x 0.25 x 0.18 m, 8 mm walls, driver at [0.08, 0.15] on the front)
/// as an imported shell, with the driver on a mounting point at the same place.
pub fn imported_sealed_test() -> Value {
    let mut design = example();
    let shell = part(
        "shell",
        "solid",
        &[
            box_mesh([0.0; 3], [0.16, 0.18, 0.25], true),
            box_mesh([0.008; 3], [0.152, 0.172, 0.242], false),
        ],
    );
    design["speaker"]["enclosure"]["outer"] = json!({ "kind": "mesh", "parts": [shell],
        "points": [ { "id": "p1", "point_m": [0.08, 0.0, 0.15], "normal": [0.0, -1.0, 0.0] } ] });
    design["speaker"]["enclosure"]["mounts"][0]["surface"] = json!("p1");
    design["speaker"]["enclosure"]["mounts"][0]["at_m"] = json!([0.0, 0.0]);
    design
}

#[test]
fn an_imported_box_sounds_like_the_same_box_built_as_a_box() {
    let plain: seatlock_core::DesignFile = serde_json::from_value(example()).unwrap();
    let imported: seatlock_core::DesignFile =
        serde_json::from_value(imported_sealed_test()).unwrap();
    for radiation in ["point", "baffle"] {
        let request = SimulateRequest {
            radiation: radiation.into(),
            enclosure: "waveguide".into(),
            ..Default::default()
        };
        let a = simulate(&plain, &request, &builtin());
        let b = simulate(&imported, &request, &builtin());
        assert!(
            b.issues
                .iter()
                .all(|i| i.severity != seatlock_core::issue::Severity::Error),
            "{:?}",
            b.issues
        );
        let worst = a
            .summed_spl_db
            .iter()
            .zip(&b.summed_spl_db)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(
            worst < 0.05,
            "{radiation}: the imported box differs by {worst} dB"
        );
    }
    // The cabinet report: the same plastic and air, and a print tree from the meshes.
    let ra = geometry_report(&plain).unwrap();
    let rb = geometry_report(&imported).unwrap();
    eprintln!(
        "air {} vs {}, plastic {} vs {}",
        ra.interior_volume_m3, rb.interior_volume_m3, ra.solid_volume_m3, rb.solid_volume_m3
    );
    assert!((ra.interior_volume_m3 - rb.interior_volume_m3).abs() < 3e-3 * ra.interior_volume_m3);
    assert!((ra.solid_volume_m3 - rb.solid_volume_m3).abs() < 1e-3 * ra.solid_volume_m3);
    assert!(rb.can_export_print, "{:?}", rb.warnings);
}

#[test]
fn each_part_is_priced_by_its_own_material_and_damping_is_bought_by_volume() {
    let mut design = imported_sealed_test();
    let wool = part(
        "wool",
        "damping",
        &[box_mesh([0.03, 0.1, 0.03], [0.13, 0.16, 0.1], true)],
    );
    design["speaker"]["enclosure"]["outer"]["parts"][0]["material"] = json!("pla");
    design["speaker"]["enclosure"]["outer"]["parts"]
        .as_array_mut()
        .unwrap()
        .push(wool);
    let design: seatlock_core::DesignFile = serde_json::from_value(design).unwrap();
    let report = geometry_report(&design).unwrap();
    let lines: Vec<String> = report.cost.lines.iter().map(|l| l.item.clone()).collect();
    assert!(lines.iter().any(|l| l == "Printed PLA, shell"), "{lines:?}");
    let damping = report
        .cost
        .lines
        .iter()
        .find(|l| l.item.starts_with("Damping, wool"))
        .unwrap();
    // A stereo pair: two blocks of 0.1 x 0.06 x 0.07 m.
    assert!(
        (damping.quantity - 2.0 * 0.42).abs() < 0.01,
        "{}",
        damping.quantity
    );
}

#[test]
fn a_damping_part_calms_the_cabinets_resonances_in_the_3d_model() {
    let run = |sigma: Option<f64>| {
        let mut design = imported_sealed_test();
        if let Some(sigma) = sigma {
            // Wool filling the back half of the box.
            let mut wool = part(
                "wool",
                "damping",
                &[box_mesh([0.008, 0.09, 0.008], [0.152, 0.172, 0.242], true)],
            );
            wool["flow_resistivity_pa_s_m2"] = json!(sigma);
            design["speaker"]["enclosure"]["outer"]["parts"]
                .as_array_mut()
                .unwrap()
                .push(wool);
        }
        let design: seatlock_core::DesignFile = serde_json::from_value(design).unwrap();
        let request = SimulateRequest {
            enclosure: "fdtd".into(),
            fdtd_dx_m: Some(0.01),
            ..Default::default()
        };
        let r = simulate(&design, &request, &builtin());
        assert!(
            r.issues
                .iter()
                .all(|i| i.severity != seatlock_core::issue::Severity::Error),
            "{:?}",
            r.issues
        );
        if sigma.is_some() {
            assert!(
                r.fidelity
                    .ignored
                    .iter()
                    .any(|l| l.contains("Damping material")),
                "{:?}",
                r.fidelity.ignored
            );
        }
        // How far the 3D result strays from the quick model, which has no standing waves.
        let quick = simulate(&design, &SimulateRequest::default(), &builtin());
        let stray: Vec<f64> = r.radiators[0]
            .impedance_ohm
            .iter()
            .zip(&quick.radiators[0].impedance_ohm)
            .map(|(a, b)| (a - b).abs())
            .collect();
        (r.frequencies_hz.clone(), stray)
    };
    let (f, bare) = run(None);
    let (_, damped) = run(Some(10000.0));
    // The cabinet's first standing wave along its height shows in the driver's impedance near
    // 700 Hz; damping lowers its peak. Below 100 Hz the box is unchanged.
    let band = |lo: f64, hi: f64, z: &[f64]| {
        f.iter()
            .zip(z)
            .filter(|(x, _)| **x >= lo && **x <= hi)
            .map(|(_, v)| *v)
            .fold(0.0, f64::max)
    };
    let (ripple_bare, ripple_damped) = (band(500.0, 3000.0, &bare), band(500.0, 3000.0, &damped));
    eprintln!("largest standing-wave effect on the impedance, 500 Hz to 3 kHz: {ripple_bare:.3} ohm bare, {ripple_damped:.3} ohm damped");
    assert!(ripple_damped < 0.7 * ripple_bare);
}

#[test]
fn a_port_on_an_imported_cabinet_sounds_like_the_same_port_on_a_box() {
    let port = json!([{ "id": "port", "from": "main", "to": "outside",
        "path_m": [[0.04, 0.09, 0.05], [0.04, 0.0, 0.05]],
        "section": { "shape": "round", "start_m": [0.03], "end_m": [0.03] } }]);
    let mut plain = example();
    plain["speaker"]["enclosure"]["ducts"] = port.clone();
    let mut imported = imported_sealed_test();
    imported["speaker"]["enclosure"]["ducts"] = port;
    imported["speaker"]["enclosure"]["outer"]["points"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({ "id": "port-mouth", "point_m": [0.04, 0.0, 0.05], "normal": [0.0, -1.0, 0.0] }),
        );
    let plain: seatlock_core::DesignFile = serde_json::from_value(plain).unwrap();
    let imported: seatlock_core::DesignFile = serde_json::from_value(imported).unwrap();
    let request = SimulateRequest {
        enclosure: "waveguide".into(),
        radiation: "baffle".into(),
        ..Default::default()
    };
    let a = simulate(&plain, &request, &builtin());
    let b = simulate(&imported, &request, &builtin());
    assert!(
        b.issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        b.issues
    );
    let worst = a
        .summed_spl_db
        .iter()
        .zip(&b.summed_spl_db)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.1, "the imported ported box differs by {worst} dB");
    let ra = geometry_report(&plain).unwrap();
    let rb = geometry_report(&imported).unwrap();
    eprintln!(
        "air {} vs {}, plastic {} vs {}, print {} {:?}",
        ra.interior_volume_m3,
        rb.interior_volume_m3,
        ra.solid_volume_m3,
        rb.solid_volume_m3,
        rb.can_export_print,
        rb.warnings
    );
    assert!(rb.can_export_print, "{:?}", rb.warnings);
    assert!((ra.solid_volume_m3 - rb.solid_volume_m3).abs() < 2e-3 * ra.solid_volume_m3);
}
