use std::collections::HashMap;

use num_complex::Complex64;
use serde_json::{Map, Value};

use crate::issue::Issue;

/// Which extension family a `kind` string belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    OuterShape,
    Radiator,
    EnclosureModel,
    RadiationModel,
    RoomModel,
    DspBlock,
}

/// A point on a mounting surface and the direction that surface faces, outward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountPose {
    pub position_m: [f64; 3],
    pub outward: [f64; 3],
}

/// What a shape answers when it has not implemented part of the contract yet.
pub const SHAPE_NOT_FINISHED: &str = "not finished yet";

fn not_finished<T>() -> Result<T, String> {
    Err(SHAPE_NOT_FINISHED.to_string())
}

/// The outer-shape contract of README section 3.3. Every question a shape cannot answer
/// returns an error that says it is not finished; nothing ever falls back to a default box.
pub trait OuterShape: Send + Sync {
    fn interior_volume_m3(&self) -> Result<f64, String>;
    fn baffle_width_m(&self) -> f64;

    /// Bounding size of the outside, `[x, y, z]` in m. The origin is the outer
    /// front-bottom-left corner (README section 4).
    fn outer_size_m(&self) -> Result<[f64; 3], String> {
        not_finished()
    }

    /// The named mounting surfaces.
    fn surfaces(&self) -> Vec<String> {
        Vec::new()
    }

    /// Position and outward direction of a point on a surface, given in that surface's
    /// own coordinates.
    fn mount_pose(&self, _surface: &str, _at_m: [f64; 2]) -> Result<MountPose, String> {
        not_finished()
    }

    /// The surface whose outward direction is closest to `outward`, if one faces that way.
    fn surface_facing(&self, _outward: [f64; 3]) -> Option<String> {
        None
    }

    /// Corners of a flat surface's outline, in order around it. A curved edge is many
    /// short straight edges. Edge diffraction samples this outline (README section 5.6).
    fn surface_outline(&self, _surface: &str) -> Result<Vec<[f64; 3]>, String> {
        not_finished()
    }

    /// True when the point is in the cabinet's air. The voxel grid samples this.
    fn contains_air(&self, _point_m: [f64; 3]) -> Result<bool, String> {
        not_finished()
    }

    /// Thickness of the panel a mount on `surface` is cut through.
    fn panel_thickness_m(&self, _surface: &str) -> Result<f64, String> {
        not_finished()
    }

    /// Printed solid volume of one cabinet, with cut-outs `(surface, area in m^2)` removed.
    fn print_solid_m3(&self, _cutouts: &[(String, f64)]) -> Result<f64, String> {
        not_finished()
    }

    /// Stock pieces cut for one cabinet: `(stock id, cut length in m)`.
    fn stock_cuts(&self) -> Vec<(String, f64)> {
        Vec::new()
    }

    /// Flat panels cut from sheet stock for one cabinet: `(stock id, name, [a, b] in m)`.
    fn stock_panels(&self) -> Vec<(String, String, [f64; 2])> {
        Vec::new()
    }

    /// Sizes `[x, y, z]` in m of the separately printed parts, for fitting them to the printer.
    fn printed_part_sizes_m(&self) -> Vec<[f64; 3]> {
        self.outer_size_m()
            .map(|size| vec![size])
            .unwrap_or_default()
    }

    /// Plain-language notes about numbers the shape had to take from the user, and similar.
    fn notes(&self) -> Vec<String> {
        Vec::new()
    }

    /// The printed parts as a constructive solid geometry tree in JSON, with each cut-out
    /// given as `(pose, diameter in m)`. The page builds it with manifold-3d (README 6.3).
    fn print_tree(&self, _cutouts: &[(MountPose, f64)]) -> Result<Value, String> {
        not_finished()
    }

    /// True when the point is in the printed plastic of the cabinet's walls, before any
    /// cut-out. Alignment pins for print splitting are placed where this holds.
    fn contains_solid(&self, _point_m: [f64; 3]) -> Result<bool, String> {
        not_finished()
    }

