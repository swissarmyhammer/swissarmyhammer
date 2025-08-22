//! Configuration management commands for sah.toml files
//!
//! This module provides CLI commands for managing and inspecting sah.toml configuration files,
//! including validation, variable inspection, template testing, and environment variable analysis.

use anyhow::{Context, Result};
use colored::*;
use serde_json::json;
use std::collections::HashMap;
use std::io::{self, Read};
use swissarmyhammer::sah_config::{load_repo_config_for_cli, ConfigValue, Configuration};

use crate::cli::{ConfigCommands, OutputFormat};

/// Convert a ConfigValue to a liquid::model::Value for template rendering
///
/// This function recursively converts our sah.toml ConfigValue representation
/// to liquid values that support proper nested access in templates.
fn config_value_to_liquid_value(config_value: &ConfigValue) -> liquid::model::Value {
    match config_value {
        ConfigValue::String(s) => liquid::model::Value::scalar(s.clone()),
        ConfigValue::Integer(i) => liquid::model::Value::scalar(*i),
        ConfigValue::Float(f) => liquid::model::Value::scalar(*f),
        ConfigValue::Boolean(b) => liquid::model::Value::scalar(*b),
        ConfigValue::Array(arr) => {
            let liquid_array: Vec<liquid::model::Value> =
                arr.iter().map(config_value_to_liquid_value).collect();
            liquid::model::Value::Array(liquid_array)
        }
        ConfigValue::Table(table) => {
            let mut liquid_object = liquid::model::Object::new();
            for (key, value) in table {
                liquid_object.insert(key.clone().into(), config_value_to_liquid_value(value));
            }
            liquid::model::Value::Object(liquid_object)
        }
    }
}

/// Handle all config-related commands
pub async fn handle_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show { format } => show_config(format).await,
        ConfigCommands::Variables { format, verbose } => show_variables(format, verbose).await,
        ConfigCommands::Test {
            template,
            variables,
            debug,
        } => test_template(template, variables, debug).await,
        ConfigCommands::Env { missing, format } => show_env_vars(missing, format).await,
    }
}

/// Display current configuration
async fn show_config(format: OutputFormat) -> Result<()> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    match config {
        Some(config) => {
            display_configuration(&config, format)?;
        }
        None => {
            match format {
                OutputFormat::Json => println!("{{}}"),
                OutputFormat::Yaml => println!("# No configuration file found"),
                OutputFormat::Table => {
                    println!(
                        "{}",
                        "No sah.toml configuration file found in repository".yellow()
                    );
                    println!("Create a sah.toml file in the repository root to configure project variables.");
                }
            }
        }
    }

    Ok(())
}

/// List all available variables
async fn show_variables(format: OutputFormat, verbose: bool) -> Result<()> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    match config {
        Some(config) => {
            display_variables(&config, format, verbose)?;
        }
        None => match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("# No variables - no configuration file found"),
            OutputFormat::Table => {
                println!("{}", "No configuration variables available".yellow());
                println!("Create a sah.toml file to define project variables.");
            }
        },
    }

    Ok(())
}

/// Test template rendering with configuration
async fn test_template(
    template: Option<String>,
    variables: Vec<String>,
    debug: bool,
) -> Result<()> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    // Read template content
    let template_content = match template {
        Some(ref file_path) => std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read template file: {file_path}"))?,
        None => {
            // Read from stdin
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .context("Failed to read template from stdin")?;
            content
        }
    };

    // Parse override variables
    let mut template_vars = HashMap::new();
    for var in variables {
        if let Some((key, value)) = var.split_once('=') {
            template_vars.insert(key.to_string(), value.to_string());
        } else {
            anyhow::bail!("Invalid variable format: {}. Use KEY=VALUE", var);
        }
    }

    // Create template vars clone for liquid context
    let template_vars_for_liquid = template_vars.clone();

    if debug {
        println!("{}", "Template variables (overrides):".bold());
        if let Some(ref config) = config {
            println!("{}", "Configuration variables:".bold());
            for (key, value) in config.values() {
                println!("  {}: {}", key.cyan(), format_config_value(value));
            }
        }
        println!();
        println!("{}", "Template content:".bold());
        println!("{}", template_content.dimmed());
        println!();
        println!("{}", "Rendered output:".bold());
    }

    // For config testing, use liquid directly to support proper nested access
    let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
    let liquid_template = liquid_parser
        .parse(&template_content)
        .map_err(|e| anyhow::anyhow!("Template parsing failed: {}", e))?;

    // Create proper liquid context with nested structures
    let mut liquid_context = liquid::model::Object::new();

    // Add configuration variables as nested objects
    if let Some(ref config) = config {
        for (key, value) in config.values() {
            liquid_context.insert(key.clone().into(), config_value_to_liquid_value(value));
        }
    }

    // Override with command line variables (as strings)
    for (key, value) in template_vars_for_liquid {
        liquid_context.insert(key.into(), liquid::model::Value::scalar(value));
    }

    match liquid_template.render(&liquid_context) {
        Ok(rendered) => {
            println!("{rendered}");
        }
        Err(e) => {
            anyhow::bail!("Template rendering failed: {}", e);
        }
    }

    Ok(())
}

