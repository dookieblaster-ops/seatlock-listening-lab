//! The `dml_panel` radiator (README section 5.9): a plate driven by an inertial exciter.
//!
//! The plate's own modes (unit modal mass, loss as a complex stiffness) are the basis. The
//! exciter's force at one point, the coil's mass there, and a sealed backing's air spring are
//! lumped parts joined to that basis in closed form:
//!
//! ```text
//! G_n   = 1 / (lambda_n * (1 + j*eta) - omega^2)          one mode's response
//! g_ee  = sum phi_e^2 * G,  g_es = sum phi_e * s * G,  g_ss = sum s^2 * G
//!         phi_e: mode shape at the exciter;  s: integral of the mode shape over the panel
//! alpha = g_ee - kappa * g_es^2 / (1 + kappa * g_ss)       deflection at the exciter per force,
//!         kappa = RHO0*C0^2 / V_cavity (0 without a sealed backing)
//! Y_p   = j*omega*alpha / (1 + j*omega*m_coil * j*omega*alpha)   panel mobility with the coil
//! Y_t   = Y_p + 1 / (j*omega*m_magnet)                     coil against magnet
//! Z_in  = Ze + bl^2 * Y_t / (1 + Zs * Y_t)                 Zs = k_s/(j*omega) + r_s
//! F     = bl * I / (1 + Zs * Y_t)                          force on the panel and coil
//! ```

use num_complex::Complex64;
use seatlock_core::catalog;
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::issue::Issue;
use seatlock_core::registry::{Radiator, RadiatorEval, SurfaceElement};
use serde_json::{Map, Value};
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use crate::plate::{Edges, Plate, PlateModes};

/// Polynomial degree of the plate basis along the panel's longer side.
const DEGREE: usize = 28;
/// Largest element side, in m, and the most elements along a side.
const ELEMENT_M: f64 = 0.0125;
const MOST_ELEMENTS: usize = 40;

#[derive(Clone, Debug)]
struct Exciter {
    id: String,
    re_ohm: f64,
    le_h: f64,
    bl_tm: f64,
    coil_mass_kg: f64,
    magnet_mass_kg: f64,
    suspension_n_per_m: f64,
    suspension_ns_per_m: f64,
    xmax_m: Option<f64>,
    pe_w: f64,
}

pub struct DmlPanel {
    modes: Arc<PlateModes>,
    loss_factor: f64,
    exciter: Exciter,
    /// Exciter position from the panel's lower-left corner, in m.
    exciter_at_m: [f64; 2],
    /// Air volume sealed behind the panel, in m^3.
    cavity_m3: f64,
    /// Elements along the width and the height.
    grid: [usize; 2],
    /// Mode shapes at each element's middle, `[element][mode]`.
    shapes: Vec<Vec<f64>>,
    /// Mode shapes at the exciter.
    at_exciter: Vec<f64>,
    /// Integral of each mode shape over the panel.
    integrals: Vec<f64>,
}

/// What one frequency's solution needs to place every mode.
struct Drive {
    impedance: Complex64,
    current: Complex64,
    /// Force on the plate itself per volt, after the coil's mass.
    force: Complex64,
    /// `integral of w over the panel` per volt.
    sigma: Complex64,
    /// Coil against magnet, peak travel per volt.
    travel_peak_m: f64,
    /// Each mode's response `G_n`.
    gains: Vec<Complex64>,
    kappa: f64,
}

fn cached_modes(plate: Plate) -> Result<Arc<PlateModes>, String> {
    type Key = [u64; 7];
    type Solved = Vec<(Key, Arc<PlateModes>)>;
    static CACHE: OnceLock<Mutex<Solved>> = OnceLock::new();
    let key: Key = [
        plate.width_m.to_bits(),
        plate.height_m.to_bits(),
        plate.thickness_m.to_bits(),
        plate.youngs_pa.to_bits(),
        plate.density_kg_m3.to_bits(),
        plate.poisson.to_bits(),
        plate.edges as u64,
    ];
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(held) = cache.lock() {
        if let Some((_, modes)) = held.iter().find(|(known, _)| *known == key) {
            return Ok(modes.clone());
        }
    }
    let modes = Arc::new(PlateModes::solve(plate, DEGREE)?);
    if let Ok(mut held) = cache.lock() {
        if held.len() >= 8 {
            held.remove(0);
        }
        held.push((key, modes.clone()));
    }
    Ok(modes)
}

