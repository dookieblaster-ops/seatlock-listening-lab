//! The `bem` radiation model (README section 5.6): exterior CBIE on a mesh of the cabinet's
//! outer shape, with baffle fallback above the mesh's frequency limit. Mouth radiation
//! impedances (self and mutual) from the same solve join the waveguide network (README 2.6).

use num_complex::Complex64;
use seatlock_acoustics::BaffleRadiation;
use seatlock_bem::{
    field_pressure, mean_edge_length, mesh_outer_shape, mesh_with_open_apertures, MouthAperture,
    NeumannSystem, OuterMeshKind, TriangleMesh,
};
use seatlock_core::constants::C0;
use seatlock_core::registry::{
    Progress, RadiationModel, RadiationScene, SoundPower, SourcePatch, Transfer,
};
use serde_json::{Map, Value};

/// Default face subdivisions for the AABB seed grid (each face → `n²` quads → `2 n²` triangles).
pub const DEFAULT_SUBDIVISIONS: u32 = 4;
/// Aim for at least this many elements along one wavelength before handing over to baffle.
const ELEMENTS_PER_WAVELENGTH: f64 = 6.0;

pub const SHAPE_MESH_NOTE: &str = "Boundary-element outside: the mesh follows the cabinet's \
     outer shape from a bounding-box seed when that shell is star-convex from the box centre; \
     re-entrant skins are remeshed from an occupancy grid.";
pub const REMESH_NOTE: &str = "Boundary-element outside: re-entrant outer shape remeshed from \
     an occupancy grid (not the bounding-box seed).";
pub const CHIEF_NOTE: &str = "Boundary-element outside: CHIEF interior points keep irregular \
     frequencies nonsingular on constant elements.";
pub const OPENINGS_NOTE: &str = "Boundary-element outside: openings are true open apertures \
     cut into the outer mesh (exact-area disks), not rigid skin patches.";
pub const NETWORK_NOTE: &str = "Boundary-element outside: each mouth's radiation impedance \
     matrix from the mesh (self and mutual) replaces the analytic radiation loads in the \
     detailed air network.";
/// Rim segments on each open-aperture disk (CHK-BEM-10).
const APERTURE_SEGMENTS: usize = 16;

pub struct BemRadiation {
    pub subdivisions: u32,
}

/// Radiation impedance matrix of the mouth apertures: `Z[i][j] = p_avg on mouth i` under unit
/// volume velocity on mouth j alone (others rigid). One matrix per frequency
/// (`out[bin][i][j]`). Above the mesh frequency limit the diagonal is the analytic flush
/// impedance and mutual terms are zero. Mouths are true open apertures cut into the mesh.
pub fn mouth_impedance_matrices(
    scene: &RadiationScene,
    mouths: &[SourcePatch],
    freqs_hz: &[f64],
    subdivisions: u32,
) -> Result<Vec<Vec<Vec<Complex64>>>, String> {
    let n = mouths.len();
    let mut out = vec![vec![vec![Complex64::new(0.0, 0.0); n]; n]; freqs_hz.len()];
    if n == 0 || freqs_hz.is_empty() {
        return Ok(out);
    }
    let (mesh, mouth_tris, _kind) = open_mouth_mesh(scene, mouths, subdivisions)?;
    let edge = mean_edge_length(&mesh);
    let valid = valid_hz(edge);
    let placements: Vec<Option<Vec<Complex64>>> = mouth_tris
        .iter()
        .map(|tris| unit_velocity(&mesh, tris))
        .collect();

    for (bin, &freq) in freqs_hz.iter().enumerate() {
        if freq <= 0.0 {
            continue;
        }
        if freq > valid {
            let k = 2.0 * std::f64::consts::PI * freq / C0;
            for (i, mouth) in mouths.iter().enumerate() {
                out[bin][i][i] = seatlock_acoustics::radiation_impedance(mouth.area_m2, true, k);
            }
            continue;
        }
        let system = NeumannSystem::assemble(&mesh, freq)?;
        for (j, placement) in placements.iter().enumerate() {
            let Some(v_n) = placement else {
                let k = 2.0 * std::f64::consts::PI * freq / C0;
                out[bin][j][j] =
                    seatlock_acoustics::radiation_impedance(mouths[j].area_m2, true, k);
                continue;
            };
            let p_surf = system.solve_velocity(v_n)?;
            for (i, tris) in mouth_tris.iter().enumerate() {
                if tris.is_empty() {
                    if i == j {
                        let k = 2.0 * std::f64::consts::PI * freq / C0;
                        out[bin][i][i] =
                            seatlock_acoustics::radiation_impedance(mouths[i].area_m2, true, k);
                    }
                    continue;
                }
                out[bin][i][j] = area_mean_pressure(&mesh, tris, &p_surf);
            }
        }
    }
    Ok(out)
}

