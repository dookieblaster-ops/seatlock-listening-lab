//! Looped chamber networks in the detailed waveguide model (README §5.4 Level 3).

use seatlock_acoustics::{solve, solve_mesh, AcousticNode, MeshDuct, WaveguideMesh};
use std::f64::consts::PI;

#[test]
fn mesh_matches_tree_on_a_ported_box() {
    let volume = 0.02;
    let radius = 0.02;
    let length = 0.12;
    let area = PI * radius * radius;
    let tree = AcousticNode::Parallel(vec![
        AcousticNode::Chamber {
            volume_m3: volume,
            slot: 0,
            fill_sigma: None,
            extra_fills: Vec::new(),
        },
        AcousticNode::Duct {
            length_m: length,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: 0.849 * radius,
            load: Box::new(AcousticNode::Radiation {
                area_m2: area,
                flush: false,
            }),
        },
    ]);
    let mesh = WaveguideMesh {
        volumes_m3: vec![volume],
        fill_parts: vec![],
        fill_sigma: vec![None],
        edges: vec![MeshDuct {
            from: 0,
            to: None,
            length_m: length,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: 0.849 * radius,
            slices: None,
            outside_slot: Some(0),
            mouth_area_m2: area,
            passive: None,
        }],
        driver: 0,
    };
    for freq in [30.0, 80.0, 200.0, 800.0] {
        let t = solve(&tree, freq);
        let m = solve_mesh(&mesh, freq, false, &[], None);
        let err = (t.z_in - m.z_in).norm() / t.z_in.norm().max(1e-20);
        assert!(
            err < 1e-9,
            "ported box at {freq} Hz: tree {} mesh {} rel {err}",
            t.z_in,
            m.z_in
        );
        assert_eq!(m.openings.len(), 1);
        let herr = (t.openings[0] - m.openings[0]).norm();
        assert!(
            herr < 1e-9,
            "H at {freq}: tree {} mesh {}",
            t.openings[0],
            m.openings[0]
        );
    }
}

#[test]
fn a_loop_of_two_identical_ducts_matches_one_duct_of_double_area() {
    // Two equal sealed chambers joined by two identical ducts (a loop). At low frequency
    // that is the same as one duct with twice the area (two air masses in parallel).
    let v = 0.01;
    let area = 0.005;
    let coupler = 0.08;
    let added = 2.0 * 0.849 * (area / PI).sqrt();
    let looped = WaveguideMesh {
        volumes_m3: vec![v, v],
        fill_parts: vec![],
        fill_sigma: vec![None, None],
        edges: vec![
            MeshDuct {
                from: 0,
                to: Some(1),
                length_m: coupler,
                area_in_m2: area,
                area_out_m2: area,
                added_length_m: added,
                slices: None,
                outside_slot: None,
                mouth_area_m2: area,
                passive: None,
            },
            MeshDuct {
                from: 0,
                to: Some(1),
                length_m: coupler,
                area_in_m2: area,
                area_out_m2: area,
                added_length_m: added,
                slices: None,
                outside_slot: None,
                mouth_area_m2: area,
                passive: None,
            },
        ],
        driver: 0,
    };
    // Two ducts of area A vs one of 2A with the same added length so L/S matches.
    let wide_matched = WaveguideMesh {
        volumes_m3: vec![v, v],
        fill_parts: vec![],
        fill_sigma: vec![None, None],
        edges: vec![MeshDuct {
            from: 0,
            to: Some(1),
            length_m: coupler,
            area_in_m2: 2.0 * area,
            area_out_m2: 2.0 * area,
            added_length_m: added,
            slices: None,
            outside_slot: None,
            mouth_area_m2: 2.0 * area,
            passive: None,
        }],
        driver: 0,
    };
    for freq in [20.0, 80.0, 300.0] {
        let a = solve_mesh(&looped, freq, false, &[], None);
        let b = solve_mesh(&wide_matched, freq, false, &[], None);
        let err = (a.z_in - b.z_in).norm() / a.z_in.norm().max(1e-20);
        assert!(
            err < 1e-9,
            "loop vs double-area at {freq} Hz: {} vs {} rel {err}",
            a.z_in,
            b.z_in
        );
    }
}

#[test]
fn mesh_matches_tree_on_two_chambers_joined_by_one_duct() {
    let v0 = 0.012;
    let v1 = 0.008;
    let area = 0.004;
    let length = 0.09;
    let r = (area / PI).sqrt();
    let added = 2.0 * 0.849 * r;
    let tree = AcousticNode::Parallel(vec![
        AcousticNode::Chamber {
            volume_m3: v0,
            slot: 0,
            fill_sigma: None,
            extra_fills: Vec::new(),
        },
        AcousticNode::Duct {
            length_m: length,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: added,
            load: Box::new(AcousticNode::Chamber {
                volume_m3: v1,
                slot: 1,
                fill_sigma: None,
                extra_fills: Vec::new(),
            }),
        },
    ]);
    let mesh = WaveguideMesh {
        volumes_m3: vec![v0, v1],
        fill_parts: vec![],
        fill_sigma: vec![None, None],
        edges: vec![MeshDuct {
            from: 0,
            to: Some(1),
            length_m: length,
            area_in_m2: area,
            area_out_m2: area,
            added_length_m: added,
            slices: None,
            outside_slot: None,
            mouth_area_m2: area,
            passive: None,
        }],
        driver: 0,
    };
    for freq in [25.0, 100.0, 400.0, 1500.0] {
        let t = solve(&tree, freq);
        let m = solve_mesh(&mesh, freq, false, &[], None);
        let err = (t.z_in - m.z_in).norm() / t.z_in.norm().max(1e-20);
        assert!(
            err < 1e-8,
            "two chambers at {freq} Hz: tree {} mesh {} rel {err}",
            t.z_in,
            m.z_in
        );
    }
}
