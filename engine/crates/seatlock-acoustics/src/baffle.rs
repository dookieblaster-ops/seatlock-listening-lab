//! Baffle radiation: piston directivity and edge diffraction. No `step` shelf.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

/// Bessel J1. The piston directivity check depends on it.
pub fn bessel_j1(x: f64) -> f64 {
    if x.abs() < 1e-8 {
        return x / 2.0;
    }
    // The power series loses its digits to cancellation for large arguments (a large piston,
    // or a cabinet wall, off axis at high frequency); there the asymptotic form of Numerical
    // Recipes' `bessj1` holds to about 1e-8.
    if x.abs() >= 12.0 {
        let ax = x.abs();
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 2.356_194_491;
        let p = 1.0
            + y * (0.183_105e-2
                + y * (-0.351_639_649_6e-4 + y * (0.245_752_017_4e-5 + y * (-0.240_337_019e-6))));
        let q = 0.046_874_999_95
            + y * (-0.200_269_087_3e-3
                + y * (0.844_919_909_6e-5 + y * (-0.882_289_87e-6 + y * 0.105_787_412e-6)));
        let value = (0.636_619_772 / ax).sqrt() * (xx.cos() * p - z * xx.sin() * q);
        return if x < 0.0 { -value } else { value };
    }
    let mut sum = 0.0;
    let mut term = x / 2.0;
    sum += term;
    for m in 1..30 {
        term *= -(x * x) / (4.0 * m as f64 * (m as f64 + 1.0));
        sum += term;
        if term.abs() < 1e-15 * sum.abs().max(1.0) {
            break;
        }
    }
    sum
}

/// `2 J1(x) / x`, which is 1 at x = 0.
pub fn piston_directivity(k_a_sin_theta: f64) -> f64 {
    let x = k_a_sin_theta.abs();
    if x < 1e-8 {
        return 1.0;
    }
    2.0 * bessel_j1(x) / x
}

#[derive(Clone, Copy, Debug)]
pub struct EdgePoint {
    pub position_m: [f64; 3],
}

/// Pressure relative to half-space (`B = 2`) for one source on a flat baffle.
/// `listener_m` and `source_m` are in the same coordinates. The outline is the baffle edge.
pub fn diffraction_factor(
    source_m: [f64; 3],
    listener_m: [f64; 3],
    outline: &[EdgePoint],
    freq_hz: f64,
) -> Complex64 {
    if outline.is_empty() {
        return Complex64::new(1.0, 0.0);
    }
    let r0 = distance(source_m, listener_m).max(1e-6);
    let k = 2.0 * PI * freq_hz / C0;
    let n = outline.len();
    let mut sum = Complex64::new(0.0, 0.0);
    for edge in outline {
        let d_i = distance(source_m, edge.position_m);
        let e_i = distance(edge.position_m, listener_m).max(1e-9);
        let phase = -k * (d_i + e_i - r0);
        sum += Complex64::from_polar(r0 / e_i, phase);
    }
    Complex64::new(1.0, 0.0) - sum / (2.0 * n as f64)
}

/// The front (or any constant-y) rectangular baffle edge, spaced evenly around the perimeter.
pub fn rectangular_outline(width_m: f64, height_m: f64, y_m: f64, count: usize) -> Vec<EdgePoint> {
    let count = count.max(64);
    let width = width_m.max(1e-4);
    let height = height_m.max(1e-4);
    let perimeter = 2.0 * (width + height);
    (0..count)
        .map(|index| {
            let distance_along = perimeter * index as f64 / count as f64;
            let (x, z) = if distance_along < width {
                (distance_along, 0.0)
            } else if distance_along < width + height {
                (width, distance_along - width)
            } else if distance_along < 2.0 * width + height {
                (width - (distance_along - width - height), height)
            } else {
                (0.0, height - (distance_along - 2.0 * width - height))
            };
            EdgePoint {
                position_m: [x, y_m, z],
            }
        })
        .collect()
}

/// Length of a closed outline through `corners`.
pub fn outline_perimeter_m(corners: &[[f64; 3]]) -> f64 {
    (0..corners.len())
        .map(|i| distance(corners[i], corners[(i + 1) % corners.len()]))
        .sum()
}

