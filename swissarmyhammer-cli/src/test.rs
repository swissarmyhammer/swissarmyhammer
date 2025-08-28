use anyhow::{anyhow, Result};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Input};
use std::collections::HashMap;
use std::fs;

use swissarmyhammer::PromptResolver;
use swissarmyhammer::{Prompt, PromptLibrary};

use crate::exit_codes::EXIT_SUCCESS;

/// Configuration for running a prompt test
#[derive(Default)]
pub struct TestConfig {
    pub prompt_name: Option<String>,
    pub file: Option<String>,
    pub arguments: Vec<String>,
    pub raw: bool,
    pub copy: bool,
    pub save: Option<String>,
    pub debug: bool,
}

pub struct TestRunner {
    library: PromptLibrary,
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            library: PromptLibrary::new(),
        }
    }

    pub async fn run(&mut self, config: TestConfig) -> Result<i32> {
        // Load all prompts first
        self.load_prompts()?;

        // Get the prompt to test
        let prompt = self.get_prompt(config.prompt_name.as_deref(), config.file.as_deref())?;

        // Collect arguments
        let args = if config.arguments.is_empty() {
            // Interactive mode - but only if we're in a terminal
            if atty::is(atty::Stream::Stdin) {
                self.collect_arguments_interactive(&prompt)?
            } else {
                // Non-interactive mode when not in terminal (CI/testing)
                self.collect_arguments_non_interactive(&prompt)?
            }
        } else {
            // Non-interactive mode
            self.parse_arguments(&config.arguments)?
        };

        // All template variables are now passed through the regular --var mechanism

        // Show debug information if requested
        if config.debug {
            self.show_debug_info(&prompt, &args)?;
        }

        // Render the prompt with environment variables support
        let rendered = self.render_prompt_with_env(&prompt, &args)?;

        // Output the result
        self.output_result(&rendered, config.raw, config.copy, config.save.as_deref())?;

        Ok(EXIT_SUCCESS)
    }

    fn load_prompts(&mut self) -> Result<()> {
        let mut resolver = PromptResolver::new();
        resolver.load_all_prompts(&mut self.library)?;
        Ok(())
    }

    fn get_prompt(&self, prompt_name: Option<&str>, file_path: Option<&str>) -> Result<Prompt> {
        match (prompt_name, file_path) {
            (Some(name), None) => {
                // Test by name
                self.library
                    .list()?
                    .into_iter()
                    .find(|p| p.name == name)
                    .ok_or_else(|| anyhow!("Prompt '{}' not found", name))
            }
            (None, Some(path)) => {
                // Test from file
                // Load from file path
                let content = std::fs::read_to_string(path)?;
                // Parse the prompt from the file content
                // For now, create a simple prompt from the content
                Ok(swissarmyhammer::Prompt::new("test-prompt", content))
            }
            (Some(_), Some(_)) => Err(anyhow!("Cannot specify both prompt name and file path")),
            (None, None) => Err(anyhow!("Must specify either prompt name or file path")),
        }
    }

    fn parse_arguments(&self, arguments: &[String]) -> Result<HashMap<String, String>> {
        let mut args = HashMap::new();

        for arg in arguments {
            if let Some((key, value)) = arg.split_once('=') {
                args.insert(key.to_string(), value.to_string());
            } else {
                return Err(anyhow!(
                    "Invalid argument format: '{}'. Use key=value format",
                    arg
                ));
            }
        }

        Ok(args)
    }

    fn collect_arguments_interactive(&self, prompt: &Prompt) -> Result<HashMap<String, String>> {
        let mut args = HashMap::new();
        let theme = ColorfulTheme::default();

        if prompt.parameters.is_empty() {
            println!("{}", "ℹ No arguments required for this prompt".blue());
            return Ok(args);
        }

        println!(
            "{}",
            "📝 Please provide values for the following arguments:"
                .bold()
                .blue()
        );
        println!();

        for arg in &prompt.parameters {
            let prompt_text = if arg.required {
                format!("{} (required): {}", arg.name.bold(), &arg.description)
            } else {
                format!("{} (optional): {}", arg.name.bold(), &arg.description)
            };

            loop {
                let mut input = Input::<String>::with_theme(&theme).with_prompt(&prompt_text);

                if let Some(default) = &arg.default {
                    let default_str = match default {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => default.to_string(),
                    };
                    input = input.default(default_str).show_default(true);
                }

                match input.interact_text() {
                    Ok(value) => {
                        if value.is_empty() && arg.required && arg.default.is_none() {
                            println!("{}", "❌ This argument is required".red());
                            continue;
                        }

                        if !value.is_empty() {
                            args.insert(arg.name.clone(), value);
                        } else if let Some(default) = &arg.default {
                            let default_str = match default {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => default.to_string(),
                            };
                            args.insert(arg.name.clone(), default_str);
                        }
                        break;
                    }
                    Err(e) => {
                        return Err(anyhow!("Failed to read input: {}", e));
                    }
                }
            }
        }

        println!();
        Ok(args)
    }

    fn collect_arguments_non_interactive(
        &self,
        prompt: &Prompt,
    ) -> Result<HashMap<String, String>> {
        let mut args = HashMap::new();

        if prompt.parameters.is_empty() {
            return Ok(args);
        }

        // In non-interactive mode, only use default values for optional arguments
        // Required arguments without defaults will cause template to show undefined variable placeholders
        for arg in &prompt.parameters {
            if let Some(default) = &arg.default {
                let default_str = match default {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => default.to_string(),
                };
                args.insert(arg.name.clone(), default_str);
            }
        }

        Ok(args)
    }

    fn show_debug_info(&self, prompt: &Prompt, args: &HashMap<String, String>) -> Result<()> {
        println!("{}", "🔍 Debug Information".bold().yellow());
        println!("{}", "─".repeat(50));

        println!("{}", "📄 Prompt Details:".bold());
        println!("  Name: {}", prompt.name);
        if let Some(description) = &prompt.description {
            println!("  Description: {description}");
        }
        if let Some(category) = &prompt.category {
            println!("  Category: {category}");
        }
        if let Some(source) = &prompt.source {
            println!("  Source: {}", source.display());
        }
        println!();

        println!("{}", "📋 Template Content:".bold());
        for (i, line) in prompt.template.lines().enumerate() {
            println!("  {:3}: {}", i + 1, line.dimmed());
        }
        println!();

        println!("{}", "🔧 Arguments Provided:".bold());
        if args.is_empty() {
            println!("  {}", "None".dimmed());
        } else {
            for (key, value) in args {
                println!("  {} = {}", key.cyan(), value.green());
            }
        }
        println!();

        println!("{}", "⚙️ Template Processing:".bold());
        println!("  Engine: Liquid");
        println!("  Backward Compatibility: Enabled");
        println!();

        println!("{}", "─".repeat(50));
        println!();

        Ok(())
    }

    fn render_prompt_with_env(
        &self,
        prompt: &Prompt,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        {
            let template_context = swissarmyhammer_config::TemplateContext::with_template_vars(
                args.iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            )
            .map_err(|e| {
                swissarmyhammer::SwissArmyHammerError::Template(format!(
                    "Template context error: {e}"
                ))
            })?;
            Ok(self.library.render(&prompt.name, &template_context)?)
        }
    }

    fn output_result(
        &self,
        rendered: &str,
        raw: bool,
        copy: bool,
        save_path: Option<&str>,
    ) -> Result<()> {
        // Display the result
        if raw {
            print!("{rendered}");
        } else {
            println!("{}", "✨ Rendered Output:".bold().green());
            println!("{}", "─".repeat(50));
            println!("{rendered}");
            println!("{}", "─".repeat(50));
        }

        // Note: ABORT ERROR string-based detection removed - abort handling now done via
        // ExecutorError::Abort at the workflow execution level

        // Copy to clipboard if requested
        if copy {
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => match clipboard.set_text(rendered) {
                    Ok(_) => println!("{}", "📋 Copied to clipboard!".green()),
                    Err(e) => println!(
                        "{}",
                        format!("⚠️  Failed to copy to clipboard: {e}").yellow()
                    ),
                },
                Err(e) => {
                    println!("{}", format!("⚠️  Clipboard not available: {e}").yellow());
                }
            }
        }

        // Save to file if requested
        if let Some(path) = save_path {
            fs::write(path, rendered)?;
            println!("{}", format!("💾 Saved to: {path}").green());
        }

        Ok(())
    }
}

