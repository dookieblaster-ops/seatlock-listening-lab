use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[cfg(feature = "typescript")]
use ts_rs::TS;

/// A part that comes in kinds: the kind string plus whatever fields that kind uses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/KindPart.ts")
)]
pub struct KindPart {
    pub kind: String,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub fields: Map<String, Value>,
}

/// Level 1 design file. Unknown keys on every struct are kept and written back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/DesignFile.ts")
)]
pub struct DesignFile {
    pub schema_version: u32,
    pub name: String,
    pub speaker: SpeakerSection,
    pub dsp: DspSection,
    pub electronics: ElectronicsSection,
    pub build: BuildSection,
    pub target: String,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/SpeakerSection.ts")
)]
pub struct SpeakerSection {
    pub enclosure: EnclosureSection,
    pub radiators: Vec<KindPart>,
    pub listening: ListeningSection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<KindPart>,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(
        export,
        export_to = "../../../../web/src/generated/EnclosureSection.ts"
    )
)]
pub struct EnclosureSection {
    pub outer: KindPart,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub chambers: Vec<Value>,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub ducts: Vec<Value>,
    pub mounts: Vec<MountRecord>,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub solids: Vec<Value>,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/MountRecord.ts")
)]
pub struct MountRecord {
    pub id: String,
    pub surface: String,
    pub at_m: [f64; 2],
    pub rear_into: String,
    #[serde(default)]
    pub recess_m: f64,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(
        export,
        export_to = "../../../../web/src/generated/ListeningSection.ts"
    )
)]
pub struct ListeningSection {
    pub distance_m: f64,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/DspSection.ts")
)]
pub struct DspSection {
    pub sample_rate_hz: u32,
    #[serde(default)]
    pub whole_speaker: Vec<KindPart>,
    pub per_radiator: Vec<PerRadiatorDsp>,
    pub correction: CorrectionSettings,
    /// Passive R/L/C network from one amplifier to the drivers (README section 7). When set,
    /// it sets each radiator's drive voltage; digital crossover blocks on each radiator are
    /// left out of the graphs (gain, delay, and polarity still apply).
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Record<string, unknown> | null"))]
    pub passive: Option<Value>,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/PerRadiatorDsp.ts")
)]
pub struct PerRadiatorDsp {
    pub blocks: Vec<KindPart>,
    #[serde(default)]
    pub delay_s: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub invert: bool,
    #[cfg_attr(feature = "typescript", ts(type = "Record<string, unknown> | null"))]
    pub limiter: Option<Value>,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(
        export,
        export_to = "../../../../web/src/generated/CorrectionSettings.ts"
    )
)]
pub struct CorrectionSettings {
    pub enabled: bool,
    pub protected_band_hz: [f64; 2],
    pub max_filters: u32,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(
        export,
        export_to = "../../../../web/src/generated/ElectronicsSection.ts"
    )
)]
pub struct ElectronicsSection {
    pub amplifiers: Vec<String>,
    pub dsp_device: Option<String>,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/BuildSection.ts")
)]
pub struct BuildSection {
    pub material: String,
    pub infill_fraction: f64,
    pub stereo_pair: bool,
    #[serde(default)]
    pub extras_cost: f64,
    #[serde(default, flatten)]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub extra: Map<String, Value>,
}

pub fn canonical_json(design: &DesignFile) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(design).map_err(|err| err.to_string())?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}
