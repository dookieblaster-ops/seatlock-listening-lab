//! True open mouths cut into an outer-shape mesh (CHK-BEM-10 support).

use seatlock_bem::{box_aabb, mesh_with_open_apertures, MouthAperture};
use std::f64::consts::PI;

#[test]
fn an_open_aperture_disk_matches_the_mouth_area() {
    let size = [4.0, 4.0, 4.0];
    let mesh = box_aabb(size, 16).unwrap();
    let radius = 0.15;
    let aperture = MouthAperture {
        position_m: [2.0, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        radius_m: radius,
    };
    let (cut, disks) = mesh_with_open_apertures(&mesh, &[aperture], 16).unwrap();
    assert_eq!(disks.len(), 1);
    assert!(!disks[0].is_empty(), "expected aperture disk triangles");
    let disk_area: f64 = disks[0].iter().map(|&i| cut.areas[i]).sum();
    let want = PI * radius * radius;
    let rel = (disk_area - want).abs() / want;
    assert!(
        rel < 0.02,
        "disk area {disk_area:.6} vs πr² {want:.6} (rel {rel:.3})"
    );
    // Closed surface for exterior CBIE: total area should stay near the box (removed skin
    // replaced by disk + collar).
    let box_area = 2.0 * (size[0] * size[1] + size[1] * size[2] + size[2] * size[0]);
    let area_rel = (cut.surface_area() - box_area).abs() / box_area;
    assert!(
        area_rel < 0.05,
        "cut surface {:.4} vs box {:.4} (rel {area_rel:.3})",
        cut.surface_area(),
        box_area
    );
    for &i in &disks[0] {
        let n = cut.normals[i];
        let align =
            n[0] * aperture.outward[0] + n[1] * aperture.outward[1] + n[2] * aperture.outward[2];
        assert!(align > 0.95, "disk normal {n:?} align {align:.3}");
    }
}

#[test]
fn two_open_apertures_keep_separate_disks() {
    let size = [4.0, 4.0, 4.0];
    let mesh = box_aabb(size, 16).unwrap();
    let radius = 0.05;
    let a = MouthAperture {
        position_m: [1.8, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        radius_m: radius,
    };
    let b = MouthAperture {
        position_m: [2.2, 0.0, 2.0],
        outward: [0.0, -1.0, 0.0],
        radius_m: radius,
    };
    let (cut, disks) = mesh_with_open_apertures(&mesh, &[a, b], 12).unwrap();
    assert_eq!(disks.len(), 2);
    let area_a: f64 = disks[0].iter().map(|&i| cut.areas[i]).sum();
    let area_b: f64 = disks[1].iter().map(|&i| cut.areas[i]).sum();
    let want = PI * radius * radius;
    assert!((area_a - want).abs() / want < 0.03, "A {area_a} vs {want}");
    assert!((area_b - want).abs() / want < 0.03, "B {area_b} vs {want}");
    let overlap: Vec<_> = disks[0].iter().filter(|i| disks[1].contains(i)).collect();
    assert!(overlap.is_empty(), "disks share triangles {overlap:?}");
}
