//! Mirdan Doctor - Diagnostic checks for Mirdan setup and configuration.
//!
//! Checks:
//! 1. Mirdan binary in PATH
//! 2. Agents detected
//! 3. Install stack (per-component install status from [`crate::status`])
//! 4. Registry reachable
//! 5. Credentials valid

use std::env;
use std::path::PathBuf;

use swissarmyhammer_common::lifecycle::InitScope;
use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};

use crate::agents;
use crate::registry::get_registry_url;
use crate::status;

/// Mirdan diagnostic runner.
pub struct MirdanDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for MirdanDoctor {
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl MirdanDoctor {
    /// Create a new MirdanDoctor instance.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all Mirdan diagnostic checks.
    pub async fn run_diagnostics(&mut self) -> i32 {
        self.check_mirdan_in_path();
        self.check_agents_detected();
        self.check_install_stack();
        self.check_registry_reachable().await;
        self.check_credentials();

        self.get_exit_code()
    }

    /// Check the install-status of every sah-managed component for every
    /// doctor-enabled detected agent across the project and user scopes.
    ///
    /// Adds one [`Check`] per applicable [`ComponentStatus`] — sourced from
    /// [`crate::status::check_all_doctored`] and converted via
    /// [`crate::status::statuses_to_checks`] — so `mirdan doctor` reports the
    /// install stack only for agents that opt in via `doctor: true` in
    /// `agents_default.yaml`. `NotApplicable` statuses are filtered inside
    /// `statuses_to_checks`; the scope-pair policy is applied there too, so
    /// project-missing rows demote to Ok when the user scope is installed.
    ///
    /// If the agents config cannot be loaded, an error check is added in its
    /// place; the dedicated [`Self::check_agents_detected`] check reports the
    /// same failure with a fix hint.
    fn check_install_stack(&mut self) {
        let config = match agents::load_agents_config() {
            Ok(config) => config,
            Err(e) => {
                self.add_check(Check {
                    name: "Install Stack".to_string(),
                    status: CheckStatus::Error,
                    message: format!("Failed to load agents config: {}", e),
                    fix: None,
                });
                return;
            }
        };

        let statuses = status::check_all_doctored(&config, &[InitScope::Project, InitScope::User]);
        for check in status::statuses_to_checks(&statuses) {
            self.add_check(check);
        }
    }

    /// Check if mirdan binary is in PATH.
    fn check_mirdan_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) {
            "mirdan.exe"
        } else {
            "mirdan"
        };

