//! Drivers with a public source. The catalog starts empty on purpose.

/// A part that may be simulated. Numbers stay out of this crate until a sheet is cited.
pub struct DriverRecord {
    pub id: &'static str,
    pub source: &'static str,
    pub read_on: &'static str,
}

/// No driver is included until its sheet has been read again for this crate.
pub fn drivers() -> &'static [DriverRecord] {
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_starts_empty() {
        assert!(drivers().is_empty());
    }
}
