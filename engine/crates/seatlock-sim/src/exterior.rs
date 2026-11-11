//! The `exterior` radiation model (README section 5.6): the 3D solver on the air outside the
//! cabinet, by reciprocity. One run per listening point gives the pressure there from every
//! surface source at once.

use num_complex::Complex64;
use seatlock_acoustics::BaffleRadiation;
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::{
    Progress, RadiationModel, RadiationScene, SoundPower, SourcePatch, Transfer,
};
use seatlock_fdtd::exterior::{skin_transfer, Domain, Incident, Options};
use serde_json::{Map, Value};
use std::f64::consts::PI;

/// Air kept around the cabinet on every side, in m.
pub const MARGIN_M: f64 = 0.2;
/// Beyond this distance the listening point's wave is taken as plane at the cabinet.
const FAR_FIELD_M: f64 = 20.0;
const FAR_M: f64 = 100.0;

pub const STAIR_STEP_NOTE: &str = "Full 3D outside: the cabinet is rigid, and curved or slanted \
     surfaces are stair-stepped, which can read up to 3 dB high in the deep shadow of a curved \
     cabinet near the grid's limit.";
pub const POWER_NOTE: &str =
    "Full 3D outside: sound power comes from 14 directions around the cabinet.";

pub struct ExteriorRadiation {
    pub dx_m: f64,
}

pub fn parse(fields: &Map<String, Value>) -> Result<Box<dyn RadiationModel>, String> {
    let dx_m = fields.get("dx_m").and_then(Value::as_f64).unwrap_or(0.01);
    if !(0.002..=0.05).contains(&dx_m) {
        return Err(format!(
            "a cell of {:.1} mm is outside the 2 to 50 mm this solver takes.",
            dx_m * 1000.0
        ));
    }
    Ok(Box::new(ExteriorRadiation { dx_m }))
}

/// The cabinet as a rigid body in its block of air, and the skin cells in front of each source.
struct Body {
    domain: Domain,
    record: Vec<usize>,
    /// Per source: its cells' places in `record`, or `None` for a source without a direction.
    cells_of: Vec<Option<std::ops::Range<usize>>>,
}

