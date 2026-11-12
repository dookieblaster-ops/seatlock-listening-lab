//! Relaxed faces and the gradient of `Z_ab` and `H_k` with respect to them (README 5.5,
//! "Shaping the inside").

use num_complex::Complex64;
use seatlock_fdtd::adjoint::{sensitivity, Face, Problem};
use seatlock_fdtd::grid::{self, Grid, Options};

/// A rigid box of `n` cells with a square source patch of `patch` cells at a corner of its
/// low-x wall.
fn closed_box(n: [usize; 3], dx: f64, patch: usize) -> Grid {
    let cells = n[0] * n[1] * n[2];
    let mut grid = Grid {
        n,
        dx,
        origin_m: [0.0; 3],
        air: vec![true; cells],
        source: Vec::new(),
        absorbing: Vec::new(),
        openings: Vec::new(),
        true_volume_m3: cells as f64 * dx * dx * dx,
    };
    for j in 0..patch {
        for k in 0..patch {
            grid.source.push(grid.cell(0, j, k));
        }
    }
    grid
}

/// A box of `inside` cells with an opening of `mouth` by `mouth` cells, one cell deep, in the
/// middle of its high-x wall, into an exterior block that reaches `margin` cells past the
/// mouth. The source patch is at a corner of the low-x wall.
fn vented_box(inside: [usize; 3], mouth: usize, margin: usize, dx: f64) -> Grid {
    let ny = mouth + 2 * margin;
    let nz = ny;
    let nx = inside[0] + 1 + margin;
    let cells = nx * ny * nz;
    let mut grid = Grid {
        n: [nx, ny, nz],
        dx,
        origin_m: [0.0; 3],
        air: vec![false; cells],
        source: Vec::new(),
        absorbing: Vec::new(),
        openings: vec![Vec::new()],
        true_volume_m3: (inside[0] * inside[1] * inside[2]) as f64 * dx * dx * dx,
    };
    let (j0, k0) = ((ny - inside[1]) / 2, (nz - inside[2]) / 2);
    for i in 0..inside[0] {
        for j in j0..j0 + inside[1] {
            for k in k0..k0 + inside[2] {
                let c = grid.cell(i, j, k);
                grid.air[c] = true;
            }
        }
    }
    let m0 = margin;
    for j in m0..m0 + mouth {
        for k in m0..m0 + mouth {
            let c = grid.cell(inside[0], j, k);
            grid.air[c] = true;
            let slot = grid.face_slot(0, inside[0] + 1, j, k);
            grid.openings[0].push((0, slot, 1.0));
        }
    }
    let mouth_at = [
        (inside[0] + 1) as f64 * dx,
        ny as f64 * dx * 0.5,
        nz as f64 * dx * 0.5,
    ];
    for i in inside[0] + 1..nx {
        for j in 0..ny {
            for k in 0..nz {
                let c = grid.cell(i, j, k);
                grid.air[c] = true;
                let count = (i + 1 == nx) as u8
                    + (j == 0) as u8
                    + (j + 1 == ny) as u8
                    + (k == 0) as u8
                    + (k + 1 == nz) as u8;
                if count > 0 {
                    let r = ((0..3)
                        .map(|axis| {
                            let center = ([i, j, k][axis] as f64 + 0.5) * dx;
                            (center - mouth_at[axis]).powi(2)
                        })
                        .sum::<f64>())
                    .sqrt();
                    grid.absorbing.push((c, count, r));
                }
            }
        }
    }
    for j in j0..j0 + 2 {
        for k in k0..k0 + 2 {
            grid.source.push(grid.cell(0, j, k));
        }
    }
    grid
}

