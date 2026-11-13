//! Cabinet wall vibration (README section 5.11): each flat wall is a plate, simply supported
//! along its edges, made of the wall build's layers, pushed by the pressure inside the cabinet.
//!
//! ```text
//! a wall's layers (back face to front):  Q_k = E_k*(1 + j*eta_k) / (1 - nu_k^2)
//!   D_local    = sum Q_k * t_k^3 / 12            each layer bending about its own middle
//!   D_sandwich = sum Q_k * t_k * z_k^2           layers carried apart by the core
//!   S          = d^2 / sum(t_k / G_k)            shear through the thickness, G complex too
//!   D(k)       = D_local + D_sandwich / (1 + D_sandwich*k^2 / S)
//! ```
//!
//! `D(k)` is the sandwich-beam result of Mead and Markus (the Ross-Kerwin-Ungar model for a
//! symmetric stack): for a soft lossy core between stiff skins its loss factor is
//! `eta = beta*Y*g / (1 + (2 + Y)*g + (1 + Y)*(1 + beta^2)*g^2)`, with `Y = D_sandwich/D_local`,
//! `g = S/(D_sandwich*k^2)`, and `beta` the core's loss factor. A stiff core gives the
//! laminate's own stiffness; a loose fill (sand) has no stiffness and no shear, so the skins
//! bend apart, and its loss enters as the fill's loss factor times its share of the mass
//! (the energy it carries at a resonance is its share of the kinetic energy).
//!
//! The plate's modes are those of an isotropic plate of stiffness `D(0)` (for supported
//! edges the shapes do not depend on the stiffness); each mode then takes the complex
//! stiffness at its own wavenumber, `k^2 = sqrt(lambda*m/D(0))`:
//!
//! ```text
//! lambda_n = lambda_ref,n * D(k_n) / D(0) * (1 + j*eta_fill)
//! s_n      = integral of the mode's shape over the wall          (unit modal mass)
//! Y_wall   = j*omega * sum s_n^2 / (lambda_n - omega^2)          outward volume velocity per pascal inside
//! ```

use num_complex::Complex64;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use crate::laminate::Stiffness;
use crate::plate::{Edges, Plate, PlateModes};

/// Polynomial degree of a wall's basis along its longer side.
pub const WALL_DEGREE: usize = 16;
/// Largest element side on a wall, in m, and the fewest and most elements along a side. A wall
/// moves most below a few kilohertz, where a few patches place its sound well enough; each
/// patch costs as much as a driver in every direction the radiation model is asked about.
const ELEMENT_M: f64 = 0.08;
const FEWEST_ELEMENTS: usize = 1;
const MOST_ELEMENTS: usize = 3;

/// One layer of a wall.
#[derive(Clone, Debug)]
pub struct WallLayer {
    pub name: String,
    pub thickness_m: f64,
    /// Young's modulus; 0 for a loose fill that carries no stiffness.
    pub youngs_pa: f64,
    pub poisson: f64,
    pub density_kg_m3: f64,
    pub loss_factor: f64,
}

impl WallLayer {
    fn bonded(&self) -> bool {
        self.youngs_pa > 0.0
    }

    fn plane_stiffness(&self) -> Complex64 {
        Complex64::new(self.youngs_pa, self.youngs_pa * self.loss_factor)
            / (1.0 - self.poisson * self.poisson)
    }

    fn shear(&self) -> Complex64 {
        Complex64::new(self.youngs_pa, self.youngs_pa * self.loss_factor)
            / (2.0 * (1.0 + self.poisson))
    }
}

/// What a wall's layers present to its plate model, per unit area.
#[derive(Clone, Copy, Debug)]
pub struct WallStack {
    pub mass_per_area: f64,
    pub thickness_m: f64,
    pub poisson: f64,
    pub d_local: Complex64,
    pub d_sandwich: Complex64,
    /// Shear stiffness through the thickness per width; `None` when it is not modeled (one
    /// layer), zero when a loose layer keeps the outer layers from working together.
    pub shear: Option<Complex64>,
    /// Loss factor a loose fill adds: its own times its share of the mass.
    pub fill_loss: f64,
}

