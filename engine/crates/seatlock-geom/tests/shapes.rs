//! The four Level 2 outer shapes answer the whole outer-shape contract (README sections 3.3
//! and 6.2): mounts, outlines, the air test, print volume, and the print tree.

use seatlock_core::registry::{OuterShape, Registry};
use serde_json::{json, Value};

fn shape(fields: Value) -> Box<dyn OuterShape> {
    let mut registry = Registry::new();
    seatlock_geom::register(&mut registry);
    let map = fields.as_object().unwrap().clone();
    let kind = map["kind"].as_str().unwrap().to_string();
    registry.create_outer(&kind, &map).unwrap()
}

fn rounded() -> Value {
    json!({ "kind": "rounded", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18,
            "wall_m": 0.008, "edge_radius_m": 0.02 })
}
fn chamfered() -> Value {
    json!({ "kind": "chamfered", "width_m": 0.16, "height_m": 0.25, "depth_m": 0.18,
            "wall_m": 0.008, "chamfer_m": 0.02 })
}
fn sloped() -> Value {
    json!({ "kind": "sloped", "width_m": 0.16, "height_m": 0.25, "depth_bottom_m": 0.20,
            "depth_top_m": 0.15, "wall_m": 0.008 })
}
fn prism() -> Value {
    // A hexagonal front, 0.20 m across the flats' corners, extruded 0.18 m deep.
    let outline: Vec<[f64; 2]> = (0..6)
        .map(|i| {
            let angle = std::f64::consts::PI / 3.0 * i as f64;
            [0.1 + 0.1 * angle.cos(), 0.1 + 0.1 * angle.sin()]
        })
        .collect();
    json!({ "kind": "prism", "length_m": 0.18, "wall_m": 0.008, "outline_m": outline })
}

fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("rounded", rounded()),
        ("chamfered", chamfered()),
        ("sloped", sloped()),
        ("prism", prism()),
    ]
}

