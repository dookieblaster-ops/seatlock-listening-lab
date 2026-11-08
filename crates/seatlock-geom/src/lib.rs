//! Parametric cabinet solid. Empty until a solid exports and its volume matches the box model.
//!
//! A fold or a novel chamber is not a frequency response. Helmholtz does not know about a bend.

/// True only after an exported solid reports the litres the box model was given.
pub fn solid_matches_box() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_solid_is_claimed() {
        assert!(!solid_matches_box());
    }
}
