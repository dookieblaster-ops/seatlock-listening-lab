//! Validation of the `exterior` radiation solver (README section 5.6): a closed form (the rigid
//! sphere), reciprocity against a run the other way round, and the low-frequency limit.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_fdtd::exterior::{radiate_from_face, skin_transfer, Domain, Incident, Options};
use std::f64::consts::PI;

fn quiet() -> impl FnMut(usize, usize) -> bool {
    |_, _| true
}

/// Spherical Hankel function of the second kind, `h_n(x) = j_n(x) - j*y_n(x)`, by its finite sum.
fn hankel2(n: usize, x: f64) -> Complex64 {
    // h_n^(1)(x) = (-j)^(n+1) * e^(jx)/x * sum_k j^k (n+k)! / (k! (n-k)! (2x)^k); h^(2) is its
    // complex conjugate for real x.
    let mut sum = Complex64::new(0.0, 0.0);
    let mut term = 1.0_f64; // (n+k)! / (k! (n-k)! (2x)^k)
    for k in 0..=n {
        if k > 0 {
            term *= ((n + k) * (n + 1 - k)) as f64 / (k as f64 * 2.0 * x);
        }
        sum += Complex64::i().powi(k as i32) * term;
    }
    let first =
        Complex64::new(0.0, -1.0).powi(n as i32 + 1) * Complex64::from_polar(1.0 / x, x) * sum;
    first.conj()
}

/// Derivative of the spherical Hankel function: `h_n' = h_(n-1) - (n+1)/x * h_n`.
fn hankel2_prime(n: usize, x: f64) -> Complex64 {
    if n == 0 {
        return -hankel2(1, x);
    }
    hankel2(n - 1, x) - hankel2(n, x) * ((n + 1) as f64 / x)
}

fn legendre(n: usize, x: f64) -> f64 {
    let (mut previous, mut current) = (1.0, x);
    if n == 0 {
        return 1.0;
    }
    for m in 1..n {
        let next = ((2 * m + 1) as f64 * x * current - m as f64 * previous) / (m + 1) as f64;
        previous = current;
        current = next;
    }
    current
}

/// Total pressure on a rigid sphere of radius `a` for a plane wave of unit pressure, at angle
/// `theta` from the direction the wave comes from:
/// `p = -j/(ka)^2 * sum (2m+1) j^m P_m(cos theta) / h_m'(ka)`.
fn sphere_surface_pressure(ka: f64, theta: f64) -> Complex64 {
    let terms = (ka + 12.0).ceil() as usize;
    let mut sum = Complex64::new(0.0, 0.0);
    for m in 0..terms {
        sum += Complex64::i().powi(m as i32) * ((2 * m + 1) as f64 * legendre(m, theta.cos()))
            / hankel2_prime(m, ka);
    }
    Complex64::new(0.0, -1.0) / (ka * ka) * sum
}

#[test]
fn the_sphere_series_has_the_right_limits() {
    // Small sphere: the wave passes as if nothing were there.
    assert!((sphere_surface_pressure(0.01, 0.0).norm() - 1.0).abs() < 1e-3);
    // Large sphere: pressure doubles at the point facing the wave.
    assert!((sphere_surface_pressure(40.0, 0.0).norm() - 2.0).abs() < 0.05);
}

/// Levels on a rigid sphere of radius 0.1 m, in dB against the incident wave, at the points
/// 0, 90, and 180 degrees from where a plane wave comes from.
fn sphere_levels(dx: f64, freqs: &[f64]) -> Vec<[f64; 3]> {
    sphere_levels_on(dx, freqs, false)
}

