//! Chamber damping fill for the everyday networks (README §5.4 / §5.5).
//!
//! A listed chamber's `fill` gives a flow resistivity `σ` (Pa·s/m²) from `catalog/damping/`.
//! The full 3D model uses that figure in a Rayleigh update on every air cell. The quick and
//! detailed models use the same `σ` in the Delany–Bazley equivalent-fluid bulk modulus so the
//! chamber's compliance becomes complex and lossy:
//!
//! [DB70] Delany M. E. and Bazley E. N., Acoustical properties of fibrous absorbent materials,
//!        Applied Acoustics 3, 1970, pp. 105–116.
//! [AC92] Allard J.-F. and Champoux Y., New empirical equations for sound propagation in rigid
//!        frame fibrous materials, J. Acoust. Soc. Am. 91(6), 1992, pp. 3346–3353
//!        (SI form of the Delany–Bazley fit with `X = ρ₀ f / σ`).
//!
//! Validity quoted by Delany–Bazley: `0.01 < f/σ < 1` (f in Hz, σ in SI). Outside that band
//! the formulas are still evaluated and the fidelity label should say so. The 3D Rayleigh model
//! is a different equivalent fluid (real bulk modulus, complex density); the two will not match
//! bin-for-bin, and the label says which model each path uses.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};

/// Complex bulk modulus `K̃` (Pa) of a fibrous fill from flow resistivity (Allard–Champoux
/// SI form of Delany–Bazley). Empty air is `ρ₀ c₀²`.
///
/// Allard–Champoux write `Zc` and `kc` with `−j` for the `e^{+jωt}` wave convention. The
/// cavity compliance here is `Y = j ω V / K̃` under the same time convention, which is passive
/// only when `Im(K̃) ≥ 0`. Forming `K̃ = ρ₀ c₀² · Zc/kc` from the `−j` wave quantities gives
/// `Im(K̃) < 0` (active). The magnitudes are those of Delany–Bazley; the imag signs used here
/// are the original Delany–Bazley (`+j`) signs so `Y` absorbs power. Lined-duct propagation
/// should keep the Allard–Champoux `−j` wave quantities separately if added later.
pub fn delany_bazley_bulk_modulus(sigma_pa_s_m2: f64, freq_hz: f64) -> Complex64 {
    let sigma = sigma_pa_s_m2.max(1.0);
    let freq = freq_hz.max(1e-6);
    let x = (RHO0 * freq / sigma).max(1e-9);
    // Zc /(ρ0 c0) and kc · c0 / ω — Delany–Bazley +j imag parts for passive Cap (see above).
    let zc_norm = Complex64::new(1.0 + 0.0571 * x.powf(-0.754), 0.0870 * x.powf(-0.732));
    let kc_norm = Complex64::new(1.0 + 0.0978 * x.powf(-0.700), 0.1890 * x.powf(-0.595));
    // K̃ = ω Zc / kc = ρ0 c0² · (Zc_norm / kc_norm).
    RHO0 * C0 * C0 * zc_norm / kc_norm
}

/// Acoustic admittance of a chamber volume with optional fibrous fill: `Y = j ω V / K`.
pub fn chamber_admittance(volume_m3: f64, freq_hz: f64, fill_sigma: Option<f64>) -> Complex64 {
    let omega = 2.0 * std::f64::consts::PI * freq_hz.max(1e-6);
    let k = match fill_sigma.filter(|s| *s > 0.0) {
        Some(sigma) => delany_bazley_bulk_modulus(sigma, freq_hz),
        None => Complex64::new(RHO0 * C0 * C0, 0.0),
    };
    Complex64::new(0.0, omega) * volume_m3 / k
}

/// Parallel Caps: empty air volume plus one Delany–Bazley Cap per filled part
/// `(volume_m3, σ)`. Used for imported damping blocks that only pack part of the air
/// (README §5.4 / §6.2); placement is ignored — only volumes and σ enter the everyday model.
pub fn chamber_admittance_with_parts(
    air_volume_m3: f64,
    filled_parts: &[(f64, f64)],
    freq_hz: f64,
) -> Complex64 {
    let mut y = chamber_admittance(air_volume_m3, freq_hz, None);
    for &(volume, sigma) in filled_parts {
        if volume > 0.0 && sigma > 0.0 {
            y += chamber_admittance(volume, freq_hz, Some(sigma));
        }
    }
    y
}

/// True when `f/σ` is outside the Delany–Bazley quoted band (σ in SI, f in Hz).
pub fn delany_bazley_out_of_band(sigma_pa_s_m2: f64, freq_hz: f64) -> bool {
    let sigma = sigma_pa_s_m2.max(1e-12);
    let ratio = freq_hz.max(0.0) / sigma;
    !(0.01..=1.0).contains(&ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_fill_matches_the_lossless_compliance() {
        let v = 0.02;
        let freq = 50.0;
        let y = chamber_admittance(v, freq, None);
        let omega = 2.0 * std::f64::consts::PI * freq;
        let expect = Complex64::new(0.0, omega * v / (RHO0 * C0 * C0));
        assert!(
            (y - expect).norm() / expect.norm() < 1e-12,
            "{y} vs {expect}"
        );
    }

    #[test]
    fn filled_chamber_has_a_real_admittance_part() {
        // Isover AFr11: σ = 11000. At 100 Hz, X = ρ₀ f / σ ≈ 0.011 (f/σ ≈ 0.009).
        let y = chamber_admittance(0.02, 100.0, Some(11_000.0));
        assert!(y.re > 0.0, "fill should add loss: {y}");
        let empty = chamber_admittance(0.02, 100.0, None);
        assert!(
            y.im > empty.im,
            "fill softens the compliance (larger Im Y): {y} vs {empty}"
        );
    }

    #[test]
    fn out_of_band_flag_matches_delany_bazley_limits() {
        assert!(!delany_bazley_out_of_band(10_000.0, 100.0)); // f/σ = 0.01
        assert!(delany_bazley_out_of_band(10_000.0, 50.0)); // 0.005
        assert!(delany_bazley_out_of_band(100.0, 200.0)); // 2.0
    }

    #[test]
    fn partial_fill_parts_add_in_parallel() {
        let air = 0.015;
        let filled = 0.005;
        let sigma = 11_000.0;
        let freq = 100.0;
        let y = chamber_admittance_with_parts(air, &[(filled, sigma)], freq);
        let expect =
            chamber_admittance(air, freq, None) + chamber_admittance(filled, freq, Some(sigma));
        assert!(
            (y - expect).norm() < 1e-12 * expect.norm(),
            "{y} vs {expect}"
        );
        assert!(y.re > 0.0, "partial fill should add loss: {y}");
        let all_air = chamber_admittance(air + filled, freq, None);
        assert!(y.re > all_air.re, "loss vs empty air of same total volume");
    }
}
