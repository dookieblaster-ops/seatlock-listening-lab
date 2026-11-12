//! A thin rectangular plate in bending (README section 5.9): its resonances and mode shapes,
//! by the Rayleigh-Ritz method with a polynomial basis.
//!
//! ```text
//! D = E*h^3 / (12*(1 - nu^2)),   m = rho*h
//! strain energy   (D/2) * integral of w_xx^2 + w_yy^2 + 2*nu*w_xx*w_yy + 2*(1 - nu)*w_xy^2
//! kinetic energy  (m/2) * omega^2 * integral of w^2
//! ```
//!
//! The basis is `X_i(xi) * Y_j(eta)` on the square `[-1, 1]^2`, with
//! `X_i = (1 - xi^2)^q * P_i(xi)`: Legendre polynomials times a factor that makes every basis
//! function meet the edge condition, `q = 0` for free edges, 1 for simply supported (zero
//! deflection), 2 for clamped (zero deflection and slope). A plate with the same condition on
//! all four edges is symmetric about both center lines, so its modes are even or odd in each
//! direction, and the problem splits into four smaller ones.

// Matrix code reads best with its indices, as in the formulas it follows.
#![allow(clippy::needless_range_loop)]

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edges {
    Free,
    Supported,
    Clamped,
}

impl Edges {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "free" => Some(Edges::Free),
            "supported" | "simply_supported" => Some(Edges::Supported),
            "clamped" => Some(Edges::Clamped),
            _ => None,
        }
    }

    fn power(self) -> usize {
        match self {
            Edges::Free => 0,
            Edges::Supported => 1,
            Edges::Clamped => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Plate {
    /// Size along the surface's `u` direction, in m.
    pub width_m: f64,
    /// Size along the surface's `v` direction, in m.
    pub height_m: f64,
    pub thickness_m: f64,
    pub youngs_pa: f64,
    pub density_kg_m3: f64,
    pub poisson: f64,
    pub edges: Edges,
}

impl Plate {
    /// Bending stiffness `D`, in N*m.
    pub fn stiffness(&self) -> f64 {
        self.youngs_pa * self.thickness_m.powi(3) / (12.0 * (1.0 - self.poisson * self.poisson))
    }

    /// Mass per area, in kg/m^2.
    pub fn mass_per_area(&self) -> f64 {
        self.density_kg_m3 * self.thickness_m
    }

    /// Resonance `(i, j)` of the simply supported plate, in closed form.
    pub fn supported_mode_hz(&self, i: usize, j: usize) -> f64 {
        PI / 2.0
            * (self.stiffness() / self.mass_per_area()).sqrt()
            * ((i as f64 / self.width_m).powi(2) + (j as f64 / self.height_m).powi(2))
    }
}

/// One resonance: its frequency and its shape as coefficients of the basis.
#[derive(Clone, Debug)]
struct Mode {
    freq_hz: f64,
    /// Basis degrees along `u` and along `v` used by this mode's symmetry class.
    degrees_u: Vec<usize>,
    degrees_v: Vec<usize>,
    /// `coefficients[a * degrees_v.len() + b]` multiplies `X_(degrees_u[a]) * Y_(degrees_v[b])`.
    coefficients: Vec<f64>,
}

/// The plate's resonances up to the frequency the basis resolves, with shapes scaled so that
/// `integral of m * w^2 = 1` (unit modal mass).
#[derive(Clone, Debug)]
pub struct PlateModes {
    plate: Plate,
    modes: Vec<Mode>,
    /// Modes above this frequency were left out: the basis does not resolve them.
    pub resolved_up_to_hz: f64,
}

/// Normalized Legendre polynomials `sqrt((2i+1)/2) * P_i` and their first two derivatives at
/// `x`, for degrees below `count`, each times the edge factor `(1 - x^2)^q`.
fn basis_at(x: f64, count: usize, q: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut p = vec![0.0; count.max(2)];
    let mut d1 = vec![0.0; count.max(2)];
    let mut d2 = vec![0.0; count.max(2)];
    p[0] = 1.0;
    p[1] = x;
    d1[1] = 1.0;
    for i in 1..count.max(2) - 1 {
        p[i + 1] = ((2 * i + 1) as f64 * x * p[i] - i as f64 * p[i - 1]) / (i + 1) as f64;
        d1[i + 1] = d1[i - 1] + (2 * i + 1) as f64 * p[i];
        d2[i + 1] = d2[i - 1] + (2 * i + 1) as f64 * d1[i];
    }
    let (g, g1, g2) = match q {
        0 => (1.0, 0.0, 0.0),
        1 => (1.0 - x * x, -2.0 * x, -2.0),
        _ => (
            (1.0 - x * x).powi(2),
            -4.0 * x * (1.0 - x * x),
            12.0 * x * x - 4.0,
        ),
    };
    let mut value = Vec::with_capacity(count);
    let mut slope = Vec::with_capacity(count);
    let mut curve = Vec::with_capacity(count);
    for i in 0..count {
        let norm = ((2 * i + 1) as f64 / 2.0).sqrt();
        value.push(norm * g * p[i]);
        slope.push(norm * (g1 * p[i] + g * d1[i]));
        curve.push(norm * (g2 * p[i] + 2.0 * g1 * d1[i] + g * d2[i]));
    }
    (value, slope, curve)
}

/// Gauss-Legendre points and weights on `[-1, 1]`.
fn gauss_legendre(count: usize) -> Vec<(f64, f64)> {
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let mut x = (PI * (index as f64 + 0.75) / (count as f64 + 0.5)).cos();
        let mut slope = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for i in 1..count {
                let next = ((2 * i + 1) as f64 * x * p1 - i as f64 * p0) / (i + 1) as f64;
                p0 = p1;
                p1 = next;
            }
            slope = count as f64 * (x * p1 - p0) / (x * x - 1.0);
            let step = p1 / slope;
            x -= step;
            if step.abs() < 1e-15 {
                break;
            }
        }
        points.push((x, 2.0 / ((1.0 - x * x) * slope * slope)));
    }
    points
}