    /// True when the point is inside the cabinet's outer surface: its air, its walls of any
    /// material, or its printed parts. The 3D solver treats every other point as outside air.
    fn contains_cabinet(&self, _point_m: [f64; 3]) -> Result<bool, String> {
        not_finished()
    }

    /// The cabinet's inside air as a CSG tree. Partition walls between chambers are this air
    /// minus the chambers (README section 6.2).
    fn air_tree(&self) -> Result<Value, String> {
        not_finished()
    }
}

/// A radiator evaluated at 1 V RMS, before the DSP chain.
pub struct RadiatorEval {
    pub volume_velocity: Vec<Complex64>,
    pub impedance: Vec<Complex64>,
    pub excursion_m: Vec<f64>,
    pub current_a: Vec<Complex64>,
    pub re_ohm: f64,
    pub xmax_m: f64,
    pub pe_w: f64,
    pub has_excursion_limit: bool,
    /// False for a driver with a sealed back: the box does not load it, and it has no port.
    pub loads_box: bool,
    pub warnings: Vec<Issue>,
}

pub trait Radiator: Send + Sync {
    fn evaluate(&self, freqs_hz: &[f64], z_ab: &[Complex64]) -> RadiatorEval;
}

pub trait EnclosureModel: Send + Sync {
    fn z_ab(&self, volume_m3: f64, freqs_hz: &[f64]) -> Vec<Complex64>;
}

/// A patch of moving surface on the cabinet (README section 5.6): where it is, which way it
/// faces, and its area. Its volume velocity is given beside it, per frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourcePatch {
    pub position_m: [f64; 3],
    pub outward: [f64; 3],
    pub area_m2: f64,
}

impl SourcePatch {
    /// Radius of the round piston with this patch's area.
    pub fn radius_m(&self) -> f64 {
        (self.area_m2.max(0.0) / std::f64::consts::PI).sqrt()
    }
}

/// What a radiation model may ask about the cabinet and the request.
pub struct RadiationScene<'a> {
    pub shape: &'a dyn OuterShape,
    /// The request's `space`: `full`, `half`, or `step` (README section 5.6).
    pub space: &'a str,
    pub baffle_width_m: f64,
    /// The point the listening distance is measured from: the middle of the cabinet's front.
    /// The listener is on its `-y` side.
    pub reference_m: [f64; 3],
}

/// The sound each source sends to one point.
#[derive(Clone, Debug, Default)]
pub struct Transfer {
    /// `[source][frequency]`: pressure at the point per unit volume velocity of that source.
    pub per_source: Vec<Vec<Complex64>>,
    /// Plain-language lines for the fidelity label: what the model could not represent.
    pub notes: Vec<String>,
}

/// Radiated sound power, in W at each frequency, with fidelity notes.
#[derive(Clone, Debug, Default)]
pub struct SoundPower {
    pub power_w: Vec<f64>,
    pub notes: Vec<String>,
}

/// `progress(done, total)`; returning false asks a long job to stop.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize) -> bool;

pub trait RadiationModel: Send + Sync {
    /// Simple sources at one point (README section 5.6, `point`): the summed pressure of
    /// `sources`, each a volume velocity per frequency, at `distance_m`.
    fn pressure(
        &self,
        sources: &[Vec<Complex64>],
        freqs_hz: &[f64],
        distance_m: f64,
        space: &str,
        baffle_width_m: f64,
    ) -> Vec<Complex64>;

    /// Pressure at `point_m` per unit volume velocity of each positioned source, around the
    /// cabinet's outer shape. A model that does not write this treats every source as a simple
    /// source at the scene's reference point, through [`RadiationModel::pressure`].
    fn transfer(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        freqs_hz: &[f64],
        point_m: [f64; 3],
        _progress: Progress,
    ) -> Result<Transfer, String> {
        let unit = vec![vec![Complex64::new(1.0, 0.0); freqs_hz.len()]];
        let distance = distance_m(point_m, scene.reference_m).max(1e-6);
        let one = self.pressure(&unit, freqs_hz, distance, scene.space, scene.baffle_width_m);
        Ok(Transfer {
            per_source: vec![one; sources.len()],
            notes: Vec::new(),
        })
    }