/// Outer-shape mesh with each mouth cut as an open aperture disk. Returns the mesh, the
/// disk triangle indices per mouth, and whether the skin was remeshed.
pub fn open_mouth_mesh(
    scene: &RadiationScene,
    mouths: &[SourcePatch],
    subdivisions: u32,
) -> Result<(TriangleMesh, Vec<Vec<usize>>, OuterMeshKind), String> {
    let size = scene.shape.outer_size_m()?;
    let (seed, kind) = mesh_outer_shape(size, subdivisions, |p| scene.shape.contains_cabinet(p))?;
    let apertures: Vec<MouthAperture> = mouths
        .iter()
        .filter(|m| m.area_m2 > 0.0 && m.outward != [0.0; 3])
        .map(|m| MouthAperture {
            position_m: m.position_m,
            outward: m.outward,
            radius_m: m.radius_m().max(1e-4),
        })
        .collect();
    if apertures.is_empty() {
        return Ok((seed, vec![Vec::new(); mouths.len()], kind));
    }
    let (mesh, disks) = mesh_with_open_apertures(&seed, &apertures, APERTURE_SEGMENTS)?;
    // Map back onto the original mouth list (skipped empty mouths stay empty).
    let mut out = vec![Vec::new(); mouths.len()];
    let mut disk_i = 0;
    for (i, mouth) in mouths.iter().enumerate() {
        if mouth.area_m2 > 0.0 && mouth.outward != [0.0; 3] {
            out[i] = disks[disk_i].clone();
            disk_i += 1;
        }
    }
    Ok((mesh, out, kind))
}

/// Diagonal of [`mouth_impedance_matrices`] (self-impedance per mouth per frequency).
pub fn mouth_self_impedances(
    scene: &RadiationScene,
    mouths: &[SourcePatch],
    freqs_hz: &[f64],
    subdivisions: u32,
) -> Result<Vec<Vec<Complex64>>, String> {
    let matrices = mouth_impedance_matrices(scene, mouths, freqs_hz, subdivisions)?;
    let n = mouths.len();
    let mut out = vec![vec![Complex64::new(0.0, 0.0); freqs_hz.len()]; n];
    for (bin, matrix) in matrices.iter().enumerate() {
        for i in 0..n {
            out[i][bin] = matrix[i][i];
        }
    }
    Ok(out)
}

fn area_mean_pressure(mesh: &TriangleMesh, triangles: &[usize], p: &[Complex64]) -> Complex64 {
    let mut sum = Complex64::new(0.0, 0.0);
    let mut area = 0.0;
    for &i in triangles {
        sum += p[i] * mesh.areas[i];
        area += mesh.areas[i];
    }
    if area <= 0.0 {
        Complex64::new(0.0, 0.0)
    } else {
        sum / area
    }
}

pub fn parse(fields: &Map<String, Value>) -> Result<Box<dyn RadiationModel>, String> {
    let subdivisions = fields
        .get("subdivisions")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(DEFAULT_SUBDIVISIONS);
    if !(1..=16).contains(&subdivisions) {
        return Err(format!(
            "BEM subdivisions of {subdivisions} are outside the 1 to 16 this solver takes."
        ));
    }
    Ok(Box::new(BemRadiation { subdivisions }))
}

