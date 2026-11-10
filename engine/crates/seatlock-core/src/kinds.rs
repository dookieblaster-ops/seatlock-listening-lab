use std::collections::HashSet;

/// Known `kind` strings per extensible family at Level 0 (README section 3.3).
#[derive(Debug, Clone)]
pub struct KindRegistry {
    outer_shape: HashSet<&'static str>,
    radiator: HashSet<&'static str>,
    room: HashSet<&'static str>,
    dsp_block: HashSet<&'static str>,
}

impl Default for KindRegistry {
    fn default() -> Self {
        Self::level0()
    }
}

impl KindRegistry {
    pub fn level0() -> Self {
        Self {
            outer_shape: HashSet::from(["box"]),
            radiator: HashSet::from(["cone"]),
            room: HashSet::from(["free"]),
            dsp_block: HashSet::from(["biquad", "gain"]),
        }
    }

    pub fn is_known_outer_shape(&self, kind: &str) -> bool {
        self.outer_shape.contains(kind)
    }

    pub fn is_known_radiator(&self, kind: &str) -> bool {
        self.radiator.contains(kind)
    }

    pub fn is_known_room(&self, kind: &str) -> bool {
        self.room.contains(kind)
    }

    pub fn is_known_dsp_block(&self, kind: &str) -> bool {
        self.dsp_block.contains(kind)
    }
}
