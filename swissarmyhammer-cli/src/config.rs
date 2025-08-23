//! Configuration management commands for sah.toml files
//!
//! This module provides CLI commands for managing and inspecting sah.toml configuration files,
//! including validation, variable inspection, and environment variable analysis.

use anyhow::Result;
use colored::*;
use serde_json::json;

use swissarmyhammer_config::{ConfigProvider, TemplateContext};

use crate::cli::{ConfigCommands, OutputFormat};

/// Handle all config-related commands
pub async fn handle_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show { format } => show_config(format).await,
        ConfigCommands::Variables { format, verbose } => show_variables(format, verbose).await,
        ConfigCommands::Env { missing, format } => show_env_vars(missing, format).await,
    }
}

/// Display current configuration
async fn show_config(format: OutputFormat) -> Result<()> {
    let provider = ConfigProvider::new();
    let template_context = provider
        .load_template_context()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    if template_context.is_empty() {
        match format {
            OutputFormat::Json => println!("{{}}"),
            OutputFormat::Yaml => println!("# No configuration file found"),
            OutputFormat::Table => {
                println!(
                    "{}",
                    "No sah.toml configuration file found in repository".yellow()
                );
                println!(
                    "Create a sah.toml file in the repository root to configure project variables."
                );
            }
        }
    } else {
        display_configuration(&template_context, format)?;
    }

    Ok(())
}

/// Show configuration variables
async fn show_variables(format: OutputFormat, verbose: bool) -> Result<()> {
    let provider = ConfigProvider::new();
    let template_context = provider
        .load_template_context()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    if template_context.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("# No variables - no configuration file found"),
            OutputFormat::Table => {
                println!("{}", "No configuration variables available".yellow());
                println!("Create a sah.toml file to define project variables.");
            }
        }
    } else {
        display_variables(&template_context, format, verbose)?;
    }

    Ok(())
}

/// Show environment variable usage
async fn show_env_vars(missing: bool, format: OutputFormat) -> Result<()> {
    let provider = ConfigProvider::new();
    let template_context = provider
        .load_template_context()
        .map_err(|e| anyhow::anyhow!("Failed to load repository configuration: {}", e))?;

    if template_context.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => {
                println!("# No environment variables - no configuration file found")
            }
            OutputFormat::Table => {
                println!("{}", "No configuration file found".yellow());
            }
        }
    } else {
        display_env_vars(&template_context, missing, format)?;
    }

    Ok(())
}

/// Helper function to display configuration in various formats
fn display_configuration(config: &TemplateContext, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(config.vars())?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(config.vars())?);
        }
        OutputFormat::Table => {
            println!("{}", "Configuration Variables:".bold());
            for (key, value) in config.vars() {
                let type_str = json_value_type_name(value);
                println!(
                    "  {} {} {}",
                    key.cyan(),
                    type_str.dimmed(),
                    format_json_value(value)
                );
            }
        }
    }
    Ok(())
}

/// Display variables with optional verbose information
fn display_variables(config: &TemplateContext, format: OutputFormat, verbose: bool) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let variables: Vec<_> = config
                .vars()
                .iter()
                .map(|(key, value)| {
                    if verbose {
                        json!({
                            "name": key,
                            "type": json_value_type_name(value),
                            "value": value
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
                for (key, value) in config.vars() {
                    println!("- name: {key}");
                    println!("  type: {}", json_value_type_name(value));
                    println!("  value: {}", serde_yaml::to_string(value)?);
                }
            } else {
                let keys: Vec<_> = config.vars().keys().collect();
                println!("{}", serde_yaml::to_string(&keys)?);
            }
        }
        OutputFormat::Table => {
            println!("{}", "Available Variables:".bold());
            for (key, value) in config.vars() {
                if verbose {
                    println!(
                        "  {} ({}) = {}",
                        key.cyan(),
                        json_value_type_name(value).dimmed(),
                        format_json_value(value)
                    );
                } else {
                    println!("  {}", key.cyan());
                }
            }
        }
    }
    Ok(())
}

/// Display environment variables found in configuration
fn display_env_vars(config: &TemplateContext, missing: bool, format: OutputFormat) -> Result<()> {
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
                if let Some(def) = default {
                    println!("  default: {def}");
                }
                println!("  current: {}", current.as_deref().unwrap_or("null"));
                println!("  missing: {}", current.is_none());
            }
        }
        OutputFormat::Table => {
            if missing {
                println!("{}", "Missing Environment Variables:".bold().red());
            } else {
                println!("{}", "Environment Variables Used:".bold());
            }

            for (name, default, current) in &filtered_vars {
                let status = if current.is_some() {
                    "✓".green()
                } else {
                    "✗".red()
                };

                print!("  {} {}", status, name.cyan());
                if let Some(def) = default {
                    print!(" (default: {})", def.dimmed());
                }
                if let Some(curr) = current {
                    println!(" = {}", curr);
                } else {
                    println!(" = {}", "NOT SET".red());
                }
            }

            if filtered_vars.is_empty() {
                if missing {
                    println!("  {}", "All environment variables are set!".green());
                } else {
                    println!(
                        "  {}",
                        "No environment variables used in configuration".dimmed()
                    );
                }
            }
        }
    }
    Ok(())
}

