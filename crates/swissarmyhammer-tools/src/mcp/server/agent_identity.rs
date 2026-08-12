//! The kanban actor a connecting MCP client is given.
//!
//! A client announces a name; the board needs a stable id, a colour, and an
//! avatar for it. All three are derived from that name alone, so the same
//! client reconnecting is the same actor.

use super::McpServer;
use std::path::PathBuf;

impl McpServer {
    /// Ensure an agent actor exists for the connecting MCP client.
    ///
    /// Slugifies the client name as the actor ID, derives a deterministic color,
    /// and generates a geometric SVG avatar. Idempotent via `ensure: true`.
    pub(super) async fn ensure_agent_actor(&self, client_name: &str) {
        use swissarmyhammer_kanban::actor::AddActor;
        use swissarmyhammer_kanban::{Execute, KanbanContext};

        let working_dir = self
            .tool_context
            .working_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let kanban_dir = working_dir.join(".kanban");

        if !kanban_dir.is_dir() {
            tracing::debug!("no .kanban directory, skipping agent actor creation");
            return;
        }

        let actor_id = slugify(client_name);
        let color = agent_deterministic_color(&actor_id);

        // No stored avatar — frontend renders initials as fallback
        let ctx = KanbanContext::new(kanban_dir);
        let cmd = AddActor::new(actor_id.as_str(), client_name)
            .with_ensure()
            .with_color(&color);

        match cmd.execute(&ctx).await.into_result() {
            Ok(result) => {
                let created = result["created"].as_bool().unwrap_or(false);
                if created {
                    tracing::info!(id = %actor_id, name = %client_name, "created MCP agent actor");
                } else {
                    tracing::debug!(id = %actor_id, "MCP agent actor already exists");
                }
                // Store the actor_id so tool calls can auto-inject it
                *self.tool_context.session_actor.write().await = Some(actor_id);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to ensure MCP agent actor");
            }
        }
    }
}

/// Slugify a name into a valid actor ID (lowercase, hyphens for spaces/special chars).
fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Curated palette for agent actors (cooler tones to distinguish from human actors).
const AGENT_COLORS: &[&str] = &[
    "5a67d8", "3182ce", "319795", "2f855a", "805ad5", "6b46c1", "2b6cb0", "2c7a7b", "4c51bf",
    "38a169",
];

/// Derive a deterministic hex color for an agent actor.
fn agent_deterministic_color(id: &str) -> String {
    let hash: u64 = id
        .bytes()
        .fold(5381u64, |h, b| h.wrapping_mul(33).wrapping_add(b as u64));
    AGENT_COLORS[(hash as usize) % AGENT_COLORS.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Claude Code"), "claude-code");
        assert_eq!(slugify("my_agent"), "my-agent");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("UPPER"), "upper");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn test_agent_deterministic_color_stable() {
        let c1 = agent_deterministic_color("claude-code");
        let c2 = agent_deterministic_color("claude-code");
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 6);
    }
}
