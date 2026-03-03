//! Mirdan New - Scaffold a new skill, validator, tool, or plugin package.

use std::fs;
use std::path::PathBuf;

use crate::agents;
use crate::package_type::is_valid_package_name;
use crate::registry::RegistryError;

/// Run the `mirdan new skill` command.
///
/// Creates a skill scaffold following the agentskills.io spec.
pub fn run_new_skill(
    name: &str,
    global: bool,
    agent_filter: Option<&str>,
) -> Result<(), RegistryError> {
    if !is_valid_package_name(name) {
        return Err(RegistryError::Validation(format!(
            "Invalid package name '{}'. Must be 1-64 chars, lowercase alphanumeric with hyphens, \
             no leading/trailing/consecutive hyphens.",
            name
        )));
    }

    let base_dir = if global {
        // Deploy to first matching agent's global skill dir
        let config = agents::load_agents_config()?;
        let agents = agents::resolve_target_agents(&config, agent_filter)?;
        let agent = agents.first().ok_or_else(|| {
            RegistryError::Validation("No agents detected for global skill creation".to_string())
        })?;
        agents::agent_global_skill_dir(&agent.def).join(name)
    } else {
        // Create in current directory
        PathBuf::from(name)
    };

    if base_dir.exists() {
        return Err(RegistryError::Validation(format!(
            "Directory already exists: {}",
            base_dir.display()
        )));
    }

    // Create directory structure per agentskills.io spec
    let scripts_dir = base_dir.join("scripts");
    let references_dir = base_dir.join("references");
    let assets_dir = base_dir.join("assets");
    fs::create_dir_all(&scripts_dir)?;
    fs::create_dir_all(&references_dir)?;
    fs::create_dir_all(&assets_dir)?;

    // Write SKILL.md
    let skill_md = format!(
        r#"---
name: {name}
description: "TODO: Describe what this skill does"
metadata:
  version: "0.1.0"
---

# {name}

TODO: Describe the purpose and usage of this skill.

## What This Skill Does

Explain the capability this skill provides to AI coding agents.

## Usage

Describe when and how an agent should use this skill.
"#,
        name = name
    );
    fs::write(base_dir.join("SKILL.md"), skill_md)?;

    // Write reference file
    let reference_md = format!(
        r#"# {name} Reference

TODO: Add reference documentation, API specs, or other context the agent needs.
"#,
        name = name
    );
    fs::write(references_dir.join("REFERENCE.md"), reference_md)?;

    let scope = if global { "global" } else { "project" };
    println!("Created {} skill '{}':\n", scope, name);
    println!("  {}/", base_dir.display());
    println!("  ├── SKILL.md");
    println!("  ├── scripts/");
    println!("  ├── references/");
    println!("  │   └── REFERENCE.md");
    println!("  └── assets/");
    println!();
    println!("Next steps:");
    println!("  1. Edit SKILL.md to describe the skill");
    println!("  2. Add scripts to scripts/");
    println!("  3. Add reference docs to references/");
    println!("  4. Run 'mirdan publish' when ready to share");

    Ok(())
}

/// Run the `mirdan new validator` command.
///
/// Creates a validator scaffold following the AVP spec.
pub fn run_new_validator(name: &str, global: bool) -> Result<(), RegistryError> {
    if !is_valid_package_name(name) {
        return Err(RegistryError::Validation(format!(
            "Invalid package name '{}'. Must be 1-64 chars, lowercase alphanumeric with hyphens, \
             no leading/trailing/consecutive hyphens.",
            name
        )));
    }

    let base_dir = if global {
        dirs::home_dir()
            .ok_or_else(|| RegistryError::Validation("Could not find home directory".to_string()))?
            .join(".avp")
            .join("validators")
            .join(name)
    } else {
        PathBuf::from(name)
    };

    if base_dir.exists() {
        return Err(RegistryError::Validation(format!(
            "Directory already exists: {}",
            base_dir.display()
        )));
    }

    // Create directory structure
    let rules_dir = base_dir.join("rules");
    fs::create_dir_all(&rules_dir)?;

    // Write VALIDATOR.md
    let validator_md = format!(
        r#"---
name: {name}
description: "TODO: Describe what this RuleSet validates"
metadata:
  version: "0.1.0"
trigger: PostToolUse
match:
  tools: [Write, Edit]
tags: []
---

# {name}

TODO: Describe the purpose and behavior of this RuleSet.
Rules are automatically discovered from the `rules/` directory.
"#,
        name = name
    );
    fs::write(base_dir.join("VALIDATOR.md"), validator_md)?;

    // Write example rule
    let example_rule = r#"---
name: example-rule
description: "An example validation rule"
---

# Example Rule

Check that the code change follows project conventions.

## Validation Criteria

- TODO: Define what this rule checks
- TODO: Define pass/fail conditions

## Instructions

Review the tool input and output. If the change violates the criteria above,
report a failure with a clear explanation.
"#;
    fs::write(rules_dir.join("example.md"), example_rule)?;

    // Write README.md
    let readme = format!(
        r#"# {name}

A validator RuleSet for the Agent Validator Protocol.

## Usage

Install with Mirdan:

```bash
mirdan install {name}
```

## Development

Edit `VALIDATOR.md` to configure the RuleSet metadata.
Add rule files to the `rules/` directory.

When ready to publish:

```bash
mirdan publish
```
"#,
        name = name
    );
    fs::write(base_dir.join("README.md"), readme)?;

    let scope = if global { "global (user)" } else { "project" };
    println!("Created {} validator '{}':\n", scope, name);
    println!("  {}/", base_dir.display());
    println!("  ├── VALIDATOR.md");
    println!("  ├── README.md");
    println!("  └── rules/");
    println!("      └── example.md");
    println!();
    println!("Next steps:");
    println!("  1. Edit VALIDATOR.md to set description, trigger, and match criteria");
    println!("  2. Add rule files to rules/");
    println!("  3. Run 'mirdan publish' when ready to share");

    Ok(())
}

