//! The `bem` radiation model (README section 5.6): exterior CBIE on a mesh of the cabinet's
//! outer shape, with baffle fallback above the mesh's frequency limit.

use num_complex::Complex64;
use seatlock_acoustics::BaffleRadiation;
use seatlock_bem::{
    field_pressure, mean_edge_length, mesh_from_aabb_seed, NeumannSystem, TriangleMesh,
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
     outer shape from a bounding-box seed; deeply re-entrant skins are not yet remeshed.";
pub const CHIEF_NOTE: &str = "Boundary-element outside: CHIEF interior points keep irregular \
     frequencies nonsingular on constant elements.";
pub const OPENINGS_NOTE: &str = "Boundary-element outside: openings are rigid patches that \
     move, not true mouths.";

pub struct BemRadiation {
    pub subdivisions: u32,
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

/// Triangles that belong to a source: facing the same way and near its piston.
fn triangles_of_source(mesh: &TriangleMesh, source: &SourcePatch) -> Vec<usize> {
    if source.outward == [0.0; 3] || source.area_m2 <= 0.0 {
        return Vec::new();
    }
    let radius = source.radius_m().max(1e-4);
    let reach = radius * 1.5 + mean_triangle_scale(mesh) * 0.75;
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

    fn mesh_for(&self, scene: &RadiationScene) -> Result<(TriangleMesh, f64, f64), String> {
        let size = scene.shape.outer_size_m()?;
        let mesh =
            mesh_from_aabb_seed(size, self.subdivisions, |p| scene.shape.contains_cabinet(p))?;
        let edge = mean_edge_length(&mesh);
        Ok((mesh, edge, valid_hz(edge)))
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
        let (mesh, _edge, valid) = self.mesh_for(scene)?;
        let bins: Vec<usize> = (0..freqs_hz.len())
            .filter(|&bin| freqs_hz[bin] > 0.0 && freqs_hz[bin] <= valid)
            .collect();
        if bins.is_empty() {
            return Ok(transfers);
        }

        let placements: Vec<Option<Vec<Complex64>>> = sources
            .iter()
            .map(|source| unit_velocity(&mesh, &triangles_of_source(&mesh, source)))
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

        let notes = vec![
            self.label(valid),
            SHAPE_MESH_NOTE.to_string(),
            CHIEF_NOTE.to_string(),
            OPENINGS_NOTE.to_string(),
        ];
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
