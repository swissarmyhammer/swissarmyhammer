use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope};
use swissarmyhammer_common::reporter::InitReporter;

use super::applier::APPLIER_COMPONENT;
use super::*;
use serial_test::serial;
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::CurrentDirGuard;

use crate::test_support::MirdanConfigGuard;

/// Write a synthetic single-agent config that detects `project_dir` and
/// declares a relative skill dir (`.fake/skills`), agent dir (`.fake/agents`),
/// `.mcp.json` MCP config, settings file (`.fake/settings.json`), and
/// instructions file (`.fake/CLAUDE.md`) — the artifact kinds a profile
/// installs (skills/agents/mcp + statusline).
fn write_profile_agents_config(project_dir: &Path) -> PathBuf {
    let agents_yaml = format!(
        r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    agent_path: .fake/agents
    settings_path: .fake/settings.json
    instructions_path: .fake/CLAUDE.md
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

/// A sample profile: register an MCP server, deploy one builtin skill that
/// uses `{% include %}` partials (`commit`) and one builtin agent
/// (`reviewer`).
fn sample_profile() -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("sample")),
        skills: Some(Selector::Single("commit".to_string())),
        agents: Some(Selector::Single("reviewer".to_string())),
        validators: None,
        statusline: false,
        edit_redirect: false,
    }
}

/// `init_profile` with `root: None` installs skills (symlinked + rendered),
/// the MCP server, and agents into CWD-relative locations; `deinit_profile`
/// removes them.
#[test]
#[serial]
fn init_profile_installs_and_deinit_removes_cwd_rooted() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&project).unwrap();
    let config_path = write_profile_agents_config(&project);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = sample_profile();
    let reporter = NullReporter;
    let results = init_profile(&profile, InitScope::Project, None, &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "init_profile must not error: {results:?}"
    );

    // Skill: stored centrally and symlinked into the agent's skill dir.
    let skill_store = project.join(".skills/commit/SKILL.md");
    assert!(
        skill_store.exists(),
        "skill should be stored: {skill_store:?}"
    );
    let skill_link = project.join(".fake/skills/commit");
    assert!(
        std::fs::symlink_metadata(&skill_link).is_ok(),
        "skill should be symlinked into agent dir: {skill_link:?}"
    );

    // The deployed SKILL.md was rendered with Liquid + the partial library,
    // so no `{% include %}` references survive.
    let rendered = std::fs::read_to_string(&skill_store).unwrap();
    assert!(
        !rendered.contains("{% include"),
        "partials must be expanded in the deployed SKILL.md"
    );

    // Agent: stored and symlinked into the agent's agent dir.
    assert!(
        project.join(".agents/reviewer/AGENT.md").exists(),
        "agent should be stored"
    );
    assert!(
        std::fs::symlink_metadata(project.join(".fake/agents/reviewer")).is_ok(),
        "agent should be symlinked into agent dir"
    );

    // MCP server registered in the agent's .mcp.json.
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["sample"]["command"], "sample");

    // Deinit removes everything.
    let results = deinit_profile(&profile, InitScope::Project, None, &reporter);
    assert!(results
        .iter()
        .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    assert!(
        std::fs::symlink_metadata(&skill_link).is_err(),
        "skill symlink should be removed on deinit"
    );
    assert!(
        std::fs::symlink_metadata(project.join(".fake/agents/reviewer")).is_err(),
        "agent symlink should be removed on deinit"
    );
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert!(
        mcp["mcpServers"]["sample"].is_null(),
        "MCP server should be unregistered on deinit"
    );
}

/// `init_profile` with an explicit `root` targets that root for every
/// project-scope artifact and never reads the process working directory.
#[test]
#[serial]
fn init_profile_explicit_root_targets_given_root() {
    // The install root is a temp dir distinct from the (arbitrary) CWD.
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();

    // CWD points somewhere else entirely; nothing must land here.
    let cwd_dir = tempfile::tempdir().unwrap();
    let cwd = cwd_dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();

    // Detection keys off the absolute root dir, independent of CWD.
    let config_path = write_profile_agents_config(&root);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = sample_profile();
    let reporter = NullReporter;
    let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "explicit-root init must not error: {results:?}"
    );

    // Artifacts land under the explicit root.
    assert!(root.join(".skills/commit/SKILL.md").exists());
    assert!(std::fs::symlink_metadata(root.join(".fake/skills/commit")).is_ok());
    assert!(root.join(".agents/reviewer/AGENT.md").exists());
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["sample"]["command"], "sample");

    // Nothing was written to the CWD.
    assert!(
        !cwd.join(".skills").exists(),
        "explicit-root install must not touch CWD"
    );
    assert!(!cwd.join(".mcp.json").exists());

    // Explicit-root deinit cleans the root.
    deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(std::fs::symlink_metadata(root.join(".fake/skills/commit")).is_err());
}

