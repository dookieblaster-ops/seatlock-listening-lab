//! A panel built from layers (README section 5.9, "Layered and direction-dependent panels"):
//! its bending stiffnesses, mass, shear stiffness through its thickness, and loss factor, from
//! the layers' catalog records.
//!
//! Classical laminate theory with each layer orthotropic in the panel's plane, its strong
//! direction along the panel's `u` (or `v` when the layer is turned by 90 degrees):
//!
//! ```text
//! nu_vu = nu_uv * E_v / E_u
//! Q11 = E_u / (1 - nu_uv*nu_vu)   Q22 = E_v / (1 - nu_uv*nu_vu)   Q12 = nu_uv * Q22   Q66 = G_uv
//! I_k = (z_top^3 - z_bottom^3) / 3, about the middle of the stack
//! D11 = sum Q11*I   D22 = sum Q22*I   D12 = sum Q12*I   D66 = sum Q66*I
//! ```
//!
//! For one isotropic layer that is `D = E*h^3 / (12*(1 - nu^2))`, `D12 = nu*D`,
//! `D66 = (1 - nu)*D/2`. Bending and stretching are taken as uncoupled, which is exact for a
//! stack that mirrors about its middle; another stack is treated as if it did, and the label
//! says so.
//!
//! Shear through the thickness lowers every resonance of a thick panel or a sandwich with a
//! soft core. Its stiffness per width is `S = (5/6)*G_t*h` for one layer (Mindlin) and
//! `S = d^2 / sum(t_k / G_t,k)` for several, with `d` the distance between the middles of the
//! two outer layers (for a sandwich with thin skins, `G_core * d^2 / t_core`). A mode of
//! bending wavenumber `k` then has `omega^2 = omega_bend^2 / (1 + D*k^2/S)`, the result for a
//! simply supported Mindlin plate without rotary inertia, and an estimate for other edges.

use seatlock_core::catalog;
use serde_json::Value;

/// A loss factor the lab assumes when a panel material's record does not publish one.
pub const ASSUMED_LOSS_FACTOR: f64 = 0.02;
/// A Poisson's ratio the lab assumes when a panel material's record does not publish one.
pub const ASSUMED_POISSON: f64 = 0.3;

/// What a layered panel presents to the plate model, per unit area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stiffness {
    pub d11: f64,
    pub d22: f64,
    pub d12: f64,
    pub d66: f64,
    pub mass_per_area: f64,
    /// Shear stiffness through the thickness per width, N/m; infinite when not modeled.
    pub shear_per_width: f64,
    pub thickness_m: f64,
}

/// One layer as the laminate sees it.
#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub material_id: String,
    pub thickness_m: f64,
    pub e_u: f64,
    pub e_v: f64,
    pub nu_uv: f64,
    pub g_uv: f64,
    pub g_transverse: f64,
    pub density_kg_m3: f64,
    pub loss_factor: f64,
    /// Lines for the fidelity label: what the lab assumed for this layer.
    pub assumed: Vec<String>,
}

/// The stack's stiffness and a loss factor weighted by each layer's share of the bending
/// stiffness.
pub struct Laminate {
    pub stiffness: Stiffness,
    pub loss_factor: f64,
    pub notes: Vec<String>,
}

fn number(record: &Value, key: &str) -> Option<f64> {
    record
        .get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
}

impl Layer {
    /// A layer of catalog panel material `id`, `thickness_m` thick, its strong direction along
    /// `u`, or along `v` when `turned` (a quarter turn in the panel's plane).
    pub fn from_catalog(id: &str, thickness_m: f64, turned: bool) -> Result<Layer, String> {
        let record = catalog::panel_material(id)
            .ok_or_else(|| format!("Panel material {id} is not in the catalog."))?;
        Layer::from_record(id, &record, thickness_m, turned)
    }

