#![forbid(unsafe_code)]
//! Pure domain logic, types, and storage trait contracts for Rotameter.
//!
//! This crate is intentionally free of OS I/O, async runtimes, and storage engines.

/// Lockstep workspace semantic version shared by all Rotameter crates.
pub const WORKSPACE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Static identity metadata for a compiled Rotameter workspace crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentInfo {
    /// Crate name as declared in its manifest.
    pub name: &'static str,
    /// Crate version in lockstep with the workspace.
    pub version: &'static str,
}

/// Returns static metadata describing `rotameter-core`.
///
/// # Examples
///
/// ```
/// let info = rotameter_core::component_info();
/// assert_eq!(info.name, "rotameter-core");
/// assert_eq!(info.version, rotameter_core::WORKSPACE_VERSION);
/// ```
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-core",
        version: WORKSPACE_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_component_metadata_is_populated() {
        let info = component_info();
        assert_eq!(info.name, "rotameter-core");
        assert_ne!(info.version, "");
    }
}