        let mut found_path = None;
        for path in paths {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                found_path = Some(exe_path);
                break;
            }
        }

        if let Some(path) = found_path {
            self.add_check(Check {
                name: "Mirdan in PATH".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", path.display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "Mirdan in PATH".to_string(),
                status: CheckStatus::Warning,
                message: "mirdan not found in PATH".to_string(),
                fix: Some(
                    "Add mirdan to your PATH or install with `cargo install --path mirdan-cli`"
                        .to_string(),
                ),
            });
        }
    }

    /// Check which agents are detected.
    fn check_agents_detected(&mut self) {
        match agents::load_agents_config() {
            Ok(config) => {
                let detected = agents::detect_agents(&config);
                let count = detected.iter().filter(|a| a.detected).count();
                let names: Vec<String> = detected
                    .iter()
                    .filter(|a| a.detected)
                    .map(|a| a.def.name.clone())
                    .collect();

                if count > 0 {
                    self.add_check(Check {
                        name: "Agents Detected".to_string(),
                        status: CheckStatus::Ok,
                        message: format!("{} agent(s): {}", count, names.join(", ")),
                        fix: None,
                    });
                } else {
                    self.add_check(Check {
                        name: "Agents Detected".to_string(),
                        status: CheckStatus::Warning,
                        message: "No agents detected (will fallback to Claude Code)".to_string(),
                        fix: Some(
                            "Install an AI coding agent like Claude Code, Cursor, or Windsurf"
                                .to_string(),
                        ),
                    });
                }
            }
            Err(e) => {
                self.add_check(Check {
                    name: "Agents Detected".to_string(),
                    status: CheckStatus::Error,
                    message: format!("Failed to load agents config: {}", e),
                    fix: Some(
                        "Check $XDG_CONFIG_HOME/mirdan/ (or ~/.config/mirdan/) for syntax errors"
                            .to_string(),
                    ),
                });
            }
        }
    }

    /// Check if the registry is reachable.
    async fn check_registry_reachable(&mut self) {
        let url = get_registry_url();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                self.add_check(Check {
                    name: "Registry Reachable".to_string(),
                    status: CheckStatus::Error,
                    message: format!("Failed to create HTTP client: {}", e),
                    fix: None,
                });
                return;
            }
        };

        match client.get(format!("{}/api/health", url)).send().await {
            Ok(response) if response.status().is_success() => {
                self.add_check(Check {
                    name: "Registry Reachable".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("{} is reachable", url),
                    fix: None,
                });
            }
            Ok(response) => {
                self.add_check(Check {
                    name: "Registry Reachable".to_string(),
                    status: CheckStatus::Warning,
                    message: format!("{} returned status {}", url, response.status()),
                    fix: Some("Check MIRDAN_REGISTRY_URL if using a custom registry".to_string()),
                });
            }
            Err(e) => {
                self.add_check(Check {
                    name: "Registry Reachable".to_string(),
                    status: CheckStatus::Warning,
                    message: format!("Cannot reach {}: {}", url, e),
                    fix: Some(
                        "Check your network connection or MIRDAN_REGISTRY_URL setting".to_string(),
                    ),
                });
            }
        }
    }

    /// Check if credentials are present and configured.
    fn check_credentials(&mut self) {
        match crate::auth::load_credentials() {
            Some(creds) if !creds.token.is_empty() => {
                let source = if env::var("MIRDAN_TOKEN").is_ok() {
                    "MIRDAN_TOKEN env var"
                } else {
                    "$XDG_CONFIG_HOME/mirdan/credentials (or ~/.config/mirdan/credentials)"
                };
                self.add_check(Check {
                    name: "Credentials".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Token present (from {})", source),
                    fix: None,
                });
            }
            _ => {
                self.add_check(Check {
                    name: "Credentials".to_string(),
                    status: CheckStatus::Warning,
                    message: "Not logged in".to_string(),
                    fix: Some("Run 'mirdan login' to authenticate with the registry".to_string()),
                });
            }
        }
    }
}