/// Any closed outline, given by its corners in order, spaced evenly into `count` edge elements
/// (at least 64). A curve is passed as many short straight edges.
pub fn polygon_outline(corners: &[[f64; 3]], count: usize) -> Vec<EdgePoint> {
    let count = count.max(64);
    if corners.len() < 2 {
        return Vec::new();
    }
    let perimeter = outline_perimeter_m(corners);
    let mut points = Vec::with_capacity(count);
    let mut edge = 0;
    let mut start_of_edge = 0.0;
    for index in 0..count {
        let along = perimeter * index as f64 / count as f64;
        loop {
            let a = corners[edge];
            let b = corners[(edge + 1) % corners.len()];
            let length = distance(a, b);
            if along <= start_of_edge + length || edge + 1 == corners.len() {
                let t = if length > 0.0 {
                    ((along - start_of_edge) / length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                points.push(EdgePoint {
                    position_m: [
                        a[0] + (b[0] - a[0]) * t,
                        a[1] + (b[1] - a[1]) * t,
                        a[2] + (b[2] - a[2]) * t,
                    ],
                });
                break;
            }
            start_of_edge += length;
            edge += 1;
        }
    }
    points
}

/// Edge elements for an outline of this perimeter at `freq_hz`: at least 64, and at least 4 per
/// wavelength.
pub fn outline_count_for_perimeter(perimeter_m: f64, freq_hz: f64) -> usize {
    let wavelength = C0 / freq_hz.max(1.0);
    ((4.0 * perimeter_m.max(0.0) / wavelength).ceil() as usize).max(64)
}

/// Where on-axis edge diffraction is 3 dB below half space, for a source at `source_m` on the
/// surface with these corners, heard straight out along `outward`.
pub fn baffle_step_hz_for_outline(
    corners: &[[f64; 3]],
    source_m: [f64; 3],
    outward: [f64; 3],
) -> f64 {
    let outline = polygon_outline(corners, 128);
    let listener = [
        source_m[0] + outward[0],
        source_m[1] + outward[1],
        source_m[2] + outward[2],
    ];
    let target = 10_f64.powf(-3.0 / 20.0);
    let mut previous = 30.0;
    for step in 0..60 {
        let freq = 30.0 * 1.1_f64.powi(step);
        let level = diffraction_factor(source_m, listener, &outline, freq).norm();
        if level >= target {
            return previous;
        }
        previous = freq;
    }
    previous
}

/// How many edge elements a baffle needs at `freq_hz`: at least 64, and at least 4 per wavelength.
pub fn outline_count(width_m: f64, height_m: f64, freq_hz: f64) -> usize {
    let perimeter = 2.0 * (width_m.max(0.0) + height_m.max(0.0));
    let wavelength = C0 / freq_hz.max(1.0);
    ((4.0 * perimeter / wavelength).ceil() as usize).max(64)
}

/// Frequency where on-axis edge diffraction is 3 dB below half space. The lowshelf uses this.
pub fn baffle_step_hz(width_m: f64, height_m: f64) -> f64 {
    let outline = rectangular_outline(width_m, height_m, 0.0, 96);
    let source = [width_m / 2.0, 0.0, height_m / 2.0];
    let listener = [width_m / 2.0, -1.0, height_m / 2.0];
    let target = 10_f64.powf(-3.0 / 20.0);
    let mut previous = 30.0;
    for step in 0..60 {
        let freq = 30.0 * 1.1_f64.powi(step);
        let level = diffraction_factor(source, listener, &outline, freq).norm();
        if level >= target {
            return previous;
        }
        previous = freq;
    }
    115.0 / width_m.max(0.05)
}

pub fn circular_outline(
    center: [f64; 3],
    radius_m: f64,
    normal: [f64; 3],
    count: usize,
) -> Vec<EdgePoint> {
    let count = count.max(64);
    let tangent = if normal[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = normalize(cross(normal, tangent));
    let v = normalize(cross(normal, u));
    (0..count)
        .map(|index| {
            let angle = 2.0 * PI * index as f64 / count as f64;
            EdgePoint {
                position_m: [
                    center[0] + radius_m * (angle.cos() * u[0] + angle.sin() * v[0]),
                    center[1] + radius_m * (angle.cos() * u[1] + angle.sin() * v[1]),
                    center[2] + radius_m * (angle.cos() * u[2] + angle.sin() * v[2]),
                ],
            }
        })
        .collect()
}

/// Half-space pressure of one source, times piston directivity and the diffraction factor.
pub fn baffle_source_pressure(
    volume_velocity: Complex64,
    freq_hz: f64,
    source_m: [f64; 3],
    listener_m: [f64; 3],
    outward: [f64; 3],
    piston_radius_m: f64,
    outline: &[EdgePoint],
) -> Complex64 {
    let k = 2.0 * PI * freq_hz / C0;
    let offset = [
        listener_m[0] - source_m[0],
        listener_m[1] - source_m[1],
        listener_m[2] - source_m[2],
    ];
    let range = distance(source_m, listener_m).max(1e-6);
    let facing = (offset[0] * outward[0] + offset[1] * outward[1] + offset[2] * outward[2]) / range;
    let sin_theta = (1.0 - facing.clamp(-1.0, 1.0).powi(2)).sqrt();
    let directivity = piston_directivity(k * piston_radius_m * sin_theta);
    let half = Complex64::new(0.0, k * C0 * RHO0)
        * volume_velocity
        * 2.0
        * Complex64::from_polar(1.0, -k * range)
        / (4.0 * PI * range);
    half * directivity * diffraction_factor(source_m, listener_m, outline, freq_hz)
}

/// One source heard from behind its baffle's plane, in the same simple model: the direct
/// sound does not get there, and the edge's secondary sources send what the front loses, with
/// the opposite sign. At low frequency this is the full-space level, as in front; at high
/// frequency the edge's contributions cancel and the baffle shadows its back.
pub fn baffle_shadow_pressure(
    volume_velocity: Complex64,
    freq_hz: f64,
    source_m: [f64; 3],
    listener_m: [f64; 3],
    outline: &[EdgePoint],
) -> Complex64 {
    if outline.is_empty() {
        return Complex64::new(0.0, 0.0);
    }
    let k = 2.0 * PI * freq_hz / C0;
    let mut sum = Complex64::new(0.0, 0.0);
    for edge in outline {
        let d_i = distance(source_m, edge.position_m);
        let e_i = distance(edge.position_m, listener_m).max(1e-9);
        sum += Complex64::from_polar(1.0 / e_i, -k * (d_i + e_i));
    }
    Complex64::new(0.0, k * C0 * RHO0) * volume_velocity * sum / (4.0 * PI * outline.len() as f64)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / n, v[1] / n, v[2] / n]
}