/// Every face that touches a cell of the box `lo..hi` and lies between two cells of the grid.
fn faces_of(grid: &Grid, lo: [usize; 3], hi: [usize; 3]) -> Vec<Face> {
    let stride = [grid.n[1] * grid.n[2], grid.n[2], 1];
    let mut faces = Vec::new();
    for i in lo[0]..hi[0] {
        for j in lo[1]..hi[1] {
            for k in lo[2]..hi[2] {
                let c = grid.cell(i, j, k);
                let at = [i, j, k];
                for axis in 0..3 {
                    if at[axis] > 0 {
                        faces.push((axis as u8, c));
                    }
                    if at[axis] + 1 == hi[axis] && at[axis] + 1 < grid.n[axis] {
                        faces.push((axis as u8, c + stride[axis]));
                    }
                }
            }
        }
    }
    faces
}

fn run_f32(grid: &Grid, relaxed: Vec<(u8, usize, f32)>, freqs: &[f64]) -> grid::Response {
    grid::run(
        grid,
        &Options {
            max_steps: 60_000,
            frequencies_hz: freqs.to_vec(),
            progress_every: 0,
            extrapolate: false,
            z_scale: Vec::new(),
            relaxed,
            loss_per_s: Vec::new(),
        },
        &mut |_, _| true,
    )
}

#[test]
fn a_relaxed_face_at_zero_or_one_reproduces_the_plain_grid_bit_for_bit() {
    // CHK-FDTD-1's box.
    let grid = closed_box([30, 20, 15], 0.01, 4);
    let freqs: Vec<f64> = (1..=60).map(|index| index as f64 * 25.0).collect();
    let plain = run_f32(&grid, Vec::new(), &freqs);

    // Open fraction 1 on every face of a region: nothing changes.
    let region = faces_of(&grid, [10, 5, 4], [20, 15, 11]);
    let ones = run_f32(
        &grid,
        region
            .iter()
            .map(|&(axis, slot)| (axis, slot, 1.0))
            .collect(),
        &freqs,
    );
    assert_eq!(plain.steps, ones.steps);
    assert_eq!(plain.z_real, ones.z_real);
    assert_eq!(plain.z_imag, ones.z_imag);

    // Open fraction 0 on every face of a block of cells: the same as a grid in which those
    // cells are solid.
    let (lo, hi) = ([12, 6, 5], [16, 9, 8]);
    let zeros = run_f32(
        &grid,
        faces_of(&grid, lo, hi)
            .iter()
            .map(|&(axis, slot)| (axis, slot, 0.0))
            .collect(),
        &freqs,
    );
    let mut solid = grid.clone();
    for i in lo[0]..hi[0] {
        for j in lo[1]..hi[1] {
            for k in lo[2]..hi[2] {
                let c = solid.cell(i, j, k);
                solid.air[c] = false;
            }
        }
    }
    let carved = run_f32(&solid, Vec::new(), &freqs);
    assert_eq!(carved.steps, zeros.steps);
    assert_eq!(carved.z_real, zeros.z_real);
    assert_eq!(carved.z_imag, zeros.z_imag);
    assert_ne!(plain.z_real, zeros.z_real);

    // A half-open region is neither.
    let half = run_f32(
        &grid,
        faces_of(&grid, lo, hi)
            .iter()
            .map(|&(axis, slot)| (axis, slot, 0.5))
            .collect(),
        &freqs,
    );
    assert_ne!(half.z_real, plain.z_real);
    assert_ne!(half.z_real, zeros.z_real);
}

#[test]
fn the_double_precision_run_matches_the_everyday_solver() {
    let grid = closed_box([15, 10, 8], 0.02, 3);
    let freqs = [60.0, 200.0, 450.0, 700.0, 1000.0, 1500.0];
    let region = faces_of(&grid, [6, 3, 2], [10, 7, 6]);
    let single = run_f32(
        &grid,
        region
            .iter()
            .map(|&(axis, slot)| (axis, slot, 0.5))
            .collect(),
        &freqs,
    );
    let relaxed: Vec<(u8, usize, f64)> = region
        .iter()
        .map(|&(axis, slot)| (axis, slot, 0.5))
        .collect();
    let double = sensitivity(&Problem {
        grid: &grid,
        relaxed: &relaxed,
        faces: &[],
        frequencies_hz: &freqs,
        max_steps: 60_000,
        quiet: 1e-4,
        decay_per_s: 0.0,
    });
    assert_eq!(single.steps, double.steps);
    for (bin, z) in double.z.iter().enumerate() {
        let other = Complex64::new(single.z_real[bin], single.z_imag[bin]);
        let error = (z - other).norm() / z.norm();
        assert!(error < 1e-3, "{} Hz: {z} against {other}", freqs[bin]);
    }
}

