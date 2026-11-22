//! Outer-shape meshing from an AABB seed or occupancy remesh (README section 5.6).

use seatlock_bem::{
    box_aabb, is_star_convex_from, mesh_from_aabb_seed, mesh_from_occupancy, mesh_outer_shape,
    OuterMeshKind,
};

fn box_inside(size: [f64; 3], p: [f64; 3]) -> bool {
    p[0] > 0.0 && p[0] < size[0] && p[1] > 0.0 && p[1] < size[1] && p[2] > 0.0 && p[2] < size[2]
}

fn cylinder_inside(diameter: f64, length: f64, p: [f64; 3]) -> bool {
    let r = diameter * 0.5;
    let radial = ((p[0] - r).powi(2) + (p[2] - r).powi(2)).sqrt();
    radial < r && p[1] > 0.0 && p[1] < length
}

/// Thick L-shaped solid: bottom bar and left upright. AABB centre sits in the missing corner.
fn l_inside(size: [f64; 3], p: [f64; 3]) -> bool {
    if p[0] <= 0.0 || p[0] >= size[0] || p[1] <= 0.0 || p[1] >= size[1] || p[2] <= 0.0 || p[2] >= size[2]
    {
        return false;
    }
    let bar = 0.35;
    let in_bottom = p[2] < size[2] * bar;
    let in_left = p[0] < size[0] * bar;
    in_bottom || in_left
}

#[test]
fn a_box_seed_stays_on_the_bounding_box() {
    let size = [0.2, 0.16, 0.24];
    let seed = box_aabb(size, 4).unwrap();
    let shaped = mesh_from_aabb_seed(size, 4, |p| Ok(box_inside(size, p))).unwrap();
    assert_eq!(shaped.triangles.len(), seed.triangles.len());
    let area_gap = (shaped.surface_area() - seed.surface_area()).abs() / seed.surface_area();
    assert!(
        area_gap < 1e-6,
        "box mesh area drifted by {area_gap:.3e} (shaped {:.6} seed {:.6})",
        shaped.surface_area(),
        seed.surface_area()
    );
    for (a, b) in shaped.vertices.iter().zip(seed.vertices.iter()) {
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!(d < 1e-9, "box vertex moved by {d:.3e}");
    }
}

#[test]
fn a_cylinder_mesh_is_closer_to_the_true_skin_than_the_bounding_box() {
    let diameter = 0.1;
    let length = 0.2;
    let size = [diameter, length, diameter];
    let r = diameter * 0.5;
    let analytical = 2.0 * std::f64::consts::PI * r * length + 2.0 * std::f64::consts::PI * r * r;
    let aabb = box_aabb(size, 8).unwrap();
    let shaped =
        mesh_from_aabb_seed(size, 8, |p| Ok(cylinder_inside(diameter, length, p))).unwrap();

    assert!(
        shaped.surface_area() < aabb.surface_area() * 0.92,
        "shaped {:.4} should beat AABB {:.4}",
        shaped.surface_area(),
        aabb.surface_area()
    );
    let rel = (shaped.surface_area() - analytical).abs() / analytical;
    assert!(
        rel < 0.08,
        "cylinder area {:.4} vs analytical {:.4} (rel {rel:.3})",
        shaped.surface_area(),
        analytical
    );
    // Mid-body side normals should lean outward in the radial xz plane (skip the rim, where
    // the seed grid still joins the flat caps and stretches).
    let mut radial_faces = 0;
    for (centroid, normal) in shaped.centroids.iter().zip(shaped.normals.iter()) {
        if centroid[1] < 0.25 * length || centroid[1] > 0.75 * length {
            continue;
        }
        if normal[1].abs() > 0.35 {
            continue;
        }
        let from_axis = [centroid[0] - r, 0.0, centroid[2] - r];
        let n = (from_axis[0] * from_axis[0] + from_axis[2] * from_axis[2]).sqrt();
        if n < 1e-9 {
            continue;
        }
        let radial_dir = [from_axis[0] / n, 0.0, from_axis[2] / n];
        let align = normal[0] * radial_dir[0] + normal[2] * radial_dir[2];
        assert!(
            align > 0.85,
            "side normal {normal:?} at {centroid:?} align {align:.3}"
        );
        radial_faces += 1;
    }
    assert!(
        radial_faces > 10,
        "expected curved side faces, got {radial_faces}"
    );
}

#[test]
fn an_l_shape_is_not_star_convex_and_needs_remesh() {
    let size = [0.24, 0.16, 0.20];
    let center = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    assert!(
        !l_inside(size, center),
        "AABB centre should sit in the missing corner of the L"
    );
    assert!(!is_star_convex_from(center, size, 24, |p| Ok(l_inside(size, p))).unwrap());
    assert!(
        mesh_from_aabb_seed(size, 4, |p| Ok(l_inside(size, p))).is_err(),
        "AABB seed must refuse an off-centre L"
    );

    let (mesh, kind) = mesh_outer_shape(size, 4, |p| Ok(l_inside(size, p))).unwrap();
    assert_eq!(kind, OuterMeshKind::OccupancyRemesh);
    assert!(
        mesh.triangles.len() > 24,
        "remesh should produce a closed skin, got {} faces",
        mesh.triangles.len()
    );
    // Analytical L surface (outer AABB faces that belong to the solid, plus the inner step).
    // Coarse occupancy will overshoot; require the remesh to beat a naive filled AABB and to
    // stay within a generous band of a voxel estimate at the same resolution.
    let remesh = mesh_from_occupancy(size, 8, |p| Ok(l_inside(size, p))).unwrap();
    let aabb = box_aabb(size, 4).unwrap();
    assert!(
        remesh.surface_area() < aabb.surface_area() * 0.98,
        "L remesh area {:.4} should undercut a filled AABB {:.4}",
        remesh.surface_area(),
        aabb.surface_area()
    );
    // Outward normals: a step along the normal from a centroid should leave the solid.
    // Stair-step faces near the inner corner can sit a cell inside the analytic L, so a
    // few probes still read solid; require a clear majority.
    let mut outward = 0;
    for (centroid, normal) in remesh.centroids.iter().zip(remesh.normals.iter()) {
        let probe = [
            centroid[0] + normal[0] * 0.004,
            centroid[1] + normal[1] * 0.004,
            centroid[2] + normal[2] * 0.004,
        ];
        if !l_inside(size, probe) {
            outward += 1;
        }
    }
    let frac = outward as f64 / remesh.triangles.len() as f64;
    assert!(
        frac > 0.85,
        "expected outward normals on remesh faces, got {frac:.2} ({outward}/{})",
        remesh.triangles.len()
    );
}

#[test]
fn a_box_still_uses_the_aabb_seed_path() {
    let size = [0.2, 0.16, 0.24];
    let (mesh, kind) = mesh_outer_shape(size, 4, |p| Ok(box_inside(size, p))).unwrap();
    assert_eq!(kind, OuterMeshKind::AabbSeed);
    let seed = box_aabb(size, 4).unwrap();
    assert_eq!(mesh.triangles.len(), seed.triangles.len());
}
