//! Radiators, enclosure loads, radiation models, room estimate, and clean-level limits.

mod cone;
mod point;
mod sealed;

pub use cone::{derive_mechanical, terminal_at, Mechanical, Published, Terminal};
pub use point::{baffle_gain, pressure_from_volume_velocity};
pub use sealed::z_ab_sealed;

use num_complex::Complex64;
use seatlock_core::catalog;
use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_core::issue::Issue;
use seatlock_core::registry::{
    EnclosureModel, RadiationModel, Radiator, RadiatorEval, Registry, RoomModel,
};
use serde_json::{Map, Value};
use std::f64::consts::PI;

pub fn register(registry: &mut Registry) {
    registry.register_radiator("cone", parse_cone);
    registry.register_enclosure("lumped", parse_lumped);
    registry.register_radiation("point", parse_point);
    registry.register_room("free", parse_free);
}

struct ConeRadiator {
    published: Published,
    missing_displacement: bool,
    driver_id: String,
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
            has_excursion_limit: true,
            warnings,
        }
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
    let published = published_from_catalog(&record)?;
    let (_displacement_m3, missing_displacement, driver_id) = cone_displacement_m3(fields);
    Ok(Box::new(ConeRadiator {
        published,
        missing_displacement,
        driver_id,
    }))
}

pub fn published_from_catalog(record: &Value) -> Result<Published, String> {
    let published = record
        .get("published")
        .ok_or_else(|| "missing published parameters".to_string())?;
    Ok(Published {
        fs_hz: req(published, "fs_hz")?,
        qes: req(published, "qes")?,
        qms: req(published, "qms")?,
        vas_m3: req(published, "vas_m3")?,
        re_ohm: req(published, "re_ohm")?,
        le_h: req(published, "le_h")?,
        sd_m2: req(published, "sd_m2")?,
        xmax_m: req(published, "xmax_m")?,
        pe_w: req(published, "pe_w")?,
    })
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

struct PointRadiation;

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
}

fn parse_point(_fields: &Map<String, Value>) -> Result<Box<dyn RadiationModel>, String> {
    Ok(Box::new(PointRadiation))
}

struct FreeRoom;

impl RoomModel for FreeRoom {
    fn at_listener(&self, direct: &[Complex64]) -> Vec<Complex64> {
        direct.to_vec()
    }
}

fn parse_free(_fields: &Map<String, Value>) -> Result<Box<dyn RoomModel>, String> {
    Ok(Box::new(FreeRoom))
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

pub fn sealed_alignment(published: &Published, volume_m3: f64) -> (f64, f64) {
    let mech = derive_mechanical(published);
    let alpha = published.vas_m3 / volume_m3;
    let ratio = (1.0 + alpha).sqrt();
    (published.fs_hz * ratio, mech.qts * ratio)
}