/// Run the `mirdan new tool` command.
///
/// Creates a tool scaffold with TOOL.md (MCP server definition) and README.md.
pub fn run_new_tool(name: &str, global: bool) -> Result<(), RegistryError> {
    if !is_valid_package_name(name) {
        return Err(RegistryError::Validation(format!(
            "Invalid package name '{}'. Must be 1-64 chars, lowercase alphanumeric with hyphens, \
             no leading/trailing/consecutive hyphens.",
            name
        )));
    }

    let base_dir = if global {
        dirs::home_dir()
            .ok_or_else(|| RegistryError::Validation("Could not find home directory".to_string()))?
            .join(".tools")
            .join(name)
    } else {
        PathBuf::from(name)
    };

    if base_dir.exists() {
        return Err(RegistryError::Validation(format!(
            "Directory already exists: {}",
            base_dir.display()
        )));
    }

    fs::create_dir_all(&base_dir)?;

    // Write TOOL.md
    let tool_md = format!(
        r#"---
name: {name}
description: "TODO: Describe what this tool does"
metadata:
  version: "0.1.0"
mcp:
  command: npx
  args:
    - "-y"
    - "@your-scope/{name}"
  transport: stdio
  env: {{}}
---

# {name}

TODO: Describe the purpose and usage of this MCP tool.

## What This Tool Does

Explain the capability this MCP server provides to AI coding agents.

## Configuration

Describe any environment variables or configuration needed.
"#,
        name = name
    );
    fs::write(base_dir.join("TOOL.md"), tool_md)?;

    // Write README.md
    let readme = format!(
        r#"# {name}

An MCP tool for AI coding agents.

## Usage

Install with Mirdan:

```bash
mirdan install {name}
```

## Development

Edit `TOOL.md` to configure the MCP server definition.

When ready to publish:

```bash
mirdan publish
```
"#,
        name = name
    );
    fs::write(base_dir.join("README.md"), readme)?;

    let scope = if global { "global" } else { "project" };
    println!("Created {} tool '{}':\n", scope, name);
    println!("  {}/", base_dir.display());
    println!("  ├── TOOL.md");
    println!("  └── README.md");
    println!();
    println!("Next steps:");
    println!("  1. Edit TOOL.md to configure the MCP server command and args");
    println!("  2. Set environment variables in the mcp.env section");
    println!("  3. Run 'mirdan publish' when ready to share");

    Ok(())
}

