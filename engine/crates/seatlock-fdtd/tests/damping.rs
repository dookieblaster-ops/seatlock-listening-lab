//! Damping material in the 3D solver (README 5.5): a flow resistivity on the faces of the
//! damped cells, against the closed form of a resistive duct.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_fdtd::grid::{self, Grid, Options};

/// A duct 0.4 m long, 4 x 4 cells of 10 mm, driven over its whole end, rigid at the far end;
/// every face between two cells carries `sigma`.
fn duct(sigma: f64, freqs: &[f64]) -> Vec<Complex64> {
    let n = [40usize, 4, 4];
    let mut grid = Grid {
        n,
        dx: 0.01,
        origin_m: [0.0; 3],
        air: vec![true; n[0] * n[1] * n[2]],
        source: Vec::new(),
        absorbing: Vec::new(),
        openings: Vec::new(),
        true_volume_m3: 0.4 * 0.04 * 0.04,
    };
    for j in 0..4 {
        for k in 0..4 {
            grid.source.push(grid.cell(0, j, k));
        }
    }
    let mut damping = Vec::new();
    if sigma > 0.0 {
        for i in 1..n[0] {
            for j in 0..n[1] {
                for k in 0..n[2] {
                    damping.push((0u8, grid.cell(i, j, k), sigma as f32));
                }
            }
        }
    }
    let dt = grid.time_step_s();
    let response = grid::run(
        &grid,
        &Options {
            max_steps: (2.0 / dt) as usize,
            frequencies_hz: freqs.to_vec(),
            progress_every: 0,
            extrapolate: false,
            z_scale: Vec::new(),
            relaxed: Vec::new(),
            loss_per_s: Vec::new(),
            damping,
        },
        &mut |_, _| true,
    );
    response
        .z_real
        .iter()
        .zip(&response.z_imag)
        .map(|(re, im)| Complex64::new(*re, *im))
        .collect()
}

/// A closed duct of length `L` with the Rayleigh medium's density `rho - j sigma / omega`:
/// `p(x) = A cos(k_c (L - x))`, `U(x) = j A sin(k_c (L - x)) / Z_c`. The solver records the
/// pressure at the first cells' centres, half a cell in, and the flow at the driven face, so
/// the closed form is `p(dx / 2) / U(0) = -j Z_c cos(k_c (L - dx / 2)) / sin(k_c L)`.
fn closed_form(sigma: f64, f: f64) -> Complex64 {
    let (l, s, half) = (0.4, 0.04 * 0.04, 0.005);
    let w = 2.0 * std::f64::consts::PI * f;
    let rho = Complex64::new(RHO0, -sigma / w);
    let bulk = RHO0 * C0 * C0;
    let k = w * (rho / bulk).sqrt();
    let zc = (rho * bulk).sqrt() / s;
    let j = Complex64::new(0.0, 1.0);
    -j * zc * (k * (l - half)).cos() / (k * l).sin()
}

#[test]
fn a_damped_duct_matches_the_resistive_closed_form() {
    let freqs = [60.0, 150.0, 300.0, 420.0, 600.0];
    for sigma in [0.0, 2000.0, 10000.0] {
        let got = duct(sigma, &freqs);
        for (f, z) in freqs.iter().zip(&got) {
            // Without damping the duct's half-wave resonance (429 Hz) has no width, so
            // values near it are not comparable; the lossless case is checked below 400 Hz.
            if sigma == 0.0 && *f > 400.0 {
                continue;
            }
            let want = closed_form(sigma, *f);
            let err = (z - want).norm() / want.norm();
            eprintln!(
                "sigma {sigma} f {f}: {z:.4e} against {want:.4e}, {:.2} %",
                err * 100.0
            );
            assert!(
                err < 0.005,
                "sigma {sigma} at {f} Hz: off by {:.1} %",
                err * 100.0
            );
        }
    }
}