/// The one-dimensional integrals the plate's energies are built from, for `count` functions.
struct Line {
    /// `integral of X_i * X_k`
    a0: Vec<Vec<f64>>,
    /// `integral of X_i' * X_k'`
    a1: Vec<Vec<f64>>,
    /// `integral of X_i'' * X_k''`
    a2: Vec<Vec<f64>>,
    /// `integral of X_i'' * X_k`
    b: Vec<Vec<f64>>,
}

fn line(count: usize, q: usize) -> Line {
    let zero = vec![vec![0.0; count]; count];
    let mut out = Line {
        a0: zero.clone(),
        a1: zero.clone(),
        a2: zero.clone(),
        b: zero,
    };
    for (x, weight) in gauss_legendre(count + 2 * q + 2) {
        let (value, slope, curve) = basis_at(x, count, q);
        for i in 0..count {
            for k in 0..count {
                out.a0[i][k] += weight * value[i] * value[k];
                out.a1[i][k] += weight * slope[i] * slope[k];
                out.a2[i][k] += weight * curve[i] * curve[k];
                out.b[i][k] += weight * curve[i] * value[k];
            }
        }
    }
    out
}

/// Eigenvalues and eigenvectors of a symmetric matrix by cyclic Jacobi rotations. Returns the
/// eigenvalues and a matrix whose column `k` is eigenvector `k`.
fn symmetric_eigen(rows: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = rows.len();
    let mut a: Vec<f64> = rows.into_iter().flatten().collect();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let scale: f64 = (0..n)
        .map(|i| a[i * n + i].abs())
        .fold(0.0, f64::max)
        .max(1e-300);
    for _sweep in 0..60 {
        let mut off = 0.0;
        for i in 0..n {
            for j in i + 1..n {
                off += a[i * n + j] * a[i * n + j];
            }
        }
        if off.sqrt() < 1e-13 * scale {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let pivot = a[p * n + q];
                if pivot.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * pivot);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (x, y) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * x - s * y;
                    a[k * n + q] = s * x + c * y;
                }
                for k in 0..n {
                    let (x, y) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * x - s * y;
                    a[q * n + k] = s * x + c * y;
                }
                for k in 0..n {
                    let (x, y) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * x - s * y;
                    v[k * n + q] = s * x + c * y;
                }
            }
        }
    }
    (
        (0..n).map(|i| a[i * n + i]).collect(),
        v.chunks(n.max(1)).map(<[f64]>::to_vec).collect(),
    )
}