#[allow(dead_code)]
pub fn get_prompt_validation(prompt: &Prompt) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check for required arguments
    for arg in &prompt.parameters {
        if arg.required && arg.default.is_none() {
            errors.push(format!(
                "Required argument '{}' has no default value",
                arg.name
            ));
        }
    }

    // Check for unused arguments in template
    let template_vars = extract_template_variables(&prompt.template);
    for arg in &prompt.parameters {
        if !template_vars.contains(&arg.name) {
            warnings.push(format!(
                "Argument '{}' is defined but not used in template",
                arg.name
            ));
        }
    }

    // Check for undefined variables in template
    for var in &template_vars {
        if !prompt.parameters.iter().any(|arg| &arg.name == var) {
            errors.push(format!(
                "Template variable '{{{{ {var} }}}}' is not defined in arguments"
            ));
        }
    }

    (errors, warnings)
}

#[allow(dead_code)]
fn extract_template_variables(template: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
    re.captures_iter(template)
        .map(|cap| cap[1].to_string())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer::common::{Parameter, ParameterType};

    #[test]
    fn test_runner_creation() {
        let runner = TestRunner::new();
        assert!(runner.library.list().unwrap().is_empty());
    }

    #[test]
    fn test_parse_arguments() {
        let runner = TestRunner::new();
        let args = vec!["name=test".to_string(), "value=123".to_string()];
        let parsed = runner.parse_arguments(&args).unwrap();

        assert_eq!(parsed.get("name").unwrap(), "test");
        assert_eq!(parsed.get("value").unwrap(), "123");
    }

    #[test]
    fn test_parse_arguments_invalid_format() {
        let runner = TestRunner::new();
        let args = vec!["invalid".to_string()];
        let result = runner.parse_arguments(&args);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_prompt_validation() {
        let prompt = Prompt::new("test", "Hello {{ name }}!")
            .add_parameter(Parameter::new("name", "", ParameterType::String).required(true))
            .add_parameter(
                Parameter::new("unused", "", ParameterType::String)
                    .required(false)
                    .with_default(serde_json::Value::String("default".to_string())),
            );

        let (errors, warnings) = get_prompt_validation(&prompt);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Required argument 'name' has no default value"));

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Argument 'unused' is defined but not used"));
    }

    #[test]
    fn test_parse_arguments_with_set_variables() {
        let runner = TestRunner::new();

        // Test parsing regular arguments
        let args = vec!["name=John".to_string(), "age=30".to_string()];
        let parsed = runner.parse_arguments(&args).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("name").unwrap(), "John");
        assert_eq!(parsed.get("age").unwrap(), "30");

        // Test parsing set variables (same format)
        let set_vars = vec!["author=Jane".to_string(), "version=1.0".to_string()];
        let parsed_set = runner.parse_arguments(&set_vars).unwrap();
        assert_eq!(parsed_set.len(), 2);
        assert_eq!(parsed_set.get("author").unwrap(), "Jane");
        assert_eq!(parsed_set.get("version").unwrap(), "1.0");
    }
}
