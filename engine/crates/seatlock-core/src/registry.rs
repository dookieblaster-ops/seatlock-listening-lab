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

pub trait OuterShape: Send + Sync {
    fn interior_volume_m3(&self) -> Result<f64, String>;
    fn baffle_width_m(&self) -> f64;
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
