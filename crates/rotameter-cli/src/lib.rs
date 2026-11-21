#![forbid(unsafe_code)]
//! Unified feature-gated CLI library for Rotameter.

#[cfg(all(feature = "server", feature = "sqlite"))]
pub mod serve_store;

use rotameter_core::ComponentInfo;

/// Returns metadata for all workspace components compiled under the active feature set.
pub fn enabled_components() -> Vec<ComponentInfo> {
    let components = [
        rotameter_core::component_info(),
        rotameter_client::component_info(),
        #[cfg(feature = "server")]
        rotameter_server::component_info(),
        #[cfg(feature = "sqlite")]
        rotameter_store_sqlite::component_info(),
        #[cfg(feature = "mcp")]
        rotameter_mcp::component_info(),
        #[cfg(feature = "tui")]
        rotameter_tui::component_info(),
    ];
    components.to_vec()
}

/// Formats the CLI version banner listing enabled workspace components.
pub fn version_banner() -> String {
    let names: Vec<&'static str> = enabled_components().into_iter().map(|c| c.name).collect();
    format!(
        "rotameter {} ({})",
        rotameter_core::WORKSPACE_VERSION,
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_components_match_workspace_version() {
        let components = enabled_components();
        let names: Vec<&str> = components.iter().map(|c| c.name).collect();

        assert!(names.contains(&"rotameter-core"));
        assert!(names.contains(&"rotameter-client"));
        #[cfg(feature = "server")]
        assert!(names.contains(&"rotameter-server"));
        #[cfg(feature = "sqlite")]
        assert!(names.contains(&"rotameter-store-sqlite"));
        #[cfg(feature = "mcp")]
        assert!(names.contains(&"rotameter-mcp"));
        #[cfg(feature = "tui")]
        assert!(names.contains(&"rotameter-tui"));

        for component in components {
            assert_eq!(component.version, rotameter_core::WORKSPACE_VERSION);
        }
    }

    #[test]
    fn version_banner_lists_core_and_client() {
        let banner = version_banner();
        assert!(banner.starts_with("rotameter "));
        assert!(banner.contains("rotameter-core"));
        assert!(banner.contains("rotameter-client"));
    }
}
