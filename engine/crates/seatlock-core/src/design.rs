use serde::{Deserialize, Serialize};

#[cfg(feature = "typescript")]
use ts_rs::TS;

/// Level 1 design file root (README section 4.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/SpeakerSection.ts")
)]
pub struct SpeakerSection {
    pub enclosure: EnclosureSection,
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub radiators: Vec<serde_json::Value>,
    pub listening: ListeningSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(
        export,
        export_to = "../../../../web/src/generated/EnclosureSection.ts"
    )
)]
pub struct EnclosureSection {
    #[cfg_attr(feature = "typescript", ts(type = "Record<string, unknown>"))]
    pub outer: serde_json::Value,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub chambers: Vec<serde_json::Value>,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub ducts: Vec<serde_json::Value>,
    pub mounts: Vec<MountRecord>,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub solids: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/DspSection.ts")
)]
pub struct DspSection {
    pub sample_rate_hz: u32,
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub whole_speaker: Vec<serde_json::Value>,
    pub per_radiator: Vec<PerRadiatorDsp>,
    pub correction: CorrectionSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/PerRadiatorDsp.ts")
)]
pub struct PerRadiatorDsp {
    #[cfg_attr(feature = "typescript", ts(type = "Array<Record<string, unknown>>"))]
    pub blocks: Vec<serde_json::Value>,
    #[serde(default)]
    pub delay_s: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub invert: bool,
    #[cfg_attr(feature = "typescript", ts(type = "Record<string, unknown> | null"))]
    pub limiter: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
