#![cfg(feature = "typescript")]

use std::path::PathBuf;

use seatlock_core::design::{
    BuildSection, CorrectionSettings, DesignFile, DspSection, ElectronicsSection, EnclosureSection,
    ListeningSection, MountRecord, PerRadiatorDsp, SpeakerSection,
};
use seatlock_core::issue::Issue;
use ts_rs::TS;

#[test]
fn export_typescript_bindings() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../web/src/generated");
    std::fs::create_dir_all(&out).expect("create generated dir");

    DesignFile::export().expect("export DesignFile");
    Issue::export().expect("export Issue");
    SpeakerSection::export().expect("export SpeakerSection");
    EnclosureSection::export().expect("export EnclosureSection");
    MountRecord::export().expect("export MountRecord");
    ListeningSection::export().expect("export ListeningSection");
    DspSection::export().expect("export DspSection");
    PerRadiatorDsp::export().expect("export PerRadiatorDsp");
    CorrectionSettings::export().expect("export CorrectionSettings");
    ElectronicsSection::export().expect("export ElectronicsSection");
    BuildSection::export().expect("export BuildSection");
}