    /// Radiated sound power for sources with these volume velocities, `[source][frequency]`.
    /// A model that does not write this integrates `|p|^2 / (RHO0*C0)` over a far sphere, from
    /// its own [`RadiationModel::transfer`] in [`POWER_DIRECTIONS`] directions.
    fn sound_power(
        &self,
        scene: &RadiationScene,
        sources: &[SourcePatch],
        volume_velocity: &[Vec<Complex64>],
        freqs_hz: &[f64],
        progress: Progress,
    ) -> Result<SoundPower, String> {
        sound_power_by_directions(self, scene, sources, volume_velocity, freqs_hz, progress)
    }
}

/// Directions used by the default sound power integral.
pub const POWER_DIRECTIONS: usize = 48;

fn distance_m(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `count` directions spread evenly over the sphere (a Fibonacci spiral), each with weight
/// `1 / count`.
pub fn sphere_directions(count: usize) -> Vec<[f64; 3]> {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    (0..count)
        .map(|index| {
            let z = 1.0 - (2.0 * index as f64 + 1.0) / count as f64;
            let ring = (1.0 - z * z).max(0.0).sqrt();
            let angle = golden * index as f64;
            [ring * angle.cos(), ring * angle.sin(), z]
        })
        .collect()
}

/// Sound power as the far-field integral of `|p|^2 / (RHO0*C0)` (README section 5.6).
///
/// With a few sources, the geometry's part of the integral (how each pair of sources adds up
/// over all directions) is computed on every sixth frequency and interpolated, then combined
/// with the volume velocities on the full grid, so a sharp filter in a source's drive is kept.
/// With many sources the integral is taken directly at every frequency.
#[allow(clippy::needless_range_loop)]
pub fn sound_power_by_directions<M: RadiationModel + ?Sized>(
    model: &M,
    scene: &RadiationScene,
    sources: &[SourcePatch],
    volume_velocity: &[Vec<Complex64>],
    freqs_hz: &[f64],
    progress: Progress,
) -> Result<SoundPower, String> {
    use crate::constants::{C0, RHO0};
    const FAR_M: f64 = 100.0;
    let directions = sphere_directions(POWER_DIRECTIONS);
    let area_each = 4.0 * std::f64::consts::PI * FAR_M * FAR_M / directions.len() as f64;
    let scale = area_each / (RHO0 * C0);
    let count = sources.len();
    let bins = freqs_hz.len();
    let mut notes: Vec<String> = Vec::new();
    let mut power_w = vec![0.0; bins];
    let far_point = |direction: &[f64; 3]| {
        [
            scene.reference_m[0] + FAR_M * direction[0],
            scene.reference_m[1] + FAR_M * direction[1],
            scene.reference_m[2] + FAR_M * direction[2],
        ]
    };
    let mut keep_notes = |transfer: &Transfer| {
        for note in &transfer.notes {
            if !notes.contains(note) {
                notes.push(note.clone());
            }
        }
    };
    if count == 0 || bins == 0 {
        return Ok(SoundPower { power_w, notes });
    }
    if count > 8 {
        for (index, direction) in directions.iter().enumerate() {
            let transfer = model.transfer(
                scene,
                sources,
                freqs_hz,
                far_point(direction),
                &mut *progress,
            )?;
            keep_notes(&transfer);
            for (bin, slot) in power_w.iter_mut().enumerate() {
                let mut sum = Complex64::new(0.0, 0.0);
                for (source, row) in transfer.per_source.iter().enumerate() {
                    sum += volume_velocity[source][bin] * row[bin];
                }
                *slot += scale * sum.norm_sqr();
            }
            if !progress(index + 1, directions.len()) {
                return Err("it was canceled.".into());
            }
        }
        return Ok(SoundPower { power_w, notes });
    }
    // Coarse frequencies: every sixth, and the last.
    let mut coarse: Vec<usize> = (0..bins).step_by(6).collect();
    if *coarse.last().unwrap_or(&0) != bins - 1 {
        coarse.push(bins - 1);
    }
    let coarse_hz: Vec<f64> = coarse.iter().map(|&bin| freqs_hz[bin]).collect();
    // coupling[i][j][coarse bin], for j >= i, divided by f^2 so that it interpolates well.
    let mut coupling = vec![vec![vec![Complex64::new(0.0, 0.0); coarse.len()]; count]; count];
    for direction in &directions {
        let transfer = model.transfer(
            scene,
            sources,
            &coarse_hz,
            far_point(direction),
            &mut *progress,
        )?;
        keep_notes(&transfer);
        for i in 0..count {
            for j in i..count {
                for (bin, freq) in coarse_hz.iter().enumerate() {
                    coupling[i][j][bin] += transfer.per_source[i][bin]
                        * transfer.per_source[j][bin].conj()
                        * (scale / (freq * freq));
                }
            }
        }
    }
    let mut upper = 0usize;
    for (bin, freq) in freqs_hz.iter().enumerate() {
        while upper + 1 < coarse.len() && coarse_hz[upper] < *freq {
            upper += 1;
        }
        let lower = upper.saturating_sub(1);
        let t = if upper == lower || coarse_hz[upper] <= coarse_hz[lower] {
            0.0
        } else {
            ((freq / coarse_hz[lower]).ln() / (coarse_hz[upper] / coarse_hz[lower]).ln())
                .clamp(0.0, 1.0)
        };
        let mut total = 0.0;
        for i in 0..count {
            for j in i..count {
                let c = coupling[i][j][lower] * (1.0 - t) + coupling[i][j][upper] * t;
                let pair = volume_velocity[i][bin] * volume_velocity[j][bin].conj() * c;
                total += if i == j { pair.re } else { 2.0 * pair.re };
            }
        }
        power_w[bin] = (total * freq * freq).max(0.0);
    }
    Ok(SoundPower { power_w, notes })
}

/// A mirror image of the listening position that a room asks the radiation model about, and
/// the factor its sound is scaled by (a wall's reflection).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomImage {
    pub point_m: [f64; 3],
    pub gain: f64,
}