impl Default for MirdanDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the doctor command and display results.
pub async fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = MirdanDoctor::new();
    let exit_code = doctor.run_diagnostics().await;
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let doctor = MirdanDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    #[test]
    fn test_check_mirdan_in_path() {
        let mut doctor = MirdanDoctor::new();
        doctor.check_mirdan_in_path();
        assert_eq!(doctor.checks().len(), 1);
        assert_eq!(doctor.checks()[0].name, "Mirdan in PATH");
    }

    #[test]
    fn test_check_agents_detected() {
        let mut doctor = MirdanDoctor::new();
        doctor.check_agents_detected();
        assert_eq!(doctor.checks().len(), 1);
        assert_eq!(doctor.checks()[0].name, "Agents Detected");
    }

    #[test]
    fn test_check_credentials() {
        let mut doctor = MirdanDoctor::new();
        doctor.check_credentials();
        assert_eq!(doctor.checks().len(), 1);
        assert_eq!(doctor.checks()[0].name, "Credentials");
    }

    #[test]
    fn test_default() {
        let doctor = MirdanDoctor::default();
        assert!(doctor.checks().is_empty());
    }

    /// The install-stack must only emit checks for agents whose `AgentDef.doctor`
    /// is `true`. Given a config with a doctored agent alongside `cursor` (no
    /// `doctor` field, so `false`), no check name may contain the cursor agent's
    /// human name — the YAML allowlist alone decides who appears.
    ///
    /// Both agents are given **real** tempdir detection paths so they both
    /// pass `get_detected_agents`. Without that, the `cursor` entry would be
    /// dropped before the doctor filter ever ran (and `claude-code` would be
    /// injected via the fallback in `agents::get_detected_agents`), so the
    /// filter under test would never see cursor as input. The positive control
    /// — asserting that the doctor-enabled agent's checks **are** present —
    /// then proves detection actually fired for the doctored side.
    #[test]
    fn test_check_install_stack_only_emits_doctored_agents() {
        use crate::agents::{AgentDef, AgentsConfig, DetectMethod, SymlinkPolicy};
        use crate::status::{check_all_doctored, to_check, ComponentState};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let detect_doctored = dir.path().join("detect-doctored");
        let detect_cursor = dir.path().join("detect-cursor");
        std::fs::create_dir_all(&detect_doctored).unwrap();
        std::fs::create_dir_all(&detect_cursor).unwrap();

        let doctored = AgentDef {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            project_path: ".claude/skills".to_string(),
            global_path: "~/.claude/skills".to_string(),
            detect: vec![DetectMethod::Dir {
                dir: detect_doctored.to_string_lossy().to_string(),
            }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: true,
        };
        let cursor = AgentDef {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            project_path: ".cursor/skills".to_string(),
            global_path: "~/.cursor/skills".to_string(),
            detect: vec![DetectMethod::Dir {
                dir: detect_cursor.to_string_lossy().to_string(),
            }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: false,
        };
        let config = AgentsConfig {
            agents: vec![doctored, cursor],
        };

        // Drive the production filter-and-convert loop directly with the
        // synthetic config so the test does not depend on what is installed on
        // the host.
        let statuses = check_all_doctored(&config, &[InitScope::Project, InitScope::User]);
        let checks: Vec<Check> = statuses
            .iter()
            .filter(|s| s.state != ComponentState::NotApplicable)
            .map(to_check)
            .collect();

        // Positive control: the doctored agent's detection actually fired and
        // produced rows. Without this assertion, an empty `checks` vec would
        // trivially satisfy the "no Cursor" check below — masking a broken
        // detection setup as a passing filter test.
        assert!(
            checks.iter().any(|c| c.name.contains("Claude Code")),
            "doctored agent 'Claude Code' should appear in install-stack checks; \
             missing means detection didn't fire and the filter assertion is vacuous"
        );
        // The actual filter assertion: cursor was detected (so it entered the
        // input set) but `check_all_doctored` excluded it because `doctor: false`.
        for check in &checks {
            assert!(
                !check.name.contains("Cursor"),
                "non-doctored agent 'Cursor' must not appear in install-stack checks; got '{}'",
                check.name
            );
        }
    }

    #[test]
    fn test_check_install_stack_adds_component_checks() {
        let mut doctor = MirdanDoctor::new();
        doctor.check_install_stack();

        // With no agent installed, detection falls back to Claude Code, so the
        // install stack contributes one check per applicable component across the
        // project and user scopes. The exact count depends on which components
        // Claude Code defines paths for, but there must be several, and each must
        // be named in the `Agent · scope · Component` form `status::to_check`
        // produces.
        let checks = doctor.checks();
        assert!(!checks.is_empty(), "install stack should contribute checks");
        assert!(
            checks.iter().all(|c| c.name.contains(" · ")),
            "every install-stack check name should use the ' · ' separator"
        );
        // The fallback agent is Claude Code; its name must appear in the checks.
        assert!(
            checks.iter().any(|c| c.name.contains("Claude Code")),
            "fallback Claude Code agent should appear in install-stack checks"
        );
        // Both scopes should be represented.
        assert!(checks.iter().any(|c| c.name.contains("project")));
        assert!(checks.iter().any(|c| c.name.contains("user")));
    }
}
