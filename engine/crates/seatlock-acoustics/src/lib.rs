//! Radiators, enclosure loads, radiation models, room estimate, and clean-level limits.

mod baffle;
mod cone;
pub mod dml;
pub mod network;
pub mod plate;
mod point;
mod radiation;
mod response;
mod room;
mod sealed;
mod vented;
mod waveguide;

pub use baffle::{
    baffle_shadow_pressure, baffle_source_pressure, baffle_step_hz, baffle_step_hz_for_outline,
    circular_outline, diffraction_factor, outline_count, outline_count_for_perimeter,
    outline_perimeter_m, piston_directivity, polygon_outline, rectangular_outline, EdgePoint,
};
pub use cone::{derive_mechanical, terminal_at, Mechanical, Published, Terminal};
pub use point::{baffle_gain, pressure_from_volume_velocity};
pub use radiation::{BaffleRadiation, PointRadiation, EDGE_NOTE, POINT_FALLBACK_NOTE, SHADOW_NOTE};
pub use response::blend_measured_curve;
pub use room::{absorption_area_m2, diffuse_pressure_pa, SimpleRoom};
pub use sealed::z_ab_sealed;
pub use vented::{
    effective_port_length_m, end_correction_m, tuning_hz, vented_sample, PortSpec, VentedSample,
};
pub use waveguide::{
    closed_duct_impedance, helmholtz, ported_box, solve, solve_lossy, AcousticNode,
    WaveguideResult, WALL_LOSS,
};

use num_complex::Complex64;
use seatlock_core::catalog;
use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_core::issue::Issue;
use seatlock_core::registry::{EnclosureModel, Radiator, RadiatorEval, Registry};
use serde_json::{Map, Value};
use std::f64::consts::PI;

pub fn register(registry: &mut Registry) {
    registry.register_radiator("cone", parse_cone);
    registry.register_radiator("dml_panel", dml::parse_dml_panel);
    registry.register_enclosure("lumped", parse_lumped);
    registry.register_enclosure("waveguide", parse_waveguide);
    registry.register_radiation("point", radiation::parse_point);
    registry.register_radiation("baffle", radiation::parse_baffle);
    registry.register_room("free", room::parse_free);
    registry.register_room("simple", room::parse_simple);
}

struct ConeRadiator {
    published: Published,
    missing_displacement: bool,
    driver_id: String,
    loads_box: bool,
    has_xmax: bool,
    note: Option<String>,
}

impl Radiator for ConeRadiator {
    fn evaluate(&self, freqs_hz: &[f64], z_ab: &[Complex64]) -> RadiatorEval {
        let mech = derive_mechanical(&self.published);
        let mut volume_velocity = Vec::with_capacity(freqs_hz.len());
        let mut impedance = Vec::with_capacity(freqs_hz.len());
        let mut excursion_m = Vec::with_capacity(freqs_hz.len());
        let mut current_a = Vec::with_capacity(freqs_hz.len());
        for (index, freq) in freqs_hz.iter().enumerate() {
            let zab = z_ab.get(index).copied().unwrap_or(Complex64::new(0.0, 0.0));
            let terminal = terminal_at(&self.published, &mech, zab, 1.0, *freq);
            volume_velocity.push(terminal.volume_velocity);
            impedance.push(terminal.impedance);
            excursion_m.push(terminal.excursion_m);
            current_a.push(terminal.current_a);
        }
        let mut warnings = Vec::new();
        if let Some(note) = &self.note {
            warnings.push(Issue::warning(
                "missing_field",
                note.clone(),
                format!("catalog/{}/published.vas_m3", self.driver_id),
            ));
        }
        if !self.has_xmax {
            warnings.push(Issue::warning(
                "missing_field",
                format!(
                    "Driver {} does not publish cone travel, so the maximum clean level ignores excursion.",
                    self.driver_id
                ),
                format!("catalog/{}/published.xmax_m", self.driver_id),
            ));
        }
        if self.missing_displacement {
            warnings.push(Issue::warning(
                "missing_field",
                format!(
                    "Driver {} has no published displacement, so the air volume ignores the space the driver takes up.",
                    self.driver_id
                ),
                format!("catalog/{}/mounting.displacement_m3", self.driver_id),
            ));
        }
        RadiatorEval {
            volume_velocity,
            impedance,
            excursion_m,
            current_a,
            re_ohm: self.published.re_ohm,
            xmax_m: self.published.xmax_m,
            pe_w: self.published.pe_w,
            has_excursion_limit: self.has_xmax,
            loads_box: self.loads_box,
            warnings,
        }
    }