/// Solves `K c = lambda M c` for symmetric `K` and positive definite `M`. Returns `lambda` and
/// vectors with `c' M c = 1`, one per row.
fn generalized_eigen(k: &[Vec<f64>], m: &[Vec<f64>]) -> Result<Vec<(f64, Vec<f64>)>, String> {
    let n = k.len();
    // Cholesky: M = L L'.
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = m[i][j];
            for p in 0..j {
                sum -= l[i][p] * l[j][p];
            }
            if i == j {
                if sum <= 0.0 {
                    return Err("the panel's mass matrix is not positive definite.".into());
                }
                l[i][i] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    // C = inv(L) K inv(L)': first solve L Y = K, then L C' = Y'.
    let forward = |rhs: &[f64]| -> Vec<f64> {
        let mut x = vec![0.0; n];
        for i in 0..n {
            let mut sum = rhs[i];
            for p in 0..i {
                sum -= l[i][p] * x[p];
            }
            x[i] = sum / l[i][i];
        }
        x
    };
    let y: Vec<Vec<f64>> = (0..n)
        .map(|col| forward(&(0..n).map(|row| k[row][col]).collect::<Vec<_>>()))
        .collect();
    // y[col][row] = (inv(L) K)[row][col]. C[row][col] = (inv(L) K inv(L)')[row][col].
    let mut c = vec![vec![0.0; n]; n];
    for row in 0..n {
        let solved = forward(&(0..n).map(|col| y[col][row]).collect::<Vec<_>>());
        c[row][..n].copy_from_slice(&solved[..n]);
    }
    for i in 0..n {
        for j in 0..i {
            let mean = 0.5 * (c[i][j] + c[j][i]);
            c[i][j] = mean;
            c[j][i] = mean;
        }
    }
    let (values, vectors) = symmetric_eigen(c);
    let mut out = Vec::with_capacity(n);
    for (index, value) in values.iter().enumerate() {
        // c = inv(L') y.
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let mut sum = vectors[i][index];
            for p in i + 1..n {
                sum -= l[p][i] * x[p];
            }
            x[i] = sum / l[i][i];
        }
        out.push((*value, x));
    }
    Ok(out)
}

