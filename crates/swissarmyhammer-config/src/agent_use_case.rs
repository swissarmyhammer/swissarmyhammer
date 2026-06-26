//! Use-case-based agent selection.
//!
//! Different operations can resolve to different agents (a fast/cheap model for
//! rule checking, a capable model for workflow execution, etc.). [`AgentUseCase`]
//! enumerates the known use cases; consumers map a use case to a concrete agent
//! configuration, falling back to the [`AgentUseCase::Root`] agent when a use
//! case is unconfigured.
//!
//! This enum is the single source of truth for use-case identity and is placed
//! in `swissarmyhammer-config` so both the config layer and the tools layer can
//! share it (see `ideas/rule_agent.md`, "Design Decisions": enum placement).

use serde::{Deserialize, Serialize};

/// Enumeration of agent use cases.
///
/// Each variant identifies a distinct operation that may be assigned its own
/// agent. The config surface maps lowercase string keys (`"root"`, `"rules"`,
/// `"workflows"`, `"expectations"`) directly onto these variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentUseCase {
    /// Default/fallback agent for general operations.
    Root,
    /// Agent for rule checking operations.
    Rules,
    /// Agent for workflow execution (plan, review, implement, etc.).
    Workflows,
    /// Agent that drives `expect` expectation runs.
    Expectations,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lowercase wire string for each use case. Tests assert against this
    /// table rather than re-typing the literals at each call site.
    const WIRE_CASES: &[(AgentUseCase, &str)] = &[
        (AgentUseCase::Root, "root"),
        (AgentUseCase::Rules, "rules"),
        (AgentUseCase::Workflows, "workflows"),
        (AgentUseCase::Expectations, "expectations"),
    ];

    #[test]
    fn serializes_to_lowercase_strings() {
        for (use_case, wire) in WIRE_CASES {
            let json = serde_json::to_string(use_case).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
        }
    }

    #[test]
    fn deserializes_from_lowercase_strings() {
        for (use_case, wire) in WIRE_CASES {
            let parsed: AgentUseCase = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, *use_case);
        }
    }

    #[test]
    fn round_trips_through_serde() {
        for (use_case, _) in WIRE_CASES {
            let json = serde_json::to_string(use_case).unwrap();
            let parsed: AgentUseCase = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, *use_case);
        }
    }
}