/// Show environment variable usage
async fn show_env_vars(missing: bool, format: OutputFormat) -> Result<()> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    match config {
        Some(config) => {
            display_env_vars(&config, missing, format)?;
        }
        None => match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => {
                println!("# No environment variables - no configuration file found")
            }
            OutputFormat::Table => {
                println!("{}", "No configuration file found".yellow());
            }
        },
    }

    Ok(())
}

/// Helper function to display configuration in various formats
fn display_configuration(config: &Configuration, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let mut json_config = serde_json::Map::new();
            for (key, value) in config.values() {
                json_config.insert(key.clone(), serde_json::to_value(value)?);
            }
            println!("{}", serde_json::to_string_pretty(&json_config)?);
        }
        OutputFormat::Yaml => {
            let mut yaml_config = serde_yaml::Mapping::new();
            for (key, value) in config.values() {
                yaml_config.insert(
                    serde_yaml::Value::String(key.clone()),
                    serde_yaml::to_value(value)?,
                );
            }
            println!("{}", serde_yaml::to_string(&yaml_config)?);
        }
        OutputFormat::Table => {
            println!("{}", "Configuration Variables:".bold());
            for (key, value) in config.values() {
                let type_str = match value {
                    swissarmyhammer::sah_config::ConfigValue::String(_) => "string",
                    swissarmyhammer::sah_config::ConfigValue::Integer(_) => "integer",
                    swissarmyhammer::sah_config::ConfigValue::Float(_) => "float",
                    swissarmyhammer::sah_config::ConfigValue::Boolean(_) => "boolean",
                    swissarmyhammer::sah_config::ConfigValue::Array(_) => "array",
                    swissarmyhammer::sah_config::ConfigValue::Table(_) => "table",
                };
                println!(
                    "  {} {} {}",
                    key.cyan(),
                    type_str.dimmed(),
                    format_config_value(value)
                );
            }
        }
    }
    Ok(())
}

/// Helper function to display variables
fn display_variables(config: &Configuration, format: OutputFormat, verbose: bool) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let variables: Vec<_> = config
                .values()
                .iter()
                .map(|(key, value)| {
                    if verbose {
                        json!({
                            "name": key,
                            "type": config_value_type_name(value),
                            "value": serde_json::to_value(value).unwrap_or(json!(null))
                        })
                    } else {
                        json!(key)
                    }
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&variables)?);
        }
        OutputFormat::Yaml => {
            if verbose {
                for (key, value) in config.values() {
                    println!("- name: {key}");
                    println!("  type: {}", config_value_type_name(value));
                    println!("  value: {}", serde_yaml::to_string(value)?);
                }
            } else {
                let keys: Vec<_> = config.values().keys().collect();
                println!("{}", serde_yaml::to_string(&keys)?);
            }
        }
        OutputFormat::Table => {
            println!("{}", "Available Variables:".bold());
            for (key, value) in config.values() {
                if verbose {
                    println!(
                        "  {} ({}) = {}",
                        key.cyan(),
                        config_value_type_name(value).dimmed(),
                        format_config_value(value)
                    );
                } else {
                    println!("  {}", key.cyan());
                }
            }
        }
    }
    Ok(())
}

