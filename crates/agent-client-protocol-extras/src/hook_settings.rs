//! Loader for Claude Code's `.claude/settings.json` chain into a [`HookConfig`].
//!
//! Claude Code reads hook configuration from a precedence chain of settings
//! files. This module reads that same chain for a given working directory and
//! produces the [`HookConfig`] that the rest of `agent-client-protocol-extras`
//! already understands (and can turn into runtime registrations via
//! [`HookConfig::build_registrations`]).
//!
//! # Precedence chain (lowest → highest)
//!
//! 1. **User**:    `~/.claude/settings.json`
//! 2. **Project**: `<cwd>/.claude/settings.json`
//! 3. **Local**:   `<cwd>/.claude/settings.local.json`
//!
//! Hooks are merged *additively* across these sources: Claude runs every
//! matching hook from every settings source, so for each event name the matcher
//! groups from all files are concatenated in chain order (user → project →
//! local). There is no override — a `PreToolUse` group in the user file and a
//! `PreToolUse` group in the project file both end up in the result.
//!
//! # Path resolution
//!
//! The user home directory comes from [`dirs::home_dir`] (the same crate mirdan
//! uses), matching Claude Code's `~/.claude/settings.json` convention. The
//! project files are resolved relative to the passed `cwd` directly — the ACP
//! session cwd already *is* the project/workspace directory, so there is no
//! ancestor walk-up in v1. A walk-up to the nearest `.claude`-bearing ancestor
//! can be added later if a real need appears.
//!
//! This module deliberately does **not** use `swissarmyhammer-directory`: that
//! crate manages tool-owned directories (`.swissarmyhammer`, `.avp`, …) and has
//! no knowledge of `.claude`.
//!
//! # What is read
//!
//! Only the top-level `hooks` key is read from each file. Every other key
//! (`permissions`, `env`, `statusLine`, `model`, …) is ignored. A file with no
//! `hooks` key contributes nothing.
//!
//! `disableAllHooks: true` in *any* applicable file disables hooks overall: the
//! returned [`HookConfig`] is empty. This matches Claude Code's intent that the
//! flag is a hard off-switch rather than a per-file opt-out.
//!
//! # Robustness
//!
//! A missing or blank file is skipped. A file whose `hooks` value is malformed
//! (does not deserialize into the expected shape) is logged at `warn` and that
//! file's hooks are skipped; the loader never panics or fails the agent. Use
//! [`load_hook_config`] for this lenient behavior; [`try_load_hook_config`]
//! surfaces I/O and parse errors for callers that want them.
//!
//! # Out of scope
//!
//! Plugin hooks (`hooks/hooks.json`), managed-policy settings, and skill/agent
//! frontmatter hooks. Only the three `settings.json` files above are read.

use std::path::{Path, PathBuf};

use serde_json::Value;
use swissarmyhammer_common::json::{read_json_file, JsonFileError};

use crate::hook_config::{HookConfig, HookEventKindConfig, MatcherGroup};

/// Top-level key carrying the hook configuration in a settings file.
const HOOKS_KEY: &str = "hooks";

/// Top-level boolean key that disables all hooks when `true`.
const DISABLE_ALL_HOOKS_KEY: &str = "disableAllHooks";