impl DmlPanel {
    fn drive(&self, freq_hz: f64) -> Drive {
        let omega = 2.0 * PI * freq_hz.max(1e-3);
        let jw = Complex64::new(0.0, omega);
        let stiffness = Complex64::new(1.0, self.loss_factor);
        let gains: Vec<Complex64> = self
            .modes
            .frequencies_hz()
            .iter()
            .map(|freq| {
                let lambda = (2.0 * PI * freq).powi(2);
                Complex64::new(1.0, 0.0) / (stiffness * lambda - omega * omega)
            })
            .collect();
        let zero = Complex64::new(0.0, 0.0);
        let (mut g_ee, mut g_es, mut g_ss) = (zero, zero, zero);
        for (mode, gain) in gains.iter().enumerate() {
            let (at, sum) = (self.at_exciter[mode], self.integrals[mode]);
            g_ee += *gain * (at * at);
            g_es += *gain * (at * sum);
            g_ss += *gain * (sum * sum);
        }
        let kappa = if self.cavity_m3 > 0.0 {
            RHO0 * C0 * C0 / self.cavity_m3
        } else {
            0.0
        };
        let spring = Complex64::new(1.0, 0.0) + g_ss * kappa;
        let alpha = g_ee - g_es * g_es * kappa / spring;
        let plate_mobility = jw * alpha;
        let coil = Complex64::new(1.0, 0.0) + jw * self.exciter.coil_mass_kg * plate_mobility;
        let panel_mobility = plate_mobility / coil;
        let magnet_mobility = Complex64::new(1.0, 0.0) / (jw * self.exciter.magnet_mass_kg);
        let both = panel_mobility + magnet_mobility;
        let suspension = Complex64::new(
            self.exciter.suspension_ns_per_m,
            -self.exciter.suspension_n_per_m / omega,
        );
        let ze = Complex64::new(self.exciter.re_ohm, omega * self.exciter.le_h);
        let shared = Complex64::new(1.0, 0.0) + suspension * both;
        let impedance = ze + both * (self.exciter.bl_tm * self.exciter.bl_tm) / shared;
        let current = Complex64::new(1.0, 0.0) / impedance;
        let on_panel = current * self.exciter.bl_tm / shared;
        // Coil velocity minus magnet velocity is `Y_t * F`.
        let travel_peak_m = std::f64::consts::SQRT_2 * (both * on_panel).norm() / omega;
        let force = on_panel / coil;
        Drive {
            impedance,
            current,
            force,
            sigma: g_es * force / spring,
            travel_peak_m,
            gains,
            kappa,
        }
    }

    /// Each mode's amplitude per volt.
    fn amplitudes(&self, drive: &Drive) -> Vec<Complex64> {
        drive
            .gains
            .iter()
            .enumerate()
            .map(|(mode, gain)| {
                *gain
                    * (drive.force * self.at_exciter[mode]
                        - drive.sigma * (drive.kappa * self.integrals[mode]))
            })
            .collect()
    }
}

