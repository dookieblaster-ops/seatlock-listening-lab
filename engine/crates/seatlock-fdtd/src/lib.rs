//! Finite-difference time-domain solver for cabinet air.

use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

/// Rigid rectangular cavity with a resistive source patch on the low-x wall.
pub struct Cavity {
    pub size_m: [f64; 3],
    pub dx: f64,
    /// Cell origin of the patch on the wall, in (y, z) cells.
    pub patch_origin: [usize; 2],
    pub patch_cells: [usize; 2],
}

#[derive(Clone, Debug)]
pub struct ImpedanceCurve {
    pub frequencies_hz: Vec<f64>,
    pub real: Vec<f64>,
    pub imag: Vec<f64>,
    pub steps: usize,
    pub elapsed_ms: f64,
    pub grid_volume_m3: f64,
    pub true_volume_m3: f64,
    pub warning: Option<String>,
    pub ignored: Vec<String>,
}

pub fn run_cavity(cavity: &Cavity) -> ImpedanceCurve {
    let nx = (cavity.size_m[0] / cavity.dx).round() as usize;
    let ny = (cavity.size_m[1] / cavity.dx).round() as usize;
    let nz = (cavity.size_m[2] / cavity.dx).round() as usize;
    let mut source = Vec::new();
    for j in cavity.patch_origin[0]..cavity.patch_origin[0] + cavity.patch_cells[0] {
        for k in cavity.patch_origin[1]..cavity.patch_origin[1] + cavity.patch_cells[1] {
            if j < ny && k < nz {
                source.push([0, j, k]);
            }
        }
    }
    let true_volume = cavity.size_m[0] * cavity.size_m[1] * cavity.size_m[2];
    run_grid(nx, ny, nz, cavity.dx, &source, true_volume)
}