/// Run the `mirdan new plugin` command.
///
/// Creates a Claude Code plugin scaffold with .claude-plugin/plugin.json,
/// commands/, skills/, and README.md.
pub fn run_new_plugin(name: &str, global: bool) -> Result<(), RegistryError> {
    if !is_valid_package_name(name) {
        return Err(RegistryError::Validation(format!(
            "Invalid package name '{}'. Must be 1-64 chars, lowercase alphanumeric with hyphens, \
             no leading/trailing/consecutive hyphens.",
            name
        )));
    }

    let base_dir = if global {
        let config = agents::load_agents_config()?;
        let claude = config
            .agents
            .iter()
            .find(|a| a.id == "claude-code")
            .ok_or_else(|| {
                RegistryError::Validation(
                    "Claude Code agent not found in config for global plugin creation".to_string(),
                )
            })?;
        agents::agent_global_plugin_dir(claude)
            .ok_or_else(|| {
                RegistryError::Validation(
                    "Claude Code agent has no global plugin path configured".to_string(),
                )
            })?
            .join(name)
    } else {
        PathBuf::from(name)
    };

    if base_dir.exists() {
        return Err(RegistryError::Validation(format!(
            "Directory already exists: {}",
            base_dir.display()
        )));
    }

    // Create directory structure
    let plugin_meta_dir = base_dir.join(".claude-plugin");
    let commands_dir = base_dir.join("commands");
    let skills_dir = base_dir.join("skills");
    fs::create_dir_all(&plugin_meta_dir)?;
    fs::create_dir_all(&commands_dir)?;
    fs::create_dir_all(&skills_dir)?;

    // Write .claude-plugin/plugin.json
    let plugin_json = serde_json::json!({
        "name": name,
        "description": "TODO: Describe what this plugin does",
        "author": {
            "name": "TODO: Your name"
        }
    });
    fs::write(
        plugin_meta_dir.join("plugin.json"),
        serde_json::to_string_pretty(&plugin_json).unwrap() + "\n",
    )?;

    // Write example command
    let example_cmd = format!(
        r#"---
description: "Example command for {name}"
allowed-tools:
  - Read
  - Glob
  - Grep
---

# Example Command

This is an example slash command provided by the {name} plugin.
Describe what this command does and how it should be used.
"#,
        name = name
    );
    fs::write(commands_dir.join("example.md"), example_cmd)?;

    // Write README.md
    let readme = format!(
        r#"# {name}

A Claude Code plugin.

## Usage

Install with Mirdan:

```bash
mirdan install {name}
```

## Structure

- `.claude-plugin/plugin.json` - Plugin manifest
- `commands/` - Slash commands
- `skills/` - Skills bundled with this plugin

## Development

Edit `plugin.json` to update plugin metadata.
Add commands to `commands/` and skills to `skills/`.

When ready to publish:

```bash
mirdan publish
```
"#,
        name = name
    );
    fs::write(base_dir.join("README.md"), readme)?;

    let scope = if global { "global" } else { "project" };
    println!("Created {} plugin '{}':\n", scope, name);
    println!("  {}/", base_dir.display());
    println!("  ├── .claude-plugin/");
    println!("  │   └── plugin.json");
    println!("  ├── commands/");
    println!("  │   └── example.md");
    println!("  ├── skills/");
    println!("  └── README.md");
    println!();
    println!("Next steps:");
    println!("  1. Edit .claude-plugin/plugin.json to set description and author");
    println!("  2. Add slash commands to commands/");
    println!("  3. Add skills to skills/");
    println!("  4. Run 'mirdan publish' when ready to share");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_skill_creates_structure() {
        let dir = tempfile::tempdir().unwrap();
        let name = "test-skill";
        let skill_dir = dir.path().join(name);

        // Create in the temp dir by changing cwd temporarily
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = run_new_skill(name, false, None);
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("scripts").is_dir());
        assert!(skill_dir.join("references").is_dir());
        assert!(skill_dir.join("references/REFERENCE.md").exists());
        assert!(skill_dir.join("assets").is_dir());
    }

    #[test]
    fn test_new_validator_creates_structure() {
        let dir = tempfile::tempdir().unwrap();
        let name = "test-validator";
        let val_dir = dir.path().join(name);

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = run_new_validator(name, false);
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(val_dir.join("VALIDATOR.md").exists());
        assert!(val_dir.join("README.md").exists());
        assert!(val_dir.join("rules").is_dir());
        assert!(val_dir.join("rules/example.md").exists());
    }

    #[test]
    fn test_new_skill_invalid_name() {
        assert!(run_new_skill("INVALID", false, None).is_err());
        assert!(run_new_skill("", false, None).is_err());
        assert!(run_new_skill("double--hyphen", false, None).is_err());
    }

    #[test]
    fn test_new_validator_invalid_name() {
        assert!(run_new_validator("INVALID", false).is_err());
        assert!(run_new_validator("", false).is_err());
    }

    #[test]
    fn test_new_skill_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let name = "existing-skill";
        std::fs::create_dir(dir.path().join(name)).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = run_new_skill(name, false, None);
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn test_new_tool_creates_structure() {
        let dir = tempfile::tempdir().unwrap();
        let name = "test-tool";
        let tool_dir = dir.path().join(name);

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = run_new_tool(name, false);
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(tool_dir.join("TOOL.md").exists());
        assert!(tool_dir.join("README.md").exists());

        // Verify TOOL.md has mcp section
        let content = std::fs::read_to_string(tool_dir.join("TOOL.md")).unwrap();
        assert!(content.contains("mcp:"));
        assert!(content.contains("command:"));
    }

    #[test]
    fn test_new_tool_invalid_name() {
        assert!(run_new_tool("INVALID", false).is_err());
        assert!(run_new_tool("", false).is_err());
    }

    #[test]
    fn test_new_plugin_creates_structure() {
        let dir = tempfile::tempdir().unwrap();
        let name = "test-plugin";
        let plugin_dir = dir.path().join(name);

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = run_new_plugin(name, false);
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(plugin_dir.join(".claude-plugin/plugin.json").exists());
        assert!(plugin_dir.join("commands/example.md").exists());
        assert!(plugin_dir.join("skills").is_dir());
        assert!(plugin_dir.join("README.md").exists());

        // Verify plugin.json
        let content =
            std::fs::read_to_string(plugin_dir.join(".claude-plugin/plugin.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["name"].as_str().unwrap(), "test-plugin");
    }

    #[test]
    fn test_new_plugin_invalid_name() {
        assert!(run_new_plugin("INVALID", false).is_err());
        assert!(run_new_plugin("", false).is_err());
    }
}