/// A profile that declares `statusline` writes the `statusLine` block into
/// the detected agent's settings file, and `deinit_profile` removes it.
/// Exercised with an explicit `root` to prove step 4 is CWD-free.
#[test]
#[serial]
fn init_profile_statusline_install_and_deinit() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();

    // CWD elsewhere: nothing must land in it.
    let cwd_dir = tempfile::tempdir().unwrap();
    let cwd = cwd_dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();

    let config_path = write_profile_agents_config(&root);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = Profile {
        statusline: true,
        ..Profile::default()
    };
    let reporter = NullReporter;
    let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "statusline init must not error: {results:?}"
    );

    // Statusline block written to the agent's settings file under root.
    let settings_path = root.join(".fake/settings.json");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(settings["statusLine"]["type"], "command");
    assert_eq!(settings["statusLine"]["command"], "sah statusline");

    // Nothing leaked into the CWD.
    assert!(!cwd.join(".fake").exists(), "step 4 must not touch CWD");

    // Deinit strips the statusline block.
    let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(results
        .iter()
        .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(
        settings.get("statusLine").is_none(),
        "statusLine must be removed on deinit"
    );
}

/// `init_profile` with `edit_redirect: true` merges the superseded-native deny
/// into the detected agent's settings file (no `PreToolUse` redirect hook),
/// preserving unrelated keys; `deinit_profile` strips the deny back out.
#[test]
#[serial]
fn init_profile_installs_edit_redirect_and_deinit_removes() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();
    let cwd = cwd_dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();

    let config_path = write_profile_agents_config(&root);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    // Pre-existing unrelated settings the install must not clobber.
    let settings_path = root.join(".fake/settings.json");
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&serde_json::json!({ "model": "opus" })).unwrap(),
    )
    .unwrap();

    let profile = Profile {
        edit_redirect: true,
        ..Profile::default()
    };
    let reporter = NullReporter;
    let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "edit-redirect init must not error: {results:?}"
    );

    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    // Unrelated key preserved.
    assert_eq!(settings["model"], serde_json::json!("opus"));
    // Every superseded native denied.
    let deny = settings["permissions"]["deny"].as_array().unwrap();
    for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
        assert!(
            deny.iter().any(|v| v == &serde_json::json!(tool)),
            "{tool} must be denied; got {deny:?}"
        );
    }
    // No PreToolUse redirect hook installed — the deny is the whole mechanism.
    assert!(
        settings.get("hooks").is_none(),
        "edit-redirect must not install any hooks; got {settings:?}"
    );

    // Nothing leaked into the CWD.
    assert!(
        !cwd.join(".fake").exists(),
        "edit-redirect must not touch CWD"
    );

    // Deinit strips the deny + redirect but keeps the unrelated key.
    let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(results
        .iter()
        .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(settings["model"], serde_json::json!("opus"));
    let deny = settings["permissions"]["deny"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
        assert!(
            !deny.iter().any(|v| v == &serde_json::json!(tool)),
            "{tool} deny must be removed on deinit"
        );
    }
    // No hooks were ever installed, so none should appear.
    assert!(
        settings.get("hooks").is_none(),
        "no hooks must be present after deinit; got {settings:?}"
    );
}

/// `Selector::Named` resolves in source order, skipping unknown names.
#[test]
fn selector_named_resolves_known_and_skips_unknown() {
    let available: std::collections::HashSet<String> =
        ["a", "b", "c"].into_iter().map(str::to_string).collect();
    let got = Selector::Named(vec![
        "b".to_string(),
        "missing".to_string(),
        "a".to_string(),
    ])
    .select(&available);
    assert_eq!(got, vec!["b".to_string(), "a".to_string()]);
}

/// `Selector::Single` for an unknown name selects nothing.
#[test]
fn selector_single_unknown_is_empty() {
    let available: std::collections::HashSet<String> = std::collections::HashSet::new();
    assert!(Selector::Single("nope".to_string())
        .select(&available)
        .is_empty());
}

/// `ProfileMcpServer::serve` builds the canonical self-launching triple:
/// the binary registers under its own name and runs `<name> serve`.
#[test]
fn profile_mcp_server_serve_builds_self_launching_triple() {
    let server = ProfileMcpServer::serve("shelltool");
    assert_eq!(server.name, "shelltool");
    assert_eq!(server.command, "shelltool");
    assert_eq!(server.args, vec!["serve".to_string()]);
}

/// A test `Initializable` that records the lifecycle method invoked, so the
/// `*_with_registry` helpers can be checked for both result aggregation and
/// the profile-vs-registry ordering.
struct RecordingComponent;

impl swissarmyhammer_common::lifecycle::Initializable for RecordingComponent {
    fn name(&self) -> &str {
        "recording-component"
    }
    fn category(&self) -> &str {
        "test"
    }
    fn init(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        vec![InitResult::ok("recording-component", "init ran")]
    }
    fn deinit(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        vec![InitResult::ok("recording-component", "deinit ran")]
    }
}