/// Closed box of air. Source cells sit on the low-x wall and use that face as the resistive source.
pub fn run_grid(
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    source_cells: &[[usize; 3]],
    true_volume_m3: f64,
) -> ImpedanceCurve {
    let started = clock_start();
    let mut is_source = vec![false; nx * ny * nz];
    for cell in source_cells {
        if cell[0] < nx && cell[1] < ny && cell[2] < nz {
            is_source[(cell[0] * ny + cell[1]) * nz + cell[2]] = true;
        }
    }
    let dt = 0.95 * dx / (C0 * 3.0_f64.sqrt());
    let kp = RHO0 * C0 * C0 * dt / dx;
    let y_face = 1.0 / (RHO0 * C0);
    let rho_c = RHO0 * C0;

    let mut pressure = vec![0.0_f32; nx * ny * nz];
    let mut next = vec![0.0_f32; nx * ny * nz];
    let mut vx = vec![0.0_f32; (nx + 1) * ny * nz];
    let mut vy = vec![0.0_f32; nx * (ny + 1) * nz];
    let mut vz = vec![0.0_f32; nx * ny * (nz + 1)];

    let sigma = 8.0 * dt;
    let t0 = 4.0 * sigma;
    let mut samples_p = Vec::new();
    let mut samples_u = Vec::new();
    let mut peak = 0.0_f64;
    let mut steps = 0usize;
    let max_steps = 40_000;

    let p_at =
        |pressure: &[f32], i: usize, j: usize, k: usize| -> f32 { pressure[(i * ny + j) * nz + k] };

    while steps < max_steps {
        let time = steps as f64 * dt;
        let pulse = (-((time - t0) / sigma).powi(2)).exp();
        // Interior face velocities. Positive velocity points toward +axis.
        for i in 1..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let ahead = p_at(&pressure, i, j, k);
                    let behind = p_at(&pressure, i - 1, j, k);
                    let slot = (i * ny + j) * nz + k;
                    vx[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind) as f64) as f32;
                }
            }
        }
        for i in 0..nx {
            for j in 1..ny {
                for k in 0..nz {
                    let ahead = p_at(&pressure, i, j, k);
                    let behind = p_at(&pressure, i, j - 1, k);
                    let slot = (i * (ny + 1) + j) * nz + k;
                    vy[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind) as f64) as f32;
                }
            }
        }
        for i in 0..nx {
            for j in 0..ny {
                for k in 1..nz {
                    let ahead = p_at(&pressure, i, j, k);
                    let behind = p_at(&pressure, i, j, k - 1);
                    let slot = (i * ny + j) * (nz + 1) + k;
                    vz[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind) as f64) as f32;
                }
            }
        }

        let mut p_sum = 0.0_f64;
        let mut u_sum = 0.0_f64;
        let mut faces = 0.0_f64;
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let mut net = 0.0_f64;
                    if i < nx - 1 {
                        net += vx[((i + 1) * ny + j) * nz + k] as f64;
                    }
                    if i > 0 {
                        net -= vx[(i * ny + j) * nz + k] as f64;
                    }
                    if j < ny - 1 {
                        net += vy[(i * (ny + 1) + (j + 1)) * nz + k] as f64;
                    }
                    if j > 0 {
                        net -= vy[(i * (ny + 1) + j) * nz + k] as f64;
                    }
                    if k < nz - 1 {
                        net += vz[(i * ny + j) * (nz + 1) + (k + 1)] as f64;
                    }
                    if k > 0 {
                        net -= vz[(i * ny + j) * (nz + 1) + k] as f64;
                    }

                    let on_patch = is_source[(i * ny + j) * nz + k];
                    let y_sum = if on_patch { y_face } else { 0.0 };
                    let v_source = if on_patch { pulse } else { 0.0 };
                    let g = 0.5 * kp * y_sum;
                    let old = p_at(&pressure, i, j, k) as f64;
                    let updated = (old * (1.0 - g) - kp * net + kp * v_source) / (1.0 + g);
                    next[(i * ny + j) * nz + k] = updated as f32;
                    if on_patch {
                        let mean = 0.5 * (updated + old);
                        let v_net = v_source - mean / rho_c;
                        p_sum += mean;
                        u_sum += v_net * dx * dx;
                        faces += 1.0;
                    }
                }
            }
        }
        std::mem::swap(&mut pressure, &mut next);
        let aperture_p = if faces > 0.0 { p_sum / faces } else { 0.0 };
        samples_p.push(aperture_p);
        samples_u.push(u_sum);
        peak = peak.max(aperture_p.abs());
        steps += 1;
        if time > t0 + 20.0 * sigma && peak > 0.0 && aperture_p.abs() < 1e-4 * peak && steps > 500 {
            // Keep going until the whole tail is quiet, checked on a short horizon below.
        }
        if time > t0 + 30.0 * sigma && peak > 0.0 {
            let quiet = samples_p
                .iter()
                .rev()
                .take(200)
                .all(|sample| sample.abs() < 1e-4 * peak);
            if quiet && steps > 800 {
                break;
            }
        }
    }

    let n = samples_p.len().next_power_of_two().max(8192);
    let mut spec_p = vec![0.0; n];
    let mut spec_u_re = vec![0.0; n];
    let mut spec_u_im = vec![0.0; n];
    let mut spec_p_im = vec![0.0; n];
    for (index, (p, u)) in samples_p.iter().zip(&samples_u).enumerate() {
        spec_p[index] = *p;
        spec_u_re[index] = *u;
    }
    fft(&mut spec_p, &mut spec_p_im);
    fft(&mut spec_u_re, &mut spec_u_im);

    let mut frequencies_hz = Vec::new();
    let mut real = Vec::new();
    let mut imag = Vec::new();
    let bins = n / 2;
    for bin in 1..bins {
        let freq = bin as f64 / (n as f64 * dt);
        if freq > C0 / (10.0 * dx) {
            break;
        }
        let p = num_complex(spec_p[bin], spec_p_im[bin]);
        let u = num_complex(spec_u_re[bin], spec_u_im[bin]);
        let z = if u.0 * u.0 + u.1 * u.1 > 1e-30 {
            div(p, u)
        } else {
            (0.0, 0.0)
        };
        frequencies_hz.push(freq);
        real.push(z.0);
        imag.push(z.1);
    }

    let grid_volume_m3 = nx as f64 * ny as f64 * nz as f64 * dx * dx * dx;
    let (warning, mut ignored) = volume_warning(
        &frequencies_hz,
        &real,
        &imag,
        grid_volume_m3,
        true_volume_m3,
        dx * (nx.max(ny).max(nz) as f64),
    );
    ignored.push(PEAK_HEIGHTS_NOT_MODELED.into());
    ImpedanceCurve {
        frequencies_hz,
        real,
        imag,
        steps,
        elapsed_ms: elapsed_ms(started),
        grid_volume_m3,
        true_volume_m3,
        warning,
        ignored,
    }
}

fn clock_start() -> Option<std::time::Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(std::time::Instant::now())
    }
}