impl WallStack {
    /// Stack `layers`, listed from the wall's inside face to its outside face.
    pub fn from_layers(layers: &[WallLayer]) -> Result<WallStack, String> {
        if layers.is_empty() {
            return Err("A wall needs at least one layer.".into());
        }
        if layers
            .iter()
            .any(|l| !(l.thickness_m > 0.0 && l.density_kg_m3 > 0.0))
        {
            return Err("Every wall layer needs a thickness and a density above zero.".into());
        }
        if !layers[0].bonded() || !layers[layers.len() - 1].bonded() {
            return Err("A wall's outer layers must be stiff.".into());
        }
        let h: f64 = layers.iter().map(|l| l.thickness_m).sum();
        let zero = Complex64::new(0.0, 0.0);
        let (mut d_local, mut d_sandwich, mut mass, mut lossy_mass) = (zero, zero, 0.0, 0.0);
        let mut z = -h / 2.0;
        for layer in layers {
            let middle = z + layer.thickness_m / 2.0;
            mass += layer.density_kg_m3 * layer.thickness_m;
            if layer.bonded() {
                let q = layer.plane_stiffness();
                d_local += q * layer.thickness_m.powi(3) / 12.0;
                d_sandwich += q * layer.thickness_m * middle * middle;
            } else {
                lossy_mass += layer.loss_factor * layer.density_kg_m3 * layer.thickness_m;
            }
            z += layer.thickness_m;
        }
        let shear = if layers.len() == 1 {
            None
        } else if layers.iter().any(|l| !l.bonded()) {
            Some(zero)
        } else {
            let compliance: Complex64 = layers
                .iter()
                .map(|l| Complex64::new(l.thickness_m, 0.0) / l.shear())
                .sum();
            let d = h - (layers[0].thickness_m + layers[layers.len() - 1].thickness_m) / 2.0;
            Some(Complex64::new(d * d, 0.0) / compliance)
        };
        Ok(WallStack {
            mass_per_area: mass,
            thickness_m: h,
            poisson: layers[0].poisson,
            d_local,
            d_sandwich,
            shear,
            fill_loss: lossy_mass / mass,
        })
    }

    /// Complex bending stiffness for a bending wave of wavenumber squared `k2`.
    pub fn stiffness(&self, k2: f64) -> Complex64 {
        let sandwich = match self.shear {
            None => self.d_sandwich,
            Some(s) if s.norm() == 0.0 => Complex64::new(0.0, 0.0),
            Some(s) => self.d_sandwich / (1.0 + self.d_sandwich * k2 / s),
        };
        self.d_local + sandwich
    }

    /// The stiffness the modes are solved with: the stack's own at long wavelengths.
    pub fn reference(&self) -> f64 {
        self.stiffness(0.0).re
    }
}

/// One wall: its modes with their complex stiffness, and the integrals of their shapes over
/// the wall and over each element.
#[derive(Clone, Debug)]
pub struct WallPanel {
    pub width_m: f64,
    pub height_m: f64,
    /// `lambda_n`, complex: the mode's `omega^2` with its loss.
    pub lambdas: Vec<Complex64>,
    /// `integral of shape` over the whole wall.
    pub integrals: Vec<f64>,
    /// Elements along the width and the height.
    pub grid: [usize; 2],
    /// `element_integrals[element][mode]`, elements ordered `iu * grid[1] + iv`.
    pub element_integrals: Vec<Vec<f64>>,
    /// Modes above this were left out: the basis does not resolve them.
    pub resolved_up_to_hz: f64,
}

type Shapes = Arc<(PlateModes, Vec<f64>, Vec<Vec<f64>>, [usize; 2])>;

/// Modes and shape integrals of a supported plate, kept for reuse: a box has three wall sizes
/// and editing anything but the cabinet does not change them.
fn shapes(plate: Plate) -> Result<Shapes, String> {
    type Key = [u64; 4];
    static CACHE: OnceLock<Mutex<Vec<(Key, Shapes)>>> = OnceLock::new();
    let key = [
        plate.width_m.to_bits(),
        plate.height_m.to_bits(),
        plate.layup.d12.to_bits() ^ plate.layup.d11.to_bits(),
        plate.layup.mass_per_area.to_bits(),
    ];
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(held) = cache.lock() {
        if let Some((_, found)) = held.iter().find(|(known, _)| *known == key) {
            return Ok(found.clone());
        }
    }
    let modes = PlateModes::solve(plate, WALL_DEGREE)?;
    let count =
        |side: f64| ((side / ELEMENT_M).ceil() as usize).clamp(FEWEST_ELEMENTS, MOST_ELEMENTS);
    let grid = [count(plate.width_m), count(plate.height_m)];
    let by_mode = modes.cell_integrals(grid[0], grid[1]);
    let integrals: Vec<f64> = by_mode.iter().map(|cells| cells.iter().sum()).collect();
    let elements: Vec<Vec<f64>> = (0..grid[0] * grid[1])
        .map(|cell| by_mode.iter().map(|cells| cells[cell]).collect())
        .collect();
    let solved: Shapes = Arc::new((modes, integrals, elements, grid));
    if let Ok(mut held) = cache.lock() {
        if held.len() >= 16 {
            held.remove(0);
        }
        held.push((key, solved.clone()));
    }
    Ok(solved)
}

