//! ── Scope-aware appliers (the single per-agent iteration + reporting site) ──
//!
//! Tools and CLIs call these to apply a declarative change — register me as an
//! MCP server, deny a tool — across every detected agent. Each applier loads
//! the detected agents, dispatches to the right [`strategy::AgentConfigStrategy`]
//! via [`strategy::strategy_for`], applies the change, and emits reporter
//! events. They take a `swissarmyhammer_common` [`InitScope`] + [`InitReporter`]
//! so the same implementation serves the shell tool, `sah`, and `shelltool`.

use swissarmyhammer_common::lifecycle::{InitResult, InitScope};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

use crate::agents::{self, AgentDef};
use crate::mcp_config::McpServerEntry;
use crate::registry::RegistryError;
use crate::strategy::{self, AgentConfigStrategy};

/// Map an [`InitScope`] to the boolean `global` flag the deploy/store helpers
/// expect: only `User` scope is global; `Project`/`Local` are project-scoped.
pub(crate) fn scope_is_global(scope: InitScope) -> bool {
    matches!(scope, InitScope::User)
}

/// Component name used in the `InitResult`s the appliers return.
pub(crate) const APPLIER_COMPONENT: &str = "agent-config";

/// Load detected agents, or return a single error `InitResult` describing the
/// failure (so callers can short-circuit without a panic).
fn detected_agents_or_error() -> Result<Vec<crate::agents::DetectedAgent>, Vec<InitResult>> {
    match agents::load_agents_config() {
        Ok(config) => Ok(agents::get_detected_agents(&config)),
        Err(e) => Err(vec![InitResult::error(
            APPLIER_COMPONENT,
            format!("failed to load agents config: {e}"),
        )]),
    }
}

/// Apply `action` to every detected agent's strategy, emitting an Action event
/// (with `verb`) for each agent that changed and a Warning for each error, then
/// aggregate into a single `InitResult`.
///
/// A thin strategy-dispatching adapter over [`for_each_detected_agent`], which
/// owns the iteration, event emission, and aggregation.
fn for_each_agent_strategy(
    scope: InitScope,
    reporter: &dyn InitReporter,
    verb: &str,
    action: impl Fn(&dyn AgentConfigStrategy, &AgentDef) -> Result<bool, RegistryError>,
    action_message: impl Fn(&AgentDef) -> String,
) -> Vec<InitResult> {
    for_each_detected_agent(
        scope,
        reporter,
        |agent, _global| {
            let strategy = strategy::strategy_for(agent);
            Ok(action(strategy.as_ref(), agent)?.then(|| AgentAction {
                verb: verb.to_string(),
                message: action_message(agent),
            }))
        },
        |changed| {
            InitResult::ok(
                APPLIER_COMPONENT,
                format!("{verb} applied to {changed} agent(s)"),
            )
        },
    )
}

/// A per-agent change produced by a [`for_each_detected_agent`] closure: the
/// reporter `verb` and human-readable `message` describing what changed.
pub(crate) struct AgentAction {
    /// Reporter Action verb (e.g. `"Installed"`, `"Removed"`, `"Registered"`).
    pub(crate) verb: String,
    /// Reporter Action message describing the agent and what was applied.
    pub(crate) message: String,
}

/// Drive an applier over every detected agent for the root-explicit init path.
///
/// Owns the structural skeleton shared by the statusline and MCP
/// register/unregister appliers: load detected agents (short-circuiting to an
/// error `InitResult` on failure), compute the `global` scope flag, run `apply`
/// per agent, emit an Action event for each `Ok(Some(_))` change, emit a Warning
/// (labelled with `scope`) for each `Err`, count the changes, and aggregate into
/// a single `InitResult` built by `summary` from the change count.
///
/// `apply` receives each [`AgentDef`] plus the resolved `global` flag and returns
/// `Ok(Some(action))` when the agent changed, `Ok(None)` when it was already in
/// the desired state or skipped, or `Err` on failure.
pub(crate) fn for_each_detected_agent(
    scope: InitScope,
    reporter: &dyn InitReporter,
    apply: impl Fn(&AgentDef, bool) -> Result<Option<AgentAction>, RegistryError>,
    summary: impl Fn(usize) -> InitResult,
) -> Vec<InitResult> {
    let agents = match detected_agents_or_error() {
        Ok(a) => a,
        Err(results) => return results,
    };
    let global = scope_is_global(scope);

    let mut changed = 0usize;
    for agent in &agents {
        match apply(&agent.def, global) {
            Ok(Some(action)) => {
                reporter.emit(&InitEvent::Action {
                    verb: action.verb,
                    message: action.message,
                });
                changed += 1;
            }
            Ok(None) => {}
            Err(e) => reporter.emit(&InitEvent::Warning {
                message: format!("{} ({}): {e}", agent.def.name, scope_label(scope)),
            }),
        }
    }

    vec![summary(changed)]
}

