//! The `lumped` model for several chambers (README sections 5.3 and 6.2): chambers are
//! compliances at the nodes of a network, and ducts are air masses on its edges, either
//! between two chambers or from a chamber to the outside.

use num_complex::Complex64;
use seatlock_core::constants::RHO0;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Duct {
    pub from: usize,
    /// `None` for a duct that opens to the outside.
    pub to: Option<usize>,
    pub area_m2: f64,
    pub length_eff_m: f64,
    /// When set, this outside branch is a passive radiator (README §5): `length_eff_m` is
    /// ignored and the PR acoustic impedance is used instead.
    pub passive: Option<crate::passive_radiator::PassiveRadiatorMech>,
}

impl Duct {
    /// A pure air-mass duct (no passive radiator).
    pub fn mass(from: usize, to: Option<usize>, area_m2: f64, length_eff_m: f64) -> Self {
        Self {
            from,
            to,
            area_m2,
            length_eff_m,
            passive: None,
        }
    }
}

pub struct NetworkSample {
    /// Pressure in the driver's chamber per unit volume velocity pushed in.
    pub z_ab: Complex64,
    /// `H_k` for each duct to the outside, in the order the ducts were given.
    pub h_out: Vec<Complex64>,
    /// Pressure in every chamber per unit volume velocity pushed in, by chamber.
    pub pressures: Vec<Complex64>,
}

/// Solves the network at one frequency with `U_in = 1` pushed into `driver`.
pub fn lumped_network(
    volumes_m3: &[f64],
    ducts: &[Duct],
    driver: usize,
    freq_hz: f64,
) -> NetworkSample {
    lumped_network_with_walls(volumes_m3, ducts, driver, freq_hz, &[], &[], &[])
}

/// [`lumped_network`] with each chamber's outer walls a shunt and partitions as mutual
/// admittances: `walls[i]` is the outward volume velocity of chamber `i`'s outer walls per
/// pascal inside; each `partitions` entry `(a, b, Y)` is volume velocity from `a` into `b`
/// per pascal of `(P_a - P_b)` (README 5.11). Missing wall entries are rigid.
/// `fill_sigma[i]` is the chamber's fibrous flow resistivity (Pa·s/m²) when filled; Delany–
/// Bazley complex bulk modulus then replaces `ρ₀ c₀²` in that chamber's compliance.
pub fn lumped_network_with_walls(
    volumes_m3: &[f64],
    ducts: &[Duct],
    driver: usize,
    freq_hz: f64,
    walls: &[Complex64],
    partitions: &[(usize, usize, Complex64)],
    fill_sigma: &[Option<f64>],
) -> NetworkSample {
    let n = volumes_m3.len();
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let zero = Complex64::new(0.0, 0.0);
    let mut matrix = vec![vec![zero; n]; n];
    for (i, volume) in volumes_m3.iter().enumerate() {
        let sigma = fill_sigma.get(i).copied().flatten();
        matrix[i][i] += crate::fill::chamber_admittance(*volume, freq_hz, sigma);
        matrix[i][i] += walls.get(i).copied().unwrap_or_default();
    }
    for &(a, b, y) in partitions {
        if a >= n || b >= n || a == b {
            continue;
        }
        matrix[a][a] += y;
        matrix[b][b] += y;
        matrix[a][b] -= y;
        matrix[b][a] -= y;
    }
    let impedance = |duct: &Duct| {
        if let Some(pr) = duct.passive {
            return pr.acoustic_impedance(freq_hz);
        }
        Complex64::new(
            0.0,
            omega * RHO0 * duct.length_eff_m / duct.area_m2.max(1e-12),
        )
    };
    for duct in ducts {
        let y = 1.0 / impedance(duct);
        matrix[duct.from][duct.from] += y;
        if let Some(to) = duct.to {
            matrix[to][to] += y;
            matrix[duct.from][to] -= y;
            matrix[to][duct.from] -= y;
        }
    }
    let mut rhs = vec![zero; n];
    rhs[driver] = Complex64::new(1.0, 0.0);
    let pressure = solve(matrix, rhs);
    let h_out = ducts
        .iter()
        .filter(|duct| duct.to.is_none())
        .map(|duct| pressure[duct.from] / impedance(duct))
        .collect();
    NetworkSample {
        z_ab: pressure[driver],
        h_out,
        pressures: pressure,
    }
}

/// Gaussian elimination with partial pivoting; the networks here have a few nodes.
fn solve(mut a: Vec<Vec<Complex64>>, mut b: Vec<Complex64>) -> Vec<Complex64> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].norm().partial_cmp(&a[j][col].norm()).unwrap())
            .unwrap();
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let pivot_row = a[col].clone();
            for (cell, value) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *cell -= factor * value;
            }
            let value = b[col];
            b[row] -= factor * value;
        }
    }
    let mut x = vec![Complex64::new(0.0, 0.0); n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for (coefficient, value) in a[row].iter().zip(&x).skip(row + 1) {
            sum -= coefficient * value;
        }
        x[row] = sum / a[row][row];
    }
    x
}
