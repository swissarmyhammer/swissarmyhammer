//! Pure skill-content helpers shared across CLI tools.
//!
//! Provides the common pipeline for resolving a builtin skill and formatting a
//! SKILL.md file with YAML frontmatter. This crate is deployment-free: the
//! filesystem staging + agent deployment step lives in
//! `mirdan::install::stage_and_deploy_skill`, so the dependency edge runs
//! mirdan → skills (not the other way around).
//!
//! Used by `shelltool-cli`, `code-context-cli`, and `kanban-cli` to avoid
//! duplicating the resolve → format logic. Template rendering (which depends on
//! `swissarmyhammer-templating`) is left to each CLI's thin wrapper because
//! adding that crate here would create a dependency cycle.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

use crate::skill::SAH_INTERNAL_FRONTMATTER_KEYS;
use crate::{Skill, SkillResolver};

/// YAML frontmatter fields for a SKILL.md file.
///
/// Mirrors the fields expected by [`crate::skill_loader::parse_skill_md`]
/// (the deserialization counterpart in the skill loader). Using `serde_yaml_ng`
/// to serialize this struct produces properly escaped YAML, even when field
/// values contain special characters like colons or quotes.
#[derive(Serialize)]
struct SkillFrontmatter<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(rename = "allowed-tools", skip_serializing_if = "Option::is_none")]
    allowed_tools: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<&'a str>,
    // BTreeMap keeps metadata keys sorted so the generated SKILL.md is
    // deterministic across runs (HashMap iteration order is not).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    metadata: BTreeMap<String, String>,
    /// Frontmatter keys SAH does not model, written back out verbatim.
    ///
    /// Declared last so these keys follow the modeled ones in the generated
    /// YAML. Filled from [`Skill::extra`], which never overlaps the fields
    /// above, so no key is written twice.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml_ng::Value>,
}

/// Resolve a builtin skill by name from the skill registry.
///
/// Looks up `name` in the set of compiled-in builtin skills.
///
/// # Errors
///
/// Returns an error message if no builtin skill matches `name`.
pub fn resolve_skill(name: &str) -> Result<Skill, String> {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    builtins
        .get(name)
        .cloned()
        .ok_or_else(|| format!("builtin '{name}' skill not found"))
}

/// Resolve all builtin skills tagged with the given init `profile`.
///
/// A skill belongs to a profile when its `profiles` frontmatter list contains
/// `profile`. Returns the matching skills in arbitrary order. Skills without a
/// `profiles` key (the default empty list) never match any profile.
pub fn resolve_profile_skills(profile: &str) -> Vec<Skill> {
    let resolver = SkillResolver::new();
    resolver
        .resolve_builtins()
        .into_values()
        .filter(|skill| skill.profiles.iter().any(|p| p == profile))
        .collect()
}

