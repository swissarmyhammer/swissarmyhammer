//! Production-path consistency tests for the four real CLI profiles.
//!
//! Card "Real-path tests: every profile init/deinit is consistent and
//! round-trips". Where [`profile_tests`] exercises the installer with a
//! synthetic `sample_profile`, this module reconstructs the *actual* profiles
//! declared by the four consumers — sah ([`apps/swissarmyhammer-cli`]),
//! shelltool, kanban-cli, and code-context — from the same public mirdan
//! primitives (`ProfileMcpServer::serve`, `Selector::*`) those CLIs use, then
//! drives them all through the single [`init_profile`] / [`deinit_profile`]
//! path. The point is the "one mechanism, no drift" guarantee: every profile
//! installs the same way (store + symlink, never copied files), registers its
//! MCP server in the right place, and round-trips clean — and a regression that
//! reintroduced a per-app installer or a copy-vs-symlink fork would fail here.
//!
//! Scope note: these reconstructions cover the install *mechanism* across the
//! four profile *shapes*; they deliberately do **not** enumerate each profile's
//! real skill set. That authority — "which skills does this CLI actually
//! deploy" — lives in each app's own `commands::registry` test, which drives the
//! real `profile(scope)` through [`init_profile`] via the shared
//! [`crate::test_support`] asserters and so can never silently mirror a bug in
//! the real profile. mirdan cannot import the app crates, so the mechanism
//! coverage lives here while the skill-set authority lives there.
//!
//! Tests are HOME/tempdir-isolated (mirroring the `MIRDAN_AGENTS_CONFIG`
//! isolation in [`profile_tests`]) and `#[serial]` because they mutate the
//! process CWD and shared env; nothing leaks into the repo. They reuse the
//! public [`crate::test_support`] scaffolding (`write_single_agent_config`,
//! `assert_no_init_error`, `read_json`) so the in-crate and app-crate tests
//! share one config writer and one set of asserters.

use super::*;
use crate::test_support::{
    assert_no_init_error, read_json, write_single_agent_config, MirdanConfigGuard,
};
use serial_test::serial;
use swissarmyhammer_common::lifecycle::{InitResult, InitScope};
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

/// One CLI consumer's real install profile, reconstructed from the same
/// public mirdan primitives the consumer's `registry.rs`/`profile.rs` uses.
///
/// These reconstructions exist only to exercise the shared install
/// *mechanism* (store + symlink, MCP registration, round-trip, scope matrix)
/// across the four profile *shapes* — the coverage that legitimately must
/// live in mirdan, which cannot import the app crates. They deliberately do
/// **not** enumerate each profile's real skill set: the authoritative,
/// drift-proof check of "which skills does this CLI actually deploy" lives in
/// each app's own `commands::registry` test, which drives the real
/// `profile(scope)` through [`init_profile`] (see
/// `apps/*/src/commands/registry.rs`). Enumerating the set here too would
/// only re-introduce the mirror-the-bug drift this card set out to remove.
struct CliProfile {
    /// Consumer label, for assertion messages.
    label: &'static str,
    /// The registered MCP server name (`<name> serve`).
    server: &'static str,
    /// A single representative skill the profile is known to select, used to
    /// probe the store+symlink deploy mechanism. The mechanism is identical
    /// regardless of which builtin we probe, so one name suffices — this is
    /// not an assertion about the profile's full skill set (owned by the
    /// per-CLI registry tests).
    probe_skill: &'static str,
    /// Build the profile for `scope`, mirroring the consumer's `profile(scope)`.
    build: fn(InitScope) -> Profile,
}

/// sah's profile — the "bigger profile": all builtin skills + all builtin
/// agents + statusline (`apps/swissarmyhammer-cli/.../profile.rs`).
fn sah_profile(_scope: InitScope) -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("sah")),
        skills: Some(Selector::All),
        agents: Some(Selector::All),
        validators: Some(Selector::All),
        statusline: true,
        edit_redirect: true,
    }
}

/// shelltool's profile — `shelltool serve` + the single `shell` skill,
/// deployed at every scope (`apps/shelltool-cli/.../registry.rs`).
fn shelltool_profile(_scope: InitScope) -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("shelltool")),
        skills: Some(Selector::Single("shell".to_string())),
        ..Default::default()
    }
}