/// Compares the gradient with finite differences on `picks` (indices into `region`), with
/// every face of `region` at the open fraction `state`. Returns the worst error as a fraction
/// of each face's largest finite difference over the frequencies.
fn worst_gradient_error(
    grid: &Grid,
    region: &[Face],
    state: f64,
    picks: &[usize],
    freqs: &[f64],
    decay_per_s: f64,
) -> f64 {
    let max_steps = 4_000_000;
    let base: Vec<(u8, usize, f64)> = region
        .iter()
        .map(|&(axis, slot)| (axis, slot, state))
        .collect();
    let faces: Vec<Face> = picks.iter().map(|&pick| region[pick]).collect();
    let problem = |relaxed: &[(u8, usize, f64)], with_faces: bool| {
        let result = sensitivity(&Problem {
            grid,
            relaxed,
            faces: if with_faces { &faces } else { &[] },
            frequencies_hz: freqs,
            max_steps,
            quiet: 1e-9,
            decay_per_s,
        });
        assert!(
            result.decayed,
            "the run did not ring out in {max_steps} steps"
        );
        result
    };
    let here = problem(&base, true);
    eprintln!("state {state}: {} steps, {} runs", here.steps, here.runs);
    let nf = freqs.len();
    // Near 0 a face is the heavy neck of a tiny resonator whose pitch rises as the square
    // root of its open fraction, so the response bends within a very small step.
    let h = if state <= 0.0 { 2e-5 } else { 0.01 };
    let moved = |pick: usize, by: f64| {
        let mut relaxed = base.clone();
        relaxed[pick].2 = state + by;
        problem(&relaxed, false)
    };
    let mut worst = 0.0_f64;
    for (face, &pick) in picks.iter().enumerate() {
        // Central differences inside the range; at its ends a second-order one-sided
        // difference, because an open fraction below 0 is a negative mass (unstable).
        let outputs = |r: &seatlock_fdtd::adjoint::Sensitivity| -> Vec<Complex64> {
            let mut all = r.z.clone();
            for h_k in &r.h {
                all.extend(h_k);
            }
            all
        };
        let fd: Vec<Complex64> = if state <= 0.0 || state >= 1.0 {
            let sign = if state <= 0.0 { 1.0 } else { -1.0 };
            let (a, b, c) = (
                outputs(&here),
                outputs(&moved(pick, sign * h)),
                outputs(&moved(pick, sign * 2.0 * h)),
            );
            (0..a.len())
                .map(|i| (-3.0 * a[i] + 4.0 * b[i] - c[i]) / (2.0 * h) * sign)
                .collect()
        } else {
            let (a, b) = (outputs(&moved(pick, -h)), outputs(&moved(pick, h)));
            (0..a.len()).map(|i| (b[i] - a[i]) / (2.0 * h)).collect()
        };
        let mut gradient: Vec<Complex64> = here.dz[face * nf..(face + 1) * nf].to_vec();
        for dh in &here.dh {
            gradient.extend(&dh[face * nf..(face + 1) * nf]);
        }
        for (block, name) in gradient.chunks(nf).zip(["Z_ab", "H"]) {
            let offset = if name == "Z_ab" { 0 } else { nf };
            let reference = &fd[offset..offset + nf];
            let scale = reference.iter().map(|x| x.norm()).fold(0.0, f64::max);
            let error = block
                .iter()
                .zip(reference)
                .map(|(g, f)| (g - f).norm())
                .fold(0.0, f64::max)
                / scale;
            let relative = scale
                / if name == "Z_ab" {
                    here.z.iter().map(|x| x.norm()).fold(0.0, f64::max)
                } else {
                    1.0
                };
            eprintln!(
                "  face {:?} {name}: error {:.3} %, size {relative:.2e}",
                region[pick],
                error * 100.0
            );
            assert!(scale > 0.0, "face {:?} has no effect at all", region[pick]);
            worst = worst.max(error);
        }
    }
    worst
}