/// Format a skill as a complete SKILL.md file with YAML frontmatter.
///
/// Combines the skill's frontmatter fields (`name`, `description`,
/// `allowed_tools`, `license`, `compatibility`, `context`, `agent`, `metadata`)
/// into YAML frontmatter and appends
/// the already-rendered `instructions` as the body. The `metadata` parameter
/// is passed separately because it may have had template variables rendered.
///
/// Frontmatter keys SAH does not model — `hooks`, `model`, `paths` and the rest
/// of the harness's own set — are carried on [`Skill::extra`] and written back
/// out verbatim after the modeled ones, so the deploy round-trip is lossless.
/// The one exception is [`SAH_INTERNAL_FRONTMATTER_KEYS`], which drive SAH's own
/// machinery and are dropped here.
///
/// Uses `serde_yaml_ng` to serialize the frontmatter, ensuring that values
/// containing YAML special characters (colons, quotes, newlines) are properly
/// escaped. The output is compatible with [`crate::skill_loader::parse_skill_md`].
///
/// # Panics
///
/// Panics if `serde_yaml_ng` fails to serialize the frontmatter struct, which
/// should be unreachable since all fields are plain strings.
pub fn format_skill_md(
    skill: &Skill,
    instructions: &str,
    metadata: &HashMap<String, String>,
) -> String {
    let allowed_tools = if skill.allowed_tools.is_empty() {
        None
    } else {
        Some(skill.allowed_tools.join(" "))
    };

    let frontmatter = SkillFrontmatter {
        name: skill.name.as_str(),
        description: &skill.description,
        allowed_tools,
        license: skill.license.as_deref(),
        compatibility: skill.compatibility.as_deref(),
        context: skill.context.as_deref(),
        agent: skill.agent.as_deref(),
        metadata: metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        // Strip the internal keys again on the way out. The loader already
        // keeps them out of `Skill::extra`, but a `Skill` can also be built by
        // hand, so enforcing it here too means no internal key can reach a
        // deployed file by any route.
        extra: skill
            .extra
            .iter()
            .filter(|(key, _)| !SAH_INTERNAL_FRONTMATTER_KEYS.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    };

    let yaml = serde_yaml_ng::to_string(&frontmatter)
        .expect("SkillFrontmatter serialization should not fail");

    format!("---\n{yaml}---\n\n{instructions}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SkillName, SkillResources, SkillSource};

    #[test]
    fn test_resolve_skill_nonexistent_returns_error() {
        let result = resolve_skill("nonexistent-skill-that-does-not-exist");
        assert!(result.is_err(), "nonexistent skill should return Err");
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_format_skill_md_escapes_yaml_special_chars() {
        let skill = Skill {
            name: SkillName::new("test-skill").unwrap(),
            description: "description with: colons, \"quotes\", and {braces}".to_string(),
            license: None,
            compatibility: None,
            context: None,
            agent: None,
            metadata: HashMap::new(),
            allowed_tools: vec!["tool-a".to_string(), "tool-b".to_string()],
            profiles: vec![],
            extra: BTreeMap::new(),
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "# Instructions\n\nDo the thing.", &skill.metadata);

        assert!(md.starts_with("---\n"), "should start with frontmatter");
        assert!(
            md.contains("\n---\n"),
            "should have closing frontmatter delimiter"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("format_skill_md output should be parseable as valid SKILL.md");

        assert_eq!(parsed.name.as_str(), "test-skill");
        assert_eq!(parsed.description, skill.description);
        assert_eq!(parsed.allowed_tools, vec!["tool-a", "tool-b"]);
        assert_eq!(parsed.instructions, "# Instructions\n\nDo the thing.");
    }

    #[test]
    fn test_format_skill_md_omits_empty_allowed_tools() {
        let skill = Skill {
            name: SkillName::new("minimal").unwrap(),
            description: "a minimal skill".to_string(),
            license: None,
            compatibility: None,
            context: None,
            agent: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            profiles: vec![],
            extra: BTreeMap::new(),
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            !md.contains("allowed-tools"),
            "empty allowed_tools should be omitted from frontmatter"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "minimal");
        assert!(parsed.allowed_tools.is_empty());
    }

    #[test]
    fn test_format_skill_md_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());
        metadata.insert("version".to_string(), "1.2.3".to_string());

        let skill = Skill {
            name: SkillName::new("meta-skill").unwrap(),
            description: "a skill with metadata".to_string(),
            license: Some("MIT".to_string()),
            compatibility: None,
            context: None,
            agent: None,
            metadata: metadata.clone(),
            allowed_tools: vec![],
            profiles: vec![],
            extra: BTreeMap::new(),
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &metadata);

        assert!(
            md.contains("metadata:"),
            "frontmatter should contain metadata block"
        );
        assert!(
            md.contains("author: swissarmyhammer"),
            "metadata should contain author"
        );
        assert!(
            md.contains("version: 1.2.3"),
            "metadata should contain rendered version"
        );
        assert!(
            md.contains("license: MIT"),
            "frontmatter should contain license"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "meta-skill");
        assert_eq!(parsed.metadata.get("author").unwrap(), "swissarmyhammer");
        assert_eq!(parsed.metadata.get("version").unwrap(), "1.2.3");
        assert_eq!(parsed.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_format_skill_md_omits_empty_metadata() {
        let skill = Skill {
            name: SkillName::new("no-meta").unwrap(),
            description: "skill without metadata".to_string(),
            license: None,
            compatibility: None,
            context: None,
            agent: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            profiles: vec![],
            extra: BTreeMap::new(),
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            !md.contains("metadata:"),
            "empty metadata should be omitted from frontmatter"
        );
        assert!(
            !md.contains("license"),
            "None license should be omitted from frontmatter"
        );
        assert!(
            !md.contains("compatibility"),
            "None compatibility should be omitted from frontmatter"
        );
    }

    /// Regression: `compatibility` round-trips through `format_skill_md` and
    /// `parse_skill_md` so the generated `.skills/` copy matches the builtin
    /// source instead of silently dropping tool-prerequisite metadata.
    #[test]
    fn test_format_skill_md_round_trips_compatibility() {
        let compatibility =
            "Requires the `code_context` MCP tool for symbol lookup and blast-radius analysis.";
        let skill = Skill {
            name: SkillName::new("compat-skill").unwrap(),
            description: "a skill that declares its tool prerequisites".to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
            compatibility: Some(compatibility.to_string()),
            context: None,
            agent: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            profiles: vec![],
            extra: BTreeMap::new(),
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            md.contains("compatibility:"),
            "frontmatter should contain compatibility field, got:\n{md}"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.compatibility.as_deref(), Some(compatibility));
    }

    /// Regression: `context` and `agent` round-trip through `parse_skill_md` and
    /// `format_skill_md` so `sah init` / deploy does not silently drop a skill's
    /// execution strategy (e.g. `context: fork`) or its delegated agent
    /// (e.g. `agent: explorer`) when it re-renders the SKILL.md.
    #[test]
    fn test_format_skill_md_round_trips_context_and_agent() {
        let src = "---\n\
            name: explore\n\
            description: Understand how unfamiliar code works\n\
            context: fork\n\
            agent: explorer\n\
            ---\n\n\
            body";

        let skill = crate::skill_loader::parse_skill_md(src, SkillSource::Builtin)
            .expect("source SKILL.md should parse");

        let md = format_skill_md(&skill, &skill.instructions, &skill.metadata);

        assert!(
            md.contains("context: fork"),
            "context field should survive the deploy round-trip, got:\n{md}"
        );
        assert!(
            md.contains("agent: explorer"),
            "agent field should survive the deploy round-trip, got:\n{md}"
        );

        let reparsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("formatted output should parse as valid SKILL.md");
        assert_eq!(reparsed.context.as_deref(), Some("fork"));
        assert_eq!(reparsed.agent.as_deref(), Some("explorer"));
    }

    /// Parse a SKILL.md's YAML frontmatter into a raw key → value mapping so a
    /// test can compare the whole key set at once instead of one field at a time.
    ///
    /// Deliberately a [`serde_yaml_ng::Mapping`] and not a `BTreeMap`: only the
    /// `Mapping` deserializer rejects a duplicate YAML key. A `BTreeMap` keeps
    /// the last value silently, which would make every caller here blind to a
    /// key emitted twice — exactly the `#[serde(flatten)]` failure these tests
    /// exist to catch.
    fn frontmatter_map(md: &str) -> serde_yaml_ng::Mapping {
        let after_open = md
            .trim()
            .strip_prefix("---")
            .expect("SKILL.md must open with frontmatter");
        let end = after_open
            .find("\n---")
            .expect("SKILL.md frontmatter must be terminated");
        serde_yaml_ng::from_str(&after_open[..end])
            .expect("frontmatter must be a YAML mapping with no duplicate keys")
    }

    /// A SKILL.md carrying official Claude Code frontmatter fields that SAH does
    /// not model — `hooks`, `model`, `paths`, `disable-model-invocation`.
    const UNMODELED_KEYS_SRC: &str = r#"---
name: finish
description: Drive kanban tasks from ready to done
license: MIT OR Apache-2.0
model: opus
paths: src/**/*.rs
disable-model-invocation: true
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
---

body
"#;

    /// Regression: frontmatter keys SAH does not model must survive the deploy
    /// round-trip. `format_skill_md` rebuilds the frontmatter from a closed
    /// struct, so any key the loader does not name is silently dropped — which
    /// is how the skill-scoped `hooks:` block in `builtin/skills/finish` was
    /// lost, leaving `/finish` with no Stop hook to keep the ralph loop alive.
    #[test]
    fn test_format_skill_md_round_trips_unmodeled_frontmatter_keys() {
        let skill = crate::skill_loader::parse_skill_md(UNMODELED_KEYS_SRC, SkillSource::Builtin)
            .expect("source SKILL.md should parse");

        let md = format_skill_md(&skill, &skill.instructions, &skill.metadata);
        let deployed = frontmatter_map(&md);

        for (key, value) in frontmatter_map(UNMODELED_KEYS_SRC) {
            assert_eq!(
                deployed.get(&key),
                Some(&value),
                "frontmatter key `{key:?}` must survive the deploy round-trip, got:\n{md}"
            );
        }

        // Textual survival is not enough: a deployed file is the loader's input
        // on the next init, so it must parse back into the same `extra`. Without
        // this, `format_skill_md` could emit keys the loader cannot read again
        // and the loss would reappear one deploy later.
        let reparsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("deployed SKILL.md must parse back through the loader");
        assert_eq!(
            reparsed.extra, skill.extra,
            "reparsing the deployed file must yield the same unmodeled keys, got:\n{md}"
        );
    }

    /// `profiles` is SAH-internal — it selects which skills an init profile
    /// deploys. It must be readable from the builtin source and absent from the
    /// deployed copy, so a leak into `extra` never reaches a file the harness
    /// reads.
    #[test]
    fn test_format_skill_md_omits_sah_internal_profiles() {
        let src = r#"---
name: finish
description: Drive kanban tasks from ready to done
profiles:
  - kanban
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
---

body
"#;

        let skill = crate::skill_loader::parse_skill_md(src, SkillSource::Builtin)
            .expect("source SKILL.md should parse");
        assert_eq!(
            skill.profiles,
            vec!["kanban"],
            "`profiles` must be readable after parse"
        );

        let md = format_skill_md(&skill, &skill.instructions, &skill.metadata);
        let deployed = frontmatter_map(&md);
        for internal in SAH_INTERNAL_FRONTMATTER_KEYS {
            assert!(
                !deployed.contains_key(*internal),
                "SAH-internal `{internal}` must not reach the deployed SKILL.md, got:\n{md}"
            );
        }
        assert!(
            deployed.contains_key("hooks"),
            "dropping the internal keys must not also drop unmodeled keys, got:\n{md}"
        );
    }

    /// The deploy-side internal-key filter, exercised directly.
    ///
    /// Every other test builds its `Skill` through `parse_skill_md`, where serde
    /// routes `profiles` to its named field — so `extra` is already clean and the
    /// filter in `format_skill_md` never fires. Only a hand-built `Skill` can put
    /// an internal key into `extra`, which is exactly the case that filter guards:
    /// without it the key would flatten straight back into the deployed file.
    #[test]
    fn test_format_skill_md_drops_internal_keys_planted_in_extra() {
        let mut extra = BTreeMap::new();
        for internal in SAH_INTERNAL_FRONTMATTER_KEYS {
            extra.insert(
                (*internal).to_string(),
                serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String(
                    "kanban".to_string(),
                )]),
            );
        }
        extra.insert(
            "model".to_string(),
            serde_yaml_ng::Value::String("opus".to_string()),
        );

        let skill = Skill {
            name: SkillName::new("planted").unwrap(),
            description: "a skill whose extra map carries an internal key".to_string(),
            license: None,
            compatibility: None,
            context: None,
            agent: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            profiles: vec![],
            extra,
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);
        let deployed = frontmatter_map(&md);

        for internal in SAH_INTERNAL_FRONTMATTER_KEYS {
            assert!(
                !deployed.contains_key(*internal),
                "`{internal}` planted in `extra` must still be dropped on deploy, got:\n{md}"
            );
        }
        assert_eq!(
            deployed.get("model"),
            Some(&serde_yaml_ng::Value::String("opus".to_string())),
            "filtering internal keys must not disturb the other unmodeled keys, got:\n{md}"
        );
    }

    /// Guard for the `#[serde(flatten)]` hazard: a key the frontmatter struct
    /// names — including the renamed `allowed-tools` — must be consumed by its
    /// typed field and never also land in the unmodeled catch-all. A key in both
    /// places would be written twice, producing frontmatter with a duplicate
    /// YAML key.
    ///
    /// Both checks below are needed, because they catch different duplicates:
    ///
    /// - re-parsing into `Skill` catches a duplicated *modeled* key, via the
    ///   `duplicate field` error `serde_derive` emits for named fields (it emits
    ///   this on the flatten path too). Note this is a serde-derive guarantee,
    ///   not a serde_yaml_ng one — the YAML layer only rejects duplicates when
    ///   the target is a `Mapping`, as in `frontmatter_map`;
    /// - the occurrence count catches the key being written twice at the text
    ///   level, independent of how any deserializer treats it.
    #[test]
    fn test_format_skill_md_writes_each_modeled_key_once() {
        let src = r#"---
name: every-field
description: A skill that sets every modeled frontmatter key
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool.
context: fork
agent: explorer
allowed-tools: tool-a tool-b
profiles:
  - kanban
metadata:
  author: swissarmyhammer
  version: "1.0"
---

body
"#;

        let skill = crate::skill_loader::parse_skill_md(src, SkillSource::Builtin)
            .expect("source SKILL.md should parse");

        let md = format_skill_md(&skill, &skill.instructions, &skill.metadata);

        crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("formatted output must not contain a duplicated modeled key");
        // Reject a duplicate of any key, modeled or not, at the YAML layer.
        frontmatter_map(&md);

        for key in ["name", "description", "license", "allowed-tools", "agent"] {
            assert_eq!(
                md.matches(&format!("\n{key}:")).count(),
                1,
                "`{key}` must be written exactly once, got:\n{md}"
            );
        }
    }
}