/// What a room model is given for the whole speaker (README section 5.10).
pub struct RoomSound<'a> {
    pub freqs_hz: &'a [f64],
    /// The direct sound at the listening position.
    pub direct: &'a [Complex64],
    /// The speaker's sound at each image the room asked for, before the image's gain.
    pub images: &'a [Vec<Complex64>],
    /// Radiated sound power in W; empty when the room did not ask for it.
    pub power_w: &'a [f64],
}

/// A room's answer: what adds to the direct sound as pressure, and what adds as energy.
#[derive(Clone, Debug, Default)]
pub struct InRoom {
    /// Reflections that add to the direct sound as complex pressure.
    pub reflection: Vec<Complex64>,
    /// RMS pressure of the diffuse sound, which adds as energy.
    pub diffuse_pa: Vec<f64>,
}

pub trait RoomModel: Send + Sync {
    fn at_listener(&self, direct: &[Complex64]) -> Vec<Complex64>;

    /// Mirror images of the listening position at which the room needs the speaker's sound.
    fn images(&self, _listener_m: [f64; 3], _cabinet_size_m: [f64; 3]) -> Vec<RoomImage> {
        Vec::new()
    }

    /// True when the room needs the speaker's radiated sound power.
    fn needs_sound_power(&self) -> bool {
        false
    }

    /// The room's part of the sound at the listening position. `None` means no room: only
    /// the direct sound (the `free` member).
    fn in_room(&self, _sound: &RoomSound) -> Option<InRoom> {
        None
    }

    /// Plain-language lines for the fidelity label.
    fn notes(&self) -> Vec<String> {
        Vec::new()
    }
}