/// Short scope label for reporter/warning messages.
fn scope_label(scope: InitScope) -> &'static str {
    match scope {
        InitScope::Project => "project",
        InitScope::Local => "local",
        InitScope::User => "user",
    }
}

/// Register `server_name` → `entry` as an MCP server across every detected
/// agent at `scope`, dispatching to each agent's strategy.
pub fn register_mcp_server(
    scope: InitScope,
    server_name: &str,
    entry: &McpServerEntry,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Registered",
        |strategy, agent| strategy.register_mcp(agent, scope, server_name, entry),
        |agent| format!("{server_name} MCP server for {}", agent.name),
    )
}

/// Unregister `server_name` as an MCP server across every detected agent at
/// `scope`, dispatching to each agent's strategy.
pub fn unregister_mcp_server(
    scope: InitScope,
    server_name: &str,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Removed",
        |strategy, agent| strategy.unregister_mcp(agent, scope, server_name),
        |agent| format!("{server_name} MCP server from {}", agent.name),
    )
}

/// Deny `tool` across every detected agent at `scope`, dispatching to each
/// agent's strategy. Agents with no permission mechanism are silently skipped.
pub fn deny_tool(scope: InitScope, tool: &str, reporter: &dyn InitReporter) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Configured",
        |strategy, agent| strategy.deny_tool(agent, scope, tool),
        |agent| {
            format!(
                "{tool} tool denied for {} — use the shell tool instead",
                agent.name
            )
        },
    )
}

/// Allow `tool` (remove a prior deny) across every detected agent at `scope`,
/// dispatching to each agent's strategy.
pub fn allow_tool(scope: InitScope, tool: &str, reporter: &dyn InitReporter) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Removed",
        |strategy, agent| strategy.allow_tool(agent, scope, tool),
        |agent| format!("{tool} deny rule for {}", agent.name),
    )
}

#[cfg(test)]
mod applier_tests {
    use std::path::{Path, PathBuf};

    use serial_test::serial;
    use swissarmyhammer_common::reporter::NullReporter;

    use super::super::{init_profile, Profile, Selector};
    use super::*;
    use crate::test_support::MirdanConfigGuard;

