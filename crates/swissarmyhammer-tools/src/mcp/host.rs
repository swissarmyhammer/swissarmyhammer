//! Host identity of a connecting MCP client and the served-set policy.
//!
//! The SAH MCP server composes the tool surface it advertises **per connecting
//! client**, driven by the MCP `initialize` handshake's client `Implementation`
//! name — not a static union. This module owns the two pieces that drive that
//! composition:
//!
//! 1. [`Host`] — the host identity SAH recognizes, plus [`Host::from_client_info`]
//!    which maps an rmcp [`Implementation`] name to one of those identities.
//! 2. [`Host::serves`] — the single predicate `(Host, ToolCategory) -> bool`
//!    that decides whether SAH serves a tool of a given category to a given host.
//!
//! Centralizing the name → identity mapping here means later serve-time policy
//! (e.g. the Bash-deny for Claude's native shell) reuses the exact same mapping
//! instead of re-deriving it inline.

use rmcp::model::Implementation;

use crate::mcp::tool_registry::ToolCategory;

/// A connecting MCP client's host identity, as recognized by SAH.
///
/// Determined from the client `Implementation` name reported at `initialize`.
/// Drives the per-client served-set composition via [`Host::serves`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Claude Code (or another Claude host). Has native agent tools (file
    /// read/write/edit, web, etc.) and a native `Bash`; SAH serves it the
    /// `Shared` tools plus `Replacement` tools (which supersede a native).
    Claude,
    /// The llama-agent host. Mounts its agent + replacement tools as its own
    /// in-memory built-ins, so SAH serves it `Shared` tools only.
    Llama,
    /// Any other / unknown client. Conservative default: `Shared` only.
    Other,
}

/// Substring patterns that identify a host from a client `Implementation` name.
///
/// Matched case-insensitively as substrings so version- or transport-specific
/// suffixes (e.g. `llama_agent_notifying_client`) still resolve. Order does not
/// matter because the recognized substrings are disjoint. Data-driven so adding
/// a host means adding a row here, not a new control-flow branch.
const HOST_PATTERNS: &[(&str, Host)] = &[("claude", Host::Claude), ("llama", Host::Llama)];

impl Host {
    /// Map a client `Implementation` to a [`Host`] identity.
    ///
    /// Matches the client's reported name case-insensitively against
    /// [`HOST_PATTERNS`]. An unrecognized name (or the absence of client info)
    /// resolves to [`Host::Other`], the conservative default.
    ///
    /// Known names: Claude Code reports `"claude-code"`; llama-agent reports
    /// `"llama_agent_notifying_client"`.
    pub fn from_client_info(client_info: &Implementation) -> Self {
        let name = client_info.name.to_ascii_lowercase();
        HOST_PATTERNS
            .iter()
            .find(|(pattern, _)| name.contains(pattern))
            .map(|(_, host)| *host)
            .unwrap_or(Host::Other)
    }

    /// Whether SAH serves a tool of `category` to this host.
    ///
    /// The single source of truth for the per-client served-set rule, expressed
    /// as a function of `(Host, ToolCategory)`:
    ///
    /// | category               | Claude | Llama | Other |
    /// |------------------------|--------|-------|-------|
    /// | [`ToolCategory::Shared`]      | yes | yes | yes |
    /// | [`ToolCategory::Agent`]       | no  | no  | no  |
    /// | [`ToolCategory::Replacement`] | yes | no  | no  |
    ///
    /// `Shared` tools are domain capabilities every host gets. `Agent` tools are
    /// base agent capabilities SAH never serves (off-the-shelf agents provide
    /// them natively, and llama mounts its own). `Replacement` tools supersede a
    /// named native host tool and are served only to Claude, where they reach
    /// the native host exactly once.
    pub fn serves(self, category: ToolCategory) -> bool {
        match category {
            ToolCategory::Shared => true,
            ToolCategory::Agent => false,
            ToolCategory::Replacement { .. } => self == Host::Claude,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impl_named(name: &str) -> Implementation {
        Implementation::new(name, "1.0.0")
    }

    #[test]
    fn claude_code_maps_to_claude() {
        assert_eq!(
            Host::from_client_info(&impl_named("claude-code")),
            Host::Claude
        );
    }

    #[test]
    fn llama_agent_client_maps_to_llama() {
        assert_eq!(
            Host::from_client_info(&impl_named("llama_agent_notifying_client")),
            Host::Llama
        );
    }

    #[test]
    fn unknown_client_maps_to_other() {
        assert_eq!(
            Host::from_client_info(&impl_named("some-random-mcp-client")),
            Host::Other
        );
    }

    #[test]
    fn name_match_is_case_insensitive() {
        assert_eq!(
            Host::from_client_info(&impl_named("Claude-Code")),
            Host::Claude
        );
        assert_eq!(Host::from_client_info(&impl_named("LLAMA")), Host::Llama);
    }

    #[test]
    fn claude_serves_shared_and_replacement_not_agent() {
        let claude = Host::Claude;
        assert!(claude.serves(ToolCategory::Shared));
        assert!(!claude.serves(ToolCategory::Agent));
        assert!(claude.serves(ToolCategory::Replacement { native: "Bash" }));
    }

    #[test]
    fn llama_serves_shared_only() {
        let llama = Host::Llama;
        assert!(llama.serves(ToolCategory::Shared));
        assert!(!llama.serves(ToolCategory::Agent));
        assert!(!llama.serves(ToolCategory::Replacement { native: "Bash" }));
    }

    #[test]
    fn other_serves_shared_only() {
        let other = Host::Other;
        assert!(other.serves(ToolCategory::Shared));
        assert!(!other.serves(ToolCategory::Agent));
        assert!(!other.serves(ToolCategory::Replacement { native: "Bash" }));
    }
}
