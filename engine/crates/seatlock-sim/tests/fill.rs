//! Chamber damping fill (README 5.5 / 6.2): a catalog damping material packing a listed
//! chamber's air is heard by the everyday networks (Delany–Bazley Cap) and the full 3D
//! model (Rayleigh), and priced by volume.

use seatlock_core::catalog;
use seatlock_sim::{builtin, cost, simulate, CabinetParts, SimulateRequest};
use serde_json::json;

fn sealed_with_whole_fill(fill: Option<&str>) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/sealed-test.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut design: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    // One chamber that fills the whole inside (wall 8 mm on a 160×180×250 mm box).
    design["speaker"]["enclosure"]["chambers"] = json!([{
        "id": "main",
        "min_m": [0.008, 0.008, 0.008],
        "max_m": [0.152, 0.172, 0.242],
        "fill": fill,
    }]);
    serde_json::from_value(design).unwrap()
}

#[test]
fn chamber_fill_calms_standing_waves_in_the_3d_model() {
    let run =
        |fill: Option<&str>| {
            let design = sealed_with_whole_fill(fill);
            let request = SimulateRequest {
                enclosure: "fdtd".into(),
                fdtd_dx_m: Some(0.01),
                frequencies_hz: Some(seatlock_core::grid::log_grid(100.0, 3000.0, 48.0)),
                ..Default::default()
            };
            let r = simulate(&design, &request, &builtin());
            assert!(
                r.issues
                    .iter()
                    .all(|i| i.severity != seatlock_core::issue::Severity::Error),
                "{:?}",
                r.issues
            );
            if fill.is_some() {
                assert!(
                    r.fidelity
                        .ignored
                        .iter()
                        .any(|l| l.contains("Damping fill in a chamber")),
                    "{:?}",
                    r.fidelity.ignored
                );
            }
            let quick = simulate(&design, &SimulateRequest::default(), &builtin());
            if fill.is_some() {
                assert!(
                    quick.fidelity.ignored.iter().any(|l| {
                        l.contains("Damping fill in a chamber") && l.contains("Delany")
                    }),
                    "{:?}",
                    quick.fidelity.ignored
                );
            }
            let z = r.fdtd.expect("fdtd report").z_ab;
            let mag: Vec<f64> =
                z.re.iter()
                    .zip(&z.im)
                    .map(|(re, im)| (re * re + im * im).sqrt())
                    .collect();
            (r.frequencies_hz.clone(), mag)
        };
    let (f, bare) = run(None);
    let (_, damped) = run(Some("isover-akustic-ssp2"));
    // Tallest |Z_ab| peak in the cabinet's standing-wave band. Whole-chamber Rayleigh fill
    // damps those modes; compare FDTD to FDTD.
    let peak = |z: &[f64]| {
        f.iter()
            .zip(z)
            .filter(|(x, _)| **x >= 500.0 && **x <= 3000.0)
            .map(|(_, v)| *v)
            .fold(0.0, f64::max)
    };
    let (peak_bare, peak_damped) = (peak(&bare), peak(&damped));
    eprintln!(
        "tallest |Z_ab| peak, 500 Hz to 3 kHz: {peak_bare:.3e} bare, {peak_damped:.3e} with Isover fill"
    );
    assert!(
        peak_damped < 0.7 * peak_bare,
        "fill should lower the standing-wave |Z_ab| peak: bare {peak_bare}, damped {peak_damped}"
    );
}

#[test]
fn chamber_fill_makes_the_everyday_load_lossy() {
    // Isover AFr11 σ = 11000 Pa·s/m². Delany–Bazley Cap must add Re(Z_ab) > 0.
    let sigma = catalog::damping("isover-akustic-ssp2")
        .unwrap()
        .get("flow_resistivity_pa_s_m2")
        .and_then(|v| v.as_f64())
        .unwrap();
    let filled = seatlock_acoustics::network::lumped_network_with_walls(
        &[0.005],
        &[],
        0,
        100.0,
        &[],
        &[],
        &[Some(sigma)],
    );
    let empty = seatlock_acoustics::network::lumped_network_with_walls(
        &[0.005],
        &[],
        0,
        100.0,
        &[],
        &[],
        &[None],
    );
    assert!(
        filled.z_ab.re > 1e-4,
        "Delany–Bazley fill should make Z_ab lossy: {}",
        filled.z_ab
    );
    assert!(
        empty.z_ab.re.abs() < 1e-12,
        "empty Cap stays pure reactive: {}",
        empty.z_ab
    );
}

#[test]
fn chamber_fill_is_priced_by_volume() {
    let design = sealed_with_whole_fill(Some("isover-akustic-ssp2"));
    let report = cost::report(
        &design,
        &CabinetParts {
            print_solid_m3: Some(0.001),
            stock_cuts: Vec::new(),
            stock_panels: Vec::new(),
            part_materials: Vec::new(),
            problems: Vec::new(),
        },
    );
    let line = report
        .lines
        .iter()
        .find(|l| l.item.starts_with("Damping fill"))
        .expect("damping fill line");
    let volume = 0.144 * 0.164 * 0.234;
    let litres = volume * 1000.0 * 2.0; // stereo pair
    assert!(
        (line.quantity - litres).abs() < 1e-6,
        "got {} L, want {litres} L",
        line.quantity
    );
    assert_eq!(line.unit, "L");
    let _ = catalog::damping("isover-akustic-ssp2").unwrap();
}

#[test]
fn unknown_chamber_fill_is_refused() {
    let design = sealed_with_whole_fill(Some("not-a-real-damping"));
    let r = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(
        r.issues
            .iter()
            .any(|i| i.message.contains("not a damping material")),
        "{:?}",
        r.issues
    );
}
