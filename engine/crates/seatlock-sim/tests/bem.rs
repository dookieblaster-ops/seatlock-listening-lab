//! The `bem` radiation model inside `simulate` (README section 5.6), including mouth
//! radiation impedances joined to the waveguide network (CHK-BEM-8 / CHK-BEM-9) and true
//! open apertures (CHK-BEM-10).

use num_complex::Complex64;
use seatlock_acoustics::{
    radiation_impedance, solve_with_radiation_matrix, with_fixed_radiation_loads, AcousticNode,
};
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::{RadiationScene, SourcePatch};
use seatlock_sim::bem::{self, NETWORK_NOTE, OPENINGS_NOTE};
use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};
use serde_json::{json, Value};
use std::f64::consts::PI;

fn load_example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
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
            bem_subdivisions: Some(4),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

fn run_waveguide(value: &Value, radiation: &str, freqs: &[f64]) -> SimulateResult {
    let design: seatlock_core::DesignFile = serde_json::from_value(value.clone()).unwrap();
    simulate(
        &design,
        &SimulateRequest {
            enclosure: "waveguide".into(),
            radiation: radiation.into(),
            frequencies_hz: Some(freqs.to_vec()),
            bem_subdivisions: Some(4),
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

#[test]
fn bem_is_registered_and_labels_the_outer_shape_mesh() {
    let freqs = [50.0, 100.0, 200.0];
    let result = run(&load_example("sealed-test"), "bem", &freqs);
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.fidelity.radiation, "bem");
    let label = result.fidelity.ignored.join(" ");
    assert!(
        label.contains("Boundary-element outside"),
        "label was {label}"
    );
    assert!(label.contains("outer shape"), "label was {label}");
    assert!(
        !label.contains("bounding box; curved"),
        "old AABB-only note still present: {label}"
    );
}

#[test]
fn a_front_driver_agrees_with_the_baffle_model_at_low_frequency() {
    // Below the mesh hand-over (~1 kHz for subdiv 4 on sealed-test), BEM and baffle
    // should stay within a couple of dB for a small front-facing sealed box.
    let freqs = [30.0, 60.0, 100.0, 200.0, 400.0];
    let baffle = run(&load_example("sealed-test"), "baffle", &freqs);
    let bem = run(&load_example("sealed-test"), "bem", &freqs);
    for (bin, freq) in freqs.iter().enumerate() {
        let apart = bem.summed_spl_db[bin] - baffle.summed_spl_db[bin];
        assert!(
            apart.abs() < 2.0,
            "{freq} Hz: bem vs baffle {apart:+.2} dB (bem={:.2} baffle={:.2})",
            bem.summed_spl_db[bin],
            baffle.summed_spl_db[bin]
        );
        assert!(
            apart != 0.0,
            "{freq} Hz was left as the baffle value; BEM should overwrite below the hand-over"
        );
    }
}

#[test]
fn a_tube_design_runs_under_bem_without_falling_back_to_errors() {
    let freqs = [50.0, 100.0, 200.0];
    let result = run(&load_example("pvc-tube"), "bem", &freqs);
    assert!(
        result
            .issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        result.issues
    );
    assert_eq!(result.fidelity.radiation, "bem");
    let label = result.fidelity.ignored.join(" ");
    assert!(label.contains("outer shape"), "label was {label}");
    for (bin, freq) in freqs.iter().enumerate() {
        assert!(
            result.summed_spl_db[bin].is_finite(),
            "{freq} Hz SPL was not finite"
        );
    }
}

/// CHK-BEM-8: a circular patch on a large flat face at low `ka` has self-impedance near the
/// flush analytic radiation load (README 5.4 / 5.6). Seed 16 and `ka = 0.10` keep the bin under
/// the hand-over; a 150 mm patch with a tight mouth footprint keeps the mesh area near the piston.
#[test]
fn chk_bem_8_mouth_self_impedance_matches_flush_piston_at_low_ka() {
    let registry = builtin();
    let fields = json!({
        "width_m": 4.0,
        "height_m": 4.0,
        "depth_m": 4.0,
        "wall_m": 0.02,
        "edge_radius_m": 0.0
    });
    let shape = registry
        .create_outer("box", fields.as_object().unwrap())
        .expect("box");
    let scene = RadiationScene {
        shape: shape.as_ref(),
        space: "full",
        baffle_width_m: 4.0,
        reference_m: [2.0, 0.0, 2.0],
    };
    let radius = 0.15;
    let area = PI * radius * radius;
    let mouth = SourcePatch {
        position_m: [2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        area_m2: area,
    };
    let ka = 0.10;
    let freq = ka * C0 / (2.0 * PI * radius);
    let table = bem::mouth_self_impedances(&scene, &[mouth], &[freq], 16).expect("Z_rad");
    let bem_z = table[0][0];
    let k = 2.0 * PI * freq / C0;
    let analytic = radiation_impedance(area, true, k);
    let re_err = (bem_z.re - analytic.re).abs() / analytic.re.max(1e-9);
    let im_err = (bem_z.im - analytic.im).abs() / analytic.im.abs().max(1e-9);
    assert!(
        bem_z.re > 0.0 && bem_z.im > 0.0,
        "Z_rad should be mass-like with positive resistance: {bem_z}"
    );
    // Must not be exactly the analytic fallback (that would mean the bin was above the mesh).
    assert!(
        (bem_z - analytic).norm() / analytic.norm().max(1e-12) > 1e-6,
        "Z_rad matched analytic exactly; mesh hand-over swallowed the BEM solve"
    );
    assert!(
        re_err < 0.35 && im_err < 0.25,
        "ka={ka}: bem Z={bem_z:.4} analytic={analytic:.4} re_err={re_err:.3} im_err={im_err:.3}"
    );
}

#[test]
fn fixed_radiation_loads_match_the_analytic_leaf() {
    let area = PI * 0.02 * 0.02;
    let freq = 80.0;
    let k = 2.0 * PI * freq / C0;
    let z = radiation_impedance(area, false, k);
    let analytic = AcousticNode::Parallel(vec![
        AcousticNode::Compliance { volume_m3: 0.02 },
        AcousticNode::Duct {
            length_m: 0.12,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: 0.849 * 0.02,
            load: Box::new(AcousticNode::Radiation {
                area_m2: area,
                flush: false,
            }),
        },
    ]);
    let fixed = with_fixed_radiation_loads(&analytic, &[z]);
    let a = seatlock_acoustics::solve(&analytic, freq);
    let b = seatlock_acoustics::solve(&fixed, freq);
    assert!((a.z_in - b.z_in).norm() / a.z_in.norm().max(1e-12) < 1e-9);
    assert_eq!(a.openings.len(), 1);
    assert_eq!(b.openings.len(), 1);
}

#[test]
fn bem_joins_mouth_loads_into_the_waveguide_network() {
    let freqs = [40.0, 80.0, 160.0];
    let design = load_example("two-way-vented");
    let baffle = run_waveguide(&design, "baffle", &freqs);
    let bem = run_waveguide(&design, "bem", &freqs);
    assert!(
        bem.issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        bem.issues
    );
    let label = bem.fidelity.ignored.join(" ");
    assert!(
        label.contains("radiation impedance") || label.contains(NETWORK_NOTE),
        "joined-network note missing: {label}"
    );
    // Joining BEM Z_rad must change the enclosure load the woofer sees vs analytic mouths.
    let z_b = &baffle.radiators[0].impedance_ohm;
    let z_m = &bem.radiators[0].impedance_ohm;
    let mut moved = false;
    for (bin, freq) in freqs.iter().enumerate() {
        let apart = (z_m[bin] - z_b[bin]).abs() / z_b[bin].max(1e-12);
        if apart > 1e-4 {
            moved = true;
        }
        assert!(
            z_m[bin].is_finite(),
            "{freq} Hz woofer impedance was not finite under bem"
        );
    }
    assert!(
        moved,
        "bem waveguide woofer Z matched analytic mouths at every bin; join did not take effect"
    );
}

/// CHK-BEM-9: mutual mouth impedance on a large face is reciprocal and near the half-space
/// point-source estimate at low `ka`; a two-port waveguide's load moves when mutuals are kept.
#[test]
fn chk_bem_9_mutual_mouth_impedance_is_reciprocal_and_joins_the_network() {
    let registry = builtin();
    let fields = json!({
        "width_m": 4.0,
        "height_m": 4.0,
        "depth_m": 4.0,
        "wall_m": 0.02,
        "edge_radius_m": 0.0
    });
    let shape = registry
        .create_outer("box", fields.as_object().unwrap())
        .expect("box");
    let scene = RadiationScene {
        shape: shape.as_ref(),
        space: "full",
        baffle_width_m: 4.0,
        reference_m: [2.0, 0.0, 2.0],
    };
    let radius = 0.05;
    let area = PI * radius * radius;
    let separation = 0.40;
    let mouth_a = SourcePatch {
        position_m: [2.0 - separation / 2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        area_m2: area,
    };
    let mouth_b = SourcePatch {
        position_m: [2.0 + separation / 2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        area_m2: area,
    };
    // ka=0.15 and seed 16: frequency stays under the mesh hand-over on this 4 m box.
    let ka = 0.15;
    let freq = ka * C0 / (2.0 * PI * radius);
    let matrices =
        bem::mouth_impedance_matrices(&scene, &[mouth_a, mouth_b], &[freq], 16).expect("Z");
    let z = &matrices[0];
    let z11 = z[0][0];
    let z22 = z[1][1];
    let z12 = z[0][1];
    let z21 = z[1][0];
    assert!(
        z11.re > 0.0 && z11.im > 0.0,
        "Z11 should be mass-like: {z11}"
    );
    assert!(
        z22.re > 0.0 && z22.im > 0.0,
        "Z22 should be mass-like: {z22}"
    );
    let recip = (z12 - z21).norm() / z11.norm().max(1e-12);
    assert!(
        recip < 0.05,
        "mutual not reciprocal: Z12={z12} Z21={z21} rel={recip}"
    );
    // Half-space point sources: p = j ω ρ₀ e^{-jkd} / (2 π d) for unit U (baffle image).
    let k = 2.0 * PI * freq / C0;
    let omega = 2.0 * PI * freq;
    let d = separation;
    let point = Complex64::new(0.0, omega * RHO0) * Complex64::new((-k * d).cos(), (-k * d).sin())
        / (2.0 * PI * d);
    let mutual_err = (z12 - point).norm() / point.norm().max(1e-12);
    assert!(
        mutual_err < 0.45,
        "Z12={z12} point={point} rel_err={mutual_err}"
    );
    assert!(
        z12.norm() / z11.norm() > 0.05,
        "mutual too small vs self: |Z12|/|Z11|={}",
        z12.norm() / z11.norm()
    );

    // Two equal ports: diagonal-only vs full matrix must change the chamber load.
    let duct = |z_leaf: AcousticNode| AcousticNode::Duct {
        length_m: 0.12,
        area_in_m2: area,
        area_out_m2: area,
        added_length_m: 0.849 * radius,
        load: Box::new(z_leaf),
    };
    let tree = AcousticNode::Parallel(vec![
        AcousticNode::Compliance { volume_m3: 0.03 },
        duct(AcousticNode::Radiation {
            area_m2: area,
            flush: false,
        }),
        duct(AcousticNode::Radiation {
            area_m2: area,
            flush: false,
        }),
    ]);
    let diag = vec![
        vec![z11, Complex64::new(0.0, 0.0)],
        vec![Complex64::new(0.0, 0.0), z22],
    ];
    let full = z.clone();
    let a = solve_with_radiation_matrix(&tree, freq, &diag, false, &[]);
    let b = solve_with_radiation_matrix(&tree, freq, &full, false, &[]);
    let moved = (a.z_in - b.z_in).norm() / a.z_in.norm().max(1e-12);
    assert!(
        moved > 1e-4,
        "mutual join left Z_in unchanged: diag={} full={} rel={moved}",
        a.z_in,
        b.z_in
    );
}

/// CHK-BEM-10: mouths are true open apertures (exact-area disks cut into the outer mesh),
/// not rigid skin patches. Disk area within 2 % of πr²; self-Z still within CHK-BEM-8 bounds;
/// the fidelity label names open apertures; a vented waveguide run still joins mouth loads.
#[test]
fn chk_bem_10_open_apertures_match_mouth_area_and_keep_z_rad() {
    let registry = builtin();
    let fields = json!({
        "width_m": 4.0,
        "height_m": 4.0,
        "depth_m": 4.0,
        "wall_m": 0.02,
        "edge_radius_m": 0.0
    });
    let shape = registry
        .create_outer("box", fields.as_object().unwrap())
        .expect("box");
    let scene = RadiationScene {
        shape: shape.as_ref(),
        space: "full",
        baffle_width_m: 4.0,
        reference_m: [2.0, 0.0, 2.0],
    };
    let radius = 0.15;
    let area = PI * radius * radius;
    let mouth = SourcePatch {
        position_m: [2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        area_m2: area,
    };
    let (mesh, disks, _kind) = bem::open_mouth_mesh(&scene, &[mouth], 16).expect("open mesh");
    assert_eq!(disks.len(), 1);
    let disk_area: f64 = disks[0].iter().map(|&i| mesh.areas[i]).sum();
    let area_err = (disk_area - area).abs() / area;
    assert!(
        area_err < 0.02,
        "open aperture area {disk_area:.6} vs πr² {area:.6} (rel {area_err:.3})"
    );

    let ka = 0.10;
    let freq = ka * C0 / (2.0 * PI * radius);
    let table = bem::mouth_self_impedances(&scene, &[mouth], &[freq], 16).expect("Z_rad");
    let bem_z = table[0][0];
    let k = 2.0 * PI * freq / C0;
    let analytic = radiation_impedance(area, true, k);
    let re_err = (bem_z.re - analytic.re).abs() / analytic.re.max(1e-9);
    let im_err = (bem_z.im - analytic.im).abs() / analytic.im.abs().max(1e-9);
    assert!(bem_z.re > 0.0 && bem_z.im > 0.0, "Z_rad={bem_z}");
    assert!(
        (bem_z - analytic).norm() / analytic.norm().max(1e-12) > 1e-6,
        "Z_rad matched analytic exactly; mesh hand-over swallowed the BEM solve"
    );
    assert!(
        re_err < 0.35 && im_err < 0.25,
        "ka={ka}: bem Z={bem_z:.4} analytic={analytic:.4} re_err={re_err:.3} im_err={im_err:.3}"
    );

    let freqs = [40.0, 80.0];
    let bem = run_waveguide(&load_example("two-way-vented"), "bem", &freqs);
    assert!(
        bem.issues
            .iter()
            .all(|i| i.severity != seatlock_core::issue::Severity::Error),
        "{:?}",
        bem.issues
    );
    let label = bem.fidelity.ignored.join(" ");
    assert!(
        label.contains("open aperture") || label.contains(OPENINGS_NOTE),
        "open-aperture note missing: {label}"
    );
    assert!(
        label.contains("radiation impedance") || label.contains(NETWORK_NOTE),
        "network note missing: {label}"
    );
}