/// One source's transfer at each frequency, or `None` for a source without a direction.
type PlacedRow = Option<Vec<Complex64>>;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl ExteriorRadiation {
    fn body(&self, scene: &RadiationScene, sources: &[SourcePatch]) -> Result<Body, String> {
        let size = scene.shape.outer_size_m()?;
        let dx = self.dx_m;
        let mut failure: Option<String> = None;
        let mut solid = |point: [f64; 3]| match scene.shape.contains_cabinet(point) {
            Ok(inside) => inside,
            Err(err) => {
                failure.get_or_insert(err);
                false
            }
        };
        let domain = Domain::around([0.0; 3], size, MARGIN_M, dx, &mut solid);
        if let Some(err) = failure {
            return Err(err);
        }
        // Skin cells with the directions of their solid neighbors.
        let skin: Vec<(usize, [f64; 3], Vec<[f64; 3]>)> = domain
            .skin_cells()
            .into_iter()
            .map(|cell| {
                let behind = domain
                    .solid_neighbors(cell)
                    .into_iter()
                    .map(|(axis, low_side)| {
                        let mut toward = [0.0; 3];
                        toward[axis as usize] = if low_side { -1.0 } else { 1.0 };
                        toward
                    })
                    .collect();
                (cell, domain.center_of(cell), behind)
            })
            .collect();
        if skin.is_empty() {
            return Err("the cabinet is too small for cells of this size.".into());
        }
        let mut record = Vec::new();
        let mut cells_of = Vec::with_capacity(sources.len());
        for source in sources {
            if source.outward == [0.0; 3] {
                cells_of.push(None);
                continue;
            }
            let start = record.len();
            let reach = source.radius_m();
            // (lateral distance, cell) of the nearest skin cell in front of the source's surface.
            let mut nearest: Option<(f64, usize)> = None;
            for (cell, center, behind) in &skin {
                // The body must be behind the cell as the source sees it.
                if !behind
                    .iter()
                    .any(|toward| dot(*toward, source.outward) < -0.3)
                {
                    continue;
                }
                let offset = [
                    center[0] - source.position_m[0],
                    center[1] - source.position_m[1],
                    center[2] - source.position_m[2],
                ];
                let height = dot(offset, source.outward);
                if !(-0.75 * dx..=1.75 * dx).contains(&height) {
                    continue;
                }
                let lateral = (dot(offset, offset) - height * height).max(0.0).sqrt();
                if lateral <= reach {
                    record.push(*cell);
                }
                if nearest.is_none_or(|(best, _)| lateral < best) {
                    nearest = Some((lateral, *cell));
                }
            }
            if record.len() == start {
                // A source smaller than a cell: the one cell nearest to it.
                match nearest {
                    Some((lateral, cell)) if lateral <= reach.max(2.0 * dx) => record.push(cell),
                    _ => {
                        return Err(format!(
                            "a sound source at ({:.3}, {:.3}, {:.3}) m is not on the cabinet's \
                             outer surface as the 3D grid sees it.",
                            source.position_m[0], source.position_m[1], source.position_m[2]
                        ))
                    }
                }
            }
            cells_of.push(Some(start..record.len()));
        }
        Ok(Body {
            domain,
            record,
            cells_of,
        })
    }

    /// One reciprocity run: pressure at the incident wave's source per unit volume velocity of
    /// each placed surface source, at `freqs_hz`.
    fn run(
        &self,
        body: &Body,
        incident: &Incident,
        freqs_hz: &[f64],
        progress: &mut dyn FnMut(usize, usize) -> bool,
    ) -> Result<(Vec<PlacedRow>, bool), String> {
        let run = skin_transfer(
            &body.domain,
            incident,
            &body.record,
            &Options {
                frequencies_hz: freqs_hz.to_vec(),
                progress_every: 50,
            },
            progress,
        );
        if run.canceled {
            return Err("it was canceled.".into());
        }
        let rows = body
            .cells_of
            .iter()
            .map(|cells| {
                cells.as_ref().map(|cells| {
                    let count = cells.len() as f64;
                    (0..freqs_hz.len())
                        .map(|bin| {
                            cells
                                .clone()
                                .map(|index| run.transfer[index][bin])
                                .sum::<Complex64>()
                                / count
                        })
                        .collect()
                })
            })
            .collect();
        Ok((rows, run.decayed))
    }

    fn label(&self) -> String {
        format!(
            "Full 3D outside the cabinet at {:.0} mm cells up to {:.0} Hz; above that the baffle \
             model.",
            self.dx_m * 1000.0,
            C0 / (10.0 * self.dx_m)
        )
    }
}

fn incident_from(scene: &RadiationScene, point_m: [f64; 3]) -> Incident {
    let offset = [
        point_m[0] - scene.reference_m[0],
        point_m[1] - scene.reference_m[1],
        point_m[2] - scene.reference_m[2],
    ];
    let range = dot(offset, offset).sqrt();
    if range > FAR_FIELD_M {
        Incident::Plane {
            toward: [offset[0] / range, offset[1] / range, offset[2] / range],
            distance_m: range,
            reference_m: scene.reference_m,
        }
    } else {
        Incident::Point { at_m: point_m }
    }
}

fn push_note(notes: &mut Vec<String>, note: &str) {
    if !notes.iter().any(|line| line == note) {
        notes.push(note.to_string());
    }
}

const NOT_DECAYED_NOTE: &str = "Full 3D outside: a run reached its time limit before the sound \
     died away, so its values are less certain.";

impl RadiationModel for ExteriorRadiation {
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

    fn transfer(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
        progress: Progress,
    ) -> Result<Transfer, String> {
        // The baffle model everywhere first; the 3D result then replaces what the grid resolves.
        let mut transfer =
            BaffleRadiation.transfer(scene, sources, freqs_hz, point_m, &mut *progress)?;
        let valid = C0 / (10.0 * self.dx_m);
        let bins: Vec<usize> = (0..freqs_hz.len())
            .filter(|&bin| freqs_hz[bin] <= valid)
            .collect();
        if bins.is_empty() || sources.is_empty() {
            return Ok(transfer);
        }
        let resolved: Vec<f64> = bins.iter().map(|&bin| freqs_hz[bin]).collect();
        let body = self.body(scene, sources)?;
        let (rows, decayed) =
            self.run(&body, &incident_from(scene, point_m), &resolved, progress)?;
        for (source, row) in rows.into_iter().enumerate() {
            if let Some(row) = row {
                for (slot, &bin) in bins.iter().enumerate() {
                    transfer.per_source[source][bin] = row[slot];
                }
            }
        }
        // The baffle model's own limits now apply only above the grid's limit.
        let mut notes = vec![self.label(), STAIR_STEP_NOTE.to_string()];
        if !decayed {
            notes.push(NOT_DECAYED_NOTE.to_string());
        }
        for note in transfer.notes.drain(..) {
            push_note(&mut notes, &note);
        }
        transfer.notes = notes;
        Ok(transfer)
    }