    /// Write a synthetic single-agent (generic) config whose detect dir is the
    /// project dir (so detection always fires) and whose MCP config is a
    /// relative `.mcp.json`.
    fn write_generic_agents_config(project_dir: &Path) -> PathBuf {
        let agents_yaml = format!(
            r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      servers_key: mcpServers
"#,
            detect = project_dir.display(),
        );
        let config_path = project_dir.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).unwrap();
        config_path
    }

    fn entry() -> McpServerEntry {
        McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    #[serial]
    fn register_mcp_server_iterates_detected_agent_and_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        let results = register_mcp_server(InitScope::Project, "sah", &entry(), &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(json["mcpServers"]["sah"]["command"], "sah");

        let results = unregister_mcp_server(InitScope::Project, "sah", &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert!(json["mcpServers"]["sah"].is_null());
    }

    #[test]
    #[serial]
    fn deny_tool_noop_for_agent_without_permission_mechanism() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        // The generic strategy has no deny mechanism: nothing is written and
        // the applier still returns Ok.
        let results = deny_tool(InitScope::Project, "Bash", &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));

        // Round-trip: the inverse `allow_tool` is also a clean no-op here.
        let results = allow_tool(InitScope::Project, "Bash", &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    }

    /// Real deploy path: `init_profile` writes the discovery README at the skill
    /// and agent store roots, beside the real items, when those stores are
    /// populated (a fake agent makes skill/agent deployment fire).
    #[test]
    #[serial]
    fn init_profile_writes_skill_and_agent_store_readmes() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = Profile {
            skills: Some(Selector::All),
            agents: Some(Selector::All),
            ..Profile::default()
        };
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, Some(&project), &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "init_profile must not error: {results:?}"
        );

        let skills_readme = project.join(".skills/README.md");
        assert!(
            skills_readme.is_file(),
            "skills README must be written: {skills_readme:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&skills_readme).unwrap(),
            include_str!("../../../../builtin/skills/README.md"),
        );

        let agents_readme = project.join(".agents/README.md");
        assert!(
            agents_readme.is_file(),
            "agents README must be written: {agents_readme:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&agents_readme).unwrap(),
            include_str!("../../../../builtin/agents/README.md"),
        );

        // The README is never mistaken for an item: a real builtin skill
        // materialized as its own subdirectory beside the README.
        assert!(
            project.join(".skills/commit/SKILL.md").is_file(),
            "a real builtin skill must deploy beside the README"
        );
    }

    /// Builtin skills with progressive-disclosure resources (`references/*.md`)
    /// must deploy those files beside the rendered SKILL.md — the skill body
    /// links to them relatively, so a store entry without them is broken.
    #[test]
    #[serial]
    fn init_profile_deploys_skill_reference_resources() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = Profile {
            skills: Some(Selector::All),
            ..Profile::default()
        };
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, Some(&project), &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "init_profile must not error: {results:?}"
        );

        let reference = project.join(".skills/coverage/references/SWIFT_COVERAGE.md");
        assert!(
            reference.is_file(),
            "skill references must deploy beside SKILL.md: {reference:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&reference).unwrap(),
            include_str!("../../../../builtin/skills/coverage/references/SWIFT_COVERAGE.md"),
        );
    }

    /// Parse a SKILL.md's YAML frontmatter into a raw key → value map, so a test
    /// can compare a deployed file against its builtin source key by key.
    /// A [`serde_yaml_ng::Mapping`] rather than a `BTreeMap` so that a key
    /// emitted twice is rejected here instead of silently collapsing to its
    /// last value — the `#[serde(flatten)]` failure this test guards against.
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

    /// Real deploy path: `init_profile` re-renders every builtin skill through
    /// `format_skill_md`, so a frontmatter key that formatter does not know is
    /// dropped on the way to `.skills/<name>/SKILL.md`.
    ///
    /// That is how the skill-scoped `hooks:` block in `builtin/skills/finish`
    /// was lost: Claude Code reads the deployed copy, so `/finish` ran with no
    /// Stop hook and the ralph loop died between iterations.
    ///
    /// Every source key must reach the deployed file with an equal value, except
    /// `metadata`, whose values are Liquid-rendered here.
    #[test]
    #[serial]
    fn init_profile_preserves_unmodeled_skill_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = Profile {
            skills: Some(Selector::All),
            ..Profile::default()
        };
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, Some(&project), &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "init_profile must not error: {results:?}"
        );

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../builtin/skills");
        let mut compared_keys = 0usize;
        for entry in std::fs::read_dir(&source_root).unwrap().flatten() {
            let source_md = entry.path().join("SKILL.md");
            if !source_md.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let source_fm = frontmatter_map(&std::fs::read_to_string(&source_md).unwrap());

            let deployed_md = project.join(".skills").join(&name).join("SKILL.md");
            let deployed_fm = frontmatter_map(&std::fs::read_to_string(&deployed_md).unwrap());

            for (key, value) in &source_fm {
                let key = key.as_str().expect("frontmatter keys must be strings");
                // `metadata` values are Liquid-rendered on the way out, so the
                // deployed values differ from the source by design.
                if key == "metadata" {
                    continue;
                }
                assert_eq!(
                    deployed_fm.get(key),
                    Some(value),
                    "frontmatter key `{key}` of builtin skill `{name}` must survive deploy \
                     into {deployed_md:?}"
                );
                compared_keys += 1;
            }
        }

        assert!(
            compared_keys > 0,
            "no builtin skill frontmatter was compared; the source root {source_root:?} \
             is probably wrong"
        );
    }
}
