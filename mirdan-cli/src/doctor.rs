//! Mirdan Doctor - Diagnostic checks for Mirdan setup and configuration.
//!
//! Checks:
//! 1. Mirdan binary in PATH
//! 2. Agents detected
//! 3. AVP directory exists
//! 4. Registry reachable
//! 5. Credentials valid

use std::env;
use std::path::PathBuf;

use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};

use crate::agents;
use crate::registry::get_registry_url;

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
        self.check_avp_directory();
        self.check_registry_reachable().await;
        self.check_credentials();

        self.get_exit_code()
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
                    fix: Some("Check ~/.mirdan/agents.yaml for syntax errors".to_string()),
                });
            }
        }
    }

    /// Check if .avp/ directory exists for validators.
    fn check_avp_directory(&mut self) {
        let local_avp = PathBuf::from(".avp");
        let global_avp = dirs::home_dir().map(|h| h.join(".avp"));

        let local_exists = local_avp.exists();
        let global_exists = global_avp.as_ref().is_some_and(|p| p.exists());

        if local_exists || global_exists {
            let mut locations = Vec::new();
            if local_exists {
                locations.push("project (.avp/)");
            }
            if global_exists {
                locations.push("global (~/.avp/)");
            }
            self.add_check(Check {
                name: "AVP Directory".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found: {}", locations.join(", ")),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "AVP Directory".to_string(),
                status: CheckStatus::Warning,
                message: "No .avp directory found".to_string(),
                fix: Some(
                    "Run 'mirdan new validator <name>' or 'mirdan install <package>' to create one"
                        .to_string(),
                ),
            });
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
                    "~/.mirdan/credentials"
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
                    fix: Some(
                        "Run 'mirdan login' to authenticate with the registry".to_string(),
                    ),
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
    fn test_check_avp_directory() {
        let mut doctor = MirdanDoctor::new();
        doctor.check_avp_directory();
        assert_eq!(doctor.checks().len(), 1);
        assert_eq!(doctor.checks()[0].name, "AVP Directory");
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
}