/// Extract environment variable references from configuration values
fn extract_env_vars(config: &TemplateContext) -> Vec<(String, Option<String>, Option<String>)> {
    let mut env_vars = Vec::new();

    for value in config.vars().values() {
        extract_env_vars_from_value(value, &mut env_vars);
    }

    env_vars
}

/// Recursively extract environment variables from a JSON value
fn extract_env_vars_from_value(
    value: &serde_json::Value,
    env_vars: &mut Vec<(String, Option<String>, Option<String>)>,
) {
    match value {
        serde_json::Value::String(s) => {
            // Look for ${VAR} or ${VAR:-default} patterns
            let env_var_regex = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

            for cap in env_var_regex.captures_iter(s) {
                let full_match = &cap[1];

                if let Some((var_name, default)) = full_match.split_once(":-") {
                    let current_value = std::env::var(var_name).ok();
                    env_vars.push((
                        var_name.to_string(),
                        Some(default.to_string()),
                        current_value,
                    ));
                } else {
                    let current_value = std::env::var(full_match).ok();
                    env_vars.push((full_match.to_string(), None, current_value));
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_env_vars_from_value(item, env_vars);
            }
        }
        serde_json::Value::Object(obj) => {
            for value in obj.values() {
                extract_env_vars_from_value(value, env_vars);
            }
        }
        _ => {} // Numbers, booleans, and null don't contain env vars
    }
}

/// Get the type name for a JSON value
fn json_value_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(n) if n.is_i64() => "integer",
        serde_json::Value::Number(_) => "float",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "table",
        serde_json::Value::Null => "null",
    }
}

/// Format a JSON value for display
fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("\"{s}\""),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<_> = arr.iter().take(3).map(format_json_value).collect();
            if arr.len() > 3 {
                format!("[{}, ... ({} items)]", items.join(", "), arr.len())
            } else {
                format!("[{}]", items.join(", "))
            }
        }
        serde_json::Value::Object(obj) => {
            format!("{{{} keys}}", obj.len())
        }
        serde_json::Value::Null => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_value_type_name() {
        assert_eq!(json_value_type_name(&serde_json::json!("test")), "string");
        assert_eq!(json_value_type_name(&serde_json::json!(42)), "integer");
        assert_eq!(json_value_type_name(&serde_json::json!(2.5)), "float");
        assert_eq!(json_value_type_name(&serde_json::json!(true)), "boolean");
        assert_eq!(json_value_type_name(&serde_json::json!([])), "array");
        assert_eq!(json_value_type_name(&serde_json::json!({})), "table");
        assert_eq!(json_value_type_name(&serde_json::json!(null)), "null");
    }

    #[test]
    fn test_format_json_value() {
        assert_eq!(format_json_value(&serde_json::json!("test")), "\"test\"");
        assert_eq!(format_json_value(&serde_json::json!(42)), "42");
        assert_eq!(format_json_value(&serde_json::json!(true)), "true");
        assert_eq!(format_json_value(&serde_json::json!(null)), "null");
    }

    #[test]
    fn test_extract_env_vars_simple() {
        let mut env_vars = Vec::new();
        let value = serde_json::json!("${TEST_VAR}");
        extract_env_vars_from_value(&value, &mut env_vars);

        assert_eq!(env_vars.len(), 1);
        assert_eq!(env_vars[0].0, "TEST_VAR");
        assert_eq!(env_vars[0].1, None); // no default
    }

    #[test]
    fn test_extract_env_vars_with_default() {
        let mut env_vars = Vec::new();
        let value = serde_json::json!("${TEST_VAR:-default_value}");
        extract_env_vars_from_value(&value, &mut env_vars);

        assert_eq!(env_vars.len(), 1);
        assert_eq!(env_vars[0].0, "TEST_VAR");
        assert_eq!(env_vars[0].1, Some("default_value".to_string()));
    }
}