/// `init_profile_with_registry` returns the profile install results followed
/// by the registry components' init results (profile-first ordering), and
/// `deinit_profile_with_registry` runs the registry teardown first.
#[test]
#[serial]
fn with_registry_helpers_aggregate_profile_then_registry() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&project).unwrap();
    let config_path = write_profile_agents_config(&project);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = sample_profile();
    let reporter = NullReporter;

    let mut registry = InitRegistry::new();
    registry.register(RecordingComponent);
    let init_results =
        init_profile_with_registry(&profile, &registry, InitScope::Project, None, &reporter);
    // The registry's init result is present and trails the profile results.
    let recorded = init_results
        .iter()
        .position(|r| r.name == "recording-component" && r.message == "init ran")
        .expect("registry init result present");
    let last_profile = init_results
        .iter()
        .rposition(|r| r.name != "recording-component")
        .expect("profile results present");
    assert!(
        recorded > last_profile,
        "registry init must run after profile install: {init_results:?}"
    );

    let deinit_results =
        deinit_profile_with_registry(&profile, &registry, InitScope::Project, None, &reporter);
    // The registry's deinit result leads the profile teardown.
    let recorded = deinit_results
        .iter()
        .position(|r| r.name == "recording-component" && r.message == "deinit ran")
        .expect("registry deinit result present");
    let first_profile = deinit_results
        .iter()
        .position(|r| r.name != "recording-component")
        .expect("profile teardown results present");
    assert!(
        recorded < first_profile,
        "registry deinit must run before profile teardown: {deinit_results:?}"
    );
}

/// `deinit_profile` reports each builtin family under its own component name
/// and its own item-kind label: the skill teardown under `profile-skills`
/// naming `skill`, the agent teardown under `profile-agents` naming `agent`.
///
/// The component and the kind are two separate values handed to the same call.
/// Exchanging them, or handing one family the other's pair, changes exactly
/// these two rows.
#[test]
#[serial]
fn deinit_profile_reports_each_family_under_its_own_component_and_kind() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&project).unwrap();
    let config_path = write_profile_agents_config(&project);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = sample_profile();
    let reporter = NullReporter;
    init_profile(&profile, InitScope::Project, None, &reporter);

    let results = deinit_profile(&profile, InitScope::Project, None, &reporter);
    let message_of = |component: &str| {
        results
            .iter()
            .find(|result| result.name == component)
            .unwrap_or_else(|| panic!("{component} result must be present: {results:?}"))
            .message
            .clone()
    };
    assert_eq!(message_of("profile-skills"), "Removed 1 skill(s)");
    assert_eq!(message_of("profile-agents"), "Removed 1 agent(s)");
}

/// The root-explicit MCP applier reports its own verb: `Registered` when
/// `init_profile` registers the server, `Removed` when `deinit_profile`
/// unregisters it.
///
/// The verb and the preposition are two separate values handed to the same
/// call. Exchanging them puts the preposition in this summary row.
#[test]
#[serial]
fn profile_mcp_root_explicit_reports_its_own_verb() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::new(cwd_dir.path()).unwrap();
    let config_path = write_profile_agents_config(&root);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let profile = sample_profile();
    let reporter = NullReporter;

    let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results.iter().any(|result| result.name == APPLIER_COMPONENT
            && result.message == "Registered applied to 1 agent(s)"),
        "MCP registration must report the Registered verb: {results:?}"
    );

    let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results.iter().any(|result| result.name == APPLIER_COMPONENT
            && result.message == "Removed applied to 1 agent(s)"),
        "MCP unregistration must report the Removed verb: {results:?}"
    );
}

/// `deinit_profile` reports the validator teardown under its own component
/// name, `profile-validators` — the same name `init_profile` reports the
/// validator install under.
///
/// The component name and the message are two strings handed to the same
/// call. Exchanging them, or naming a different component, changes this row.
#[test]
fn deinit_profile_reports_validators_under_the_validators_component() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();
    let profile = Profile {
        validators: Some(Selector::Single("code-hygiene".to_string())),
        ..Profile::default()
    };
    let reporter = NullReporter;

    init_profile(&profile, InitScope::Project, Some(&root), &reporter);

    let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
    assert!(
        results
            .iter()
            .any(|result| result.name == "profile-validators"
                && result.message == "Removed 1 validator set(s)"),
        "validator teardown must report under profile-validators: {results:?}"
    );
}

/// A profile with no skills/agents/mcp_server is a clean no-op.
#[test]
#[serial]
fn empty_profile_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&project).unwrap();
    let config_path = write_profile_agents_config(&project);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let reporter = NullReporter;
    let results = init_profile(&Profile::default(), InitScope::Project, None, &reporter);
    assert!(results.is_empty(), "empty profile should do nothing");
    assert!(!project.join(".skills").exists());
}
