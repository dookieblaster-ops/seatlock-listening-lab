use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Published {
    pub fs_hz: f64,
    pub qes: f64,
    pub qms: f64,
    pub vas_m3: f64,
    pub re_ohm: f64,
    pub le_h: f64,
    pub sd_m2: f64,
    pub xmax_m: f64,
    pub pe_w: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Mechanical {
    pub cms: f64,
    pub mms: f64,
    pub rms: f64,
    pub bl: f64,
    pub qts: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    pub volume_velocity: Complex64,
    pub impedance: Complex64,
    pub current_a: Complex64,
    pub excursion_m: f64,
}

pub fn derive_mechanical(published: &Published) -> Mechanical {
    let cms = published.vas_m3 / (RHO0 * C0 * C0 * published.sd_m2 * published.sd_m2);
    let omega_s = 2.0 * PI * published.fs_hz;
    let mms = 1.0 / (omega_s * omega_s * cms);
    let rms = omega_s * mms / published.qms;
    let bl = (omega_s * mms * published.re_ohm / published.qes).sqrt();
    let qts = published.qes * published.qms / (published.qes + published.qms);
    Mechanical {
        cms,
        mms,
        rms,
        bl,
        qts,
    }
}

/// Cone at `voltage_rms` with the enclosure load `z_ab` already applied to the back of the cone.
pub fn terminal_at(
    published: &Published,
    mech: &Mechanical,
    z_ab: Complex64,
    voltage_rms: f64,
    freq_hz: f64,
) -> Terminal {
    let omega = 2.0 * PI * freq_hz;
    let ze = Complex64::new(published.re_ohm, omega * published.le_h);
    let zm_mech = Complex64::new(mech.rms, omega * mech.mms - 1.0 / (omega * mech.cms));
    let zm = zm_mech + published.sd_m2 * published.sd_m2 * z_ab;
    let voltage = Complex64::new(voltage_rms, 0.0);
    let denom = ze * zm + mech.bl * mech.bl;
    let velocity = mech.bl * voltage / denom;
    let impedance = ze + mech.bl * mech.bl / zm;
    let current = voltage / impedance;
    let excursion_m = 2.0_f64.sqrt() * velocity.norm() / omega;
    Terminal {
        volume_velocity: published.sd_m2 * velocity,
        impedance,
        current_a: current,
        excursion_m,
    }
}
