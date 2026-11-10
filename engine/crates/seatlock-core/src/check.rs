use serde_json::Value;

use crate::design::{canonical_json, DesignFile};
use crate::issue::Issue;
use crate::registry::{Family, Registry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub issues: Vec<Issue>,
    pub canonical_json: String,
}

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn design_check(text: &str, registry: &Registry) -> Result<CheckResult, String> {
    let value: Value =
        serde_json::from_str(text.trim()).map_err(|err| format!("Invalid JSON: {err}"))?;
    let design: DesignFile = match serde_json::from_value(value.clone()) {
        Ok(design) => design,
        Err(err) => {
            let mut pretty =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            if !pretty.ends_with('\n') {
                pretty.push('\n');
            }
            return Ok(CheckResult {
                issues: vec![Issue::error(
                    "missing_field",
                    format!("The design file is missing something it needs: {err}"),
                    field_from_serde(&err.to_string()),
                )],
                canonical_json: pretty,
            });
        }
    };

    let mut issues = Vec::new();
    warn_kind(
        registry,
        Family::OuterShape,
        &design.speaker.enclosure.outer.kind,
        "speaker.enclosure.outer",
        "outer shape",
        &mut issues,
    );
    for (index, radiator) in design.speaker.radiators.iter().enumerate() {
        warn_kind(
            registry,
            Family::Radiator,
            &radiator.kind,
            &format!("speaker.radiators[{index}]"),
            "radiator",
            &mut issues,
        );
    }
    if let Some(room) = &design.speaker.room {
        warn_kind(
            registry,
            Family::RoomModel,
            &room.kind,
            "speaker.room",
            "room",
            &mut issues,
        );
    }
    for (index, block) in design.dsp.whole_speaker.iter().enumerate() {
        warn_kind(
            registry,
            Family::DspBlock,
            &block.kind,
            &format!("dsp.whole_speaker[{index}]"),
            "DSP block",
            &mut issues,
        );
    }
    for (chain_index, chain) in design.dsp.per_radiator.iter().enumerate() {
        for (block_index, block) in chain.blocks.iter().enumerate() {
            warn_kind(
                registry,
                Family::DspBlock,
                &block.kind,
                &format!("dsp.per_radiator[{chain_index}].blocks[{block_index}]"),
                "DSP block",
                &mut issues,
            );
        }
    }

    Ok(CheckResult {
        issues,
        canonical_json: canonical_json(&design)?,
    })
}

fn warn_kind(
    registry: &Registry,
    family: Family,
    kind: &str,
    field: &str,
    family_name: &str,
    issues: &mut Vec<Issue>,
) {
    if registry.knows(family, kind) {
        return;
    }
    issues.push(Issue::warning(
        "unsupported_kind",
        format!(
            "This build does not know the {family_name} \"{kind}\". It stays in the file and is left out of the results."
        ),
        field,
    ));
}

fn field_from_serde(message: &str) -> String {
    message
        .split("missing field `")
        .nth(1)
        .and_then(|rest| rest.split('`').next())
        .unwrap_or("")
        .to_string()
}
