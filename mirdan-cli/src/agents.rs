//! Agent detection and configuration.
//!
//! Agents are defined in a YAML config file. The default config is embedded
//! at compile time; users can override with ~/.mirdan/agents.yaml or
//! the MIRDAN_AGENTS_CONFIG env var.

use std::path::PathBuf;

use comfy_table::{presets::UTF8_FULL, Table};
use serde::{Deserialize, Serialize};

use crate::registry::RegistryError;

/// Embedded default agents config.
const DEFAULT_AGENTS_YAML: &str = include_str!("agents_default.yaml");

/// Top-level agents config.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentsConfig {
    pub agents: Vec<AgentDef>,
}

/// Policy for computing symlink names when linking from agent skill dirs to the
/// central `.skills/` store.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SymlinkPolicy {
    /// Use only the last path segment (e.g. `"anthropics/skills/algorithmic-art"` → `"algorithmic-art"`).
    /// This is the default, suitable for agents that require flat skill directories.
    #[default]
    LastSegment,
    /// Preserve the full sanitized path (for agents that support subdirectories).
    FullPath,
}

/// A single agent definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub global_path: String,
    pub detect: Vec<DetectMethod>,
    #[serde(default)]
    pub symlink_policy: SymlinkPolicy,
}

/// How to detect if an agent is installed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DetectMethod {
    Dir { dir: String },
    Command { command: String },
}

/// Result of agent detection.
#[derive(Debug, Clone)]
pub struct DetectedAgent {
    pub def: AgentDef,
    pub detected: bool,
    pub detection_detail: String,
}

/// Load agents config from the best available source.
///
/// Priority:
/// 1. MIRDAN_AGENTS_CONFIG env var
/// 2. ~/.mirdan/agents.yaml
/// 3. Embedded default
pub fn load_agents_config() -> Result<AgentsConfig, RegistryError> {
    // Check env var first
    if let Ok(path) = std::env::var("MIRDAN_AGENTS_CONFIG") {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            RegistryError::Validation(format!(
                "Cannot read MIRDAN_AGENTS_CONFIG '{}': {}",
                path, e
            ))
        })?;
        return serde_yaml::from_str(&content).map_err(|e| {
            RegistryError::Validation(format!("Invalid agents config '{}': {}", path, e))
        });
    }

    // Check user override
    if let Some(home) = dirs::home_dir() {
        let user_config = home.join(".mirdan").join("agents.yaml");
        if user_config.exists() {
            let content = std::fs::read_to_string(&user_config)?;
            return serde_yaml::from_str(&content).map_err(|e| {
                RegistryError::Validation(format!(
                    "Invalid agents config '{}': {}",
                    user_config.display(),
                    e
                ))
            });
        }
    }

    // Use embedded default
    serde_yaml::from_str(DEFAULT_AGENTS_YAML)
        .map_err(|e| RegistryError::Validation(format!("Invalid embedded agents config: {}", e)))
}

/// Expand ~ to home directory in a path string.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Detect which agents are installed.
///
/// ANY match in detect methods means the agent is present.
pub fn detect_agents(config: &AgentsConfig) -> Vec<DetectedAgent> {
    config
        .agents
        .iter()
        .map(|def| {
            let mut detected = false;
            let mut detail = String::new();

            for method in &def.detect {
                match method {
                    DetectMethod::Dir { dir } => {
                        let expanded = expand_tilde(dir);
                        if expanded.exists() {
                            detected = true;
                            detail = format!("{}", expanded.display());
                            break;
                        }
                    }
                    DetectMethod::Command { command } => {
                        if which::which(command).is_ok() {
                            detected = true;
                            detail = format!("{} (command)", command);
                            break;
                        }
                    }
                }
            }

            if !detected {
                // Show what we looked for
                let dirs: Vec<String> = def
                    .detect
                    .iter()
                    .filter_map(|m| match m {
                        DetectMethod::Dir { dir } => Some(expand_tilde(dir).display().to_string()),
                        _ => None,
                    })
                    .collect();
                if !dirs.is_empty() {
                    detail = dirs[0].clone();
                }
            }

            DetectedAgent {
                def: def.clone(),
                detected,
                detection_detail: detail,
            }
        })
        .collect()
}