    fn loads_box(&self) -> bool {
        self.loads_box
    }
}

pub fn cone_displacement_m3(fields: &Map<String, Value>) -> (f64, bool, String) {
    let driver_id = fields
        .get("driver_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let Some(record) = catalog::driver(&driver_id) else {
        return (0.0, true, driver_id);
    };
    match record
        .pointer("/mounting/displacement_m3")
        .and_then(Value::as_f64)
    {
        Some(value) => (value, false, driver_id),
        None => (0.0, true, driver_id),
    }
}

fn parse_cone(fields: &Map<String, Value>) -> Result<Box<dyn Radiator>, String> {
    let driver_id = fields
        .get("driver_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing driver_id".to_string())?;
    let record = catalog::driver(driver_id).ok_or_else(|| format!("unknown driver {driver_id}"))?;
    // Independent measurements, where the record has them, replace the maker's values
    // (README 9.1); the note names both.
    let (record, independent) = with_independent(&record);
    let published = published_from_catalog(&record)?;
    let has_xmax = record
        .pointer("/published/xmax_m")
        .and_then(Value::as_f64)
        .is_some();
    let (_displacement_m3, missing_displacement, driver_id) = cone_displacement_m3(fields);
    let loads_box = !record
        .get("sealed_back")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut notes = Vec::new();
    if let Some(note) = independent {
        notes.push(note);
    }
    if let Some(note) = derivation_note(&record) {
        notes.push(note);
    }
    if let Some(note) = printed_disagreement(&record) {
        notes.push(note);
    }
    if let Some(note) = sensitivity_disagreement(&record) {
        notes.push(note);
    }
    Ok(Box::new(ConeRadiator {
        published,
        missing_displacement,
        driver_id,
        loads_box,
        has_xmax,
        note: if notes.is_empty() {
            None
        } else {
            Some(notes.join(" "))
        },
    }))
}

/// Names of the values an independent measurement may replace, with how each is shown.
const MEASURED_FIELDS: [(&str, &str, f64, &str); 9] = [
    ("fs_hz", "resonance", 1.0, "Hz"),
    ("qes", "Qes", 1.0, ""),
    ("qms", "Qms", 1.0, ""),
    ("vas_m3", "Vas", 1000.0, "L"),
    ("re_ohm", "coil resistance", 1.0, "ohm"),
    ("le_h", "coil inductance", 1000.0, "mH"),
    ("sd_m2", "cone area", 10000.0, "cm^2"),
    ("xmax_m", "usable cone travel", 1000.0, "mm"),
    ("pe_w", "power handling", 1.0, "W"),
];