/// kanban-cli's profile — `kanban serve` + every builtin skill, deployed at
/// every scope (`apps/kanban-cli/.../registry.rs`).
fn kanban_profile(_scope: InitScope) -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("kanban")),
        skills: Some(Selector::All),
        ..Default::default()
    }
}

/// code-context's profile — `code-context serve` + the named
/// `code-context` + `explore` + `lsp` + `detected-projects` skills, deployed
/// at every scope (`apps/code-context-cli/.../registry.rs`).
fn code_context_profile(_scope: InitScope) -> Profile {
    Profile {
        mcp_server: Some(ProfileMcpServer::serve("code-context")),
        skills: Some(Selector::All),
        ..Default::default()
    }
}

/// The number of real CLI profiles reconstructed below.
const CLI_PROFILE_COUNT: usize = 4;

/// The four real CLI profiles, in the order their cards migrated them. Each
/// carries a single `probe_skill` to exercise the deploy mechanism — not the
/// full skill set, which the per-CLI registry tests own (see [`CliProfile`]).
fn cli_profiles() -> [CliProfile; CLI_PROFILE_COUNT] {
    [
        CliProfile {
            label: "sah",
            server: "sah",
            probe_skill: "commit",
            build: sah_profile,
        },
        CliProfile {
            label: "shelltool",
            server: "shelltool",
            probe_skill: "shell",
            build: shelltool_profile,
        },
        CliProfile {
            label: "kanban",
            server: "kanban",
            probe_skill: "kanban",
            build: kanban_profile,
        },
        CliProfile {
            label: "code-context",
            server: "code-context",
            probe_skill: "code-context",
            build: code_context_profile,
        },
    ]
}

/// Assert no result is an error, with a profile-labelled message. Wraps the
/// shared [`assert_no_init_error`] with a combined `<label> <phase>` label.
fn assert_no_error(label: &str, phase: &str, results: &[InitResult]) {
    assert_no_init_error(&format!("{label} {phase}"), results);
}