pub trait DspBlock: Send + Sync {
    fn frequency_response(&self, freqs_hz: &[f64], sample_rate_hz: f64) -> Vec<Complex64>;
    fn process(&mut self, input: &[f64], output: &mut [f64]);
    fn reset(&mut self);
}

type OuterCtor = fn(&Map<String, Value>) -> Result<Box<dyn OuterShape>, String>;
type RadiatorCtor = fn(&Map<String, Value>) -> Result<Box<dyn Radiator>, String>;
type EnclosureCtor = fn(&Map<String, Value>) -> Result<Box<dyn EnclosureModel>, String>;
type RadiationCtor = fn(&Map<String, Value>) -> Result<Box<dyn RadiationModel>, String>;
type RoomCtor = fn(&Map<String, Value>) -> Result<Box<dyn RoomModel>, String>;
type DspCtor = fn(&Map<String, Value>) -> Result<Box<dyn DspBlock>, String>;

/// Maps a kind string to the constructor that parses its raw fields.
#[derive(Default)]
pub struct Registry {
    outer: HashMap<String, OuterCtor>,
    radiator: HashMap<String, RadiatorCtor>,
    enclosure: HashMap<String, EnclosureCtor>,
    radiation: HashMap<String, RadiationCtor>,
    room: HashMap<String, RoomCtor>,
    dsp: HashMap<String, DspCtor>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_outer(&mut self, kind: &str, ctor: OuterCtor) {
        self.outer.insert(kind.to_string(), ctor);
    }

    pub fn register_radiator(&mut self, kind: &str, ctor: RadiatorCtor) {
        self.radiator.insert(kind.to_string(), ctor);
    }

    pub fn register_enclosure(&mut self, kind: &str, ctor: EnclosureCtor) {
        self.enclosure.insert(kind.to_string(), ctor);
    }

    pub fn register_radiation(&mut self, kind: &str, ctor: RadiationCtor) {
        self.radiation.insert(kind.to_string(), ctor);
    }

    pub fn register_room(&mut self, kind: &str, ctor: RoomCtor) {
        self.room.insert(kind.to_string(), ctor);
    }

    pub fn register_dsp(&mut self, kind: &str, ctor: DspCtor) {
        self.dsp.insert(kind.to_string(), ctor);
    }

    pub fn knows(&self, family: Family, kind: &str) -> bool {
        match family {
            Family::OuterShape => self.outer.contains_key(kind),
            Family::Radiator => self.radiator.contains_key(kind),
            Family::EnclosureModel => self.enclosure.contains_key(kind),
            Family::RadiationModel => self.radiation.contains_key(kind),
            Family::RoomModel => self.room.contains_key(kind),
            Family::DspBlock => self.dsp.contains_key(kind),
        }
    }

    pub fn create_outer(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn OuterShape>, String> {
        self.outer
            .get(kind)
            .ok_or_else(|| format!("unknown outer shape {kind}"))?(fields)
    }

    pub fn create_radiator(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn Radiator>, String> {
        self.radiator
            .get(kind)
            .ok_or_else(|| format!("unknown radiator {kind}"))?(fields)
    }

    pub fn create_enclosure(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn EnclosureModel>, String> {
        self.enclosure
            .get(kind)
            .ok_or_else(|| format!("unknown enclosure {kind}"))?(fields)
    }

    pub fn create_radiation(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn RadiationModel>, String> {
        self.radiation
            .get(kind)
            .ok_or_else(|| format!("unknown radiation {kind}"))?(fields)
    }

    pub fn create_room(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn RoomModel>, String> {
        self.room
            .get(kind)
            .ok_or_else(|| format!("unknown room {kind}"))?(fields)
    }

    pub fn create_dsp(
        &self,
        kind: &str,
        fields: &Map<String, Value>,
    ) -> Result<Box<dyn DspBlock>, String> {
        self.dsp
            .get(kind)
            .ok_or_else(|| format!("unknown dsp block {kind}"))?(fields)
    }
}