/// The record with the first independent measurement's values put in place of the maker's
/// (README 9.1), and a sentence saying who measured what. Values the measurement does not
/// give stay the maker's. A record without `independent` comes back unchanged.
pub fn with_independent(record: &Value) -> (Value, Option<String>) {
    let Some(entry) = record
        .get("independent")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
    else {
        return (record.clone(), None);
    };
    let mut merged = record.clone();
    let mut changed = Vec::new();
    if let (Some(values), Some(sheet)) = (
        entry.get("values").and_then(Value::as_object),
        merged.get_mut("published").and_then(Value::as_object_mut),
    ) {
        for (key, name, scale, unit) in MEASURED_FIELDS {
            let Some(value) = values.get(key).and_then(Value::as_f64) else {
                continue;
            };
            let maker = sheet.get(key).and_then(Value::as_f64);
            sheet.insert(key.into(), Value::from(value));
            let shown = |v: f64| {
                format!("{:.3}", v * scale)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            };
            changed.push(
                match maker {
                    Some(m) => {
                        format!("{name} {} {unit} (maker {} {unit})", shown(value), shown(m))
                    }
                    None => format!("{name} {} {unit} (maker: not printed)", shown(value)),
                }
                .replace("  ", " ")
                .replace(" )", ")"),
            );
        }
    }
    let by = entry
        .get("by")
        .and_then(Value::as_str)
        .unwrap_or("an independent test");
    let when = entry
        .get("published_on")
        .and_then(Value::as_str)
        .map(|d| format!(", {d}"))
        .unwrap_or_default();
    let mut sentence = format!("Independent measurement by {by}{when}");
    if changed.is_empty() {
        sentence.push('.');
    } else {
        sentence.push_str(&format!(" is used: {}.", changed.join("; ")));
    }
    if let Some((f, peak)) = breakup(&merged) {
        sentence.push_str(&format!(
            " It found a cone break-up peak of {peak:.0} dB at {:.1} kHz, which the cone model does not show: keep this driver's low-pass at or below {:.1} kHz.",
            f / 1000.0,
            f / 2000.0
        ));
    }
    if let Some(note) = entry.get("note").and_then(Value::as_str) {
        sentence.push(' ');
        sentence.push_str(note);
    }
    (merged, Some(sentence))
}

