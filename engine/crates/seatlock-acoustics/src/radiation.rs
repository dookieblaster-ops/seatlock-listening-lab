//! The `point` and `baffle` radiation models as registry members (README section 5.6).

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::{
    sound_power_by_directions, Progress, RadiationModel, RadiationScene, SoundPower, SourcePatch,
    Transfer,
};
use serde_json::{Map, Value};
use std::f64::consts::PI;

use crate::baffle::{
    baffle_shadow_pressure, baffle_source_pressure, diffraction_factor,
    outline_count_for_perimeter, outline_perimeter_m, polygon_outline, EdgePoint,
};
use crate::point::{baffle_gain, pressure_from_volume_velocity};

/// Above this many sources on one surface, their edge diffraction is taken from their middle.
const MANY_SOURCES: usize = 32;
pub const SHARED_EDGE_NOTE: &str = "A panel's many sources share one edge-diffraction estimate, \
     taken from the panel's middle.";
pub const POINT_FALLBACK_NOTE: &str =
    "A source on a surface without a flat outline is treated as a point source.";
pub const EDGE_NOTE: &str = "Edge diffraction ignores cabinet depth and the shape of the edge.";
pub const SHADOW_NOTE: &str = "Behind a source's own panel, only the sound that bends around the \
     panel's edge is counted; the cabinet's depth is ignored. That is rough for a source on a side, \
     top, or back; the full 3D outside model is the one to trust there.";

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// One simple source at the scene's reference point, heard at `point_m`. In front of the
/// reference point (the listener's side) the baffle gain is `B`; behind it, `2 - B`, so that
/// half space sends nothing backward and full space the same both ways.
fn point_row(scene: &RadiationScene, freqs_hz: &[f64], point_m: [f64; 3]) -> Vec<Complex64> {
    let range = distance(point_m, scene.reference_m).max(1e-6);
    let behind = point_m[1] > scene.reference_m[1];
    freqs_hz
        .iter()
        .map(|freq| {
            let front = pressure_from_volume_velocity(
                Complex64::new(1.0, 0.0),
                *freq,
                range,
                scene.space,
                scene.baffle_width_m,
            );
            if behind {
                let gain = baffle_gain(scene.space, *freq, scene.baffle_width_m);
                front * (Complex64::new(2.0, 0.0) - gain) / gain
            } else {
                front
            }
        })
        .collect()
}

pub struct PointRadiation;

impl RadiationModel for PointRadiation {
    fn pressure(
        &self,
        sources: &[Vec<Complex64>],
        freqs_hz: &[f64],
        distance_m: f64,
        space: &str,
        baffle_width_m: f64,
    ) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .enumerate()
            .map(|(index, freq)| {
                let total: Complex64 = sources.iter().map(|source| source[index]).sum();
                pressure_from_volume_velocity(total, *freq, distance_m, space, baffle_width_m)
            })
            .collect()
    }

    fn transfer_one(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
    ) -> Result<Transfer, String> {
        Ok(Transfer {
            per_source: vec![point_row(scene, freqs_hz, point_m); sources.len()],
            notes: Vec::new(),
        })
    }

    /// All sources sit at one point, so the power is that of their summed volume velocity:
    /// `RHO0*C0*k^2*|U|^2 * (|B|^2 + |2 - B|^2) / (8*pi)`, which is the full-space value for
    /// `B = 1` and twice that for `B = 2`.
    fn sound_power(
        &self,
        scene: &RadiationScene,
        _sources: &[SourcePatch],
        volume_velocity: &[Vec<Complex64>],
        freqs_hz: &[f64],
        _progress: Progress,
    ) -> Result<SoundPower, String> {
        let power_w = freqs_hz
            .iter()
            .enumerate()
            .map(|(bin, freq)| {
                let total: Complex64 = volume_velocity.iter().map(|source| source[bin]).sum();
                let k = 2.0 * PI * freq / C0;
                let gain = baffle_gain(scene.space, *freq, scene.baffle_width_m);
                let both = gain.norm_sqr() + (Complex64::new(2.0, 0.0) - gain).norm_sqr();
                RHO0 * C0 * k * k * total.norm_sqr() * both / (8.0 * PI)
            })
            .collect();
        Ok(SoundPower {
            power_w,
            notes: Vec::new(),
        })
    }
}