    pub fn from_record(
        id: &str,
        record: &Value,
        thickness_m: f64,
        turned: bool,
    ) -> Result<Layer, String> {
        if thickness_m.is_nan() || thickness_m <= 0.0 {
            return Err(format!("A layer of {id} needs a thickness above zero."));
        }
        let name = record
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .to_string();
        let density_kg_m3 = number(record, "density_kg_m3")
            .filter(|v| *v > 0.0)
            .ok_or_else(|| format!("Panel material {id} has no density_kg_m3."))?;
        let mut assumed = Vec::new();
        let core_only = record.get("core_only").and_then(Value::as_bool) == Some(true);
        let (e_u, e_v, nu_uv, g_uv) = if core_only {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let e_u = number(record, "youngs_u_pa")
                .or_else(|| number(record, "youngs_pa"))
                .filter(|v| *v > 0.0)
                .ok_or_else(|| format!("Panel material {id} has no stiffness (youngs_pa)."))?;
            let e_v = number(record, "youngs_v_pa")
                .or_else(|| number(record, "youngs_pa"))
                .filter(|v| *v > 0.0)
                .unwrap_or(e_u);
            let nu = match number(record, "poisson_uv").or_else(|| number(record, "poisson")) {
                Some(nu) if (0.0..0.5).contains(&nu) || (e_u != e_v && nu > 0.0 && nu < 1.0) => nu,
                Some(nu) => {
                    return Err(format!(
                    "Panel material {id}: Poisson's ratio {nu} is outside what a plate can have."
                ))
                }
                None => {
                    assumed.push(format!(
                        "{name}: Poisson's ratio is not published; the lab assumed {ASSUMED_POISSON}."
                    ));
                    ASSUMED_POISSON
                }
            };
            let g = match number(record, "shear_pa").filter(|v| *v > 0.0) {
                Some(g) => g,
                None => {
                    let g = (e_u * e_v).sqrt() / (2.0 * (1.0 + nu));
                    if e_u != e_v {
                        assumed.push(format!(
                            "{name}: the in-plane shear stiffness is not published; the lab \
                             used sqrt(E_u*E_v)/(2*(1 + nu))."
                        ));
                    }
                    g
                }
            };
            (e_u, e_v, nu, g)
        };
        let g_transverse = number(record, "transverse_shear_pa")
            .filter(|v| *v > 0.0)
            .or((!core_only).then_some(g_uv))
            .filter(|v| *v > 0.0)
            .ok_or_else(|| {
                format!("Panel material {id} is a core and needs transverse_shear_pa.")
            })?;
        let loss_factor = match number(record, "loss_factor").filter(|v| *v >= 0.0) {
            Some(eta) => eta,
            None => {
                assumed.push(format!(
                    "{name}: the damping (loss factor) is not published; the lab assumed \
                     {ASSUMED_LOSS_FACTOR}, so resonance heights are uncertain."
                ));
                ASSUMED_LOSS_FACTOR
            }
        };
        let (e_u, e_v, nu_uv) = if turned {
            // A quarter turn swaps the directions; nu_vu = nu_uv * E_v / E_u.
            let nu_vu = if e_u > 0.0 { nu_uv * e_v / e_u } else { 0.0 };
            (e_v, e_u, nu_vu)
        } else {
            (e_u, e_v, nu_uv)
        };
        Ok(Layer {
            name,
            material_id: id.to_string(),
            thickness_m,
            e_u,
            e_v,
            nu_uv,
            g_uv,
            g_transverse,
            density_kg_m3,
            loss_factor,
            assumed,
        })
    }

    /// Plane-stress stiffnesses `(Q11, Q22, Q12, Q66)`.
    fn reduced(&self) -> (f64, f64, f64, f64) {
        if self.e_u <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let nu_vu = self.nu_uv * self.e_v / self.e_u;
        let denominator = 1.0 - self.nu_uv * nu_vu;
        let q11 = self.e_u / denominator;
        let q22 = self.e_v / denominator;
        (q11, q22, self.nu_uv * q22, self.g_uv)
    }
}

/// Stack `layers`, listed from the panel's back face to its front face.
pub fn laminate(layers: &[Layer]) -> Result<Laminate, String> {
    if layers.is_empty() {
        return Err("A panel needs at least one layer.".into());
    }
    let h: f64 = layers.iter().map(|l| l.thickness_m).sum();
    let (mut d11, mut d22, mut d12, mut d66, mut mass) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut loss_weighted = 0.0;
    let mut z = -h / 2.0;
    for layer in layers {
        let top = z + layer.thickness_m;
        let second_moment = (top.powi(3) - z.powi(3)) / 3.0;
        let (q11, q22, q12, q66) = layer.reduced();
        d11 += q11 * second_moment;
        d22 += q22 * second_moment;
        d12 += q12 * second_moment;
        d66 += q66 * second_moment;
        loss_weighted += layer.loss_factor * (q11 + q22) / 2.0 * second_moment;
        mass += layer.density_kg_m3 * layer.thickness_m;
        z = top;
    }
    if !(d11 > 0.0 && d22 > 0.0) {
        return Err(
            "The panel's layers have no bending stiffness: a core needs skins on it.".into(),
        );
    }
    let shear_per_width = if layers.len() == 1 {
        5.0 / 6.0 * layers[0].g_transverse * h
    } else {
        let compliance: f64 = layers.iter().map(|l| l.thickness_m / l.g_transverse).sum();
        let d = h - (layers[0].thickness_m + layers[layers.len() - 1].thickness_m) / 2.0;
        d * d / compliance
    };
    let mut notes: Vec<String> = Vec::new();
    for note in layers.iter().flat_map(|l| l.assumed.iter()) {
        if !notes.contains(note) {
            notes.push(note.clone());
        }
    }
    if layers.len() > 1 {
        let n = layers.len();
        let mirrored = (0..n / 2).all(|k| {
            let (a, b) = (&layers[k], &layers[n - 1 - k]);
            a.material_id == b.material_id
                && (a.thickness_m - b.thickness_m).abs() <= 1e-9
                && (a.e_u - b.e_u).abs() <= 1e-6 * a.e_u.abs().max(1.0)
        });
        notes.push(format!(
            "The panel is {n} layers (classical laminate theory; the shear through its \
             thickness lowers its resonances by an estimate exact only for supported edges); \
             the damping is each layer's own, weighted by its share of the stiffness, which \
             leaves out the extra damping a soft core adds between stiff skins."
        ));
        if !mirrored {
            notes.push(
                "The layers do not mirror about the panel's middle; bending and stretching \
                 are treated as uncoupled, which is approximate for this stack."
                    .into(),
            );
        }
    }
    let stiffness = Stiffness {
        d11,
        d22,
        d12,
        d66,
        mass_per_area: mass,
        shear_per_width,
        thickness_m: h,
    };
    let loss_factor = loss_weighted / ((d11 + d22) / 2.0);
    Ok(Laminate {
        stiffness,
        loss_factor,
        notes,
    })
}
