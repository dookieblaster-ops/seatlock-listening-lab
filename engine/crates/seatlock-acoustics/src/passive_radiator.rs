//! Passive radiator (drone cone) as an acoustic branch (README §5 Level 3).
//!
//! The standard Small / Thiele model: the diaphragm's moving mass, suspension compliance,
//! and mechanical resistance become acoustic Map, Cap, and Rap at the chamber:
//! `Map = Mms / Sd²`, `Cap = Cms · Sd²`, `Rap = Rms / Sd²`. The branch runs from the
//! chamber to the outside (outside pressure taken as zero on the everyday model). Volume
//! velocity through the branch radiates as a piston of area `Sd`.

use num_complex::Complex64;
use std::f64::consts::PI;

/// Mechanical parameters of a passive radiator, SI.
#[derive(Clone, Copy, Debug)]
pub struct PassiveRadiatorMech {
    pub mms_kg: f64,
    pub cms_m_n: f64,
    pub rms_ns_m: f64,
    pub sd_m2: f64,
}

impl PassiveRadiatorMech {
    pub fn acoustic_impedance(&self, freq_hz: f64) -> Complex64 {
        let omega = 2.0 * PI * freq_hz.max(1e-6);
        let sd2 = self.sd_m2.max(1e-12).powi(2);
        let map = self.mms_kg / sd2;
        let cap = self.cms_m_n * sd2;
        let rap = self.rms_ns_m / sd2;
        Complex64::new(rap, omega * map) + 1.0 / Complex64::new(0.0, omega * cap.max(1e-30))
    }
}

/// Sealed chamber with one passive radiator: `Z_ab` and the PR's `H = U_pr / U_in`.
pub fn sealed_with_pr(
    volume_m3: f64,
    pr: PassiveRadiatorMech,
    freq_hz: f64,
) -> (Complex64, Complex64) {
    use seatlock_core::constants::{C0, RHO0};
    let omega = 2.0 * PI * freq_hz.max(1e-6);
    let cab = volume_m3 / (RHO0 * C0 * C0);
    let z_pr = pr.acoustic_impedance(freq_hz);
    let admit = Complex64::new(0.0, omega * cab) + 1.0 / z_pr;
    let z_ab = 1.0 / admit;
    let h = z_ab / z_pr;
    (z_ab, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seatlock_core::constants::{C0, RHO0};

    #[test]
    fn a_very_heavy_pr_looks_like_a_sealed_box() {
        let volume = 0.02;
        let pr = PassiveRadiatorMech {
            mms_kg: 100.0,
            cms_m_n: 1e-6,
            rms_ns_m: 10.0,
            sd_m2: 0.02,
        };
        for freq in [20.0, 40.0, 80.0] {
            let (z, _) = sealed_with_pr(volume, pr, freq);
            let sealed = Complex64::new(0.0, -RHO0 * C0 * C0 / (2.0 * PI * freq * volume));
            let err = (z - sealed).norm() / sealed.norm();
            assert!(err < 0.02, "at {freq} Hz rel {err}: {z} vs {sealed}");
        }
    }

    #[test]
    fn at_the_pr_resonance_the_box_load_is_resistive() {
        // fs = 1/(2π √(Mms Cms)); at that frequency Cap and Map cancel and Z_pr = Rap.
        let mms = 0.05;
        let cms = 1.0 / ((2.0 * PI * 40.0).powi(2) * mms);
        let pr = PassiveRadiatorMech {
            mms_kg: mms,
            cms_m_n: cms,
            rms_ns_m: 2.0,
            sd_m2: 0.025,
        };
        let fs = 1.0 / (2.0 * PI * (mms * cms).sqrt());
        let z = pr.acoustic_impedance(fs);
        let rap = pr.rms_ns_m / pr.sd_m2.powi(2);
        assert!(
            (z.re - rap).abs() / rap < 1e-6 && z.im.abs() / rap < 1e-4,
            "at fs {fs}: {z}, want {rap}"
        );
    }

    #[test]
    fn waveguide_passive_radiator_matches_sealed_with_pr() {
        use crate::{solve, AcousticNode};
        let volume = 0.022;
        let pr = PassiveRadiatorMech {
            mms_kg: 0.05,
            cms_m_n: 1.0 / ((2.0 * PI * 40.0).powi(2) * 0.05),
            rms_ns_m: 2.0,
            sd_m2: 0.025,
        };
        let tree = AcousticNode::Parallel(vec![
            AcousticNode::Compliance {
                volume_m3: volume,
                fill_sigma: None,
                extra_fills: Vec::new(),
            },
            AcousticNode::PassiveRadiator {
                mech: pr,
                radiation: None,
            },
        ]);
        for freq in [20.0, 40.0, 63.0, 100.0, 200.0] {
            let (z_ref, h_ref) = sealed_with_pr(volume, pr, freq);
            let wave = solve(&tree, freq);
            let z_err = (wave.z_in - z_ref).norm() / z_ref.norm();
            let h = wave.openings[0];
            let h_err = (h - h_ref).norm();
            assert!(z_err < 1e-9, "at {freq} Hz Z rel {z_err}");
            assert!(h_err < 1e-9, "at {freq} Hz H err {h_err}: {h} vs {h_ref}");
        }
    }
}
