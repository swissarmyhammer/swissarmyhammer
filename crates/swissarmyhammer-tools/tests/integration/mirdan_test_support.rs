//! Shared test support for redirecting mirdan's agent detection during MCP
//! serve-time tests.
//!
//! The serve-time native-deny path ([`apply_serve_time_native_deny`]) reads
//! mirdan's agents config through the process-global `MIRDAN_AGENTS_CONFIG` env
//! var. Any integration test that drives a **Claude** `initialize` handshake
//! against the per-client serve instance therefore touches that global, even
//! when the test only cares about the advertised tool set.
//!
//! Two facilities live here, shared by every test that touches the deny path so
//! the env handling is written once:
//! - [`MirdanConfigGuard`] — RAII redirect of `MIRDAN_AGENTS_CONFIG` into a
//!   tempdir, restoring the prior value on drop.
//! - [`write_claude_agents_config`] — write a minimal agents config whose only
//!   agent is a detected `claude-code` rooted under a tempdir.
//!
//! Because `MIRDAN_AGENTS_CONFIG` is process-global, every test that uses these
//! helpers must also join the shared `#[serial(mirdan_env)]` group so it can
//! never run concurrently with another test that reads or writes the same env
//! var.

use std::path::Path;

/// RAII guard that points `MIRDAN_AGENTS_CONFIG` at a temp agents config for the
/// duration of a test, restoring the prior value (or unsetting it) on drop.
pub struct MirdanConfigGuard {
    original: Option<String>,
}

impl MirdanConfigGuard {
    /// Redirect `MIRDAN_AGENTS_CONFIG` to `path`, capturing the prior value so
    /// [`Drop`] can restore it.
    pub fn set(path: &Path) -> Self {
        let original = std::env::var("MIRDAN_AGENTS_CONFIG").ok();
        std::env::set_var("MIRDAN_AGENTS_CONFIG", path);
        Self { original }
    }
}

impl Drop for MirdanConfigGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(v) => std::env::set_var("MIRDAN_AGENTS_CONFIG", v),
            None => std::env::remove_var("MIRDAN_AGENTS_CONFIG"),
        }
    }
}

/// Write an agents config whose only agent is a detected `claude-code` whose
/// settings files live under `root`, and return its path.
///
/// `detect` points at `root` (which exists), so the agent is detected without
/// the test touching a real `~/.claude`. `settings_path` is the absolute
/// `.claude/settings.json` under `root`; the Local-scope deny writes its
/// `settings.local.json` sibling.
pub fn write_claude_agents_config(root: &Path) -> std::path::PathBuf {
    let settings = root.join(".claude/settings.json");
    let global_settings = root.join("global-settings.json");
    let config = format!(
        r#"
agents:
  - id: claude-code
    name: Claude Code
    project_path: .claude/skills
    global_path: ~/.claude/skills
    detect:
      - dir: {root}
    settings_path: {settings}
    global_settings_path: {global_settings}
    doctor: true
"#,
        root = root.display(),
        settings = settings.display(),
        global_settings = global_settings.display(),
    );
    let path = root.join("agents.yaml");
    std::fs::write(&path, config).expect("write agents.yaml");
    path
}