fn push_note(notes: &mut Vec<String>, note: &str) {
    if !notes.iter().any(|line| line == note) {
        notes.push(note.to_string())
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn valid_hz(edge_m: f64) -> f64 {
    C0 / (ELEMENTS_PER_WAVELENGTH * edge_m.max(1e-6))
}

/// Triangles that belong to a field source: facing the same way and near its piston.
fn triangles_of_source(mesh: &TriangleMesh, source: &SourcePatch) -> Vec<usize> {
    triangles_near(
        mesh,
        source,
        source.radius_m().max(1e-4) * 1.5 + mean_triangle_scale(mesh) * 0.75,
    )
}

fn triangles_near(mesh: &TriangleMesh, source: &SourcePatch, reach: f64) -> Vec<usize> {
    if source.outward == [0.0; 3] || source.area_m2 <= 0.0 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (index, (centroid, normal)) in mesh.centroids.iter().zip(mesh.normals.iter()).enumerate() {
        if dot(*normal, source.outward) < 0.5 {
            continue;
        }
        if dist(*centroid, source.position_m) <= reach {
            hits.push(index);
        }
    }
    if hits.is_empty() {
        // Nearest same-facing triangle.
        let mut best: Option<(usize, f64)> = None;
        for (index, (centroid, normal)) in
            mesh.centroids.iter().zip(mesh.normals.iter()).enumerate()
        {
            if dot(*normal, source.outward) < 0.5 {
                continue;
            }
            let d = dist(*centroid, source.position_m);
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((index, d));
            }
        }
        if let Some((index, _)) = best {
            hits.push(index);
        }
    }
    hits
}

fn mean_triangle_scale(mesh: &TriangleMesh) -> f64 {
    let mean_area = mesh.surface_area() / mesh.triangles.len().max(1) as f64;
    mean_area.sqrt()
}

/// Unit volume velocity on the source's triangles (elsewhere zero).
fn unit_velocity(mesh: &TriangleMesh, triangles: &[usize]) -> Option<Vec<Complex64>> {
    if triangles.is_empty() {
        return None;
    }
    let area: f64 = triangles.iter().map(|&i| mesh.areas[i]).sum();
    if area <= 0.0 {
        return None;
    }
    let mut v_n = vec![Complex64::new(0.0, 0.0); mesh.triangles.len()];
    let speed = 1.0 / area;
    for &i in triangles {
        v_n[i] = Complex64::new(speed, 0.0);
    }
    Some(v_n)
}

impl BemRadiation {
    fn label(&self, valid: f64) -> String {
        format!(
            "Boundary-element outside on the outer shape ({sub}×{sub} seed per face) up to {valid:.0} Hz",
            sub = self.subdivisions,
            valid = valid
        )
    }

    fn mesh_for(
        &self,
        scene: &RadiationScene,
    ) -> Result<(TriangleMesh, OuterMeshKind, f64, f64), String> {
        let size = scene.shape.outer_size_m()?;
        let (mesh, kind) =
            mesh_outer_shape(size, self.subdivisions, |p| scene.shape.contains_cabinet(p))?;
        let edge = mean_edge_length(&mesh);
        Ok((mesh, kind, edge, valid_hz(edge)))
    }
}

impl RadiationModel for BemRadiation {
    fn pressure(
        &self,
        sources: &[Vec<Complex64>],
        freqs_hz: &[f64],
        distance_m: f64,
        space: &str,
        baffle_width_m: f64,
    ) -> Vec<Complex64> {
        BaffleRadiation.pressure(sources, freqs_hz, distance_m, space, baffle_width_m)
    }

    fn transfer_one(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
    ) -> Result<Transfer, String> {
        self.transfer(scene, sources, freqs_hz, point_m, &mut (|_, _| true))
    }

    fn transfer(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
        progress: Progress,
    ) -> Result<Transfer, String> {
        let mut many = self.transfer_many(scene, sources, freqs_hz, &[point_m], progress)?;
        many.pop()
            .ok_or_else(|| "radiation transfer returned no point.".to_string())
    }

    fn transfer_many(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        points_m: &[[f64; 3]],
        progress: Progress,
    ) -> Result<Vec<Transfer>, String> {
        let mut transfers =
            BaffleRadiation.transfer_many(scene, sources, freqs_hz, points_m, &mut *progress)?;
        if sources.is_empty() || points_m.is_empty() {
            return Ok(transfers);
        }
        // Cut every finite directed source as an open aperture so pistons and mouths share the
        // same exact-area disks (CHK-BEM-10). Empty / unplaced sources keep the baffle value.
        let directed: Vec<SourcePatch> = sources
            .iter()
            .filter(|s| s.area_m2 > 0.0 && s.outward != [0.0; 3])
            .cloned()
            .collect();
        let (mesh, disks, kind) = if directed.is_empty() {
            let (mesh, kind, _edge, _valid) = self.mesh_for(scene)?;
            (mesh, Vec::new(), kind)
        } else {
            open_mouth_mesh(scene, &directed, self.subdivisions)?
        };
        let edge = mean_edge_length(&mesh);
        let valid = valid_hz(edge);
        let bins: Vec<usize> = (0..freqs_hz.len())
            .filter(|&bin| freqs_hz[bin] > 0.0 && freqs_hz[bin] <= valid)
            .collect();
        if bins.is_empty() {
            return Ok(transfers);
        }

        let mut disk_i = 0;
        let placements: Vec<Option<Vec<Complex64>>> = sources
            .iter()
            .map(|source| {
                if source.area_m2 > 0.0 && source.outward != [0.0; 3] {
                    let tris = disks.get(disk_i).cloned().unwrap_or_default();
                    disk_i += 1;
                    unit_velocity(&mesh, &tris)
                        .or_else(|| unit_velocity(&mesh, &triangles_of_source(&mesh, source)))
                } else {
                    None
                }
            })
            .collect();

        let total_steps = bins.len().max(1);
        for (step, &bin) in bins.iter().enumerate() {
            let freq = freqs_hz[bin];
            let system = NeumannSystem::assemble(&mesh, freq)?;
            for (source, placement) in placements.iter().enumerate() {
                let Some(v_n) = placement else {
                    continue;
                };
                // One surface solve per source, then the representation formula at every point.
                let p_surf = system.solve_velocity(v_n)?;
                let omega = 2.0 * std::f64::consts::PI * freq;
                let q: Vec<_> = v_n
                    .iter()
                    .map(|&v| Complex64::new(0.0, -omega * seatlock_core::constants::RHO0) * v)
                    .collect();
                for (point_index, &point_m) in points_m.iter().enumerate() {
                    let value = field_pressure(&mesh, system.k, &p_surf, &q, point_m)?;
                    transfers[point_index].per_source[source][bin] = value;
                }
            }
            if !progress(step + 1, total_steps) {
                return Err("it was canceled.".into());
            }
        }

        let mut notes = vec![
            self.label(valid),
            SHAPE_MESH_NOTE.to_string(),
            CHIEF_NOTE.to_string(),
            OPENINGS_NOTE.to_string(),
        ];
        if kind == OuterMeshKind::OccupancyRemesh {
            notes.insert(1, REMESH_NOTE.to_string());
        }
        for transfer in &mut transfers {
            let mut merged = notes.clone();
            for note in transfer.notes.drain(..) {
                push_note(&mut merged, &note);
            }
            transfer.notes = merged;
        }
        Ok(transfers)
    }

    fn sound_power(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        volume_velocity: &[Vec<Complex64>],
        freqs_hz: &[f64],
        progress: Progress,
    ) -> Result<SoundPower, String> {
        // First slice: keep the baffle power integral. Directional pressure uses BEM above.
        let mut power =
            BaffleRadiation.sound_power(scene, sources, volume_velocity, freqs_hz, progress)?;
        push_note(
            &mut power.notes,
            "Boundary-element outside: sound power still uses the baffle integral.",
        );
        Ok(power)
    }
}
