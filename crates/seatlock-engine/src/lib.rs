//! The native listening engine. It does not render audio yet.
//!
//! Step 1 is a phone calling into this crate and playing silence. Filters come after that,
//! and only after they match the browser reference.

/// The audio callback is not implemented.
pub fn renders_audio() -> bool {
    let _ = (
        seatlock_dsp::parity_proven(),
        seatlock_box::alignment_proven(),
        seatlock_geom::solid_matches_box(),
        seatlock_catalog::drivers().len(),
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_does_not_render_audio() {
        assert!(!renders_audio());
    }
}