pub fn parse_point(_fields: &Map<String, Value>) -> Result<Box<dyn RadiationModel>, String> {
    Ok(Box::new(PointRadiation))
}

/// Each source at its real position on a flat surface: piston directivity and edge diffraction
/// from that surface's outline in front of the surface, and the edge's sound alone behind it.
pub struct BaffleRadiation;

/// Surface outlines shared across many listener points (sound power, listening window, CTA).
struct BaffleOutlines {
    outlines: Vec<(String, Vec<EdgePoint>)>,
    surface_of: Vec<Option<usize>>,
}

fn baffle_outlines(
    scene: &RadiationScene,
    sources: &[SourcePatch],
    freqs_hz: &[f64],
) -> BaffleOutlines {
    let highest = freqs_hz.iter().copied().fold(1000.0_f64, f64::max);
    let mut outlines: Vec<(String, Vec<EdgePoint>)> = Vec::new();
    let mut surface_of: Vec<Option<usize>> = Vec::with_capacity(sources.len());
    for source in sources {
        let placed = source.outward != [0.0; 3];
        let outline = placed
            .then(|| scene.shape.surface_facing(source.outward))
            .flatten()
            .and_then(|surface| {
                if let Some(found) = outlines.iter().position(|(name, _)| *name == surface) {
                    return Some(found);
                }
                let corners = scene.shape.surface_outline(&surface).ok()?;
                let count = outline_count_for_perimeter(outline_perimeter_m(&corners), highest);
                outlines.push((surface, polygon_outline(&corners, count)));
                Some(outlines.len() - 1)
            });
        surface_of.push(outline);
    }
    BaffleOutlines {
        outlines,
        surface_of,
    }
}

