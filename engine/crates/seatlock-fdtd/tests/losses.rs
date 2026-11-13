//! The air's loss in the 3D solver (README section 5.5): a uniform loss `s` applied as a decay
//! weight on every signal.

use seatlock_fdtd::grid::{self, Grid, Options};

/// CHK-FDTD-1's rigid box, 0.30 x 0.20 x 0.15 m at 10 mm, with its 4 x 4 source patch at a
/// corner of a 0.20 x 0.15 wall.
fn chk_box() -> Grid {
    let n = [30usize, 20, 15];
    let cells = n[0] * n[1] * n[2];
    let mut grid = Grid {
        n,
        dx: 0.01,
        origin_m: [0.0; 3],
        air: vec![true; cells],
        source: Vec::new(),
        absorbing: Vec::new(),
        openings: Vec::new(),
        true_volume_m3: 0.3 * 0.2 * 0.15,
    };
    for j in 0..4 {
        for k in 0..4 {
            grid.source.push(grid.cell(0, j, k));
        }
    }
    grid
}

/// `|Z_ab|^2` near the box's first resonance, with a uniform loss `s`, and the width in Hz of
/// its peak at half its height.
fn peak_width(s: f64, early_stop: bool) -> (f64, f64) {
    let grid = chk_box();
    let freqs: Vec<f64> = (0..801).map(|i| 552.0 + 0.05 * i as f64).collect();
    let dt = grid.time_step_s();
    let response = grid::run(
        &grid,
        &Options {
            max_steps: (3.0 / dt) as usize,
            frequencies_hz: freqs.clone(),
            progress_every: 0,
            extrapolate: early_stop,
            z_scale: Vec::new(),
            relaxed: Vec::new(),
            loss_per_s: vec![s; freqs.len()],
            damping: Vec::new(),
        },
        &mut |_, _| true,
    );
    assert!(response.decayed);
    let power: Vec<f64> = response
        .z_real
        .iter()
        .zip(&response.z_imag)
        .map(|(re, im)| re * re + im * im)
        .collect();
    let (top, height) =
        power.iter().enumerate().fold(
            (0, 0.0),
            |best, (i, p)| if *p > best.1 { (i, *p) } else { best },
        );
    assert!(
        top > 20 && top < freqs.len() - 20,
        "peak at the band's edge"
    );
    // Where the curve crosses half the height on either side, by interpolation.
    let cross = |range: &mut dyn Iterator<Item = usize>, step: isize| -> f64 {
        for i in range {
            let j = (i as isize + step) as usize;
            if power[j] < height / 2.0 {
                let t = (power[i] - height / 2.0) / (power[i] - power[j]);
                return freqs[i] + t * (freqs[j] - freqs[i]);
            }
        }
        panic!("the peak does not fall to half inside the band");
    };
    let low = cross(&mut (1..=top).rev(), -1);
    let high = cross(&mut (top..freqs.len() - 1), 1);
    (freqs[top], high - low)
}

#[test]
fn a_uniform_loss_widens_a_resonance_by_s_over_pi() {
    // Without it the peak already has a width: the source drains the box (README 5.5).
    let (at, bare) = peak_width(0.0, false);
    assert!((at - 572.0).abs() < 0.005 * 572.0, "peak at {at} Hz");
    for s in [10.0, 30.0] {
        let (_, wide) = peak_width(s, false);
        let added = wide - bare;
        let expected = s / std::f64::consts::PI;
        eprintln!("s = {s}: {bare:.3} Hz bare, {wide:.3} Hz with the loss, added {added:.3}, expected {expected:.3}");
        assert!((added - expected).abs() < 0.03 * expected);
    }
}

#[test]
fn the_early_stop_gives_the_same_lossy_peak() {
    let (at_full, full) = peak_width(30.0, false);
    let (at_early, early) = peak_width(30.0, true);
    eprintln!("to the end: {at_full:.2} Hz, {full:.3} Hz wide; early stop: {at_early:.2} Hz, {early:.3} Hz wide");
    assert!((at_full - at_early).abs() < 0.11);
    assert!((full - early).abs() < 0.03 * full);
}