/// Case 1 + 2 + 5: every real CLI profile installs through the *identical*
/// mechanism — each selected skill is a central store entry **symlinked**
/// (never copied) into the agent's skill dir, the MCP server lands in the
/// agent's `.mcp.json`, and `deinit_profile` round-trips every artifact away.
/// Driven with an explicit root so the four runs are independent tempdirs.
///
/// Round-trip here is asserted at `Project` scope only; `Local`-scope
/// round-trip is exercised by the code-context regression test
/// ([`code_context_local_scope_registers_in_claude_json_projects_map`]), and
/// per-scope landing is covered by [`scope_matrix_lands_artifacts_in_the_right_place`].
#[test]
#[serial]
fn every_cli_profile_installs_by_store_symlink_and_round_trips() {
    // CWD points somewhere neutral; explicit roots isolate each profile.
    let cwd_dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::new(cwd_dir.path().canonicalize().unwrap()).unwrap();
    let reporter = NullReporter;

    for cli in cli_profiles() {
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        // Root and home coincide so the generic agent's project and global
        // dirs both live under this tempdir — `Project` scope only touches
        // the project dirs anyway.
        let config_path = write_single_agent_config(&root, &root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = (cli.build)(InitScope::Project);
        let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert_no_error(cli.label, "init", &results);

        // Probe skill: central store entry + a *symlink* (not a copy) in the
        // agent dir — the single deploy mechanism, identical for all four.
        // (The full per-profile skill set is asserted by the per-CLI registry
        // tests; here we only prove the mechanism.)
        let skill = cli.probe_skill;
        let store = root.join(".skills").join(skill).join("SKILL.md");
        assert!(
            store.is_file(),
            "{}: skill `{skill}` must be in the .skills store: {store:?}",
            cli.label
        );
        let link = root.join(".fake/skills").join(skill);
        let meta = std::fs::symlink_metadata(&link).unwrap_or_else(|e| {
            panic!(
                "{}: skill `{skill}` link must exist ({link:?}): {e}",
                cli.label
            )
        });
        assert!(
                meta.file_type().is_symlink(),
                "{}: skill `{skill}` must be a SYMLINK, not a copied dir — store+symlink is the one mechanism",
                cli.label
            );

        // MCP server registered in the agent's project `.mcp.json`, launched
        // via `<name> serve`.
        let mcp = read_json(&root.join(".mcp.json"));
        assert_eq!(
            mcp["mcpServers"][cli.server]["command"], cli.server,
            "{}: MCP server `{}` must be registered",
            cli.label, cli.server
        );
        assert_eq!(mcp["mcpServers"][cli.server]["args"][0], "serve");

        // Round-trip: deinit removes the symlink and unregisters the MCP
        // server, leaving the agent config clean.
        let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert_no_error(cli.label, "deinit", &results);
        let link = root.join(".fake/skills").join(skill);
        assert!(
            std::fs::symlink_metadata(&link).is_err(),
            "{}: skill `{skill}` symlink must be removed on deinit",
            cli.label
        );
        let mcp = read_json(&root.join(".mcp.json"));
        assert!(
            mcp["mcpServers"][cli.server].is_null(),
            "{}: MCP server must be unregistered on deinit",
            cli.label
        );
    }
}

/// Case 3: an explicit-root install targets exactly that root and never
/// reads or writes the process working directory — for every CLI profile.
/// This is the property the kanban-app's long-running process relies on.
#[test]
#[serial]
fn explicit_root_install_never_touches_cwd_for_any_profile() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let cwd = cwd_dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();
    let reporter = NullReporter;

    for cli in cli_profiles() {
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let config_path = write_single_agent_config(&root, &root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = (cli.build)(InitScope::Project);
        let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert_no_error(cli.label, "explicit-root init", &results);

        // Artifacts land under the explicit root.
        assert!(
            root.join(".skills").is_dir(),
            "{}: .skills store must be under the explicit root",
            cli.label
        );
        // Nothing was written into the CWD.
        assert!(
            !cwd.join(".skills").exists() && !cwd.join(".mcp.json").exists(),
            "{}: explicit-root install must not touch CWD",
            cli.label
        );
    }
}

/// Case 4: the scope matrix. For a representative profile (shelltool), each
/// scope lands in the correct location: `Project`/`Local` deploy skills under
/// the project store; `User` deploys skills into the *global* store and
/// registers the MCP server in the agent's global config. The MCP target file
/// differs by scope (project `.mcp.json` vs the agent's global config).
#[test]
#[serial]
fn scope_matrix_lands_artifacts_in_the_right_place() {
    let reporter = NullReporter;

    // Project scope: skills + project `.mcp.json`.
    {
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let config_path = write_single_agent_config(&root, &root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results = init_profile(
            &shelltool_profile(InitScope::Project),
            InitScope::Project,
            None,
            &reporter,
        );
        assert_no_error("shelltool", "project init", &results);
        assert!(
            root.join(".skills/shell/SKILL.md").is_file(),
            "project scope must deploy the shell skill"
        );
        assert!(
            read_json(&root.join(".mcp.json"))["mcpServers"]["shelltool"]["command"] == "shelltool",
            "project scope must register MCP in project .mcp.json"
        );
    }

    // Local scope: skills still deploy; MCP target is scope-specific (the
    // generic agent still uses its project `.mcp.json` here — the Claude
    // local-scope special case is covered separately).
    {
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&root).unwrap();
        let config_path = write_single_agent_config(&root, &root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results = init_profile(
            &shelltool_profile(InitScope::Local),
            InitScope::Local,
            None,
            &reporter,
        );
        assert_no_error("shelltool", "local init", &results);
        assert!(
            root.join(".skills/shell/SKILL.md").is_file(),
            "local scope must deploy the shell skill"
        );
    }

    // User scope: skills deploy into the *global* store (`~/.skills` + the
    // agent's global skill dir), and the MCP server registers in the agent's
    // *global* config file, not a project `.mcp.json`.
    {
        let env = IsolatedTestEnvironment::new().unwrap();
        let work = env.temp_dir().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&work).unwrap();
        let config_path = write_single_agent_config(&work, &work);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results = init_profile(
            &shelltool_profile(InitScope::User),
            InitScope::User,
            None,
            &reporter,
        );
        assert_no_error("shelltool", "user init", &results);
        // The global store is `~/.skills` (HOME-rooted, via `dirs`); the
        // agent's global skill dir is its config's `global_path`
        // (`<work>/.fake/skills`, since `write_single_agent_config` roots
        // globals at its `home` argument, here `work`), which holds the symlink.
        assert!(
            env.home_path().join(".skills/shell/SKILL.md").is_file(),
            "user scope must deploy the shell skill into the global ~/.skills store"
        );
        let link = work.join(".fake/skills/shell");
        assert!(
            std::fs::symlink_metadata(&link).is_ok_and(|m| m.file_type().is_symlink()),
            "user scope must symlink the shell skill into the agent's global skill dir"
        );
        assert!(
            !work.join(".skills").exists(),
            "user scope must NOT write a project .skills store"
        );
        // The agent's global mcp config (`<work>/.fake/mcp.json`) holds the
        // registration; the project `.mcp.json` must be untouched.
        let global_mcp = work.join(".fake/mcp.json");
        assert!(
            global_mcp.is_file()
                && read_json(&global_mcp)["mcpServers"]["shelltool"]["command"] == "shelltool",
            "user scope must register MCP in the agent's global config"
        );
        assert!(
            !work.join(".mcp.json").exists(),
            "user scope must not write a project .mcp.json"
        );
    }
}

/// Case 6: the code-context local-scope MCP regression. Routing MCP
/// registration through the profile's strategy-aware applier means a real
/// `claude-code` agent at `Local` scope registers in `~/.claude.json` under
/// `projects.<root>.mcpServers` — the location the old hand-rolled
/// code-context loop silently dropped. HOME is isolated so `~/.claude.json`
/// is the tempdir's.
#[test]
#[serial]
fn code_context_local_scope_registers_in_claude_json_projects_map() {
    let env = IsolatedTestEnvironment::new().unwrap();
    // CWD is the project root; `project_key()` falls back to it (the tempdir
    // is not inside a git repo), giving a deterministic projects-map key.
    let work = env.temp_dir().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&work).unwrap();

    // A real `claude-code` agent so `strategy_for` selects ClaudeCodeStrategy
    // (its Local scope writes `~/.claude.json`, not a project `.mcp.json`).
    let agents_yaml = format!(
        r#"agents:
  - id: claude-code
    name: Claude Code
    project_path: .claude/skills
    global_path: "{home}/.claude/skills"
    settings_path: .claude/settings.json
    global_settings_path: "{home}/.claude/settings.json"
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      global_path: "{home}/.claude.json"
      servers_key: mcpServers
"#,
        detect = work.display(),
        home = env.home_path().display(),
    );
    let config_path = work.join("agents.yaml");
    std::fs::write(&config_path, &agents_yaml).unwrap();
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let reporter = NullReporter;
    // root: None so registration flows through the strategy-aware applier
    // (the explicit-root path bypasses the Claude local special case).
    let profile = code_context_profile(InitScope::Local);
    let results = init_profile(&profile, InitScope::Local, None, &reporter);
    assert_no_error("code-context", "local init", &results);

    // The MCP server lands in `~/.claude.json` under the project entry —
    // NOT in a project `.mcp.json`. This is the regression the migration fixed.
    let claude_json = env.home_path().join(".claude.json");
    assert!(
        claude_json.is_file(),
        "Claude local scope must write ~/.claude.json"
    );
    let json = read_json(&claude_json);
    let key = work.to_string_lossy().to_string();
    assert_eq!(
            json["projects"][&key]["mcpServers"]["code-context"]["command"], "code-context",
            "code-context MCP must register in ~/.claude.json projects.<root>.mcpServers (local scope), got: {json}"
        );
    assert!(
        !work.join(".mcp.json").exists(),
        "Claude local scope must NOT write a project .mcp.json"
    );

    // Round-trip: deinit prunes the local-scope registration.
    let results = deinit_profile(&profile, InitScope::Local, None, &reporter);
    assert_no_error("code-context", "local deinit", &results);
    let json = read_json(&claude_json);
    assert!(
        json["projects"][&key]["mcpServers"]
            .get("code-context")
            .is_none(),
        "deinit must remove the local-scope MCP registration"
    );
}