fn elapsed_ms(started: Option<std::time::Instant>) -> f64 {
    started
        .map(|instant| instant.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn volume_warning(
    frequencies_hz: &[f64],
    real: &[f64],
    imag: &[f64],
    grid_volume_m3: f64,
    true_volume_m3: f64,
    longest_m: f64,
) -> (Option<String>, Vec<String>) {
    let limit_hz = C0 / (50.0 * longest_m.max(dx_guard(longest_m)));
    let compliance = true_volume_m3 / (RHO0 * C0 * C0);
    let mut worst = 0.0_f64;
    for index in 0..frequencies_hz.len() {
        let freq = frequencies_hz[index];
        if freq <= 0.0 || freq >= limit_hz {
            continue;
        }
        let lumped = 1.0 / (2.0 * PI * freq * compliance);
        let magnitude = (real[index] * real[index] + imag[index] * imag[index]).sqrt();
        if lumped > 0.0 {
            worst = worst.max((magnitude - lumped).abs() / lumped);
        }
    }
    if worst > 0.03 {
        let percent = (grid_volume_m3 - true_volume_m3).abs() / true_volume_m3 * 100.0;
        let warning = format!(
            "The 3D grid holds {:.3} L of air and the box holds {:.3} L, so the low-frequency result is off by {:.1}%.",
            grid_volume_m3 * 1000.0,
            true_volume_m3 * 1000.0,
            worst * 100.0
        );
        let ignored = vec![format!("air volume on the grid differs by {percent:.1}%")];
        (Some(warning), ignored)
    } else {
        (None, Vec::new())
    }
}

fn dx_guard(longest_m: f64) -> f64 {
    longest_m.max(1e-6)
}

fn num_complex(re: f64, im: f64) -> (f64, f64) {
    (re, im)
}

fn div(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let den = b.0 * b.0 + b.1 * b.1;
    ((a.0 * b.0 + a.1 * b.1) / den, (a.1 * b.0 - a.0 * b.1) / den)
}

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let angle = -2.0 * PI / len as f64;
        let (wlen_re, wlen_im) = (angle.cos(), angle.sin());
        let mut i = 0usize;
        while i < n {
            let mut w_re = 1.0;
            let mut w_im = 0.0;
            for k in 0..len / 2 {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let v_im = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + len / 2] = u_re - v_re;
                im[i + k + len / 2] = u_im - v_im;
                let next_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = next_re;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Cells on the low-x wall whose centers lie inside a circle. Coordinates are in the air grid.
pub fn aperture_cells(
    ny: usize,
    nz: usize,
    dx: f64,
    center_yz: [f64; 2],
    radius: f64,
) -> Vec<[usize; 3]> {
    let mut cells = Vec::new();
    for j in 0..ny {
        for k in 0..nz {
            let y = (j as f64 + 0.5) * dx;
            let z = (k as f64 + 0.5) * dx;
            let dy = y - center_yz[0];
            let dz = z - center_yz[1];
            if dy * dy + dz * dz <= radius * radius {
                cells.push([0, j, k]);
            }
        }
    }
    if cells.is_empty() && ny > 0 && nz > 0 {
        cells.push([0, ny / 2, nz / 2]);
    }
    cells
}

/// Through Level 2 the air and walls are lossless, so a resonance is damped only by radiation
/// and the absorbing faces, and its height is not a property of the cabinet (README 5.5).
pub const PEAK_HEIGHTS_NOT_MODELED: &str =
    "resonance peak heights are not modeled: the 3D model has no losses yet";

#[derive(Clone, Debug)]
pub struct OpeningCurve {
    pub frequencies_hz: Vec<f64>,
    pub z_real: Vec<f64>,
    pub z_imag: Vec<f64>,
    pub h_real: Vec<f64>,
    pub h_imag: Vec<f64>,
    pub steps: usize,
    pub elapsed_ms: f64,
}

/// CHK-FDTD-3 cavity: 0.20 m cube, 40 mm neck, 100 mm long, exterior block.
/// `exterior_widths` is how many opening widths the block extends past the mouth.
pub fn helmholtz_neck(exterior_widths: f64) -> OpeningCurve {
    let dx = 0.01;
    let opening = 0.04;
    let margin = exterior_widths * opening;
    let extra = (margin - 0.08).max(0.0);
    let yz = 0.20 + 2.0 * extra;
    let nx_cavity = 20usize;
    let nx_neck = 10usize;
    let nx_ext = (margin / dx).round() as usize;
    let nx = nx_cavity + nx_neck + nx_ext;
    let ny = (yz / dx).round() as usize;
    let nz = ny;
    let inset = (ny - 20) / 2;
    let mut air = vec![false; nx * ny * nz];
    let mut exterior = vec![false; nx * ny * nz];
    let mut source = vec![false; nx * ny * nz];
    let cell = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
    for i in 0..nx_cavity {
        for j in inset..inset + 20 {
            for k in inset..inset + 20 {
                air[cell(i, j, k)] = true;
            }
        }
    }
    let neck0 = ny / 2 - 2;
    for i in nx_cavity..nx_cavity + nx_neck {
        for j in neck0..neck0 + 4 {
            for k in neck0..neck0 + 4 {
                air[cell(i, j, k)] = true;
            }
        }
    }
    for i in nx_cavity + nx_neck..nx {
        for j in 0..ny {
            for k in 0..nz {
                let index = cell(i, j, k);
                air[index] = true;
                exterior[index] = true;
            }
        }
    }
    let source0 = ny / 2 - 3;
    for j in source0..source0 + 6 {
        for k in source0..source0 + 6 {
            if j < ny && k < nz {
                source[cell(0, j, k)] = true;
            }
        }
    }
    let port_i = nx_cavity + nx_neck;
    run_masked(nx, ny, nz, dx, &air, &exterior, &source, port_i)
}

#[allow(clippy::too_many_arguments)]
fn run_masked(
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    air: &[bool],
    exterior: &[bool],
    source: &[bool],
    port_i: usize,
) -> OpeningCurve {
    let started = clock_start();
    let cell = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
    let dt = 0.95 * dx / (C0 * 3.0_f64.sqrt());
    let kp = RHO0 * C0 * C0 * dt / dx;
    let y_face = 1.0 / (RHO0 * C0);
    let rho_c = RHO0 * C0;
    let mut pressure = vec![0.0_f32; nx * ny * nz];
    let mut next = vec![0.0_f32; nx * ny * nz];
    let mut vx = vec![0.0_f32; (nx + 1) * ny * nz];
    let mut vy = vec![0.0_f32; nx * (ny + 1) * nz];
    let mut vz = vec![0.0_f32; nx * ny * (nz + 1)];
    let sigma = 8.0 * dt;
    let t0 = 4.0 * sigma;
    let mut samples_p = Vec::new();
    let mut samples_u = Vec::new();
    let mut samples_port = Vec::new();
    let mut peak = 0.0_f64;
    let mut steps = 0usize;
    let max_steps = 24_000usize;
    while steps < max_steps {
        let time = steps as f64 * dt;
        let pulse = (-((time - t0) / sigma).powi(2)).exp();
        for i in 1..nx {
            for j in 0..ny {
                for k in 0..nz {
                    if air[cell(i - 1, j, k)] && air[cell(i, j, k)] {
                        let ahead = pressure[cell(i, j, k)] as f64;
                        let behind = pressure[cell(i - 1, j, k)] as f64;
                        let slot = (i * ny + j) * nz + k;
                        vx[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind)) as f32;
                    }
                }
            }
        }
        for i in 0..nx {
            for j in 1..ny {
                for k in 0..nz {
                    if air[cell(i, j - 1, k)] && air[cell(i, j, k)] {
                        let ahead = pressure[cell(i, j, k)] as f64;
                        let behind = pressure[cell(i, j - 1, k)] as f64;
                        let slot = (i * (ny + 1) + j) * nz + k;
                        vy[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind)) as f32;
                    }
                }
            }
        }
        for i in 0..nx {
            for j in 0..ny {
                for k in 1..nz {
                    if air[cell(i, j, k - 1)] && air[cell(i, j, k)] {
                        let ahead = pressure[cell(i, j, k)] as f64;
                        let behind = pressure[cell(i, j, k - 1)] as f64;
                        let slot = (i * ny + j) * (nz + 1) + k;
                        vz[slot] += (-(dt / (RHO0 * dx)) * (ahead - behind)) as f32;
                    }
                }
            }
        }
        let mut p_sum = 0.0;
        let mut u_sum = 0.0;
        let mut faces = 0.0;
        let mut port = 0.0;
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    if !air[cell(i, j, k)] {
                        continue;
                    }
                    let mut net = 0.0;
                    if i + 1 < nx && air[cell(i + 1, j, k)] {
                        net += vx[((i + 1) * ny + j) * nz + k] as f64;
                    }
                    if i > 0 && air[cell(i - 1, j, k)] {
                        net -= vx[(i * ny + j) * nz + k] as f64;
                    }
                    if j + 1 < ny && air[cell(i, j + 1, k)] {
                        net += vy[(i * (ny + 1) + (j + 1)) * nz + k] as f64;
                    }
                    if j > 0 && air[cell(i, j - 1, k)] {
                        net -= vy[(i * (ny + 1) + j) * nz + k] as f64;
                    }
                    if k + 1 < nz && air[cell(i, j, k + 1)] {
                        net += vz[(i * ny + j) * (nz + 1) + (k + 1)] as f64;
                    }
                    if k > 0 && air[cell(i, j, k - 1)] {
                        net -= vz[(i * ny + j) * (nz + 1) + k] as f64;
                    }
                    let on_source = source[cell(i, j, k)];
                    let mut y_sum = if on_source { y_face } else { 0.0 };
                    if exterior[cell(i, j, k)] {
                        if i == 0 || i + 1 == nx {
                            y_sum += y_face;
                        }
                        if j == 0 || j + 1 == ny {
                            y_sum += y_face;
                        }
                        if k == 0 || k + 1 == nz {
                            y_sum += y_face;
                        }
                    }
                    let v_source = if on_source { pulse } else { 0.0 };
                    let g = 0.5 * kp * y_sum;
                    let old = pressure[cell(i, j, k)] as f64;
                    let updated = (old * (1.0 - g) - kp * net + kp * v_source) / (1.0 + g);
                    next[cell(i, j, k)] = updated as f32;
                    if on_source {
                        let mean = 0.5 * (updated + old);
                        let v_net = v_source - mean / rho_c;
                        p_sum += mean;
                        u_sum += v_net * dx * dx;
                        faces += 1.0;
                    }
                }
            }
        }
        let neck0 = ny / 2 - 2;
        for j in neck0..neck0 + 4 {
            for k in neck0..neck0 + 4 {
                port += vx[(port_i * ny + j) * nz + k] as f64 * dx * dx;
            }
        }
        std::mem::swap(&mut pressure, &mut next);
        let aperture_p = if faces > 0.0 { p_sum / faces } else { 0.0 };
        samples_p.push(aperture_p);
        samples_u.push(u_sum);
        samples_port.push(port);
        peak = peak.max(aperture_p.abs());
        steps += 1;
        if time > t0 + 40.0 * sigma && peak > 0.0 {
            let quiet = samples_p
                .iter()
                .rev()
                .take(300)
                .all(|sample| sample.abs() < 1e-4 * peak);
            if quiet && steps > 1000 {
                break;
            }
        }
    }
    let n = samples_p.len().next_power_of_two().max(65_536);
    let mut p_re = vec![0.0; n];
    let mut p_im = vec![0.0; n];
    let mut u_re = vec![0.0; n];
    let mut u_im = vec![0.0; n];
    let mut h_re_t = vec![0.0; n];
    let mut h_im_t = vec![0.0; n];
    for (index, ((p, u), port)) in samples_p
        .iter()
        .zip(&samples_u)
        .zip(&samples_port)
        .enumerate()
    {
        p_re[index] = *p;
        u_re[index] = *u;
        h_re_t[index] = *port;
    }
    fft(&mut p_re, &mut p_im);
    fft(&mut u_re, &mut u_im);
    fft(&mut h_re_t, &mut h_im_t);
    let mut frequencies_hz = Vec::new();
    let mut z_real = Vec::new();
    let mut z_imag = Vec::new();
    let mut h_real = Vec::new();
    let mut h_imag = Vec::new();
    for bin in 1..n / 2 {
        let freq = bin as f64 / (n as f64 * dt);
        if freq > 400.0 {
            break;
        }
        let p = (p_re[bin], p_im[bin]);
        let u = (u_re[bin], u_im[bin]);
        let port = (h_re_t[bin], h_im_t[bin]);
        let z = if u.0 * u.0 + u.1 * u.1 > 1e-30 {
            div(p, u)
        } else {
            (0.0, 0.0)
        };
        let h = if u.0 * u.0 + u.1 * u.1 > 1e-30 {
            div(port, u)
        } else {
            (0.0, 0.0)
        };
        frequencies_hz.push(freq);
        z_real.push(z.0);
        z_imag.push(z.1);
        h_real.push(h.0);
        h_imag.push(h.1);
    }
    OpeningCurve {
        frequencies_hz,
        z_real,
        z_imag,
        h_real,
        h_imag,
        steps,
        elapsed_ms: elapsed_ms(started),
    }
}

pub fn magnitude_peaks(curve: &ImpedanceCurve) -> Vec<f64> {
    let mut peaks = Vec::new();
    for index in 1..curve.frequencies_hz.len().saturating_sub(1) {
        let mag = |i: usize| (curve.real[i] * curve.real[i] + curve.imag[i] * curve.imag[i]).sqrt();
        if mag(index) > mag(index - 1) && mag(index) >= mag(index + 1) && mag(index) > mag(0) * 3.0
        {
            peaks.push(curve.frequencies_hz[index]);
        }
    }
    peaks
}
