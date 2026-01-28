//! AVP Doctor - Diagnostic checks for AVP setup and configuration.
//!
//! Checks:
//! - Hooks installed (user/project/local)
//! - Git repository (warning if not found)
//! - AVP in PATH

use std::env;
use std::path::PathBuf;

use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};

use crate::install::{settings_path, InstallTarget};

/// AVP diagnostic runner.
pub struct AvpDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for AvpDoctor {
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl AvpDoctor {
    /// Create a new AvpDoctor instance.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all AVP diagnostic checks.
    pub fn run_diagnostics(&mut self) -> i32 {
        self.check_git_repository();
        self.check_avp_in_path();
        self.check_hooks_installed();

        self.get_exit_code()
    }

    /// Check if we're in a Git repository.
    /// This is a warning (not error) since AVP can work outside git repos.
    fn check_git_repository(&mut self) {
        use swissarmyhammer_common::utils::find_git_repository_root;

        match find_git_repository_root() {
            Some(path) => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Detected at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Warning,
                    message: "Not in a Git repository".to_string(),
                    fix: Some("Run from within a Git repository or run `git init`".to_string()),
                });
            }
        }
    }

    /// Check if AVP binary is in PATH.
    fn check_avp_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) { "avp.exe" } else { "avp" };
        let mut found = false;
        let mut found_path = None;

        for path in paths {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                found = true;
                found_path = Some(exe_path);
                break;
            }
        }

        if found {
            self.add_check(Check {
                name: "AVP in PATH".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", found_path.unwrap().display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "AVP in PATH".to_string(),
                status: CheckStatus::Warning,
                message: "avp not found in PATH".to_string(),
                fix: Some(
                    "Add avp to your PATH or install with `cargo install --path avp-cli`"
                        .to_string(),
                ),
            });
        }
    }

    /// Check if AVP hooks are installed at any level.
    /// OK if hooks found anywhere, Warning only if no hooks at all.
    fn check_hooks_installed(&mut self) {
        let targets = [
            (InstallTarget::User, "user"),
            (InstallTarget::Project, "project"),
            (InstallTarget::Local, "local"),
        ];

        let mut installed_at: Vec<String> = Vec::new();

        for (target, name) in &targets {
            let path = settings_path(*target);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) {
                        if has_avp_hooks(&settings) {
                            installed_at.push(name.to_string());
                        }
                    }
                }
            }
        }

        if installed_at.is_empty() {
            self.add_check(Check {
                name: "Hooks Installed".to_string(),
                status: CheckStatus::Warning,
                message: "AVP hooks not installed".to_string(),
                fix: Some(
                    "Run `avp install project` or `avp install user` to install hooks".to_string(),
                ),
            });
        } else {
            self.add_check(Check {
                name: "Hooks Installed".to_string(),
                status: CheckStatus::Ok,
                message: format!("AVP hooks installed at: {}", installed_at.join(", ")),
                fix: None,
            });
        }
    }
}

impl Default for AvpDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if the settings value contains any AVP hooks.
fn has_avp_hooks(settings: &serde_json::Value) -> bool {
    if let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) {
        for (_event_name, event_hooks) in hooks {
            if let Some(arr) = event_hooks.as_array() {
                for hook in arr {
                    if is_avp_hook(hook) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if a hook entry is an AVP hook.
fn is_avp_hook(hook: &serde_json::Value) -> bool {
    if let Some(hooks_array) = hook.get("hooks").and_then(|h| h.as_array()) {
        for h in hooks_array {
            if let Some(cmd) = h.get("command").and_then(|c| c.as_str()) {
                if cmd == "avp" || cmd.ends_with("/avp") {
                    return true;
                }
            }
        }
    }
    false
}

/// Run the doctor command and display results.
pub fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = AvpDoctor::new();
    let exit_code = doctor.run_diagnostics();
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let doctor = AvpDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    #[test]
    fn test_run_diagnostics() {
        let mut doctor = AvpDoctor::new();
        let exit_code = doctor.run_diagnostics();

        // Should have exactly 3 checks: git, path, hooks (combined)
        assert!(!doctor.checks().is_empty());
        assert_eq!(doctor.checks().len(), 3);

        // Exit code should be 0, 1, or 2
        assert!(exit_code <= 2);
    }

    #[test]
    fn test_run_doctor() {
        // This will print to stdout, but we just verify it doesn't panic
        // and returns a valid exit code
        let exit_code = run_doctor(false);
        assert!(exit_code <= 2);
    }

    #[test]
    fn test_run_doctor_verbose() {
        // Test verbose mode
        let exit_code = run_doctor(true);
        assert!(exit_code <= 2);
    }

    #[test]
    fn test_has_avp_hooks_true() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [{ "type": "command", "command": "avp" }]
                    }
                ]
            }
        });
        assert!(has_avp_hooks(&settings));
    }

    #[test]
    fn test_has_avp_hooks_false() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [{ "type": "command", "command": "other-tool" }]
                    }
                ]
            }
        });
        assert!(!has_avp_hooks(&settings));
    }

    #[test]
    fn test_has_avp_hooks_empty() {
        let settings = serde_json::json!({});
        assert!(!has_avp_hooks(&settings));
    }

    #[test]
    fn test_has_avp_hooks_full_path() {
        let settings = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [{ "type": "command", "command": "/usr/local/bin/avp" }]
                    }
                ]
            }
        });
        assert!(has_avp_hooks(&settings));
    }

    #[test]
    fn test_is_avp_hook() {
        let avp_hook = serde_json::json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": "avp" }]
        });
        assert!(is_avp_hook(&avp_hook));

        let other_hook = serde_json::json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": "other" }]
        });
        assert!(!is_avp_hook(&other_hook));
    }

    #[test]
    fn test_default() {
        let doctor = AvpDoctor::default();
        assert!(doctor.checks().is_empty());
    }

    #[test]
    fn test_check_hooks_installed() {
        let mut doctor = AvpDoctor::new();
        doctor.check_hooks_installed();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Hooks Installed");
        // Status depends on whether hooks are actually installed
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }
}