/// `per_axis` faces along each axis on the skin of the region `lo..hi`, as indices into
/// `region`.
fn skin_picks(
    grid: &Grid,
    region: &[Face],
    lo: [usize; 3],
    hi: [usize; 3],
    per_axis: usize,
) -> Vec<usize> {
    let mut picks = Vec::new();
    for axis in 0..3u8 {
        let on_skin: Vec<usize> = (0..region.len())
            .filter(|&index| {
                let (a, slot) = region[index];
                let k = slot % grid.n[2];
                let j = (slot / grid.n[2]) % grid.n[1];
                let i = slot / (grid.n[1] * grid.n[2]);
                let at = [i, j, k][a as usize];
                a == axis && (at == lo[a as usize] || at == hi[a as usize])
            })
            .collect();
        for step in 0..per_axis {
            picks.push(on_skin[step * on_skin.len() / per_axis]);
        }
    }
    picks
}

#[test]
fn the_gradient_of_a_closed_box_matches_finite_differences() {
    // CHK-FDTD-1's proportions at 20 mm cells.
    let grid = closed_box([15, 10, 8], 0.02, 3);
    let freqs = [80.0, 250.0, 400.0, 550.0, 700.0, 900.0, 1200.0, 1500.0];
    let (lo, hi) = ([6, 3, 2], [9, 6, 5]);
    let region = faces_of(&grid, lo, hi);
    // Twelve faces on the region's skin, four along each axis: at open fraction 0 the faces
    // inside a closed block have no effect, in the gradient and in truth.
    let picks = skin_picks(&grid, &region, lo, hi, 4);
    // Without losses, where the run rings out by itself (half-open faces without losses are
    // in the test with an opening). A block of closed faces is left out here: the air sealed
    // in it, once a face opens a little, rings for millions of steps.
    let worst = worst_gradient_error(&grid, &region, 1.0, &picks, &freqs, 0.0);
    eprintln!(
        "lossless, open fraction 1: worst error {:.3} %",
        worst * 100.0
    );
    assert!(worst < 0.02, "lossless, open fraction 1: {worst}");
    // With the decay weight, at every state.
    for state in [1.0, 0.0, 0.5] {
        let worst = worst_gradient_error(&grid, &region, state, &picks, &freqs, 40.0);
        eprintln!(
            "decay 40/s, open fraction {state}: worst error {:.3} %",
            worst * 100.0
        );
        assert!(worst < 0.02, "decay 40/s, open fraction {state}: {worst}");
    }
}

#[test]
fn the_gradient_of_a_box_with_an_opening_matches_finite_differences() {
    let grid = vented_box([10, 8, 8], 4, 6, 0.02);
    let freqs = [60.0, 120.0, 180.0, 300.0, 500.0, 900.0];
    let (lo, hi) = ([4, 7, 7], [7, 10, 10]);
    let region = faces_of(&grid, lo, hi);
    let picks = skin_picks(&grid, &region, lo, hi, 2);
    for (state, decay) in [(0.5, 0.0), (1.0, 40.0), (0.0, 40.0)] {
        let worst = worst_gradient_error(&grid, &region, state, &picks, &freqs, decay);
        eprintln!(
            "decay {decay}/s, open fraction {state}: worst error {:.3} %",
            worst * 100.0
        );
        assert!(
            worst < 0.02,
            "decay {decay}/s, open fraction {state}: {worst}"
        );
    }
}