/// `smooth`: on a grid that follows the sphere's surface instead of stair-stepping it.
fn sphere_levels_on(dx: f64, freqs: &[f64], smooth: bool) -> Vec<[f64; 3]> {
    let a = 0.1;
    let mut solid = |p: [f64; 3]| p[0] * p[0] + p[1] * p[1] + p[2] * p[2] < a * a;
    let domain = if smooth {
        Domain::around_smooth([-a; 3], [a; 3], 0.2, dx, &mut solid)
    } else {
        Domain::around([-a; 3], [a; 3], 0.2, dx, &mut solid)
    };
    let skin = domain.skin_cells();
    let poles = [[0.0, -a, 0.0], [a, 0.0, 0.0], [0.0, a, 0.0]];
    let mut record = Vec::new();
    let mut groups = Vec::new();
    for pole in poles {
        let start = record.len();
        for &cell in &skin {
            let c = domain.center_of(cell);
            let d =
                ((c[0] - pole[0]).powi(2) + (c[1] - pole[1]).powi(2) + (c[2] - pole[2]).powi(2))
                    .sqrt();
            if d < 1.2 * dx {
                record.push(cell);
            }
        }
        groups.push(start..record.len());
        assert!(record.len() > start, "no skin cells at {pole:?}");
    }
    let distance = 100.0;
    let run = skin_transfer(
        &domain,
        &Incident::Plane {
            toward: [0.0, -1.0, 0.0],
            distance_m: distance,
            reference_m: [0.0; 3],
        },
        &record,
        &Options {
            frequencies_hz: freqs.to_vec(),
            progress_every: 0,
        },
        &mut quiet(),
    );
    assert!(run.decayed, "{} of {} steps", run.steps, run.max_steps);
    freqs
        .iter()
        .enumerate()
        .map(|(bin, freq)| {
            let free = 2.0 * PI * freq * RHO0 / (4.0 * PI * distance);
            let mut levels = [0.0; 3];
            for (slot, group) in groups.iter().enumerate() {
                let mut sum = Complex64::new(0.0, 0.0);
                for index in group.clone() {
                    sum += run.transfer[index][bin];
                }
                levels[slot] = 20.0 * (sum.norm() / group.len() as f64 / free).log10();
            }
            levels
        })
        .collect()
}

#[test]
fn chk_rad_3_a_rigid_sphere_matches_the_closed_form() {
    // ka = 0.18, 1, 2, 3, 4, 5, 6; the last is close to the 10 mm grid's limit of 3432 Hz.
    let freqs = [100.0, 546.0, 1092.0, 1638.0, 2185.0, 2731.0, 3277.0];
    // Worst error in dB at the lit point, the side, and the shadowed point, per cell size.
    let mut worst = Vec::new();
    for dx in [0.01, 0.005] {
        let got = sphere_levels(dx, &freqs);
        let mut errors = [0.0_f64; 3];
        for (freq, levels) in freqs.iter().zip(&got) {
            let ka = 2.0 * PI * freq / C0 * 0.1;
            for (slot, theta) in [0.0, PI / 2.0, PI].iter().enumerate() {
                let expected = 20.0 * sphere_surface_pressure(ka, *theta).norm().log10();
                errors[slot] = errors[slot].max((levels[slot] - expected).abs());
                println!(
                    "dx {dx} ka {ka:.2} point {slot}: {:+.2} dB, closed form {expected:+.2} dB",
                    levels[slot]
                );
            }
        }
        worst.push(errors);
    }
    // Where the wave arrives, and at the side: within 0.75 dB up to the grid's limit.
    for errors in &worst {
        assert!(errors[0] < 0.75 && errors[1] < 0.75, "{errors:?}");
    }
    // The deep shadow of a curved body is where stair-stepped cells hurt: the sound creeps
    // around steps instead of a smooth surface, and the level reads high near the grid's
    // limit. The error halves with the cell size, which is how a stair-step error behaves.
    assert!(worst[0][2] < 3.6, "{:?}", worst[0]);
    assert!(worst[1][2] < 1.8, "{:?}", worst[1]);
}

#[test]
fn chk_rad_4_swapping_source_and_receiver_gives_the_same_transfer() {
    // A box, so no stair-steps. The reciprocity run (a simple source at the far point, pressure
    // on the skin, body as a scatterer) against a plain radiation run (a skin face moving,
    // pressure at the far point). The plain run carries the grid's dispersion over the whole
    // path, so its phase is the less exact of the two.
    let dx = 0.01;
    let size = [0.2, 0.16, 0.24];
    let mut solid = |p: [f64; 3]| (0..3).all(|axis| p[axis] > 0.0 && p[axis] < size[axis]);
    let domain = Domain::around([0.0; 3], size, 0.4, dx, &mut solid);
    // A skin cell just in front of the box's front face (the body is on its high-y side).
    let skin = domain.cell_at([0.065, -0.5 * dx, 0.155]).unwrap();
    assert!(domain.solid_neighbors(skin).contains(&(1, false)));
    let options = Options {
        frequencies_hz: vec![50.0, 200.0, 500.0, 1000.0, 2000.0, 3000.0],
        progress_every: 0,
    };
    // One point beside the box and one behind it, in the shadow of the source's face.
    for point in [[0.40, 0.05, 0.20], [0.10, 0.36, 0.12]] {
        let far = domain.cell_at(point).unwrap();
        let forward = radiate_from_face(&domain, skin, 1, false, &[far], &options, &mut quiet());
        let back = skin_transfer(
            &domain,
            &Incident::Point {
                at_m: domain.center_of(far),
            },
            &[skin],
            &options,
            &mut quiet(),
        );
        assert!(forward.decayed && back.decayed);
        for (bin, freq) in options.frequencies_hz.iter().enumerate() {
            let ratio = back.transfer[0][bin] / forward.transfer[0][bin];
            println!(
                "{point:?} {freq} Hz: reciprocal over plain {:.3} at {:+.1} deg",
                ratio.norm(),
                ratio.arg().to_degrees()
            );
            assert!(
                (ratio.norm() - 1.0).abs() < 0.07,
                "{point:?} {freq} Hz: {}",
                ratio.norm()
            );
            assert!(ratio.arg().to_degrees().abs() < 10.0, "{point:?} {freq} Hz");
        }
    }
}

