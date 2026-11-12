//! The 3D cabinet model's early stop (README 5.5): the run stops once a fit of its fading
//! echoes (harmonic inversion) settles, and the rest of each signal is added in closed form.
//! It must give what running until the echoes die away gives, in fewer steps.

use seatlock_sim::{builtin, simulate, SimulateRequest, SimulateResult};

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(name: &str, full_decay: bool) -> SimulateResult {
    let freqs: Vec<f64> = (0..200)
        .map(|i| 10.0 * (3400.0f64 / 10.0).powf(i as f64 / 199.0))
        .collect();
    simulate(
        &example(name),
        &SimulateRequest {
            enclosure: "fdtd".into(),
            frequencies_hz: Some(freqs),
            fdtd_full_decay: full_decay,
            ..SimulateRequest::default()
        },
        &builtin(),
    )
}

fn compare(name: &str, fewer_steps_than: f64) {
    let full = run(name, true);
    let early = run(name, false);
    let (a, b) = (full.fdtd.as_ref().unwrap(), early.fdtd.as_ref().unwrap());
    let worst = full
        .summed_spl_db
        .iter()
        .zip(&early.summed_spl_db)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    println!(
        "{name}: {} steps until the echoes died, {} with the early stop; level within {worst:.3} dB",
        a.steps, b.steps
    );
    assert!(a.decayed && b.decayed);
    assert!(worst < 0.05, "{name}: {worst} dB");
    assert!(
        (b.steps as f64) < fewer_steps_than * a.steps as f64,
        "{name}: {} against {}",
        b.steps,
        a.steps
    );
}

#[test]
fn the_early_stop_matches_the_full_run_on_a_ported_two_way_in_under_half_the_steps() {
    compare("two-way-vented", 0.5);
}

#[test]
fn the_early_stop_matches_the_full_run_on_a_sealed_box() {
    compare("sealed-test", 1.0);
}