impl WallPanel {
    /// A wall `width_m` by `height_m` between its edges' middles, of `stack`.
    pub fn new(width_m: f64, height_m: f64, stack: &WallStack) -> Result<WallPanel, String> {
        let d = stack.reference();
        if !(width_m > 0.0 && height_m > 0.0 && d > 0.0) {
            return Err("A wall needs a size and a bending stiffness above zero.".into());
        }
        let nu = stack.poisson.clamp(0.0, 0.49);
        // Every wall's shapes are those of a unit isotropic plate of its size; only `nu`,
        // which does not change a supported plate's shapes, is carried.
        let unit = Plate {
            width_m,
            height_m,
            layup: Stiffness {
                d11: 1.0,
                d22: 1.0,
                d12: nu,
                d66: (1.0 - nu) / 2.0,
                mass_per_area: 1.0,
                shear_per_width: f64::INFINITY,
                thickness_m: stack.thickness_m,
            },
            edges: Edges::Supported,
        };
        let solved = shapes(unit)?;
        let (modes, integrals, elements, grid) = &*solved;
        let m = stack.mass_per_area;
        // Unit plate: lambda_unit = (2 pi f)^2 for D = 1, m = 1; shapes scale by 1/sqrt(m).
        let lambdas: Vec<Complex64> = modes
            .frequencies_hz()
            .iter()
            .map(|f| {
                let unit_lambda = (2.0 * PI * f).powi(2);
                let lambda = unit_lambda * d / m;
                let k2 = unit_lambda.sqrt();
                lambda * stack.stiffness(k2) / d * Complex64::new(1.0, stack.fill_loss)
            })
            .collect();
        let scale = 1.0 / m.sqrt();
        let resolved_up_to_hz = modes.resolved_up_to_hz * (d / m).sqrt();
        Ok(WallPanel {
            width_m,
            height_m,
            lambdas,
            integrals: integrals.iter().map(|s| s * scale).collect(),
            grid: *grid,
            element_integrals: elements
                .iter()
                .map(|row| row.iter().map(|s| s * scale).collect())
                .collect(),
            resolved_up_to_hz,
        })
    }

    /// The lowest resonance, in Hz, without the loss.
    pub fn first_resonance_hz(&self) -> f64 {
        self.lambdas
            .iter()
            .zip(&self.integrals)
            .filter(|(_, s)| s.abs() > 1e-12)
            .map(|(l, _)| l.re.max(0.0).sqrt() / (2.0 * PI))
            .fold(f64::INFINITY, f64::min)
    }

    /// Outward volume velocity of the whole wall per pascal inside.
    pub fn admittance(&self, freq_hz: f64) -> Complex64 {
        let omega = 2.0 * PI * freq_hz;
        let jw = Complex64::new(0.0, omega);
        self.lambdas
            .iter()
            .zip(&self.integrals)
            .map(|(lambda, s)| jw * (s * s) / (lambda - omega * omega))
            .sum()
    }

    /// Outward volume velocity of each element per pascal inside.
    pub fn element_admittances(&self, freq_hz: f64) -> Vec<Complex64> {
        let omega = 2.0 * PI * freq_hz;
        let jw = Complex64::new(0.0, omega);
        let gains: Vec<Complex64> = self
            .lambdas
            .iter()
            .zip(&self.integrals)
            .map(|(lambda, s)| jw * *s / (lambda - omega * omega))
            .collect();
        self.element_integrals
            .iter()
            .map(|row| row.iter().zip(&gains).map(|(e, g)| *g * *e).sum())
            .collect()
    }

    /// Each element's middle, measured from the wall's middle along its width and height.
    pub fn element_centers_m(&self) -> Vec<[f64; 2]> {
        let [nu, nv] = self.grid;
        let (du, dv) = (self.width_m / nu as f64, self.height_m / nv as f64);
        (0..nu * nv)
            .map(|index| {
                [
                    ((index / nv) as f64 + 0.5) * du - self.width_m / 2.0,
                    ((index % nv) as f64 + 0.5) * dv - self.height_m / 2.0,
                ]
            })
            .collect()
    }

    pub fn element_area_m2(&self) -> f64 {
        self.width_m * self.height_m / (self.grid[0] * self.grid[1]) as f64
    }
}
