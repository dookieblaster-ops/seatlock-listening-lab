//! Sample-domain filters. Empty until the biquads match the browser reference.

/// True only after a fixed input matches the reference engine sample for sample.
pub fn parity_proven() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_is_not_claimed() {
        assert!(!parity_proven());
    }
}
