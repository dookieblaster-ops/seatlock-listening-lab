//! Air properties and reference pressure at 20 °C and 101.325 kPa (README section 4).

/// Air density, kg/m³.
pub const RHO0: f64 = 1.204;
/// Speed of sound, m/s.
pub const C0: f64 = 343.2;
/// Reference pressure for SPL, Pa RMS.
pub const P_REF: f64 = 20e-6;