/// Get only the detected agents (with fallback to claude-code).
pub fn get_detected_agents(config: &AgentsConfig) -> Vec<DetectedAgent> {
    let all = detect_agents(config);
    let detected: Vec<_> = all.into_iter().filter(|a| a.detected).collect();

    if detected.is_empty() {
        // Fallback: assume claude-code
        if let Some(claude) = config.agents.iter().find(|a| a.id == "claude-code") {
            return vec![DetectedAgent {
                def: claude.clone(),
                detected: true,
                detection_detail: "fallback (no agents detected)".to_string(),
            }];
        }
    }

    detected
}

/// Resolve the project-level skill directory for an agent.
pub fn agent_project_skill_dir(agent: &AgentDef) -> PathBuf {
    PathBuf::from(&agent.project_path)
}

/// Resolve the global skill directory for an agent.
pub fn agent_global_skill_dir(agent: &AgentDef) -> PathBuf {
    expand_tilde(&agent.global_path)
}

/// Run the `mirdan agents` command.
pub fn run_agents(all: bool, json: bool) -> Result<(), RegistryError> {
    let config = load_agents_config()?;
    let agents = detect_agents(&config);

    let detected_count = agents.iter().filter(|a| a.detected).count();
    let total = agents.len();

    if json {
        let entries: Vec<serde_json::Value> = agents
            .iter()
            .filter(|a| all || a.detected)
            .map(|a| {
                serde_json::json!({
                    "id": a.def.id,
                    "name": a.def.name,
                    "detected": a.detected,
                    "detail": a.detection_detail,
                })
            })
            .collect();
        let output = serde_json::json!({
            "agents": entries,
            "detected": detected_count,
            "total": total,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return Ok(());
    }

    println!("Detected Agents:\n");

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Agent", "Path / Command", "Status"]);

    for agent in &agents {
        if !all && !agent.detected {
            continue;
        }
        let status = if agent.detected {
            "detected"
        } else {
            "not found"
        };
        table.add_row(vec![
            agent.def.name.clone(),
            agent.detection_detail.clone(),
            status.to_string(),
        ]);
    }

    println!("{table}");

    if all {
        println!("\n{} of {} agents detected.", detected_count, total);
    } else {
        println!(
            "\n{} of {} agents detected. Use --all to show all.",
            detected_count, total
        );
    }

    Ok(())
}

/// Resolve which agents to target, honoring an optional --agent filter.
pub fn resolve_target_agents(
    config: &AgentsConfig,
    agent_filter: Option<&str>,
) -> Result<Vec<DetectedAgent>, RegistryError> {
    let Some(filter_id) = agent_filter else {
        return Ok(get_detected_agents(config));
    };

    let all = detect_agents(config);
    let found = all.iter().find(|a| a.def.id == filter_id);
    match found {
        Some(agent) => Ok(vec![agent.clone()]),
        None => Err(RegistryError::Validation(format!(
            "Unknown agent '{}'. Run 'mirdan agents --all' to see available agents.",
            filter_id
        ))),
    }
}

/// Validate that an agent ID is known in the agents config.
///
/// Returns Ok(()) if valid, or an error listing all valid agent IDs.
pub fn validate_agent_id(config: &AgentsConfig, agent_id: &str) -> Result<(), RegistryError> {
    if config.agents.iter().any(|a| a.id == agent_id) {
        return Ok(());
    }

    let valid_ids: Vec<&str> = config.agents.iter().map(|a| a.id.as_str()).collect();
    Err(RegistryError::Validation(format!(
        "Unknown agent '{}'. Valid agents: {}",
        agent_id,
        valid_ids.join(", ")
    )))
}

/// Expand tilde in a path string (public for use by other modules).
pub fn expand_home(path: &str) -> PathBuf {
    expand_tilde(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_agents() {
        let config = load_agents_config().unwrap();
        assert!(!config.agents.is_empty());
        assert!(config.agents.len() >= 37);
    }

    #[test]
    fn test_first_agent_is_claude_code() {
        let config = load_agents_config().unwrap();
        assert_eq!(config.agents[0].id, "claude-code");
        assert_eq!(config.agents[0].name, "Claude Code");
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/.claude");
        assert!(!expanded.to_string_lossy().starts_with('~'));
        assert!(expanded.to_string_lossy().ends_with(".claude"));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_detect_agents() {
        let config = load_agents_config().unwrap();
        let agents = detect_agents(&config);
        assert_eq!(agents.len(), config.agents.len());
    }

    #[test]
    fn test_get_detected_agents_fallback() {
        // With a config that has no detectable agents, should fallback to claude-code
        let config = AgentsConfig {
            agents: vec![AgentDef {
                id: "claude-code".to_string(),
                name: "Claude Code".to_string(),
                project_path: ".claude/skills".to_string(),
                global_path: "~/.claude/skills".to_string(),
                detect: vec![DetectMethod::Dir {
                    dir: "/nonexistent/path/that/should/not/exist".to_string(),
                }],
                symlink_policy: SymlinkPolicy::default(),
            }],
        };
        let detected = get_detected_agents(&config);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].def.id, "claude-code");
    }

    #[test]
    fn test_agent_project_skill_dir() {
        let def = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
        };
        assert_eq!(agent_project_skill_dir(&def), PathBuf::from(".test/skills"));
    }

    fn mock_config() -> AgentsConfig {
        AgentsConfig {
            agents: vec![
                AgentDef {
                    id: "claude-code".to_string(),
                    name: "Claude Code".to_string(),
                    project_path: ".claude/skills".to_string(),
                    global_path: "~/.claude/skills".to_string(),
                    detect: vec![DetectMethod::Dir {
                        dir: "/nonexistent/path/that/should/not/exist".to_string(),
                    }],
                    symlink_policy: SymlinkPolicy::default(),
                },
                AgentDef {
                    id: "cursor".to_string(),
                    name: "Cursor".to_string(),
                    project_path: ".cursor/skills".to_string(),
                    global_path: "~/.cursor/skills".to_string(),
                    detect: vec![DetectMethod::Dir {
                        dir: "/nonexistent/cursor/path".to_string(),
                    }],
                    symlink_policy: SymlinkPolicy::default(),
                },
            ],
        }
    }

    #[test]
    fn test_validate_agent_id_valid() {
        let config = mock_config();
        assert!(validate_agent_id(&config, "claude-code").is_ok());
        assert!(validate_agent_id(&config, "cursor").is_ok());
    }

    #[test]
    fn test_validate_agent_id_invalid() {
        let config = mock_config();
        let err = validate_agent_id(&config, "nonexistent").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "error should contain the invalid name"
        );
        assert!(msg.contains("claude-code"), "error should list valid IDs");
        assert!(msg.contains("cursor"), "error should list valid IDs");
    }

    #[test]
    fn test_resolve_target_agents_none_filter() {
        let config = mock_config();
        let result = resolve_target_agents(&config, None).unwrap();
        // With None filter, returns detected agents (falls back to claude-code)
        assert!(!result.is_empty());
        assert_eq!(result[0].def.id, "claude-code");
    }

    #[test]
    fn test_resolve_target_agents_some_filter() {
        let config = mock_config();
        let result = resolve_target_agents(&config, Some("claude-code")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].def.id, "claude-code");
    }

    #[test]
    fn test_resolve_target_agents_unknown() {
        let config = mock_config();
        let result = resolve_target_agents(&config, Some("nonexistent"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("nonexistent"));
    }

    #[test]
    fn test_expand_tilde_bare() {
        let expanded = expand_tilde("~");
        // Bare "~" without trailing "/" should remain unchanged (no strip_prefix match)
        assert_eq!(expanded, PathBuf::from("~"));
    }

    #[test]
    fn test_agents_yaml_parsing() {
        let yaml = r#"
agents:
  - id: test-agent
    name: Test Agent
    project_path: .test/skills
    global_path: "~/.test/skills"
    detect:
      - dir: "~/.test"
      - command: test-cmd
"#;
        let config: AgentsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].id, "test-agent");
        assert_eq!(config.agents[0].detect.len(), 2);
    }
}