impl Radiator for DmlPanel {
    fn evaluate(&self, freqs_hz: &[f64], _z_ab: &[Complex64]) -> RadiatorEval {
        let mut volume_velocity = Vec::with_capacity(freqs_hz.len());
        let mut impedance = Vec::with_capacity(freqs_hz.len());
        let mut excursion_m = Vec::with_capacity(freqs_hz.len());
        let mut current_a = Vec::with_capacity(freqs_hz.len());
        for freq in freqs_hz {
            let drive = self.drive(*freq);
            // The panel's net volume velocity: j*omega times the integral of its deflection.
            volume_velocity.push(Complex64::new(0.0, 2.0 * PI * freq) * drive.sigma);
            impedance.push(drive.impedance);
            excursion_m.push(drive.travel_peak_m);
            current_a.push(drive.current);
        }
        let mut warnings = Vec::new();
        if self.exciter.xmax_m.is_none() {
            warnings.push(Issue::warning(
                "missing_field",
                format!(
                    "Exciter {} does not publish its travel, so the maximum clean level ignores it.",
                    self.exciter.id
                ),
                format!("catalog/{}/published.xmax_m", self.exciter.id),
            ));
        }
        RadiatorEval {
            volume_velocity,
            impedance,
            excursion_m,
            current_a,
            re_ohm: self.exciter.re_ohm,
            xmax_m: self.exciter.xmax_m.unwrap_or(0.0),
            pe_w: self.exciter.pe_w,
            has_excursion_limit: self.exciter.xmax_m.is_some(),
            loads_box: false,
            warnings,
        }
    }

    fn loads_box(&self) -> bool {
        false
    }

    fn surface(&self, freqs_hz: &[f64]) -> Option<Vec<SurfaceElement>> {
        let plate = self.modes.plate();
        let [count_u, count_v] = self.grid;
        let (du, dv) = (
            plate.width_m / count_u as f64,
            plate.height_m / count_v as f64,
        );
        let mut elements: Vec<SurfaceElement> = (0..count_u * count_v)
            .map(|index| SurfaceElement {
                // Measured from the mount point, which is the panel's middle.
                at_m: [
                    ((index / count_v) as f64 + 0.5) * du - plate.width_m / 2.0,
                    ((index % count_v) as f64 + 0.5) * dv - plate.height_m / 2.0,
                ],
                area_m2: du * dv,
                volume_velocity: Vec::with_capacity(freqs_hz.len()),
            })
            .collect();
        for freq in freqs_hz {
            let amplitudes = self.amplitudes(&self.drive(*freq));
            let scale = Complex64::new(0.0, 2.0 * PI * freq) * (du * dv);
            for (element, shapes) in elements.iter_mut().zip(&self.shapes) {
                let mut deflection = Complex64::new(0.0, 0.0);
                for (shape, amplitude) in shapes.iter().zip(&amplitudes) {
                    deflection += *amplitude * *shape;
                }
                element.volume_velocity.push(scale * deflection);
            }
        }
        Some(elements)
    }

    fn notes(&self) -> Vec<String> {
        vec![
            format!(
                "The panel's {} resonances up to {:.0} Hz are modeled; above that its response \
                 is not, and reads low.",
                self.modes.len(),
                self.modes.resolved_up_to_hz
            ),
            "The panel moves as in a vacuum: the air's load on it is left out, and a sealed \
             backing is one air spring."
                .into(),
            "A panel's result depends on its material's stiffness and loss, which printing makes \
             uncertain."
                .into(),
        ]
    }
}