#[test]
fn a_body_far_smaller_than_the_wavelength_changes_nothing_far_away() {
    // For a far listener the box is invisible at 10 and 20 Hz: the level on its skin is that
    // of a simple source in full space. (A listener at 1 m reads about 0.7 dB more on the near
    // face, because a source on a body acts from a point a little in front of it.)
    let dx = 0.01;
    let size = [0.2, 0.16, 0.24];
    let mut solid = |p: [f64; 3]| (0..3).all(|axis| p[axis] > 0.0 && p[axis] < size[axis]);
    let domain = Domain::around([0.0; 3], size, 0.2, dx, &mut solid);
    let skin = domain.cell_at([0.065, -0.5 * dx, 0.155]).unwrap();
    let options = Options {
        frequencies_hz: vec![10.0, 20.0],
        progress_every: 0,
    };
    let run = skin_transfer(
        &domain,
        &Incident::Plane {
            toward: [0.0, -1.0, 0.0],
            distance_m: 100.0,
            reference_m: [0.1, 0.0, 0.12],
        },
        &[skin],
        &options,
        &mut quiet(),
    );
    for (bin, freq) in options.frequencies_hz.iter().enumerate() {
        let free = 2.0 * PI * freq * RHO0 / (4.0 * PI * 100.0);
        let level = 20.0 * (run.transfer[0][bin].norm() / free).log10();
        assert!(level.abs() < 0.05, "{freq} Hz: {level:+.3} dB");
    }
}

#[test]
fn a_moving_face_on_one_solid_cell_is_a_simple_source_and_the_outer_faces_let_it_go() {
    // The radiation run alone, against free space, with the outer faces at two distances.
    let dx = 0.01;
    let options = Options {
        frequencies_hz: vec![50.0, 200.0, 500.0, 1000.0, 2000.0],
        progress_every: 0,
    };
    let mut results: Vec<Vec<Complex64>> = Vec::new();
    for margin in [0.3, 0.5] {
        let mut solid = |p: [f64; 3]| p.iter().all(|x| x.abs() < 0.005);
        let domain = Domain::around([-0.005; 3], [0.005; 3], margin, dx, &mut solid);
        let skin = domain.cell_at([0.0, -0.01, 0.0]).unwrap();
        assert!(domain.solid_neighbors(skin).contains(&(1, false)));
        let mut ratios = Vec::new();
        for point in [[0.2, 0.0, 0.0], [0.0, 0.2, 0.0], [0.12, -0.12, 0.12]] {
            let far = domain.cell_at(point).unwrap();
            let run = radiate_from_face(&domain, skin, 1, false, &[far], &options, &mut quiet());
            assert!(run.decayed);
            let c = domain.center_of(far);
            let s = domain.center_of(skin);
            let r = ((c[0] - s[0]).powi(2) + (c[1] - s[1]).powi(2) + (c[2] - s[2]).powi(2)).sqrt();
            for (bin, freq) in options.frequencies_hz.iter().enumerate() {
                let k = 2.0 * PI * freq / C0;
                let free = Complex64::new(0.0, 2.0 * PI * freq * RHO0)
                    * Complex64::from_polar(1.0 / (4.0 * PI * r), -k * r);
                let ratio = run.transfer[0][bin] / free;
                assert!(
                    (ratio.norm() - 1.0).abs() < 0.05,
                    "{point:?} {freq} Hz: {}",
                    ratio.norm()
                );
                ratios.push(ratio);
            }
        }
        results.push(ratios);
    }
    // Moving the outer faces from 0.3 m to 0.5 m changes nothing by more than 2 %.
    for (near, far) in results[0].iter().zip(&results[1]) {
        assert!((near / far - 1.0).norm() < 0.02, "{near} against {far}");
    }
}