#[test]
fn every_level_2_shape_answers_the_whole_contract() {
    for (name, fields) in all() {
        let shape = shape(fields);
        shape
            .outer_size_m()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let surfaces = shape.surfaces();
        assert!(!surfaces.is_empty(), "{name} has no surfaces");
        for surface in &surfaces {
            let outline = shape
                .surface_outline(surface)
                .unwrap_or_else(|e| panic!("{name} {surface}: {e}"));
            assert!(outline.len() >= 3, "{name} {surface}");
            assert!(shape.panel_thickness_m(surface).unwrap() > 0.0);
        }
        shape.print_solid_m3(&[]).unwrap();
        shape.print_tree(&[]).unwrap();
        assert!(shape.contains_air([0.08, 0.09, 0.1]).unwrap(), "{name}");
        assert!(
            !shape.contains_air([0.001, 0.001, 0.001]).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn chk_geo_1_voxels_agree_with_the_formula_for_every_level_2_shape() {
    for (name, fields) in all() {
        let shape = shape(fields);
        let air = shape.interior_volume_m3().unwrap();
        let voxels = seatlock_geom::voxel_air_volume_m3(shape.as_ref(), 0.005).unwrap();
        assert!(
            (voxels - air).abs() / air < 0.02,
            "{name}: formula {air:.6e}, voxels {voxels:.6e}"
        );
    }
}

#[test]
fn rounding_and_chamfers_take_air_and_plastic_away() {
    // README section 6.2: the wall follows the outer shape at wall_m.
    let (w, h, d, t, r) = (0.16_f64, 0.25_f64, 0.18_f64, 0.008_f64, 0.02_f64);
    let (a, b, c) = (w - 2.0 * t, h - 2.0 * t, d - 2.0 * t);
    let ri = r - t;
    let (ac, bc, cc) = (a - 2.0 * ri, b - 2.0 * ri, c - 2.0 * ri);
    let pi = std::f64::consts::PI;
    let rounded_air = ac * bc * cc
        + 2.0 * ri * (ac * bc + bc * cc + ac * cc)
        + pi * ri * ri * (ac + bc + cc)
        + 4.0 / 3.0 * pi * ri.powi(3);
    let got = shape(rounded()).interior_volume_m3().unwrap();
    assert!((got - rounded_air).abs() < 1e-12, "{got} {rounded_air}");

    let ci = r - t * (2.0 - 2f64.sqrt());
    let chamfered_air = a * b * c - 2.0 * ci * ci * (a + b + c) + 6.0 * ci.powi(3);
    let got = shape(chamfered()).interior_volume_m3().unwrap();
    assert!((got - chamfered_air).abs() < 1e-12, "{got} {chamfered_air}");
}

#[test]
fn a_prism_moves_its_whole_outline_inward_by_the_wall() {
    // Regular hexagon of circumradius R: area 3*sqrt(3)/2 * R^2. Moving every side inward by
    // t gives a hexagon whose apothem is (R*sqrt(3)/2 - t).
    let (radius, t, length) = (0.1_f64, 0.008_f64, 0.18_f64);
    let apothem = radius * 3f64.sqrt() / 2.0 - t;
    let inner_radius = apothem * 2.0 / 3f64.sqrt();
    let air = 1.5 * 3f64.sqrt() * inner_radius * inner_radius * (length - 2.0 * t);
    let got = shape(prism()).interior_volume_m3().unwrap();
    assert!((got - air).abs() / air < 1e-9, "{got} {air}");
    let outer = 1.5 * 3f64.sqrt() * radius * radius * length;
    let solid = shape(prism()).print_solid_m3(&[]).unwrap();
    assert!((solid - (outer - air)).abs() / solid < 1e-9);
}

#[test]
fn a_sloped_baffle_faces_forward_and_up() {
    let shape = shape(sloped());
    let pose = shape.mount_pose("front", [0.08, 0.10]).unwrap();
    assert!(pose.outward[1] < -0.9 && pose.outward[2] > 0.1, "{pose:?}");
    assert!((pose.position_m[0] - 0.08).abs() < 1e-12);
    // 0.10 m up the slope from the front-bottom edge.
    let slope = (0.05_f64.powi(2) + 0.25_f64.powi(2)).sqrt();
    assert!((pose.position_m[2] - 0.10 * 0.25 / slope).abs() < 1e-9);
    assert!((pose.position_m[1] - 0.10 * 0.05 / slope).abs() < 1e-9);
    assert_eq!(
        shape.surface_facing([0.0, -1.0, 0.0]).as_deref(),
        Some("front")
    );
    // The back stays vertical and as deep as the bottom.
    let back = shape.mount_pose("back", [0.08, 0.1]).unwrap();
    assert_eq!(back.outward, [0.0, 1.0, 0.0]);
    assert!((back.position_m[1] - 0.20).abs() < 1e-12);
}

#[test]
fn box_like_faces_place_mounts_the_way_a_box_does() {
    let reference = shape(json!({ "kind": "box", "width_m": 0.16, "height_m": 0.25,
                                  "depth_m": 0.18, "wall_m": 0.008 }));
    for fields in [rounded(), chamfered()] {
        let shape = shape(fields);
        for surface in ["front", "back", "left", "right", "top", "bottom"] {
            let at = [0.06, 0.07];
            assert_eq!(
                shape.mount_pose(surface, at).unwrap(),
                reference.mount_pose(surface, at).unwrap(),
                "{surface}"
            );
        }
    }
}

#[test]
fn a_mount_off_the_flat_part_of_a_face_is_refused() {
    for fields in [rounded(), chamfered()] {
        let err = shape(fields).mount_pose("front", [0.01, 0.10]).unwrap_err();
        assert!(err.contains("flat"), "{err}");
    }
    let err = shape(prism())
        .mount_pose("front", [0.005, 0.005])
        .unwrap_err();
    assert!(err.contains("outside"), "{err}");
}

#[test]
fn a_prism_names_its_sides_and_mounts_on_them() {
    let shape = shape(prism());
    let surfaces = shape.surfaces();
    assert_eq!(surfaces.len(), 8);
    // side-1 runs from the first corner to the second: it faces right and a little up.
    let pose = shape.mount_pose("side-1", [0.05, 0.09]).unwrap();
    assert!(pose.outward[0] > 0.8 && pose.outward[2] > 0.4, "{pose:?}");
    assert!(!shape.contains_air(pose.position_m).unwrap());
}

#[test]
fn a_wall_too_thick_for_the_outline_is_refused() {
    let mut registry = Registry::new();
    seatlock_geom::register(&mut registry);
    let fields = json!({ "length_m": 0.2, "wall_m": 0.06,
                         "outline_m": [[0.0, 0.0], [0.1, 0.0], [0.05, 0.08]] });
    let err = registry
        .create_outer("prism", fields.as_object().unwrap())
        .err()
        .unwrap();
    assert!(err.contains("wall"), "{err}");
}