pub fn parse_dml_panel(fields: &Map<String, Value>) -> Result<Box<dyn Radiator>, String> {
    let exciter_id = fields
        .get("exciter_id")
        .and_then(Value::as_str)
        .ok_or("A panel speaker needs an exciter_id.")?;
    let record = catalog::exciter(exciter_id)
        .ok_or_else(|| format!("Exciter {exciter_id} is not in the catalog."))?;
    let published = |key: &str| -> Result<f64, String> {
        record
            .pointer(&format!("/published/{key}"))
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("Exciter {exciter_id} does not publish {key}."))
    };
    let exciter = Exciter {
        id: exciter_id.to_string(),
        re_ohm: published("re_ohm")?,
        le_h: published("le_h").unwrap_or(0.0),
        bl_tm: published("bl_tm")?,
        coil_mass_kg: published("coil_mass_kg")?,
        magnet_mass_kg: published("magnet_mass_kg")?,
        suspension_n_per_m: published("suspension_n_per_m")?,
        suspension_ns_per_m: published("suspension_ns_per_m").unwrap_or(0.0),
        xmax_m: published("xmax_m").ok(),
        pe_w: published("pe_w")?,
    };
    let panel = fields
        .get("panel")
        .and_then(Value::as_object)
        .ok_or("A panel speaker needs a panel.")?;
    let size = |key: &str| -> Result<f64, String> {
        panel
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0)
            .ok_or_else(|| format!("The panel needs {key} above zero."))
    };
    let material_id = panel
        .get("material")
        .and_then(Value::as_str)
        .ok_or("The panel needs a material.")?;
    let material = catalog::panel_material(material_id)
        .ok_or_else(|| format!("Panel material {material_id} is not in the catalog."))?;
    let property = |key: &str| -> Result<f64, String> {
        material
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("Panel material {material_id} has no {key}."))
    };
    let edges_name = panel.get("edges").and_then(Value::as_str).unwrap_or("free");
    let edges = Edges::parse(edges_name)
        .ok_or_else(|| format!("Panel edges are free, supported, or clamped, not {edges_name}."))?;
    let plate = Plate {
        width_m: size("width_m")?,
        height_m: size("height_m")?,
        thickness_m: size("thickness_m")?,
        youngs_pa: property("youngs_pa")?,
        density_kg_m3: property("density_kg_m3")?,
        poisson: property("poisson")?,
        edges,
    };
    let cavity_m3 = match fields
        .get("backing")
        .and_then(Value::as_str)
        .unwrap_or("sealed")
    {
        "sealed" => {
            let depth = fields
                .get("cavity_depth_m")
                .and_then(Value::as_f64)
                .filter(|value| *value > 0.0)
                .ok_or("A sealed panel needs cavity_depth_m, the depth of the air behind it.")?;
            plate.width_m * plate.height_m * depth
        }
        "open" => {
            return Err(
                "An open-backed panel is not built yet: its back needs the panel itself \
                        in the air outside the cabinet. Use sealed backing."
                    .into(),
            )
        }
        other => return Err(format!("Panel backing is sealed or open, not {other}.")),
    };
    let exciter_at_m = match fields.get("exciter_at_m").and_then(Value::as_array) {
        Some(pair) if pair.len() == 2 => [
            pair[0].as_f64().unwrap_or(-1.0),
            pair[1].as_f64().unwrap_or(-1.0),
        ],
        _ => {
            return Err(
                "A panel speaker needs exciter_at_m, from the panel's lower-left corner.".into(),
            )
        }
    };
    if !(0.0..=plate.width_m).contains(&exciter_at_m[0])
        || !(0.0..=plate.height_m).contains(&exciter_at_m[1])
    {
        return Err("The exciter is not on the panel.".into());
    }
    let modes = cached_modes(plate)?;
    let count = |side: f64| ((side / ELEMENT_M).ceil() as usize).clamp(4, MOST_ELEMENTS);
    let grid = [count(plate.width_m), count(plate.height_m)];
    let (du, dv) = (
        plate.width_m / grid[0] as f64,
        plate.height_m / grid[1] as f64,
    );
    let mut shapes = Vec::with_capacity(grid[0] * grid[1]);
    let mut integrals = vec![0.0; modes.len()];
    for iu in 0..grid[0] {
        for iv in 0..grid[1] {
            let here = modes.shapes_at((iu as f64 + 0.5) * du, (iv as f64 + 0.5) * dv);
            for (sum, value) in integrals.iter_mut().zip(&here) {
                *sum += value * du * dv;
            }
            shapes.push(here);
        }
    }
    Ok(Box::new(DmlPanel {
        at_exciter: modes.shapes_at(exciter_at_m[0], exciter_at_m[1]),
        loss_factor: property("loss_factor")?,
        exciter,
        exciter_at_m,
        cavity_m3,
        grid,
        shapes,
        integrals,
        modes,
    }))
}

impl DmlPanel {
    /// Where the exciter sits, from the panel's lower-left corner.
    pub fn exciter_at_m(&self) -> [f64; 2] {
        self.exciter_at_m
    }
}
