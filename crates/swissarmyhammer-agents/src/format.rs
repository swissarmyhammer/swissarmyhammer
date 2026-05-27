//! AGENT.md serialization
//!
//! Converts a parsed [`Agent`] back into AGENT.md file content
//! (YAML frontmatter + rendered body). This is the inverse of
//! [`crate::agent_loader::parse_agent_md`] up to field-presence rules:
//! optional fields that were absent in the source stay absent in the output,
//! and empty collections (tools, disallowed_tools, metadata, skills) are
//! omitted.
//!
//! Skills are serialized as a YAML sequence when non-empty. This matters
//! because the CLI's deploy pipeline writes the materialized AGENT.md into
//! sah's own mirdan agent store (`~/.agents/<name>/AGENT.md` for the User
//! scope, `.agents/<name>/AGENT.md` for Project/Local), and
//! [`crate::agent_resolver::AgentResolver`] then re-reads those files with
//! precedence `builtin < user < local`. If `skills:` is dropped on output,
//! the deployed override silently strips the field — so a builtin agent
//! that ships with `skills: [test]` becomes a skills-less agent on disk
//! that overrides the builtin in the resolver, and runtime skill resolution
//! fails. Emitting `skills:` preserves the field across the
//! serialize → deploy → re-parse round-trip the resolver actually walks.
//!
//! Used by the CLI's agent install/deploy flow to materialize an `AGENT.md`
//! in a temp directory before `mirdan::install::deploy_agent_to_agents`
//! copies it into the coding-agent stores.
//!
//! # Roundtrip Guarantee
//!
//! Parsing an AGENT.md, then serializing the result with `to_agent_md`
//! using the original body, is stable for all fields that the serializer
//! actually emits. See the `tests` module for the exhaustive roundtrip
//! test.

use crate::agent::Agent;