/// Error surfaced by [`try_load_hook_config`].
///
/// [`load_hook_config`] swallows these (logging a warning) so that hook
/// loading can never fail the agent; callers that want to react to a broken
/// settings file use [`try_load_hook_config`] instead.
#[derive(Debug, thiserror::Error)]
pub enum HookSettingsError {
    /// A settings file existed but could not be read or parsed as JSON/JSONC.
    #[error(transparent)]
    File(#[from] JsonFileError),
    /// A file's `hooks` value did not deserialize into the expected shape.
    #[error("Invalid `hooks` in {path}: {source}")]
    Hooks {
        /// Path of the settings file whose `hooks` value was malformed.
        path: PathBuf,
        /// The underlying deserialization error.
        source: serde_json::Error,
    },
}

/// Resolve the user-level settings path: `~/.claude/settings.json`.
///
/// Returns `None` when no home directory can be determined (in which case the
/// user level simply contributes nothing).
fn user_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

/// Resolve the project-level settings path: `<cwd>/.claude/settings.json`.
fn project_settings_path(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("settings.json")
}

/// Resolve the local-level settings path: `<cwd>/.claude/settings.local.json`.
fn local_settings_path(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("settings.local.json")
}

/// The ordered settings paths for a working directory (user → project → local).
///
/// The user path is omitted when no home directory is available.
fn settings_paths(cwd: &Path) -> Vec<PathBuf> {
    ordered_settings_paths(user_settings_path(), cwd)
}

/// Build the ordered settings paths (user → project → local) from an already
/// resolved user path and a working directory.
///
/// A `None` `user` omits the user level — the rest of the chain still applies.
/// Split out from [`settings_paths`] so the omit-user-level branch is testable
/// without depending on the process environment.
fn ordered_settings_paths(user: Option<PathBuf>, cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(3);
    if let Some(user) = user {
        paths.push(user);
    }
    paths.push(project_settings_path(cwd));
    paths.push(local_settings_path(cwd));
    paths
}

/// Whether a parsed settings document sets `disableAllHooks: true`.
fn disables_all_hooks(doc: &Value) -> bool {
    doc.get(DISABLE_ALL_HOOKS_KEY)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Extract and deserialize the `hooks` map from one parsed settings document.
///
/// Returns:
/// - `Ok(None)` when the document has no `hooks` key (contributes nothing), and
/// - `Ok(Some(map))` for a well-formed `hooks` map, and
/// - `Err(_)` when `hooks` is present but does not deserialize.
///
/// The `path` is used only to label a deserialization error.
fn extract_hooks(doc: &Value, path: &Path) -> Result<Option<HookConfig>, HookSettingsError> {
    let Some(hooks_value) = doc.get(HOOKS_KEY) else {
        return Ok(None);
    };
    // Deserialize just the `hooks` value into HookConfig.hooks by wrapping it in
    // the shape HookConfig expects. This reuses HookConfig's own deserialization
    // (including its forward-compatible event-kind handling) rather than
    // re-implementing it.
    let wrapper = serde_json::json!({ HOOKS_KEY: hooks_value });
    serde_json::from_value::<HookConfig>(wrapper)
        .map(Some)
        .map_err(|source| HookSettingsError::Hooks {
            path: path.to_path_buf(),
            source,
        })
}

/// Merge one source's hooks into the accumulator, concatenating matcher groups
/// per event name so every source's hooks are preserved.
fn merge_hooks(
    acc: &mut std::collections::HashMap<HookEventKindConfig, Vec<MatcherGroup>>,
    incoming: HookConfig,
) {
    for (event, groups) in incoming.hooks {
        acc.entry(event).or_default().extend(groups);
    }
}

/// Read and merge the `.claude/settings.json` chain for `cwd` into a
/// [`HookConfig`], surfacing any I/O or parse error.
///
/// Reads, in precedence order, `~/.claude/settings.json`,
/// `<cwd>/.claude/settings.json`, and `<cwd>/.claude/settings.local.json`.
/// Missing or blank files are skipped. Only the top-level `hooks` key of each
/// file is used; all other keys are ignored. If any applicable file sets
/// `disableAllHooks: true`, the returned config is empty.
///
/// # Errors
///
/// Returns [`HookSettingsError::File`] when an existing file cannot be read or
/// parsed as JSON/JSONC, and [`HookSettingsError::Hooks`] when a file's `hooks`
/// value is present but malformed. Prefer [`load_hook_config`] when a broken
/// settings file should be skipped rather than propagated.
pub fn try_load_hook_config(cwd: &Path) -> Result<HookConfig, HookSettingsError> {
    let mut merged: std::collections::HashMap<HookEventKindConfig, Vec<MatcherGroup>> =
        std::collections::HashMap::new();

    for path in settings_paths(cwd) {
        // Missing/blank files read as an empty object and contribute nothing.
        let doc = read_json_file(&path)?;

        // `disableAllHooks: true` anywhere is a hard off-switch.
        if disables_all_hooks(&doc) {
            return Ok(HookConfig::default());
        }

        if let Some(config) = extract_hooks(&doc, &path)? {
            merge_hooks(&mut merged, config);
        }
    }

    Ok(HookConfig { hooks: merged })
}

/// Read and merge the `.claude/settings.json` chain for `cwd` into a
/// [`HookConfig`], never failing.
///
/// This is the lenient sibling of [`try_load_hook_config`]: a settings file
/// that cannot be read, cannot be parsed, or whose `hooks` value is malformed
/// is logged at `warn` and skipped, so a broken file degrades gracefully rather
/// than failing the agent. The precedence, merge, and `disableAllHooks`
/// semantics are otherwise identical.
///
/// Because each file is read independently, a malformed file only drops *its
/// own* contribution — well-formed files earlier and later in the chain are
/// still merged.
pub fn load_hook_config(cwd: &Path) -> HookConfig {
    let mut merged: std::collections::HashMap<HookEventKindConfig, Vec<MatcherGroup>> =
        std::collections::HashMap::new();

    for path in settings_paths(cwd) {
        let doc = match read_json_file(&path) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Skipping unreadable settings file");
                continue;
            }
        };

        if disables_all_hooks(&doc) {
            return HookConfig::default();
        }

        match extract_hooks(&doc, &path) {
            Ok(Some(config)) => merge_hooks(&mut merged, config),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Skipping malformed hooks in settings file");
            }
        }
    }

    HookConfig { hooks: merged }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    // Tests in this module mutate the process-global `HOME` env var (read by
    // `dirs::home_dir()` on unix). They are serialized with `serial_test`'s
    // default unnamed `#[serial]` group — the same domain used by
    // `session_store.rs` — so that *all* env-mutating tests across the crate
    // run in a single serialization domain rather than racing over separate
    // locks. `HomeGuard` restores the previous `HOME` on drop.

    /// Guard that points HOME at a temp dir for the duration of a test and
    /// restores the previous value on drop. Must only be used from a
    /// `#[serial]` test so no other thread observes the mutation.
    struct HomeGuard {
        previous: Option<String>,
        _home: TempDir,
        home_claude: PathBuf,
    }

    impl HomeGuard {
        fn new() -> Self {
            let previous = std::env::var("HOME").ok();
            let home = TempDir::new().unwrap();
            std::env::set_var("HOME", home.path());
            let home_claude = home.path().join(".claude");
            fs::create_dir_all(&home_claude).unwrap();
            Self {
                previous,
                _home: home,
                home_claude,
            }
        }

        /// Write the user-level `~/.claude/settings.json`.
        fn write_user_settings(&self, contents: &str) {
            fs::write(self.home_claude.join("settings.json"), contents).unwrap();
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    /// Create a project temp dir with an empty `.claude` directory.
    fn project_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        dir
    }

    fn write_project_settings(cwd: &Path, contents: &str) {
        fs::write(cwd.join(".claude").join("settings.json"), contents).unwrap();
    }

    fn write_local_settings(cwd: &Path, contents: &str) {
        fs::write(cwd.join(".claude").join("settings.local.json"), contents).unwrap();
    }

    fn pre_tool_use(config: &HookConfig) -> Vec<&MatcherGroup> {
        config
            .hooks
            .get(&HookEventKindConfig::PreToolUse)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    #[test]
    #[serial]
    fn merges_user_and_project_pre_tool_use_groups() {
        let home = HomeGuard::new();
        home.write_user_settings(
            r#"{
                "hooks": {
                    "PreToolUse": [
                        { "matcher": "Bash", "hooks": [{ "type": "command", "command": "user.sh" }] }
                    ]
                }
            }"#,
        );
        let project = project_dir();
        write_project_settings(
            project.path(),
            r#"{
                "hooks": {
                    "PreToolUse": [
                        { "matcher": "Edit", "hooks": [{ "type": "command", "command": "project.sh" }] }
                    ]
                }
            }"#,
        );

        let config = load_hook_config(project.path());

        // Both the user and project PreToolUse groups must be present.
        let groups = pre_tool_use(&config);
        assert_eq!(groups.len(), 2, "expected user + project groups merged");
        let matchers: Vec<&str> = groups.iter().filter_map(|g| g.matcher.as_deref()).collect();
        assert!(matchers.contains(&"Bash"));
        assert!(matchers.contains(&"Edit"));
    }

    #[test]
    #[serial]
    fn merge_order_is_user_then_project_then_local() {
        let home = HomeGuard::new();
        home.write_user_settings(
            r#"{ "hooks": { "PreToolUse": [{ "matcher": "User", "hooks": [{ "type": "command", "command": "u.sh" }] }] } }"#,
        );
        let project = project_dir();
        write_project_settings(
            project.path(),
            r#"{ "hooks": { "PreToolUse": [{ "matcher": "Project", "hooks": [{ "type": "command", "command": "p.sh" }] }] } }"#,
        );
        write_local_settings(
            project.path(),
            r#"{ "hooks": { "PreToolUse": [{ "matcher": "Local", "hooks": [{ "type": "command", "command": "l.sh" }] }] } }"#,
        );

        let config = load_hook_config(project.path());
        let groups = pre_tool_use(&config);
        let matchers: Vec<&str> = groups.iter().filter_map(|g| g.matcher.as_deref()).collect();
        assert_eq!(matchers, vec!["User", "Project", "Local"]);
    }

    #[test]
    #[serial]
    fn reads_only_the_hooks_key() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(
            project.path(),
            r#"{
                "permissions": { "allow": ["Bash"] },
                "env": { "FOO": "bar" },
                "statusLine": { "type": "command", "command": "x" },
                "model": "claude-3",
                "hooks": {
                    "PreToolUse": [
                        { "matcher": "Bash", "hooks": [{ "type": "command", "command": "check.sh" }] }
                    ]
                }
            }"#,
        );

        let config = load_hook_config(project.path());
        assert_eq!(pre_tool_use(&config).len(), 1);
        // Only the hooks key contributed; no spurious extra event kinds.
        assert_eq!(config.hooks.len(), 1);
    }

    #[test]
    #[serial]
    fn file_without_hooks_contributes_nothing() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(
            project.path(),
            r#"{ "permissions": { "allow": ["Bash"] }, "model": "claude-3" }"#,
        );

        let config = load_hook_config(project.path());
        assert!(config.hooks.is_empty());
    }

    #[test]
    #[serial]
    fn disable_all_hooks_yields_empty_config() {
        let home = HomeGuard::new();
        // User level provides hooks...
        home.write_user_settings(
            r#"{ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "u.sh" }] }] } }"#,
        );
        let project = project_dir();
        // ...but project disables all hooks.
        write_project_settings(project.path(), r#"{ "disableAllHooks": true }"#);

        let config = load_hook_config(project.path());
        assert!(
            config.hooks.is_empty(),
            "disableAllHooks anywhere must yield an empty config"
        );
    }

    #[test]
    #[serial]
    fn jsonc_comments_and_trailing_commas_tolerated() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(
            project.path(),
            "// project hooks\n{\n  \"hooks\": {\n    \"PreToolUse\": [\n      { \"matcher\": \"Bash\", \"hooks\": [{ \"type\": \"command\", \"command\": \"c.sh\" }], },\n    ],\n  },\n}",
        );

        let config = load_hook_config(project.path());
        assert_eq!(pre_tool_use(&config).len(), 1);
    }

    #[test]
    #[serial]
    fn missing_files_are_skipped_without_error() {
        let _home = HomeGuard::new();
        // A cwd with no .claude directory at all.
        let empty = TempDir::new().unwrap();
        let config = load_hook_config(empty.path());
        assert!(config.hooks.is_empty());
    }

    // No `#[serial]`: this exercises the pure path-ordering helper and does not
    // touch the process environment.
    #[test]
    fn no_home_dir_omits_user_level() {
        let cwd = Path::new("/work/project");

        // With a home dir the chain is user → project → local (three paths).
        let with_home =
            ordered_settings_paths(Some(PathBuf::from("/home/u/.claude/settings.json")), cwd);
        assert_eq!(
            with_home,
            vec![
                PathBuf::from("/home/u/.claude/settings.json"),
                project_settings_path(cwd),
                local_settings_path(cwd),
            ]
        );

        // With no home dir the user level is omitted; project/local remain.
        let without_home = ordered_settings_paths(None, cwd);
        assert_eq!(
            without_home,
            vec![project_settings_path(cwd), local_settings_path(cwd)]
        );
    }

    #[test]
    #[serial]
    fn blank_file_is_skipped() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(project.path(), "   \n\t");
        let config = load_hook_config(project.path());
        assert!(config.hooks.is_empty());
    }

    #[test]
    #[serial]
    fn malformed_hooks_in_one_file_skipped_but_others_kept() {
        let home = HomeGuard::new();
        // Valid user hooks.
        home.write_user_settings(
            r#"{ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "u.sh" }] }] } }"#,
        );
        let project = project_dir();
        // `hooks` is the wrong shape (a string, not a map) — malformed.
        write_project_settings(project.path(), r#"{ "hooks": "not a map" }"#);

        // Lenient loader keeps the valid user hooks and drops the malformed file.
        let config = load_hook_config(project.path());
        assert_eq!(pre_tool_use(&config).len(), 1);
    }

    #[test]
    #[serial]
    fn try_load_surfaces_malformed_hooks_error() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(project.path(), r#"{ "hooks": "not a map" }"#);

        let err = try_load_hook_config(project.path()).unwrap_err();
        assert!(matches!(err, HookSettingsError::Hooks { .. }));
    }

    #[test]
    #[serial]
    fn try_load_surfaces_parse_error() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(project.path(), "this is not json {{{");

        let err = try_load_hook_config(project.path()).unwrap_err();
        assert!(matches!(err, HookSettingsError::File(_)));
    }

    #[test]
    #[serial]
    fn loaded_config_round_trips_through_build_registrations() {
        let _home = HomeGuard::new();
        let project = project_dir();
        write_project_settings(
            project.path(),
            r#"{
                "hooks": {
                    "PreToolUse": [
                        { "matcher": "Bash", "hooks": [{ "type": "command", "command": "check.sh" }] }
                    ],
                    "PostToolUse": [
                        { "hooks": [{ "type": "command", "command": "log.sh" }] }
                    ]
                }
            }"#,
        );

        let config = load_hook_config(project.path());
        // No evaluator needed because all handlers are command handlers.
        let registrations = config.build_registrations(None).unwrap();
        assert_eq!(registrations.len(), 2);
    }

    #[test]
    #[serial]
    fn forward_compatible_event_kinds_tolerated() {
        let _home = HomeGuard::new();
        let project = project_dir();
        // SessionEnd is a forward-compatible kind not fired by ACP; it must
        // deserialize fine here and only be skipped at build_registrations.
        write_project_settings(
            project.path(),
            r#"{
                "hooks": {
                    "SessionEnd": [
                        { "hooks": [{ "type": "command", "command": "bye.sh" }] }
                    ]
                }
            }"#,
        );

        let config = load_hook_config(project.path());
        assert_eq!(config.hooks.len(), 1);
        // It is skipped, not errored, when building registrations.
        let registrations = config.build_registrations(None).unwrap();
        assert!(registrations.is_empty());
    }
}