#[test]
fn a_source_at_the_middle_of_a_thin_disc_follows_the_baffle_step() {
    // The CHK-RAD-2 arrangement (README section 13) in full 3D: a source at the middle of a
    // rigid disc of radius 0.10 m, 10 mm thick, heard on its axis from far away, against the
    // same source in half space. The simple edge model of the `baffle` radiation gives
    // `|1 - e^(-j*k*R) / 2|`: -6.02 dB at 10 Hz, +3.52 dB at 1716 Hz, -6.02 dB at 3432 Hz, and
    // +0.97 dB halfway between. Two unrelated models of the same thing agree within 0.75 dB.
    let dx = 0.005;
    let mut solid = |p: [f64; 3]| p[0] * p[0] + p[2] * p[2] < 0.01 && p[1] > 0.0 && p[1] < 0.01;
    let domain = Domain::around([-0.1, 0.0, -0.1], [0.1, 0.01, 0.1], 0.2, dx, &mut solid);
    let record: Vec<usize> = domain
        .skin_cells()
        .into_iter()
        .filter(|&cell| {
            let c = domain.center_of(cell);
            c[1] < 0.0 && (c[0] * c[0] + c[2] * c[2]).sqrt() < 1.2 * dx
        })
        .collect();
    assert!(!record.is_empty());
    let freqs = [10.0, 858.0, 1716.0, 2574.0, 3432.0];
    let run = skin_transfer(
        &domain,
        &Incident::Plane {
            toward: [0.0, -1.0, 0.0],
            distance_m: 100.0,
            reference_m: [0.0; 3],
        },
        &record,
        &Options {
            frequencies_hz: freqs.to_vec(),
            progress_every: 0,
        },
        &mut quiet(),
    );
    assert!(run.decayed);
    let level = |bin: usize| {
        let half_space = 2.0 * 2.0 * PI * freqs[bin] * RHO0 / (4.0 * PI * 100.0);
        let mut sum = Complex64::new(0.0, 0.0);
        for row in &run.transfer {
            sum += row[bin];
        }
        20.0 * (sum.norm() / record.len() as f64 / half_space).log10()
    };
    for (bin, freq) in freqs.iter().enumerate() {
        let k_r = 2.0 * PI * freq / C0 * 0.1;
        let simple = 20.0
            * (Complex64::new(1.0, 0.0) - Complex64::from_polar(0.5, -k_r))
                .norm()
                .log10();
        println!(
            "{freq} Hz: {:+.2} dB against half space, simple edge model {simple:+.2} dB",
            level(bin)
        );
        assert!(
            (level(bin) - simple).abs() < 0.75,
            "{freq} Hz: {}",
            level(bin)
        );
    }
    assert!((level(0) + 6.02).abs() < 0.05, "{}", level(0));
}

#[test]
fn a_grid_that_follows_the_sphere_is_stable_and_a_little_closer_in_its_shadow() {
    let freqs = [100.0, 546.0, 1092.0, 1638.0, 2185.0, 2731.0, 3277.0];
    for dx in [0.01, 0.005] {
        let mut errors = [[0.0_f64; 3]; 2];
        for (which, smooth) in [false, true].into_iter().enumerate() {
            let got = sphere_levels_on(dx, &freqs, smooth);
            for (freq, levels) in freqs.iter().zip(&got) {
                let ka = 2.0 * PI * freq / C0 * 0.1;
                for (slot, theta) in [0.0, PI / 2.0, PI].iter().enumerate() {
                    let expected = 20.0 * sphere_surface_pressure(ka, *theta).norm().log10();
                    let error = levels[slot] - expected;
                    if smooth {
                        println!("dx {dx} ka {ka:.2} point {slot}: {error:+.2} dB");
                    }
                    errors[which][slot] = errors[which][slot].max(error.abs());
                }
            }
        }
        println!(
            "dx {dx}: worst error lit/side/shadow, stair-steps {:.2}/{:.2}/{:.2} dB, following the surface {:.2}/{:.2}/{:.2} dB",
            errors[0][0], errors[0][1], errors[0][2], errors[1][0], errors[1][1], errors[1][2]
        );
        // What was measured (README 5.6): the shadow is a little closer, the lit point and
        // the side a little farther, so the simulator does not use this grid yet.
        assert!(errors[1][2] < errors[0][2]);
        assert!(errors[1][0] < 1.1 && errors[1][1] < 0.75);
    }
}
