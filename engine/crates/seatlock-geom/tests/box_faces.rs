use seatlock_core::registry::OuterShape;
use seatlock_geom::BoxShape;

fn shape() -> BoxShape {
    BoxShape {
        width_m: 0.16,
        height_m: 0.25,
        depth_m: 0.18,
        wall_m: 0.008,
    }
}

#[test]
fn front_lower_left_is_the_origin() {
    let pose = shape().mount_pose("front", [0.0, 0.0]).unwrap();
    assert_eq!(pose.position_m, [0.0, 0.0, 0.0]);
    assert_eq!(pose.outward, [0.0, -1.0, 0.0]);
}

#[test]
fn each_face_points_outward() {
    let box_shape = shape();
    for face in ["front", "back", "left", "right", "top", "bottom"] {
        let pose = box_shape.mount_pose(face, [0.02, 0.03]).unwrap();
        let length = pose.outward.iter().map(|c| c * c).sum::<f64>().sqrt();
        assert!((length - 1.0).abs() < 1e-12, "{face}");
    }
}

#[test]
fn inside_test_matches_the_cavity() {
    let box_shape = shape();
    assert!(box_shape.contains_air([0.08, 0.09, 0.12]));
    assert!(!box_shape.contains_air([0.002, 0.09, 0.12]));
    assert!(!box_shape.contains_air([0.08, -0.01, 0.12]));
}

#[test]
fn voxels_match_interior_volume() {
    let box_shape = shape();
    let air = box_shape.interior_volume_m3().unwrap();
    let voxels = box_shape.voxel_air_volume_m3(0.005).unwrap();
    assert!((voxels - air).abs() <= 0.02 * air, "{voxels} vs {air}");
}
