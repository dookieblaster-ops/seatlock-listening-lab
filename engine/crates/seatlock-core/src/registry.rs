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

pub trait RadiationModel: Send + Sync {
    fn pressure(
        &self,
        sources: &[Vec<Complex64>],
        freqs_hz: &[f64],
        distance_m: f64,
        space: &str,
        baffle_width_m: f64,
    ) -> Vec<Complex64>;
}

pub trait RoomModel: Send + Sync {
    fn at_listener(&self, direct: &[Complex64]) -> Vec<Complex64>;
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
