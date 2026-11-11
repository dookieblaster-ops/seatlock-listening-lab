//! The several-chamber `lumped` network against closed-form circuits (README 5.3, 6.2).

use num_complex::Complex64;
use seatlock_acoustics::network::{lumped_network, Duct};
use seatlock_acoustics::{vented_sample, PortSpec};
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

fn compliance(volume: f64) -> f64 {
    volume / (RHO0 * C0 * C0)
}

#[test]
fn one_chamber_with_ports_is_the_vented_box() {
    let ports = [
        PortSpec {
            area_m2: 9.6e-4,
            length_eff_m: 0.12686,
        },
        PortSpec {
            area_m2: 5.0e-4,
            length_eff_m: 0.2,
        },
    ];
    let ducts: Vec<Duct> = ports
        .iter()
        .map(|p| Duct {
            from: 0,
            to: None,
            area_m2: p.area_m2,
            length_eff_m: p.length_eff_m,
        })
        .collect();
    for freq in [5.0, 20.0, 40.0, 63.0, 200.0, 1000.0] {
        let reference = vented_sample(0.014142, &ports, freq, false);
        let network = lumped_network(&[0.014142], &ducts, 0, freq);
        assert!((network.z_ab - reference.z_ab).norm() / reference.z_ab.norm() < 1e-12);
        for k in 0..2 {
            assert!((network.h_out[k] - reference.h_each[k]).norm() < 1e-12);
        }
    }
}

#[test]
fn two_chambers_joined_by_a_duct_match_the_circuit() {
    // Driver in chamber A; a duct of mass M to chamber B, which is sealed:
    // Z_ab = 1 / (jw*Ca + 1 / (jw*M + 1/(jw*Cb))).
    let (va, vb, area, length) = (0.006, 0.010, 7.0e-4, 0.09);
    let duct = Duct {
        from: 0,
        to: Some(1),
        area_m2: area,
        length_eff_m: length,
    };
    for freq in [3.0, 30.0, 61.0, 150.0, 800.0] {
        let w = 2.0 * PI * freq;
        let j = Complex64::new(0.0, 1.0);
        let branch = j * w * RHO0 * length / area + 1.0 / (j * w * compliance(vb));
        let expected = 1.0 / (j * w * compliance(va) + 1.0 / branch);
        let got = lumped_network(&[va, vb], &[duct], 0, freq).z_ab;
        assert!(
            (got - expected).norm() / expected.norm() < 1e-10,
            "{freq} Hz"
        );
    }
    // Far below the duct's resonance the two chambers act as one.
    let low = lumped_network(&[va, vb], &[duct], 0, 1.0).z_ab;
    let one = 1.0 / (Complex64::new(0.0, 2.0 * PI) * compliance(va + vb));
    assert!((low - one).norm() / one.norm() < 1e-3);
}

#[test]
fn a_bandpass_sends_everything_out_through_its_vent_at_low_frequency() {
    // Driver into a sealed rear chamber A; a duct from A into front chamber B; B vented.
    let ducts = [
        Duct {
            from: 0,
            to: Some(1),
            area_m2: 7.0e-4,
            length_eff_m: 0.05,
        },
        Duct {
            from: 1,
            to: None,
            area_m2: 1.2e-3,
            length_eff_m: 0.15,
        },
    ];
    let low = lumped_network(&[0.008, 0.012], &ducts, 0, 0.5);
    assert!((low.h_out[0] - 1.0).norm() < 1e-3, "{}", low.h_out[0]);
}