/// The lowest cone break-up an independent measurement found: frequency and peak height.
pub fn breakup(record: &Value) -> Option<(f64, f64)> {
    record
        .get("independent")?
        .as_array()?
        .iter()
        .filter_map(|entry| entry.get("breakup")?.as_array())
        .flatten()
        .filter_map(|peak| Some((peak.get("f_hz")?.as_f64()?, peak.get("peak_db")?.as_f64()?)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub fn published_from_catalog(record: &Value) -> Result<Published, String> {
    let published = record
        .get("published")
        .ok_or_else(|| "missing published parameters".to_string())?;
    let fs_hz = req(published, "fs_hz")?;
    let qes = req(published, "qes")?;
    let re_ohm = req(published, "re_ohm")?;
    let sd_m2 = req(published, "sd_m2")?;
    let omega = 2.0 * PI * fs_hz;
    let vas_printed = published
        .get("vas_m3")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let mms_printed = published
        .get("mms_kg")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let bl_printed = published
        .get("bl_tm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let vas_m3 = if vas_printed > 0.0 {
        vas_printed
    } else if mms_printed > 0.0 {
        let cms = 1.0 / (omega * omega * mms_printed);
        cms * RHO0 * C0 * C0 * sd_m2 * sd_m2
    } else if bl_printed > 0.0 {
        let mms = bl_printed * bl_printed * qes / (omega * re_ohm);
        let cms = 1.0 / (omega * omega * mms);
        cms * RHO0 * C0 * C0 * sd_m2 * sd_m2
    } else if let Some(sens_1w) = sensitivity_1w_db(published, re_ohm) {
        vas_from_sensitivity(sens_1w, fs_hz, qes)
    } else {
        return Err(
            "This sheet does not print Vas, moving mass, force factor, or sensitivity, so the cone model cannot be built."
                .into(),
        );
    };
    let xmax_m = published
        .get("xmax_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Ok(Published {
        fs_hz,
        qes,
        qms: req(published, "qms")?,
        vas_m3,
        re_ohm,
        le_h: req(published, "le_h")?,
        sd_m2,
        xmax_m,
        pe_w: req(published, "pe_w")?,
    })
}

/// How the mechanical parameters were obtained, for the warning beside a driver.
pub fn derivation_note(record: &Value) -> Option<String> {
    let published = record.get("published")?;
    let vas = published
        .get("vas_m3")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if vas > 0.0 {
        return None;
    }
    if published
        .get("mms_kg")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        > 0.0
    {
        return Some(
            "Vas was not printed, so compliance was derived from the printed moving mass.".into(),
        );
    }
    if published
        .get("bl_tm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        > 0.0
    {
        return Some(
            "Vas was not printed, so moving mass was derived from the printed force factor.".into(),
        );
    }
    let re = published
        .get("re_ohm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if sensitivity_1w_db(published, re).is_some() {
        return Some("This driver is derived from sensitivity.".into());
    }
    None
}

fn sensitivity_1w_db(published: &Value, re_ohm: f64) -> Option<f64> {
    if let Some(one_watt) = published.get("sens_db_1w").and_then(Value::as_f64) {
        return Some(one_watt);
    }
    let at_2v83 = published.get("sens_db_2v83").and_then(Value::as_f64)?;
    if re_ohm <= 0.0 {
        return None;
    }
    Some(at_2v83 - 10.0 * (2.83_f64.powi(2) / re_ohm).log10())
}

fn vas_from_sensitivity(sens_db_1w: f64, fs_hz: f64, qes: f64) -> f64 {
    let k_half = 10.0 * (RHO0 * C0 / (2.0 * PI * P_REF * P_REF)).log10();
    let eta0 = 10_f64.powf((sens_db_1w - k_half) / 10.0);
    eta0 * C0.powi(3) * qes / (4.0 * PI * PI * fs_hz.powi(3))
}

/// Printed sensitivity more than 1.5 dB from the Vas path. Vas is kept.
pub fn sensitivity_disagreement(record: &Value) -> Option<String> {
    let published = record.get("published")?;
    let vas = published
        .get("vas_m3")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if vas <= 0.0 {
        return None;
    }
    let re = published.get("re_ohm")?.as_f64()?;
    let sens_1w = sensitivity_1w_db(published, re)?;
    let printed_2v83 = sens_1w + 10.0 * (2.83_f64.powi(2) / re).log10();
    let from_vas = reference_half_space_spl_db(
        &Published {
            fs_hz: published.get("fs_hz")?.as_f64()?,
            qes: published.get("qes")?.as_f64()?,
            qms: published.get("qms").and_then(Value::as_f64).unwrap_or(1.0),
            vas_m3: vas,
            re_ohm: re,
            le_h: 0.0,
            sd_m2: published
                .get("sd_m2")
                .and_then(Value::as_f64)
                .unwrap_or(0.001),
            xmax_m: 0.0,
            pe_w: 1.0,
        },
        2.83,
        1.0,
    );
    let difference = (from_vas - printed_2v83).abs();
    if difference > 1.5 {
        Some(format!(
            "The printed sensitivity is {difference:.1} dB away from the Vas calculation, so the simulation keeps Vas."
        ))
    } else {
        None
    }
}

/// The sheet printed a mechanical number more than 10% away from the Vas derivation.
/// The model keeps the derived number and the caller warns.
pub fn printed_disagreement(record: &Value) -> Option<String> {
    let published = published_from_catalog(record).ok()?;
    let mech = derive_mechanical(&published);
    let sheet = record.get("published")?;
    let mut names = Vec::new();
    let mut check = |name: &str, derived: f64, key: &str| {
        let Some(printed) = sheet.get(key).and_then(Value::as_f64) else {
            return;
        };
        if printed > 0.0 && (derived - printed).abs() / printed > 0.1 {
            names.push(name.to_string());
        }
    };
    check("moving mass", mech.mms, "mms_kg");
    check("compliance", mech.cms, "cms_m_per_n");
    check("force factor", mech.bl, "bl_tm");
    if names.is_empty() {
        None
    } else {
        Some(format!(
            "The sheet's {} differs from the value derived from Vas by more than 10 percent, so the simulation keeps the derived value.",
            names.join(" and ")
        ))
    }
}

fn req(value: &Value, key: &str) -> Result<f64, String> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing {key}"))
}

struct LumpedSealed;

impl EnclosureModel for LumpedSealed {
    fn z_ab(&self, volume_m3: f64, freqs_hz: &[f64]) -> Vec<Complex64> {
        freqs_hz
            .iter()
            .map(|freq| z_ab_sealed(volume_m3, *freq))
            .collect()
    }
}

fn parse_lumped(_fields: &Map<String, Value>) -> Result<Box<dyn EnclosureModel>, String> {
    Ok(Box::new(LumpedSealed))
}

fn parse_waveguide(fields: &Map<String, Value>) -> Result<Box<dyn EnclosureModel>, String> {
    parse_lumped(fields)
}

pub fn spl_db(pressure: Complex64) -> f64 {
    let magnitude = pressure.norm();
    if magnitude <= 0.0 {
        return -200.0;
    }
    20.0 * (magnitude / P_REF).log10()
}

pub fn reference_half_space_spl_db(published: &Published, voltage_v: f64, distance_m: f64) -> f64 {
    let eta0 =
        4.0 * PI * PI * published.fs_hz.powi(3) * published.vas_m3 / (C0.powi(3) * published.qes);
    let numerator = RHO0 * C0 * eta0 * (voltage_v * voltage_v / published.re_ohm)
        / (2.0 * PI * distance_m * distance_m);
    10.0 * (numerator / (P_REF * P_REF)).log10()
}

/// Sealed-box cone at one frequency, using the point model. `voltage_v` is RMS.
pub fn sealed_point(
    published: &Published,
    volume_m3: f64,
    voltage_v: f64,
    freq_hz: f64,
    space: &str,
    distance_m: f64,
    baffle_width_m: f64,
) -> (f64, f64, f64) {
    let mech = derive_mechanical(published);
    let zab = z_ab_sealed(volume_m3, freq_hz);
    let terminal = terminal_at(published, &mech, zab, voltage_v, freq_hz);
    let pressure = pressure_from_volume_velocity(
        terminal.volume_velocity,
        freq_hz,
        distance_m,
        space,
        baffle_width_m,
    );
    (
        spl_db(pressure),
        terminal.impedance.norm(),
        terminal.excursion_m,
    )
}

/// Vented box at one frequency. Returns SPL, impedance magnitude, cone travel, and peak port air speed.
#[allow(clippy::too_many_arguments)]
pub fn vented_point(
    published: &Published,
    volume_m3: f64,
    port: PortSpec,
    voltage_v: f64,
    freq_hz: f64,
    space: &str,
    distance_m: f64,
    baffle_width_m: f64,
) -> (f64, f64, f64, f64, f64) {
    let mech = derive_mechanical(published);
    let sample = vented_sample(volume_m3, &[port], freq_hz, false);
    let terminal = terminal_at(published, &mech, sample.z_ab, voltage_v, freq_hz);
    let into_room = terminal.volume_velocity * (Complex64::new(1.0, 0.0) - sample.h_sum);
    let pressure =
        pressure_from_volume_velocity(into_room, freq_hz, distance_m, space, baffle_width_m);
    let port_speed = std::f64::consts::SQRT_2 * (sample.h_sum * terminal.volume_velocity).norm()
        / port.area_m2.max(1e-12);
    (
        spl_db(pressure),
        terminal.impedance.norm(),
        terminal.excursion_m,
        port_speed,
        sample.h_sum.norm(),
    )
}

pub fn sealed_alignment(published: &Published, volume_m3: f64) -> (f64, f64) {
    let mech = derive_mechanical(published);
    let alpha = published.vas_m3 / volume_m3;
    let ratio = (1.0 + alpha).sqrt();
    (published.fs_hz * ratio, mech.qts * ratio)
}