    /// Fourteen plane-wave runs (the six axis directions and the eight diagonals, with the
    /// weights that integrate exactly up to fifth-order patterns), then `|p|^2 / (RHO0*C0)`
    /// summed over the far sphere. Above the grid's limit, the baffle model's sound power.
    fn sound_power(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        volume_velocity: &[Vec<Complex64>],
        freqs_hz: &[f64],
        progress: Progress,
    ) -> Result<SoundPower, String> {
        let valid = C0 / (10.0 * self.dx_m);
        let bins: Vec<usize> = (0..freqs_hz.len())
            .filter(|&bin| freqs_hz[bin] <= valid)
            .collect();
        let mut power = BaffleRadiation.sound_power(
            scene,
            sources,
            volume_velocity,
            freqs_hz,
            &mut *progress,
        )?;
        if bins.is_empty() || sources.is_empty() {
            return Ok(power);
        }
        let resolved: Vec<f64> = bins.iter().map(|&bin| freqs_hz[bin]).collect();
        let body = self.body(scene, sources)?;
        let diagonal = 1.0 / 3.0_f64.sqrt();
        let mut directions: Vec<([f64; 3], f64)> = Vec::new();
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut direction = [0.0; 3];
                direction[axis] = sign;
                directions.push((direction, 1.0 / 15.0));
            }
        }
        for corner in 0..8 {
            let sign = |bit: usize| {
                if corner >> bit & 1 == 1 {
                    diagonal
                } else {
                    -diagonal
                }
            };
            directions.push(([sign(0), sign(1), sign(2)], 3.0 / 40.0));
        }
        let sphere = 4.0 * PI * FAR_M * FAR_M / (RHO0 * C0);
        let mut total = vec![0.0; resolved.len()];
        let mut all_decayed = true;
        let count = directions.len();
        for (index, (direction, weight)) in directions.iter().enumerate() {
            let point = [
                scene.reference_m[0] + FAR_M * direction[0],
                scene.reference_m[1] + FAR_M * direction[1],
                scene.reference_m[2] + FAR_M * direction[2],
            ];
            // Sources without a direction keep the baffle model's value.
            let fallback =
                BaffleRadiation.transfer(scene, sources, &resolved, point, &mut *progress)?;
            let mut step = |steps: usize, max_steps: usize| {
                progress(index * max_steps + steps, count * max_steps)
            };
            let (rows, decayed) = self.run(
                &body,
                &Incident::Plane {
                    toward: *direction,
                    distance_m: FAR_M,
                    reference_m: scene.reference_m,
                },
                &resolved,
                &mut step,
            )?;
            all_decayed &= decayed;
            for (slot, &bin) in bins.iter().enumerate() {
                let mut sum = Complex64::new(0.0, 0.0);
                for (source, row) in rows.iter().enumerate() {
                    let unit = match row {
                        Some(row) => row[slot],
                        None => fallback.per_source[source][slot],
                    };
                    sum += volume_velocity[source][bin] * unit;
                }
                total[slot] += weight * sphere * sum.norm_sqr();
            }
        }
        for (slot, &bin) in bins.iter().enumerate() {
            power.power_w[bin] = total[slot];
        }
        let mut notes = vec![self.label(), POWER_NOTE.to_string()];
        if !all_decayed {
            notes.push(NOT_DECAYED_NOTE.to_string());
        }
        for note in power.notes.drain(..) {
            push_note(&mut notes, &note);
        }
        power.notes = notes;
        Ok(power)
    }
}
