#![forbid(unsafe_code)]
//! Read-only `stdio` Model Context Protocol (MCP) server for Rotameter.

use rotameter_client::ComponentInfo;

/// Returns static metadata describing `rotameter-mcp`.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        name: "rotameter-mcp",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns the underlying `rotameter-client` metadata linked by this crate.
pub const fn client_info() -> ComponentInfo {
    rotameter_client::component_info()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_client() {
        assert_eq!(component_info().name, "rotameter-mcp");
        assert_eq!(component_info().version, client_info().version);
    }
}
