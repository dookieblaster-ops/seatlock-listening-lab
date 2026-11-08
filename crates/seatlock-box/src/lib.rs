//! Lumped box model. Empty until a published alignment lands on its published knee.

/// True only after a known box reproduces its published F3.
pub fn alignment_proven() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_alignment_is_claimed() {
        assert!(!alignment_proven());
    }
}
