//! Constants, units, design types, errors, and the kind registry shared by the engine.

pub mod catalog;
pub mod check;
pub mod constants;
pub mod design;
pub mod grid;
pub mod issue;
pub mod registry;

pub use check::{design_check, CheckResult, ENGINE_VERSION};
pub use constants::{C0, P_REF, RHO0};
pub use design::DesignFile;
pub use issue::{Issue, Severity};
pub use registry::Registry;
