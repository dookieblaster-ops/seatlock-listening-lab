//! Closed triangle meshes for the exterior BEM.

#[derive(Clone, Debug)]
pub struct TriangleMesh {
    pub vertices: Vec<[f64; 3]>,
    /// Each triangle is three vertex indices, outward winding.
    pub triangles: Vec<[usize; 3]>,
    pub centroids: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub areas: Vec<f64>,
}

impl TriangleMesh {
    pub fn from_triangles(
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[usize; 3]>,
    ) -> Result<Self, String> {
        if triangles.is_empty() {
            return Err("mesh has no triangles".into());
        }
        let mut centroids = Vec::with_capacity(triangles.len());
        let mut normals = Vec::with_capacity(triangles.len());
        let mut areas = Vec::with_capacity(triangles.len());
        for tri in &triangles {
            let a = vertices[tri[0]];
            let b = vertices[tri[1]];
            let c = vertices[tri[2]];
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cx = ab[1] * ac[2] - ab[2] * ac[1];
            let cy = ab[2] * ac[0] - ab[0] * ac[2];
            let cz = ab[0] * ac[1] - ab[1] * ac[0];
            let twice = (cx * cx + cy * cy + cz * cz).sqrt();
            if twice < 1e-18 {
                return Err("degenerate triangle".into());
            }
            areas.push(0.5 * twice);
            normals.push([cx / twice, cy / twice, cz / twice]);
            centroids.push([
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ]);
        }
        Ok(Self {
            vertices,
            triangles,
            centroids,
            normals,
            areas,
        })
    }

    pub fn corners(&self, tri: usize) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let t = self.triangles[tri];
        (
            self.vertices[t[0]],
            self.vertices[t[1]],
            self.vertices[t[2]],
        )
    }

    pub fn surface_area(&self) -> f64 {
        self.areas.iter().sum()
    }
}