impl Agent {
    /// Serialize this agent back into AGENT.md file content.
    ///
    /// The output is YAML frontmatter (delimited by `---`) followed by
    /// `rendered_body`. The body is passed in (rather than read from
    /// `self.instructions`) because the caller typically wants to render
    /// Liquid template partials in the instructions before writing the
    /// final file to disk.
    ///
    /// Field-presence rules:
    /// - `name` and `description` are always emitted.
    /// - `model`, `isolation`, `max_turns` are emitted only when present.
    /// - `tools` is emitted only when non-empty; the single-tool form
    ///   `["*"]` is rendered as `tools: "*"`.
    /// - `disallowed_tools` is emitted only when non-empty.
    /// - `background` is emitted only when `true`.
    /// - `metadata` is emitted only when non-empty, with keys sorted
    ///   for deterministic output.
    /// - `skills` is emitted only when non-empty, as a YAML sequence.
    ///
    /// # Arguments
    /// * `rendered_body` - The agent instructions to embed after the
    ///   frontmatter, with any Liquid partials already expanded.
    ///
    /// # Returns
    /// A `String` containing complete AGENT.md content suitable for
    /// writing to disk.
    pub fn to_agent_md(&self, rendered_body: &str) -> String {
        let mut content = String::from("---\n");
        content.push_str(&format!("name: {}\n", self.name));
        content.push_str(&format!("description: {}\n", self.description));

        if let Some(ref model) = self.model {
            content.push_str(&format!("model: {}\n", model));
        }

        if !self.tools.is_empty() {
            if self.tools.len() == 1 && self.tools[0] == "*" {
                content.push_str("tools: \"*\"\n");
            } else {
                let tools = self.tools.join(" ");
                content.push_str(&format!("tools: \"{}\"\n", tools));
            }
        }

        if !self.disallowed_tools.is_empty() {
            let tools = self.disallowed_tools.join(" ");
            content.push_str(&format!("disallowed-tools: \"{}\"\n", tools));
        }

        if !self.skills.is_empty() {
            content.push_str("skills:\n");
            for skill in &self.skills {
                content.push_str(&format!("  - {}\n", skill));
            }
        }

        if let Some(ref isolation) = self.isolation {
            content.push_str(&format!("isolation: {}\n", isolation));
        }

        if let Some(max_turns) = self.max_turns {
            content.push_str(&format!("max-turns: {}\n", max_turns));
        }

        if self.background {
            content.push_str("background: true\n");
        }

        if !self.metadata.is_empty() {
            content.push_str("metadata:\n");
            let mut keys: Vec<_> = self.metadata.keys().collect();
            keys.sort();
            for key in keys {
                content.push_str(&format!("  {}: \"{}\"\n", key, self.metadata[key]));
            }
        }

        content.push_str("---\n\n");
        content.push_str(rendered_body);
        content.push('\n');

        content
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::AgentSource;
    use crate::agent_loader::parse_agent_md;

    /// Parse → serialize → parse → assert agent fields match.
    ///
    /// This is the core roundtrip guarantee: the serializer emits
    /// frontmatter that the parser accepts back into an equivalent
    /// `Agent`. We compare structurally (field-by-field) rather than
    /// byte-for-byte, because the serializer is allowed to canonicalize
    /// (e.g. sort metadata keys) — what matters is that no data is lost.
    #[test]
    fn test_roundtrip_full_agent() {
        let original = r#"---
name: full-agent
description: An agent exercising every serialized field
model: sonnet
tools: "read write bash"
disallowed-tools: "web-fetch"
skills:
  - test
  - implement
isolation: worktree
max-turns: 42
background: true
metadata:
  version: "1.0"
  author: "alice"
---

# Full Agent

Body content with **markdown**.
"#;

        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        let body = agent.instructions.clone();
        let serialized = agent.to_agent_md(&body);

        let reparsed = parse_agent_md(&serialized, AgentSource::Builtin).unwrap();

        assert_eq!(reparsed.name, agent.name);
        assert_eq!(reparsed.description, agent.description);
        assert_eq!(reparsed.model, agent.model);
        assert_eq!(reparsed.tools, agent.tools);
        assert_eq!(reparsed.disallowed_tools, agent.disallowed_tools);
        assert_eq!(reparsed.skills, agent.skills);
        assert_eq!(reparsed.isolation, agent.isolation);
        assert_eq!(reparsed.max_turns, agent.max_turns);
        assert_eq!(reparsed.background, agent.background);
        assert_eq!(reparsed.metadata, agent.metadata);
        assert_eq!(reparsed.instructions, agent.instructions);
    }

    /// An agent with a non-empty `skills` vec must round-trip
    /// parse → serialize → parse with the `skills` field preserved.
    ///
    /// This guards against the resolver-override bug: the CLI's deploy
    /// pipeline writes this serialized AGENT.md into the mirdan store, and
    /// `AgentResolver` re-reads it with precedence `builtin < user < local`.
    /// If `skills:` is dropped on output, the deployed file overrides the
    /// builtin with an empty `skills` vec and runtime skill resolution
    /// silently fails.
    #[test]
    fn test_roundtrip_preserves_skills() {
        let original = r#"---
name: skilled
description: Agent with skills frontmatter
skills:
  - test
  - implement
---

Body.
"#;

        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        assert_eq!(agent.skills, vec!["test", "implement"]);

        let serialized = agent.to_agent_md(&agent.instructions);
        assert!(
            serialized.contains("skills:\n  - test\n  - implement\n"),
            "serialized output should contain the skills sequence, got:\n{}",
            serialized
        );

        let reparsed = parse_agent_md(&serialized, AgentSource::Builtin).unwrap();
        assert_eq!(reparsed.skills, vec!["test", "implement"]);
    }

    /// An agent with no skills must omit the `skills:` key entirely,
    /// keeping the serializer symmetric with the `#[serde(default)]` shape
    /// on the parse side.
    #[test]
    fn test_empty_skills_is_omitted() {
        let original = r#"---
name: skill-less
description: No skills field
---

Body.
"#;
        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        assert!(agent.skills.is_empty());

        let serialized = agent.to_agent_md(&agent.instructions);
        assert!(!serialized.contains("skills:"));
    }

    /// Minimal agent (only required fields) roundtrips cleanly, and no
    /// optional fields leak into the output.
    #[test]
    fn test_roundtrip_minimal_agent() {
        let original = r#"---
name: tiny
description: Minimal agent
---

Just a body.
"#;

        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        let serialized = agent.to_agent_md(&agent.instructions);

        // No optional field markers should appear.
        assert!(!serialized.contains("model:"));
        assert!(!serialized.contains("tools:"));
        assert!(!serialized.contains("disallowed-tools:"));
        assert!(!serialized.contains("skills:"));
        assert!(!serialized.contains("isolation:"));
        assert!(!serialized.contains("max-turns:"));
        assert!(!serialized.contains("background:"));
        assert!(!serialized.contains("metadata:"));

        let reparsed = parse_agent_md(&serialized, AgentSource::Builtin).unwrap();
        assert_eq!(reparsed.name, agent.name);
        assert_eq!(reparsed.description, agent.description);
        assert!(reparsed.tools.is_empty());
        assert!(reparsed.disallowed_tools.is_empty());
        assert!(reparsed.metadata.is_empty());
        assert!(!reparsed.background);
    }

    /// The wildcard tool form `tools: "*"` is rendered with the quoted
    /// glob and roundtrips back to a single-element `["*"]` vector.
    #[test]
    fn test_roundtrip_wildcard_tools() {
        let original = r#"---
name: wild
description: Wildcard tools
tools: "*"
---

Body.
"#;
        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        assert_eq!(agent.tools, vec!["*"]);

        let serialized = agent.to_agent_md(&agent.instructions);
        assert!(serialized.contains("tools: \"*\"\n"));

        let reparsed = parse_agent_md(&serialized, AgentSource::Builtin).unwrap();
        assert_eq!(reparsed.tools, vec!["*"]);
    }

    /// `background: false` is the default and must not appear in output.
    #[test]
    fn test_background_false_is_omitted() {
        let original = r#"---
name: fg
description: Foreground agent
---

Body.
"#;
        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        let serialized = agent.to_agent_md(&agent.instructions);
        assert!(!serialized.contains("background"));
    }

    /// Metadata keys are sorted in the output so the serialization is
    /// deterministic regardless of `HashMap` iteration order.
    #[test]
    fn test_metadata_keys_sorted() {
        let original = r#"---
name: meta
description: Sorted metadata
metadata:
  zebra: "z"
  alpha: "a"
  mango: "m"
---

Body.
"#;
        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        let serialized = agent.to_agent_md(&agent.instructions);

        let alpha_pos = serialized.find("alpha").unwrap();
        let mango_pos = serialized.find("mango").unwrap();
        let zebra_pos = serialized.find("zebra").unwrap();
        assert!(alpha_pos < mango_pos);
        assert!(mango_pos < zebra_pos);
    }

    /// The body that follows the frontmatter survives unchanged
    /// (the parser trims surrounding whitespace, but interior content
    /// is preserved).
    #[test]
    fn test_body_passed_through() {
        let original = r#"---
name: body-agent
description: Body preservation
---

# Heading

Paragraph with **bold** and `code`.

- item one
- item two
"#;
        let agent = parse_agent_md(original, AgentSource::Builtin).unwrap();
        let serialized = agent.to_agent_md(&agent.instructions);
        assert!(serialized.contains("# Heading"));
        assert!(serialized.contains("**bold**"));
        assert!(serialized.contains("- item one"));
    }
}
