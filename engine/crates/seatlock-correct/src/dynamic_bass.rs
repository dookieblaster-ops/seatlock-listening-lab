//! C6: Level-aware bass (README §8).
//!
//! Adds a look-ahead `dynamic_bass` DSP block driven by free-air cone travel
//! `X/V = bl / (jω (Ze·Zm + bl²))`. Enclosure load is omitted: free-air travel
//! is at least as large in the deep bass for a sealed box, so the limiter is
//! slightly conservative there.

/// Fields for one C6 `dynamic_bass` correction block.
#[derive(Clone, Debug)]
pub struct DynamicBassPlan {
    pub xmax_m: f64,
    pub re_ohm: f64,
    pub le_h: f64,
    pub mms_kg: f64,
    pub cms_m_per_n: f64,
    pub rms_ns_per_m: f64,
    pub bl_tm: f64,
    pub look_ahead_ms: f64,
    pub reason: String,
    pub message: String,
}

/// Plan C6 when the radiator publishes usable cone travel and mechanical params.
#[allow(clippy::too_many_arguments)]
pub fn dynamic_bass_plan(
    xmax_m: f64,
    re_ohm: f64,
    le_h: f64,
    mms_kg: f64,
    cms_m_per_n: f64,
    rms_ns_per_m: f64,
    bl_tm: f64,
    look_ahead_ms: f64,
) -> Option<DynamicBassPlan> {
    if xmax_m <= 0.0
        || re_ohm <= 0.0
        || mms_kg <= 0.0
        || cms_m_per_n <= 0.0
        || rms_ns_per_m <= 0.0
        || bl_tm <= 0.0
    {
        return None;
    }
    let look_ahead_ms = if look_ahead_ms > 0.0 {
        look_ahead_ms
    } else {
        5.0
    };
    Some(DynamicBassPlan {
        xmax_m,
        re_ohm,
        le_h: le_h.max(0.0),
        mms_kg,
        cms_m_per_n,
        rms_ns_per_m,
        bl_tm,
        look_ahead_ms,
        reason: format!(
            "Level-aware bass (C6): look-ahead limiter from cone travel X/V, \
             xmax {xmax_mm:.1} mm.",
            xmax_mm = xmax_m * 1_000.0
        ),
        message: format!(
            "Level-aware bass (C6): look-ahead {look_ahead_ms:.0} ms from free-air X/V \
             (xmax {xmax_mm:.1} mm).",
            xmax_mm = xmax_m * 1_000.0
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_travel() {
        assert!(dynamic_bass_plan(0.0, 6.0, 0.0, 0.01, 0.001, 1.0, 5.0, 5.0).is_none());
    }

    #[test]
    fn accepts_test_woofer_params() {
        let plan =
            dynamic_bass_plan(0.004, 6.0, 0.0005, 0.012, 0.0008, 1.0, 7.0, 5.0).expect("plan");
        assert!(plan.reason.contains("C6"));
        assert!((plan.look_ahead_ms - 5.0).abs() < 1e-9);
    }
}