/// Unit icosphere of `radius_m`, subdivided `subdivisions` times from a regular icosahedron
/// (0 → 20 faces, 1 → 80, 2 → 320, 3 → 1280).
pub fn icosphere(radius_m: f64, subdivisions: u32) -> Result<TriangleMesh, String> {
    if radius_m <= 0.0 {
        return Err("radius must be positive".into());
    }
    let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let mut verts = vec![
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    for v in &mut verts {
        *v = normalize(*v);
    }
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..subdivisions {
        let mut next = Vec::with_capacity(faces.len() * 4);
        let mut midpoints = std::collections::HashMap::new();
        for face in &faces {
            let a = face[0];
            let b = face[1];
            let c = face[2];
            let ab = midpoint_index(a, b, &mut verts, &mut midpoints);
            let bc = midpoint_index(b, c, &mut verts, &mut midpoints);
            let ca = midpoint_index(c, a, &mut verts, &mut midpoints);
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        faces = next;
    }
    for v in &mut verts {
        *v = scale(normalize(*v), radius_m);
    }
    // Orient outward: centroid of each face should see positive normal · position.
    for face in &mut faces {
        let a = verts[face[0]];
        let b = verts[face[1]];
        let c = verts[face[2]];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let center = [
            (a[0] + b[0] + c[0]) / 3.0,
            (a[1] + b[1] + c[1]) / 3.0,
            (a[2] + b[2] + c[2]) / 3.0,
        ];
        if n[0] * center[0] + n[1] * center[1] + n[2] * center[2] < 0.0 {
            face.swap(1, 2);
        }
    }
    TriangleMesh::from_triangles(verts, faces)
}

fn midpoint_index(
    a: usize,
    b: usize,
    verts: &mut Vec<[f64; 3]>,
    midpoints: &mut std::collections::HashMap<(usize, usize), usize>,
) -> usize {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&index) = midpoints.get(&key) {
        return index;
    }
    let pa = verts[a];
    let pb = verts[b];
    let mid = normalize([
        (pa[0] + pb[0]) * 0.5,
        (pa[1] + pb[1]) * 0.5,
        (pa[2] + pb[2]) * 0.5,
    ]);
    let index = verts.len();
    verts.push(mid);
    midpoints.insert(key, index);
    index
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// Axis-aligned box from the lab origin (front-bottom-left) to `size_m = [x, y, z]`.
///
/// Each face is split into `subdivisions × subdivisions` quads (two triangles each).
/// Outward normals: front `y = 0` faces `-y` (toward the listener).
pub fn box_aabb(size_m: [f64; 3], subdivisions: u32) -> Result<TriangleMesh, String> {
    if size_m.iter().any(|&v| v <= 0.0) {
        return Err("box size must be positive".into());
    }
    let n = subdivisions.max(1) as usize;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    // Face: (origin corner of the face, u-axis edge, v-axis edge), outward = u × v.
    let faces: [([f64; 3], [f64; 3], [f64; 3]); 6] = [
        // Front y=0, outward -y: u = +x, v = +z.
        (
            [0.0, 0.0, 0.0],
            [size_m[0], 0.0, 0.0],
            [0.0, 0.0, size_m[2]],
        ),
        // Back y=size_y, outward +y: u = -x (so u×v = +y), v = +z from right edge.
        (
            [size_m[0], size_m[1], 0.0],
            [-size_m[0], 0.0, 0.0],
            [0.0, 0.0, size_m[2]],
        ),
        // Left x=0, outward -x: u = -y, v = +z.
        (
            [0.0, size_m[1], 0.0],
            [0.0, -size_m[1], 0.0],
            [0.0, 0.0, size_m[2]],
        ),
        // Right x=size_x, outward +x: u = +y, v = +z.
        (
            [size_m[0], 0.0, 0.0],
            [0.0, size_m[1], 0.0],
            [0.0, 0.0, size_m[2]],
        ),
        // Bottom z=0, outward -z: u = +x, v = -y.
        (
            [0.0, size_m[1], 0.0],
            [size_m[0], 0.0, 0.0],
            [0.0, -size_m[1], 0.0],
        ),
        // Top z=size_z, outward +z: u = +x, v = +y.
        (
            [0.0, 0.0, size_m[2]],
            [size_m[0], 0.0, 0.0],
            [0.0, size_m[1], 0.0],
        ),
    ];
    for &(origin, u_edge, v_edge) in &faces {
        let base = vertices.len();
        for iv in 0..=n {
            let fv = iv as f64 / n as f64;
            for iu in 0..=n {
                let fu = iu as f64 / n as f64;
                vertices.push([
                    origin[0] + fu * u_edge[0] + fv * v_edge[0],
                    origin[1] + fu * u_edge[1] + fv * v_edge[1],
                    origin[2] + fu * u_edge[2] + fv * v_edge[2],
                ]);
            }
        }
        let row = n + 1;
        for iv in 0..n {
            for iu in 0..n {
                let i00 = base + iv * row + iu;
                let i10 = i00 + 1;
                let i01 = i00 + row;
                let i11 = i01 + 1;
                triangles.push([i00, i10, i11]);
                triangles.push([i00, i11, i01]);
            }
        }
    }
    TriangleMesh::from_triangles(vertices, triangles)
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
        a[2] + t * (b[2] - a[2]),
    ]
}

fn length3(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Triangulate an outer shape by seeding the axis-aligned bounding box and pulling every
/// vertex that sits outside the true skin inward onto it.
///
/// `inside` is the cabinet occupancy test (`OuterShape::contains_cabinet`). Box-like shapes
/// whose skin fills the AABB keep the seed mesh. Curved and chamfered skins move the
/// overhanging vertices onto the surface along the ray from the box centre. Star-convex
/// cabinets from that centre are the intended case; a later mesh pass can replace this for
/// deeply re-entrant imported shells.
pub fn mesh_from_aabb_seed<F>(
    size_m: [f64; 3],
    subdivisions: u32,
    mut inside: F,
) -> Result<TriangleMesh, String>
where
    F: FnMut([f64; 3]) -> Result<bool, String>,
{
    let mut seed = box_aabb(size_m, subdivisions)?;
    let center = [size_m[0] * 0.5, size_m[1] * 0.5, size_m[2] * 0.5];
    if !inside(center)? {
        return Err(
            "The cabinet's centre is outside its outer shape, so the boundary-element mesh \
             cannot be built from the bounding box."
                .into(),
        );
    }
    let scale = size_m.iter().copied().fold(0.0_f64, f64::max).max(1e-6);
    let eps = 1e-7 * scale;
    let probe_step = 1e-5 * scale;

    for vertex in &mut seed.vertices {
        let to_center = [
            center[0] - vertex[0],
            center[1] - vertex[1],
            center[2] - vertex[2],
        ];
        let dist = length3(to_center);
        if dist <= eps {
            continue;
        }
        let dir = [
            to_center[0] / dist,
            to_center[1] / dist,
            to_center[2] / dist,
        ];
        let probe = [
            vertex[0] + dir[0] * probe_step,
            vertex[1] + dir[1] * probe_step,
            vertex[2] + dir[2] * probe_step,
        ];
        // Vertex already lies on a face that fills the AABB: keep it.
        if inside(probe)? {
            continue;
        }
        // Search the segment from the centre (inside) to a point just past the seed vertex.
        let far = lerp3(center, *vertex, 1.02);
        if inside(far).unwrap_or(true) {
            // Seed vertex was not past the skin; leave it.
            continue;
        }
        let mut lo = 0.0;
        let mut hi = 1.0;
        for _ in 0..48 {
            let mid = 0.5 * (lo + hi);
            if inside(lerp3(center, far, mid))? {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        *vertex = lerp3(center, far, 0.5 * (lo + hi));
    }

    // Centroids, normals, and areas must follow the moved vertices.
    TriangleMesh::from_triangles(seed.vertices, seed.triangles)
}

/// Mean edge length over every triangle (for the baffle frequency hand-over).
pub fn mean_edge_length(mesh: &TriangleMesh) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for tri in &mesh.triangles {
        let a = mesh.vertices[tri[0]];
        let b = mesh.vertices[tri[1]];
        let c = mesh.vertices[tri[2]];
        sum += length3([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
        sum += length3([c[0] - b[0], c[1] - b[1], c[2] - b[2]]);
        sum += length3([a[0] - c[0], a[1] - c[1], a[2] - c[2]]);
        count += 3.0;
    }
    if count <= 0.0 {
        0.0
    } else {
        sum / count
    }
}