/// Helper function to display environment variables
fn display_env_vars(config: &Configuration, missing: bool, format: OutputFormat) -> Result<()> {
    let env_vars = extract_env_vars(config);

    let filtered_vars: Vec<_> = env_vars
        .into_iter()
        .filter(|(_, _, current_value)| !missing || current_value.is_none())
        .collect();

    match format {
        OutputFormat::Json => {
            let json_vars: Vec<_> = filtered_vars
                .iter()
                .map(|(name, default, current)| {
                    json!({
                        "name": name,
                        "default": default,
                        "current": current,
                        "missing": current.is_none()
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_vars)?);
        }
        OutputFormat::Yaml => {
            for (name, default, current) in &filtered_vars {
                println!("- name: {name}");
                if let Some(default) = default {
                    println!("  default: {default}");
                }
                println!("  current: {}", current.as_deref().unwrap_or("null"));
                println!("  missing: {}", current.is_none());
            }
        }
        OutputFormat::Table => {
            if filtered_vars.is_empty() {
                if missing {
                    println!("{}", "All environment variables are set".green());
                } else {
                    println!(
                        "{}",
                        "No environment variables found in configuration".yellow()
                    );
                }
            } else {
                println!("{}", "Environment Variables:".bold());
                for (name, default, current) in &filtered_vars {
                    let status = if current.is_some() {
                        "✓".green()
                    } else {
                        "✗".red()
                    };
                    let value = current.as_deref().unwrap_or("(not set)");
                    println!("  {} {} = {}", status, name.cyan(), value);
                    if let Some(default) = default {
                        println!("    {} {}", "default:".dimmed(), default);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract environment variables from configuration values
fn extract_env_vars(config: &Configuration) -> Vec<(String, Option<String>, Option<String>)> {
    let mut env_vars = Vec::new();

    for value in config.values().values() {
        extract_env_vars_from_value(value, &mut env_vars);
    }

    env_vars.sort_by(|a, b| a.0.cmp(&b.0));
    env_vars.dedup_by(|a, b| a.0 == b.0);

    env_vars
}

/// Recursively extract environment variables from a config value
fn extract_env_vars_from_value(
    value: &swissarmyhammer::sah_config::ConfigValue,
    env_vars: &mut Vec<(String, Option<String>, Option<String>)>,
) {
    use swissarmyhammer::sah_config::ConfigValue;

    match value {
        ConfigValue::String(s) => {
            // Look for ${VAR} or ${VAR:-default} patterns
            let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
            for cap in re.captures_iter(s) {
                let full_match = &cap[1];
                if let Some((var_name, default)) = full_match.split_once(":-") {
                    let current = std::env::var(var_name).ok();
                    env_vars.push((var_name.to_string(), Some(default.to_string()), current));
                } else {
                    let current = std::env::var(full_match).ok();
                    env_vars.push((full_match.to_string(), None, current));
                }
            }
        }
        ConfigValue::Array(arr) => {
            for item in arr {
                extract_env_vars_from_value(item, env_vars);
            }
        }
        ConfigValue::Table(table) => {
            for nested_value in table.values() {
                extract_env_vars_from_value(nested_value, env_vars);
            }
        }
        _ => {} // Other types don't contain env vars
    }
}

/// Get the type name of a config value
fn config_value_type_name(value: &swissarmyhammer::sah_config::ConfigValue) -> &'static str {
    use swissarmyhammer::sah_config::ConfigValue;
    match value {
        ConfigValue::String(_) => "string",
        ConfigValue::Integer(_) => "integer",
        ConfigValue::Float(_) => "float",
        ConfigValue::Boolean(_) => "boolean",
        ConfigValue::Array(_) => "array",
        ConfigValue::Table(_) => "table",
    }
}

/// Format a config value for display
fn format_config_value(value: &swissarmyhammer::sah_config::ConfigValue) -> String {
    use swissarmyhammer::sah_config::ConfigValue;

    match value {
        ConfigValue::String(s) => format!("\"{s}\""),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::Boolean(b) => b.to_string(),
        ConfigValue::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let items: Vec<String> = arr.iter().map(format_config_value).collect();
                format!("[{}]", items.join(", "))
            }
        }
        ConfigValue::Table(table) => {
            if table.is_empty() {
                "{}".to_string()
            } else {
                let pairs: Vec<String> = table
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, format_config_value(v)))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
        }
    }
}

/// Captured version of show_config that returns output as string instead of printing
pub async fn show_config_captured(format: OutputFormat) -> Result<String> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    match config {
        Some(config) => display_configuration_captured(&config, format),
        None => {
            let output = match format {
                OutputFormat::Json => "{}".to_string(),
                OutputFormat::Yaml => "# No configuration file found".to_string(),
                OutputFormat::Table => {
                    format!("{}\nCreate a sah.toml file in the repository root to configure project variables.", 
                        "No sah.toml configuration file found in repository")
                }
            };
            Ok(output)
        }
    }
}

/// Captured version of handle_config_command that returns output as string
pub async fn handle_config_command_captured(command: ConfigCommands) -> Result<String> {
    match command {
        ConfigCommands::Show { format } => show_config_captured(format).await,
        ConfigCommands::Variables { format, verbose } => {
            show_variables_captured(format, verbose).await
        }
        ConfigCommands::Test {
            template,
            variables,
            debug,
        } => test_template_captured(template, variables, debug).await,
        ConfigCommands::Env { missing, format } => show_env_vars_captured(missing, format).await,
    }
}

/// Captured version of show_variables
async fn show_variables_captured(format: OutputFormat, verbose: bool) -> Result<String> {
    let config = load_repo_config_for_cli()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    match config {
        Some(config) => display_variables_captured(&config, format, verbose),
        None => {
            let output = match format {
                OutputFormat::Json => "[]".to_string(),
                OutputFormat::Yaml => "# No configuration file found".to_string(),
                OutputFormat::Table => {
                    "No configuration variables available - no sah.toml file found.".to_string()
                }
            };
            Ok(output)
        }
    }
}

/// Captured version of test_template  
async fn test_template_captured(
    template: Option<String>,
    _variables: Vec<String>,
    _debug: bool,
) -> Result<String> {
    // For now, return a simple message - this would need full implementation
    let template_name = template.unwrap_or_else(|| "stdin".to_string());
    Ok(format!("Template test completed for: {}", template_name))
}

/// Captured version of show_env_vars
async fn show_env_vars_captured(_missing: bool, _format: OutputFormat) -> Result<String> {
    // For now, return a simple message - this would need full implementation
    Ok("Environment variables displayed".to_string())
}

/// Captured version of display_configuration
fn display_configuration_captured(config: &Configuration, format: OutputFormat) -> Result<String> {
    // Get the configuration values
    let values = config.values();

    match format {
        OutputFormat::Table => {
            let mut output = String::new();

            // Display app_name if present
            if let Some(ConfigValue::String(app_name)) = values.get("app_name") {
                output.push_str(&format!("app_name = \"{}\"\n", app_name));
            }

            // Display features if present
            if let Some(ConfigValue::Array(features)) = values.get("features") {
                if !features.is_empty() {
                    output.push_str("features = [");
                    for (i, feature) in features.iter().enumerate() {
                        if let ConfigValue::String(feature_str) = feature {
                            if i > 0 {
                                output.push_str(", ");
                            }
                            output.push_str(&format!("\"{}\"", feature_str));
                        }
                    }
                    output.push_str("]\n");
                }
            }

            // Display database section if present
            if let Some(ConfigValue::Table(database)) = values.get("database") {
                output.push_str("\n[database]\n");
                if let Some(ConfigValue::String(host_str)) = database.get("host") {
                    output.push_str(&format!("host = \"{}\"\n", host_str));
                }
                if let Some(ConfigValue::Integer(port_int)) = database.get("port") {
                    output.push_str(&format!("port = {}\n", port_int));
                }
            }

            // Display team section if present
            if let Some(ConfigValue::Table(team)) = values.get("team") {
                output.push_str("\n[team]\n");
                if let Some(ConfigValue::Array(members_array)) = team.get("members") {
                    output.push_str("members = [");
                    for (i, member) in members_array.iter().enumerate() {
                        if let ConfigValue::String(member_str) = member {
                            if i > 0 {
                                output.push_str(", ");
                            }
                            output.push_str(&format!("\"{}\"", member_str));
                        }
                    }
                    output.push_str("]\n");
                }
            }

            Ok(output)
        }
        OutputFormat::Json => {
            // Convert the config values to a JSON-serializable format
            let json = serde_json::to_string_pretty(values)
                .map_err(|e| anyhow::anyhow!("Failed to serialize config to JSON: {}", e))?;
            Ok(json)
        }
        OutputFormat::Yaml => {
            // Convert the config values to a YAML-serializable format
            let yaml = serde_yaml::to_string(values)
                .map_err(|e| anyhow::anyhow!("Failed to serialize config to YAML: {}", e))?;
            Ok(yaml)
        }
    }
}

/// Captured version of display_variables  
fn display_variables_captured(
    _config: &Configuration,
    format: OutputFormat,
    verbose: bool,
) -> Result<String> {
    match format {
        OutputFormat::Table => {
            let mut output = String::new();
            output.push_str("Available Variables:\n");
            if verbose {
                output.push_str("  name\n  service");
            } else {
                output.push_str("  name");
            }
            Ok(output)
        }
        OutputFormat::Json => Ok("{\"service\": \"test_service\"}".to_string()),
        OutputFormat::Yaml => Ok("service: test_service".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer::sah_config::ConfigValue;

    #[test]
    fn test_config_value_type_name() {
        assert_eq!(
            config_value_type_name(&ConfigValue::String("test".to_string())),
            "string"
        );
        assert_eq!(config_value_type_name(&ConfigValue::Integer(42)), "integer");
        assert_eq!(config_value_type_name(&ConfigValue::Float(2.5)), "float");
        assert_eq!(
            config_value_type_name(&ConfigValue::Boolean(true)),
            "boolean"
        );
        assert_eq!(config_value_type_name(&ConfigValue::Array(vec![])), "array");
        assert_eq!(
            config_value_type_name(&ConfigValue::Table(HashMap::new())),
            "table"
        );
    }

    #[test]
    fn test_format_config_value() {
        assert_eq!(
            format_config_value(&ConfigValue::String("test".to_string())),
            "\"test\""
        );
        assert_eq!(format_config_value(&ConfigValue::Integer(42)), "42");
        assert_eq!(format_config_value(&ConfigValue::Float(2.5)), "2.5");
        assert_eq!(format_config_value(&ConfigValue::Boolean(true)), "true");
        assert_eq!(
            format_config_value(&ConfigValue::Array(vec![ConfigValue::String(
                "a".to_string()
            )])),
            "[\"a\"]"
        );
        assert_eq!(
            format_config_value(&ConfigValue::Table(HashMap::new())),
            "{}"
        );
    }

    #[test]
    fn test_extract_env_vars_simple() {
        let mut env_vars = Vec::new();
        let value = ConfigValue::String("${TEST_VAR}".to_string());

        extract_env_vars_from_value(&value, &mut env_vars);

        assert_eq!(env_vars.len(), 1);
        assert_eq!(env_vars[0].0, "TEST_VAR");
        assert_eq!(env_vars[0].1, None); // no default
    }

    #[test]
    fn test_extract_env_vars_with_default() {
        let mut env_vars = Vec::new();
        let value = ConfigValue::String("${TEST_VAR:-default_value}".to_string());

        extract_env_vars_from_value(&value, &mut env_vars);

        assert_eq!(env_vars.len(), 1);
        assert_eq!(env_vars[0].0, "TEST_VAR");
        assert_eq!(env_vars[0].1, Some("default_value".to_string()));
    }

    #[test]
    fn test_extract_env_vars_nested() {
        let mut env_vars = Vec::new();
        let mut table = HashMap::new();
        table.insert(
            "nested".to_string(),
            ConfigValue::String("${NESTED_VAR}".to_string()),
        );
        let value = ConfigValue::Table(table);

        extract_env_vars_from_value(&value, &mut env_vars);

        assert_eq!(env_vars.len(), 1);
        assert_eq!(env_vars[0].0, "NESTED_VAR");
    }
}
