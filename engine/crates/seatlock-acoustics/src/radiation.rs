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
    baffle_shadow_pressure, baffle_source_pressure, outline_count_for_perimeter,
    outline_perimeter_m, polygon_outline, EdgePoint,
};
use crate::point::{baffle_gain, pressure_from_volume_velocity};

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

    fn transfer(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
        _progress: Progress,
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

    fn transfer(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
        _progress: Progress,
    ) -> Result<Transfer, String> {
        let highest = freqs_hz.iter().copied().fold(1000.0_f64, f64::max);
        let mut outlines: Vec<(String, Vec<EdgePoint>)> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut per_source = Vec::with_capacity(sources.len());
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
            let Some(outline) = outline else {
                if !notes.iter().any(|note| note == POINT_FALLBACK_NOTE) {
                    notes.push(POINT_FALLBACK_NOTE.into());
                }
                per_source.push(point_row(scene, freqs_hz, point_m));
                continue;
            };
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
        Ok(Transfer { per_source, notes })
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
