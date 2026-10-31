//! Real-path tests for the legacy native-tool deny cleanup.
//!
//! Older sah versions wrote `Bash`, `Edit`, `Read` and `Write` into
//! `permissions.deny`. The installer must never write those entries again,
//! and it must remove them from every scope's settings file. Each test runs
//! the real [`init_profile`] or [`deinit_profile`] against a fake agent whose
//! settings files live in temporary directories.

use std::path::PathBuf;

use serde_json::{json, Value};
use serial_test::serial;
use swissarmyhammer_common::lifecycle::InitScope;
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

use super::profile_consistency_tests::{code_context_profile, kanban_profile, sah_profile};
use super::*;
use crate::test_support::{assert_no_init_error, write_single_agent_config, MirdanConfigGuard};

/// Every scope the installer runs at, in the order the CLI lists them.
const ALL_SCOPES: [InitScope; 3] = [InitScope::User, InitScope::Project, InitScope::Local];

/// The four native tools an older sah denied.
const LEGACY_DENIES: [&str; 4] = ["Bash", "Edit", "Read", "Write"];

/// A deny entry that belongs to the user, not to sah. Cleanup must keep it.
const UNRELATED_DENY: &str = "WebFetch";

/// The two installer entry points the tests drive.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// [`init_profile`].
    Init,
    /// [`deinit_profile`].
    Deinit,
}

/// A hermetic install environment: an isolated `$HOME`, an install root, a
/// working directory inside that root, and a fake agent config.
///
/// The guards restore the process state when the value drops. Field order
/// matters: the guards drop before the temporary directories they point at.
struct Sandbox {
    /// Restores the agent config environment variable.
    _mirdan: MirdanConfigGuard,
    /// Restores the working directory.
    _cwd: CurrentDirGuard,
    /// Owns the isolated `$HOME`.
    env: IsolatedTestEnvironment,
    /// Owns the install root directory.
    root_dir: tempfile::TempDir,
}

impl Sandbox {
    /// Build the environment and point agent detection at the fake agent.
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let cwd = CurrentDirGuard::new(&root).unwrap();
        let config = write_single_agent_config(&root, &env.home_path());
        Self {
            _mirdan: MirdanConfigGuard::set(config),
            _cwd: cwd,
            env,
            root_dir,
        }
    }

    /// The canonical install root.
    fn root(&self) -> PathBuf {
        self.root_dir.path().canonicalize().unwrap()
    }

    /// The settings file the installer reads and writes at `scope`.
    fn settings_file(&self, scope: InitScope) -> PathBuf {
        match scope {
            InitScope::User => self.env.home_path().join(".fake/settings.json"),
            InitScope::Project => self.root().join(".fake/settings.json"),
            InitScope::Local => self.root().join(".fake/settings.local.json"),
        }
    }

    /// The explicit root to pass the installer: user scope takes none.
    fn explicit_root(&self, scope: InitScope) -> Option<PathBuf> {
        match scope {
            InitScope::User => None,
            InitScope::Project | InitScope::Local => Some(self.root()),
        }
    }

    /// Write `settings` as the settings file of `scope`.
    fn seed(&self, scope: InitScope, settings: &Value) {
        let path = self.settings_file(scope);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, settings.to_string()).unwrap();
    }

    /// Run the real installer `step` for `profile` at `scope` and assert no
    /// install step errored.
    fn run(&self, step: Step, profile: &Profile, scope: InitScope) {
        let root = self.explicit_root(scope);
        let results = match step {
            Step::Init => init_profile(profile, scope, root.as_deref(), &NullReporter),
            Step::Deinit => deinit_profile(profile, scope, root.as_deref(), &NullReporter),
        };
        assert_no_init_error(&format!("{step:?}"), &results);
    }

    /// The settings file of `scope` parsed as JSON, or `Null` when absent.
    fn settings(&self, scope: InitScope) -> Value {
        let path = self.settings_file(scope);
        if !path.exists() {
            return Value::Null;
        }
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The `permissions.deny` strings of the settings file of `scope`.
    fn deny(&self, scope: InitScope) -> Vec<String> {
        deny_entries(&self.settings(scope))
    }
}