fn baffle_transfer_at(
    scene: &RadiationScene,
    sources: &[SourcePatch],
    freqs_hz: &[f64],
    point_m: [f64; 3],
    prepared: &BaffleOutlines,
) -> Transfer {
    let outlines = &prepared.outlines;
    let surface_of = &prepared.surface_of;
    let mut notes: Vec<String> = Vec::new();
    let mut per_source = Vec::with_capacity(sources.len());
    // A surface that carries many sources (a panel's grid): its edge diffraction is taken
    // once, from the middle of those sources, and each source keeps its own distance and
    // direction. Otherwise every source would need its own sum over the edge.
    let mut shared: Vec<Option<(bool, Vec<Complex64>)>> = vec![None; outlines.len()];
    for (surface, (_, edge)) in outlines.iter().enumerate() {
        let members: Vec<&SourcePatch> = sources
            .iter()
            .zip(surface_of)
            .filter(|(_, on)| **on == Some(surface))
            .map(|(source, _)| source)
            .collect();
        if members.len() <= MANY_SOURCES {
            continue;
        }
        let count = members.len() as f64;
        let mut middle = [0.0; 3];
        for member in &members {
            for (sum, part) in middle.iter_mut().zip(member.position_m) {
                *sum += part / count;
            }
        }
        let outward = members[0].outward;
        let range = distance(point_m, middle).max(1e-6);
        let in_front = (0..3)
            .map(|axis| (point_m[axis] - middle[axis]) * outward[axis])
            .sum::<f64>()
            > 0.0;
        let factor = freqs_hz
            .iter()
            .map(|freq| {
                if in_front {
                    diffraction_factor(middle, point_m, edge, *freq)
                } else {
                    let k = 2.0 * PI * freq / C0;
                    let half = Complex64::new(0.0, k * C0 * RHO0)
                        * Complex64::from_polar(2.0 / (4.0 * PI * range), -k * range);
                    baffle_shadow_pressure(Complex64::new(1.0, 0.0), *freq, middle, point_m, edge)
                        / half
                }
            })
            .collect();
        shared[surface] = Some((in_front, factor));
        if !notes.iter().any(|note| note == SHARED_EDGE_NOTE) {
            notes.push(SHARED_EDGE_NOTE.into());
        }
        if !in_front && !notes.iter().any(|note| note == SHADOW_NOTE) {
            notes.push(SHADOW_NOTE.into());
        }
    }
    for (source, outline) in sources.iter().zip(surface_of) {
        let Some(outline) = *outline else {
            if !notes.iter().any(|note| note == POINT_FALLBACK_NOTE) {
                notes.push(POINT_FALLBACK_NOTE.into());
            }
            per_source.push(point_row(scene, freqs_hz, point_m));
            continue;
        };
        if let Some((in_front, factor)) = &shared[outline] {
            let radius = if *in_front { source.radius_m() } else { 0.0 };
            per_source.push(
                freqs_hz
                    .iter()
                    .zip(factor)
                    .map(|(freq, factor)| {
                        baffle_source_pressure(
                            Complex64::new(1.0, 0.0),
                            *freq,
                            source.position_m,
                            point_m,
                            source.outward,
                            radius,
                            &[],
                        ) * *factor
                    })
                    .collect(),
            );
            continue;
        }
        let edge = &outlines[outline].1;
        let offset = [
            point_m[0] - source.position_m[0],
            point_m[1] - source.position_m[1],
            point_m[2] - source.position_m[2],
        ];
        let in_front = offset[0] * source.outward[0]
            + offset[1] * source.outward[1]
            + offset[2] * source.outward[2]
            > 0.0;
        if !in_front && !notes.iter().any(|note| note == SHADOW_NOTE) {
            notes.push(SHADOW_NOTE.into());
        }
        let unit = Complex64::new(1.0, 0.0);
        let radius = source.radius_m();
        per_source.push(
            freqs_hz
                .iter()
                .map(|freq| {
                    if in_front {
                        baffle_source_pressure(
                            unit,
                            *freq,
                            source.position_m,
                            point_m,
                            source.outward,
                            radius,
                            edge,
                        )
                    } else {
                        baffle_shadow_pressure(unit, *freq, source.position_m, point_m, edge)
                    }
                })
                .collect(),
        );
    }
    notes.push(EDGE_NOTE.into());
    Transfer { per_source, notes }
}

impl RadiationModel for BaffleRadiation {
    fn pressure(
        &self,
        sources: &[Vec<Complex64>],
        freqs_hz: &[f64],
        distance_m: f64,
        space: &str,
        baffle_width_m: f64,
    ) -> Vec<Complex64> {
        PointRadiation.pressure(sources, freqs_hz, distance_m, space, baffle_width_m)
    }

    fn transfer_one(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
    ) -> Result<Transfer, String> {
        let prepared = baffle_outlines(scene, sources, freqs_hz);
        Ok(baffle_transfer_at(
            scene, sources, freqs_hz, point_m, &prepared,
        ))
    }

    fn transfer_many(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        points_m: &[[f64; 3]],
        progress: Progress,
    ) -> Result<Vec<Transfer>, String> {
        let prepared = baffle_outlines(scene, sources, freqs_hz);
        let mut out = Vec::with_capacity(points_m.len());
        for (index, point) in points_m.iter().enumerate() {
            out.push(baffle_transfer_at(
                scene, sources, freqs_hz, *point, &prepared,
            ));
            if !progress(index + 1, points_m.len().max(1)) {
                return Err("it was canceled.".into());
            }
        }
        Ok(out)
    }

    fn sound_power(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        volume_velocity: &[Vec<Complex64>],
        freqs_hz: &[f64],
        progress: Progress,
    ) -> Result<SoundPower, String> {
        let mut power =
            sound_power_by_directions(self, scene, sources, volume_velocity, freqs_hz, progress)?;
        // Half of all directions are behind any panel; that is not a limit worth a label.
        power.notes.retain(|note| note != SHADOW_NOTE);
        Ok(power)
    }
}

pub fn parse_baffle(_fields: &Map<String, Value>) -> Result<Box<dyn RadiationModel>, String> {
    Ok(Box::new(BaffleRadiation))
}
