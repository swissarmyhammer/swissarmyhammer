//! What the server tells a client about itself at handshake: the instruction
//! text, the advertised capabilities, and the implementation identity.
//!
//! The instruction text is not static — [`build_instructions_with_health`]
//! appends a `setupStatus:` block naming any LSP server the workspace needs and
//! does not have, so a client sees the gap without running a separate check.

use rmcp::model::*;
use std::path::Path;

/// Server instructions displayed to MCP clients
const SERVER_INSTRUCTIONS: &str =
    "The only coding assistant you'll ever need. Agent-driven engineering.";

/// Build server instructions, optionally appending LSP health status.
///
/// When a work directory is provided, runs the doctor check to detect project
/// types and their LSP servers. If any LSP servers are missing, a
/// `setupStatus:` block is appended listing the missing servers with install
/// hints. If all servers are installed (or no projects are detected), returns
/// just the base instructions to avoid noise.
pub(crate) fn build_instructions_with_health(work_dir: Option<&Path>) -> String {
    let Some(path) = work_dir else {
        return SERVER_INSTRUCTIONS.to_string();
    };

    let report = crate::mcp::tools::code_context::doctor::run_doctor(path);

    let missing: Vec<_> = report.lsp_servers.iter().filter(|s| !s.installed).collect();

    if missing.is_empty() {
        return SERVER_INSTRUCTIONS.to_string();
    }

    let mut instructions = SERVER_INSTRUCTIONS.to_string();
    instructions.push_str("\n\nsetupStatus: This workspace could benefit from additional tooling.");
    for server in &missing {
        let hint = server.install_hint.as_deref().unwrap_or("see project docs");
        instructions.push_str(&format!("\n  {}: NOT INSTALLED — {}", server.name, hint));
    }

    instructions
}

/// Create ServerCapabilities for MCP protocol
pub(super) fn create_server_capabilities() -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    caps.tools = Some(ToolsCapability {
        list_changed: Some(true),
    });
    // Advertise the subscribable diagnostics resource so a host can subscribe and
    // receive `notifications/resources/updated` on every diagnostics change —
    // diagnostics without a tool call (see `mcp::diagnostics_resource`).
    caps.resources = Some(ResourcesCapability {
        subscribe: Some(true),
        list_changed: Some(false),
    });
    caps
}

/// Create Implementation information for the MCP server
pub(super) fn create_server_implementation() -> Implementation {
    Implementation::new("SwissArmyHammer", crate::VERSION)
        .with_title("SwissArmyHammer MCP Server")
        .with_website_url("https://github.com/swissarmyhammer/swissarmyhammer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_instructions_no_work_dir() {
        let result = build_instructions_with_health(None);
        assert_eq!(result, SERVER_INSTRUCTIONS);
    }

    #[test]
    fn test_build_instructions_no_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let result = build_instructions_with_health(Some(tmp.path()));
        assert_eq!(result, SERVER_INSTRUCTIONS);
    }

    #[test]
    fn test_build_instructions_with_missing_lsp() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let result = build_instructions_with_health(Some(tmp.path()));
        // Result always starts with the base instructions
        assert!(result.starts_with(SERVER_INSTRUCTIONS));
        // If rust-analyzer is not installed, we should see the setupStatus block
        if result.len() > SERVER_INSTRUCTIONS.len() {
            assert!(result.contains("setupStatus:"));
            assert!(result.contains("NOT INSTALLED"));
        }
    }

    // ---------------------------------------------------------------
    // ServerCapabilities and Implementation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_create_server_capabilities() {
        let caps = create_server_capabilities();
        assert!(
            caps.prompts.is_none(),
            "Server must not advertise the prompts capability — the MCP prompt protocol surface was removed"
        );
        assert!(caps.tools.is_some(), "Should have tools capability");
        assert_eq!(
            caps.tools.as_ref().unwrap().list_changed,
            Some(true),
            "Tools should support list_changed"
        );
        // The subscribable diagnostics resource requires the resources capability
        // with `subscribe: true` so a host may `resources/subscribe`.
        let resources = caps
            .resources
            .as_ref()
            .expect("Should advertise the resources capability for diagnostics");
        assert_eq!(
            resources.subscribe,
            Some(true),
            "Resources must support subscribe for the diagnostics resource"
        );
    }

    #[test]
    fn test_create_server_implementation() {
        let info = create_server_implementation();
        assert_eq!(info.name.as_str(), "SwissArmyHammer");
    }
}