impl PlateModes {
    /// Resonances and shapes with polynomials up to degree `degree - 1` along the longer side
    /// and in proportion along the shorter one.
    pub fn solve(plate: Plate, degree: usize) -> Result<Self, String> {
        if !(plate.width_m > 0.0
            && plate.height_m > 0.0
            && plate.thickness_m > 0.0
            && plate.youngs_pa > 0.0
            && plate.density_kg_m3 > 0.0
            && (0.0..0.5).contains(&plate.poisson))
        {
            return Err(
                "the panel needs a size, a thickness, a stiffness, a density, and a \
                        Poisson's ratio between 0 and 0.5."
                    .into(),
            );
        }
        let (a, b) = (plate.width_m, plate.height_m);
        let longer = a.max(b);
        let count = |side: f64| ((degree as f64 * side / longer).round() as usize).clamp(6, degree);
        let (count_u, count_v) = (count(a), count(b));
        let q = plate.edges.power();
        let (line_u, line_v) = (line(count_u, q), line(count_v, q));
        let d = plate.stiffness();
        let mass = plate.mass_per_area();
        let area = a * b / 4.0;
        let (su, sv) = (2.0 / a, 2.0 / b);
        let mut modes: Vec<Mode> = Vec::new();
        // Polynomials up to degree n resolve about 0.6 * n half-waves across a side within 1 %
        // (found against the simply supported closed form; the tests hold it there).
        let half_waves = |count: usize| ((count + q) as f64 * 0.6 - 1.2).max(1.0);
        let wave = ((half_waves(count_u) / a).min(half_waves(count_v) / b)).powi(2);
        let resolved_up_to_hz = PI / 2.0 * (d / mass).sqrt() * wave;
        for parity_u in 0..2 {
            for parity_v in 0..2 {
                let degrees_u: Vec<usize> = (0..count_u).filter(|i| i % 2 == parity_u).collect();
                let degrees_v: Vec<usize> = (0..count_v).filter(|j| j % 2 == parity_v).collect();
                let n = degrees_u.len() * degrees_v.len();
                let mut k = vec![vec![0.0; n]; n];
                let mut m = vec![vec![0.0; n]; n];
                for (ia, &i) in degrees_u.iter().enumerate() {
                    for (ja, &j) in degrees_v.iter().enumerate() {
                        let row = ia * degrees_v.len() + ja;
                        for (ib, &p) in degrees_u.iter().enumerate() {
                            for (jb, &r) in degrees_v.iter().enumerate() {
                                let col = ib * degrees_v.len() + jb;
                                let bend = su.powi(4) * line_u.a2[i][p] * line_v.a0[j][r]
                                    + sv.powi(4) * line_u.a0[i][p] * line_v.a2[j][r];
                                let cross = su
                                    * su
                                    * sv
                                    * sv
                                    * (line_u.b[i][p] * line_v.b[r][j]
                                        + line_u.b[p][i] * line_v.b[j][r]);
                                let twist = su * su * sv * sv * line_u.a1[i][p] * line_v.a1[j][r];
                                k[row][col] = d
                                    * area
                                    * (bend
                                        + plate.poisson * cross
                                        + 2.0 * (1.0 - plate.poisson) * twist);
                                m[row][col] = mass * area * line_u.a0[i][p] * line_v.a0[j][r];
                            }
                        }
                    }
                }
                for (lambda, coefficients) in generalized_eigen(&k, &m)? {
                    let freq_hz = lambda.max(0.0).sqrt() / (2.0 * PI);
                    if freq_hz <= resolved_up_to_hz {
                        modes.push(Mode {
                            freq_hz,
                            degrees_u: degrees_u.clone(),
                            degrees_v: degrees_v.clone(),
                            coefficients,
                        });
                    }
                }
            }
        }
        modes.sort_by(|x, y| x.freq_hz.total_cmp(&y.freq_hz));
        Ok(PlateModes {
            plate,
            modes,
            resolved_up_to_hz,
        })
    }

    pub fn plate(&self) -> &Plate {
        &self.plate
    }

    pub fn len(&self) -> usize {
        self.modes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    /// Resonance frequencies in Hz, lowest first. A free plate starts with three at zero: it
    /// can move and tilt as a whole.
    pub fn frequencies_hz(&self) -> Vec<f64> {
        self.modes.iter().map(|mode| mode.freq_hz).collect()
    }

    /// Every mode's deflection at the point `(u, v)` m from the plate's lower-left corner.
    pub fn shapes_at(&self, u_m: f64, v_m: f64) -> Vec<f64> {
        let q = self.plate.edges.power();
        let xi = 2.0 * u_m / self.plate.width_m - 1.0;
        let eta = 2.0 * v_m / self.plate.height_m - 1.0;
        let highest = |pick: fn(&Mode) -> &Vec<usize>| {
            self.modes
                .iter()
                .filter_map(|mode| pick(mode).last().copied())
                .max()
                .map_or(1, |degree| degree + 1)
        };
        let (along_u, _, _) = basis_at(xi, highest(|mode| &mode.degrees_u), q);
        let (along_v, _, _) = basis_at(eta, highest(|mode| &mode.degrees_v), q);
        self.modes
            .iter()
            .map(|mode| {
                let mut sum = 0.0;
                for (a, &i) in mode.degrees_u.iter().enumerate() {
                    let mut inner = 0.0;
                    for (b, &j) in mode.degrees_v.iter().enumerate() {
                        inner += mode.coefficients[a * mode.degrees_v.len() + b] * along_v[j];
                    }
                    sum += along_u[i] * inner;
                }
                sum
            })
            .collect()
    }
}