/// The `permissions.deny` strings of a parsed settings value. A missing key
/// reads as no entries.
fn deny_entries(settings: &Value) -> Vec<String> {
    settings
        .pointer("/permissions/deny")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// A settings value whose deny list holds every legacy entry plus one entry
/// that belongs to the user.
fn legacy_settings() -> Value {
    let mut deny: Vec<&str> = LEGACY_DENIES.to_vec();
    deny.push(UNRELATED_DENY);
    json!({ "permissions": { "deny": deny } })
}

/// Seed every legacy deny plus the unrelated one at `scope`, run `step` for
/// `profile`, and assert that the settings file of `scope` keeps only the
/// unrelated deny and that no settings file of any scope holds a legacy deny.
fn assert_step_strips_legacy_denies(step: Step, label: &str, profile: &Profile, scope: InitScope) {
    let sandbox = Sandbox::new();
    sandbox.seed(scope, &legacy_settings());

    sandbox.run(step, profile, scope);

    assert_eq!(
        sandbox.deny(scope),
        [UNRELATED_DENY],
        "{label} {step:?} at {scope:?} must strip the legacy denies and keep the unrelated one"
    );
    assert_no_legacy_deny_anywhere(&sandbox, step, label);
}

/// No legacy native tool is denied in any settings file of any scope.
fn assert_no_legacy_deny_anywhere(sandbox: &Sandbox, step: Step, label: &str) {
    for scope in ALL_SCOPES {
        let deny = sandbox.deny(scope);
        for tool in LEGACY_DENIES {
            assert!(
                !deny.iter().any(|entry| entry == tool),
                "{label} {step:?} at {scope:?} left a {tool} deny: {deny:?}"
            );
        }
    }
}

/// Run `step` for `profile` at every scope, each against a fresh sandbox.
fn assert_step_strips_at_every_scope(step: Step, label: &str, profile: &Profile) {
    for scope in ALL_SCOPES {
        assert_step_strips_legacy_denies(step, label, profile, scope);
    }
}

/// After a fresh install of `profile` at every scope, no settings file of any
/// scope holds a deny key at all, not even an empty array.
fn assert_fresh_install_writes_no_deny(label: &str, profile: &Profile) {
    for scope in ALL_SCOPES {
        let sandbox = Sandbox::new();

        sandbox.run(Step::Init, profile, scope);

        for checked in ALL_SCOPES {
            let settings = sandbox.settings(checked);
            assert!(
                settings.pointer("/permissions/deny").is_none(),
                "{label} fresh init at {scope:?} wrote a deny into the {checked:?} \
                 settings file: {settings}"
            );
        }
    }
}

/// The sah profile: all skills, agents and validators, plus the statusline.
fn sah() -> Profile {
    sah_profile(InitScope::Project)
}

/// The kanban CLI profile.
fn kanban() -> Profile {
    kanban_profile(InitScope::Project)
}

/// The code-context CLI profile.
fn code_context() -> Profile {
    code_context_profile(InitScope::Project)
}

/// The bare mirdan profile, which declares no component at all.
fn mirdan() -> Profile {
    Profile::default()
}

/// User-scope init strips every legacy deny.
#[test]
#[serial]
fn init_strips_legacy_denies_at_user_scope() {
    assert_step_strips_legacy_denies(Step::Init, "mirdan", &mirdan(), InitScope::User);
}

/// Project-scope init strips every legacy deny.
#[test]
#[serial]
fn init_strips_legacy_denies_at_project_scope() {
    assert_step_strips_legacy_denies(Step::Init, "mirdan", &mirdan(), InitScope::Project);
}

/// Local-scope init strips every legacy deny from `settings.local.json`.
#[test]
#[serial]
fn init_strips_legacy_denies_at_local_scope() {
    assert_step_strips_legacy_denies(Step::Init, "mirdan", &mirdan(), InitScope::Local);
}

/// User-scope deinit strips every legacy deny.
#[test]
#[serial]
fn deinit_strips_legacy_denies_at_user_scope() {
    assert_step_strips_legacy_denies(Step::Deinit, "mirdan", &mirdan(), InitScope::User);
}

/// Project-scope deinit strips every legacy deny.
#[test]
#[serial]
fn deinit_strips_legacy_denies_at_project_scope() {
    assert_step_strips_legacy_denies(Step::Deinit, "mirdan", &mirdan(), InitScope::Project);
}

/// Local-scope deinit strips every legacy deny from `settings.local.json`.
#[test]
#[serial]
fn deinit_strips_legacy_denies_at_local_scope() {
    assert_step_strips_legacy_denies(Step::Deinit, "mirdan", &mirdan(), InitScope::Local);
}

/// Local-scope init leaves the project settings file alone: the two files
/// are separate, and a scope cleans only its own file.
#[test]
#[serial]
fn local_init_leaves_the_project_settings_file_alone() {
    let sandbox = Sandbox::new();
    sandbox.seed(InitScope::Project, &legacy_settings());

    sandbox.run(Step::Init, &mirdan(), InitScope::Local);

    assert_eq!(sandbox.settings(InitScope::Project), legacy_settings());
}

/// Cleanup keeps every unrelated key and every unrelated deny entry.
#[test]
#[serial]
fn cleanup_keeps_unrelated_keys() {
    let sandbox = Sandbox::new();
    let hooks = json!({
        "PostToolUse": [
            { "matcher": "Write", "hooks": [{ "type": "command", "command": "fmt" }] }
        ]
    });
    sandbox.seed(
        InitScope::Project,
        &json!({
            "model": "opus",
            "permissions": { "deny": ["Edit", UNRELATED_DENY], "allow": ["Read"] },
            "hooks": hooks,
        }),
    );

    sandbox.run(Step::Init, &mirdan(), InitScope::Project);

    let settings = sandbox.settings(InitScope::Project);
    assert_eq!(
        (
            &settings["model"],
            &settings["permissions"]["allow"],
            &settings["hooks"],
            deny_entries(&settings),
        ),
        (
            &json!("opus"),
            &json!(["Read"]),
            &hooks,
            vec![UNRELATED_DENY.to_string()]
        ),
    );
}

/// A settings file the installer has nothing to clean stays byte for byte
/// the same, so the cleanup never rewrites a user's formatting for no reason.
#[test]
#[serial]
fn cleanup_does_not_rewrite_a_clean_settings_file() {
    let sandbox = Sandbox::new();
    let path = sandbox.settings_file(InitScope::Project);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = format!("{{\n    \"permissions\": {{\"deny\": [\"{UNRELATED_DENY}\"]}}\n}}\n");
    std::fs::write(&path, &original).unwrap();

    sandbox.run(Step::Init, &mirdan(), InitScope::Project);

    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

/// Guard: a real init of the sah profile leaves no legacy deny at any scope.
#[test]
#[serial]
fn sah_init_leaves_no_legacy_deny_at_any_scope() {
    assert_step_strips_at_every_scope(Step::Init, "sah", &sah());
}

/// Guard: a real init of the kanban profile leaves no legacy deny at any scope.
#[test]
#[serial]
fn kanban_init_leaves_no_legacy_deny_at_any_scope() {
    assert_step_strips_at_every_scope(Step::Init, "kanban", &kanban());
}

/// Guard: a real init of the code-context profile leaves no legacy deny at any
/// scope.
#[test]
#[serial]
fn code_context_init_leaves_no_legacy_deny_at_any_scope() {
    assert_step_strips_at_every_scope(Step::Init, "code-context", &code_context());
}

/// Guard: a real init of the mirdan profile leaves no legacy deny at any scope.
#[test]
#[serial]
fn mirdan_init_leaves_no_legacy_deny_at_any_scope() {
    assert_step_strips_at_every_scope(Step::Init, "mirdan", &mirdan());
}

/// Guard: a real deinit of every profile leaves no legacy deny at any scope.
#[test]
#[serial]
fn every_profile_deinit_leaves_no_legacy_deny_at_any_scope() {
    for (label, profile) in [
        ("sah", sah()),
        ("kanban", kanban()),
        ("code-context", code_context()),
        ("mirdan", mirdan()),
    ] {
        assert_step_strips_at_every_scope(Step::Deinit, label, &profile);
    }
}

/// Guard: a fresh sah install writes no deny at any scope.
#[test]
#[serial]
fn sah_fresh_install_writes_no_deny_at_any_scope() {
    assert_fresh_install_writes_no_deny("sah", &sah());
}

/// Guard: a fresh kanban install writes no deny at any scope.
#[test]
#[serial]
fn kanban_fresh_install_writes_no_deny_at_any_scope() {
    assert_fresh_install_writes_no_deny("kanban", &kanban());
}

/// Guard: a fresh code-context install writes no deny at any scope.
#[test]
#[serial]
fn code_context_fresh_install_writes_no_deny_at_any_scope() {
    assert_fresh_install_writes_no_deny("code-context", &code_context());
}

/// Guard: a fresh mirdan install writes no deny at any scope.
#[test]
#[serial]
fn mirdan_fresh_install_writes_no_deny_at_any_scope() {
    assert_fresh_install_writes_no_deny("mirdan", &mirdan());
}

/// The cleanup roster is exactly the four native tools an older sah denied.
/// Pinned so a silent change to the roster fails a test instead of shipping.
#[test]
fn legacy_roster_is_exactly_the_four_native_tools() {
    assert_eq!(LEGACY_NATIVE_DENY_TOOLS, LEGACY_DENIES);
}
