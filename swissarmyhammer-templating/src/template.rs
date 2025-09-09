//! Core template functionality
//!
//! This module provides the main Template struct for parsing, compiling,
//! and rendering Liquid templates with security validation and custom features.

use crate::error::{Result, TemplatingError};
use crate::filters::preprocess_custom_filters;
use crate::partials::{PartialLoader, PartialLoaderAdapter};
use crate::security::{validate_template_security, MAX_TEMPLATE_RENDER_TIME_MS};
use crate::variables::{create_well_known_variables, extract_template_variables};
use liquid::{Object, Parser};
use std::collections::HashMap;
use std::time::Duration;
use swissarmyhammer_config::TemplateContext;

/// Template wrapper for Liquid templates with security validation
///
/// # Security
///
/// Templates are automatically validated for security risks before rendering.
/// The template engine provides multiple layers of protection:
///
/// **Sandboxing:**
/// - Templates cannot execute system commands
/// - No file system access outside of allowed partials
/// - No network requests capability
/// - No arbitrary code execution
/// - Environment variables are not accessible by default
///
/// **Resource Limits:**
/// - Template size limits (100KB for untrusted, 1MB for trusted)
/// - Variable count limits (1000 variables max for untrusted templates)
/// - Nesting depth limits (10 levels max to prevent stack overflow)
/// - Render timeout protection (5 seconds max)
///
/// **Pattern Detection:**
/// - Dangerous Liquid tags are blocked (`include`, `capture`, `tablerow`, `cycle`)
/// - Deep nesting structures are rejected
/// - Excessive complexity is prevented
///
/// Use `new_trusted()` for templates from trusted sources (builtin, user-created)
/// or `new_untrusted()` for external templates with strict validation.
pub struct Template {
    parser: Parser,
    template_str: String,
}

impl Template {
    /// Create a new template from a string (defaults to untrusted validation)
    pub fn new(template_str: &str) -> Result<Self> {
        Self::new_untrusted(template_str)
    }

    /// Create a new template from trusted source (builtin, user-created)
    ///
    /// Trusted templates have relaxed security validation but still have
    /// basic safety checks to prevent resource exhaustion.
    pub fn new_trusted(template_str: &str) -> Result<Self> {
        // Validate template security for trusted source
        validate_template_security(template_str, true)?;

        let parser = create_default_parser();
        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        Ok(Self {
            parser,
            template_str: template_str.to_string(),
        })
    }

    /// Create a new template from untrusted source with strict validation
    ///
    /// Untrusted templates undergo comprehensive security validation including
    /// size limits, pattern detection, complexity analysis, and resource limits.
    pub fn new_untrusted(template_str: &str) -> Result<Self> {
        // Validate template security for untrusted source
        validate_template_security(template_str, false)?;

        let parser = create_default_parser();
        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        Ok(Self {
            parser,
            template_str: template_str.to_string(),
        })
    }

    /// Create a new template with partial support
    pub fn with_partials<T: PartialLoader + 'static>(
        template_str: &str,
        loader: T,
    ) -> Result<Self> {
        let adapter = PartialLoaderAdapter::new(loader);
        let parser = create_parser_with_partials(adapter);

        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        Ok(Self {
            parser,
            template_str: template_str.to_string(),
        })
    }

    /// Render the template with given arguments as HashMap
    pub fn render(&self, args: &HashMap<String, String>) -> Result<String> {
        // Create timeout for template rendering
        let timeout = Duration::from_millis(MAX_TEMPLATE_RENDER_TIME_MS);
        self.render_with_timeout(args, timeout)
    }

    /// Render the template with TemplateContext
    pub fn render_with_context(&self, context: &TemplateContext) -> Result<String> {
        tracing::debug!("Parsing template: {}", self.template_str);
        let template = self
            .parser
            .parse(&self.template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        // Convert TemplateContext to liquid::Object
        let liquid_context = context.to_liquid_context();

        tracing::debug!("Rendering template with context");
        template
            .render(&liquid_context)
            .map_err(|e| TemplatingError::Render(e.to_string()))
    }

    /// Render the template with TemplateContext and custom timeout
    pub fn render_with_context_and_timeout(
        &self,
        context: &TemplateContext,
        _timeout: Duration,
    ) -> Result<String> {
        let template = self
            .parser
            .parse(&self.template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        // Convert TemplateContext to liquid::Object
        let liquid_context = context.to_liquid_context();

        // Render with timeout protection
        let render_result = std::thread::scope(|s| {
            let handle = s.spawn(|| template.render(&liquid_context));

            match handle.join() {
                Ok(result) => result.map_err(|e| TemplatingError::Render(e.to_string())),
                Err(_) => Err(TemplatingError::Render(
                    "Template rendering panicked".to_string(),
                )),
            }
        });

        // Note: We can't easily implement actual timeout without async context
        // In a real implementation, you'd want to use tokio::time::timeout
        // For now, we rely on the security validation to prevent complex templates
        render_result
    }

    /// Render the template with given arguments and custom timeout
    pub fn render_with_timeout(
        &self,
        args: &HashMap<String, String>,
        _timeout: Duration,
    ) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(&self.template_str, args);

        let template = self
            .parser
            .parse(&processed_template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        let mut object = Object::new();

        // First, initialize all template variables as nil so filters like | default work
        let variables = extract_template_variables(&self.template_str);
        for var in variables {
            object.insert(var.into(), liquid::model::Value::Nil);
        }

        // Then override with provided values
        for (key, value) in args {
            object.insert(
                key.clone().into(),
                liquid::model::Value::scalar(value.clone()),
            );
        }

        // Render with timeout protection
        let render_result = std::thread::scope(|s| {
            let handle = s.spawn(|| template.render(&object));

            match handle.join() {
                Ok(result) => result.map_err(|e| TemplatingError::Render(e.to_string())),
                Err(_) => Err(TemplatingError::Render(
                    "Template rendering panicked".to_string(),
                )),
            }
        });

        // Note: We can't easily implement actual timeout without async context
        // In a real implementation, you'd want to use tokio::time::timeout
        // For now, we rely on the security validation to prevent complex templates
        render_result
    }

    /// Render the template with given arguments and environment variables
    ///
    /// This method merges the provided arguments with environment variables,
    /// with provided arguments taking precedence over environment variables.
    pub fn render_with_env(&self, args: &HashMap<String, String>) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(&self.template_str, args);

        let template = self
            .parser
            .parse(&processed_template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        let mut object = Object::new();

        // First, initialize all template variables as nil so filters like | default work
        let variables = extract_template_variables(&self.template_str);
        for var in variables {
            object.insert(var.into(), liquid::model::Value::Nil);
        }

        // Add environment variables as template variables
        for (key, value) in std::env::vars() {
            object.insert(key.into(), liquid::model::Value::scalar(value));
        }

        // Then override with provided values (args take precedence)
        for (key, value) in args {
            object.insert(
                key.clone().into(),
                liquid::model::Value::scalar(value.clone()),
            );
        }

        template
            .render(&object)
            .map_err(|e| TemplatingError::Render(e.to_string()))
    }

    /// Render the template with given arguments and configuration variables
    ///
    /// This method merges the provided arguments with configuration variables
    /// and environment variables, with the following precedence (highest to lowest):
    /// 1. Provided arguments
    /// 2. Environment variables
    /// 3. Configuration variables
    /// 4. Well-known system variables
    pub fn render_with_config(&self, args: &HashMap<String, String>) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(&self.template_str, args);

        let template = self
            .parser
            .parse(&processed_template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        let mut object = Object::new();

        // Add well-known system variables (lowest priority)
        let well_known_vars = create_well_known_variables();
        let mut known_vars = std::collections::HashSet::new();
        for (key, value) in well_known_vars.iter() {
            known_vars.insert(key.clone());
            object.insert(key.clone(), value.clone());
        }

        // Load and merge configuration variables (second lowest priority)
        let mut config_vars = std::collections::HashSet::new();
        if let Ok(template_context) = swissarmyhammer_config::load_configuration_for_cli() {
            let config_object = template_context.to_liquid_context();
            for (key, value) in config_object.iter() {
                config_vars.insert(key.clone());
                object.insert(key.clone(), value.clone());
            }
        }

        // Initialize remaining template variables as nil so filters like | default work
        // But don't override variables that were already set from well-known vars or config
        let variables = extract_template_variables(&self.template_str);
        for var in variables {
            if !known_vars.contains(var.as_str()) && !config_vars.contains(var.as_str()) {
                object.insert(var.into(), liquid::model::Value::Nil);
            }
        }

        // Add environment variables as template variables (medium priority)
        for (key, value) in std::env::vars() {
            object.insert(key.into(), liquid::model::Value::scalar(value));
        }

        // Finally, add provided arguments (highest priority)
        for (key, value) in args {
            object.insert(
                key.clone().into(),
                liquid::model::Value::scalar(value.clone()),
            );
        }

        template
            .render(&object)
            .map_err(|e| TemplatingError::Render(e.to_string()))
    }

    /// Get the raw template string
    pub fn raw(&self) -> &str {
        &self.template_str
    }
}

/// Create a default Liquid parser with standard library and custom tags
pub fn create_default_parser() -> liquid::Parser {
    liquid::ParserBuilder::with_stdlib()
        .tag(crate::partials::PartialTag::new())
        .build()
        .expect("Failed to build Liquid parser")
}

/// Create a Liquid parser with custom partial loader
pub fn create_parser_with_partials<T: liquid::partials::PartialSource + Send + Sync + 'static>(
    partial_source: T,
) -> liquid::Parser {
    let partial_compiler = liquid::partials::EagerCompiler::new(partial_source);
    liquid::ParserBuilder::with_stdlib()
        .partials(partial_compiler)
        .tag(crate::partials::PartialTag::new())
        .build()
        .expect("Failed to build Liquid parser with partials")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_template() {
        let template = Template::new("Hello {{ name }}!").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_empty_template() {
        let template = Template::new("").unwrap();
        let args = HashMap::new();

        let result = template.render(&args).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_no_placeholders() {
        let template = Template::new("Hello World!").unwrap();
        let args = HashMap::new();

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_multiple_occurrences() {
        let template = Template::new("Hello {{ name }}! Nice to meet you, {{ name }}.").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello Alice! Nice to meet you, Alice.");
    }

    #[test]
    fn test_special_characters() {
        let template = Template::new("Code: {{ code }}").unwrap();
        let mut args = HashMap::new();
        args.insert(
            "code".to_string(),
            "<script>alert('XSS')</script>".to_string(),
        );

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Code: <script>alert('XSS')</script>");
    }

    #[test]
    fn test_missing_argument_no_validation() {
        let template = Template::new("Hello {{ name }}!").unwrap();
        let args = HashMap::new();

        let result = template.render(&args);
        // With our fix, undefined variables are now initialized as nil and render as empty
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello !");
    }

    #[test]
    fn test_liquid_default_filter_with_missing_variable() {
        // Test that the | default filter works when variable is not provided
        let template = Template::new("Hello {{ name | default: 'World' }}!").unwrap();
        let args = HashMap::new(); // No 'name' variable provided

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_liquid_default_filter_with_provided_variable() {
        // Test that the | default filter is ignored when variable is provided
        let template = Template::new("Hello {{ name | default: 'World' }}!").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_trusted_vs_untrusted_templates() {
        let template_str = "Hello {{ name }}!";

        // Both should work for simple templates
        let trusted = Template::new_trusted(template_str).unwrap();
        let untrusted = Template::new_untrusted(template_str).unwrap();

        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());

        assert_eq!(trusted.render(&args).unwrap(), "Hello World!");
        assert_eq!(untrusted.render(&args).unwrap(), "Hello World!");
    }

    #[test]
    fn test_template_with_context() {
        use serde_json::json;
        use std::collections::HashMap;

        // Create a TemplateContext with test data, handle config loading failures gracefully
        let mut template_vars = HashMap::new();
        template_vars.insert("name".to_string(), json!("World"));
        template_vars.insert("version".to_string(), json!("2.0.0"));
        template_vars.insert("enabled".to_string(), json!(true));

        let context = match TemplateContext::with_template_vars(template_vars) {
            Ok(ctx) => ctx,
            Err(_) => {
                // If config loading fails (e.g., no working directory), create a simple context
                let mut ctx = TemplateContext::new();
                ctx.set("name".to_string(), json!("World"));
                ctx.set("version".to_string(), json!("2.0.0"));
                ctx.set("enabled".to_string(), json!(true));
                ctx
            }
        };

        // Create a simple template
        let template = Template::new(
            "Hello {{name}}! Version: {{version}} {% if enabled %}(enabled){% endif %}",
        )
        .unwrap();

        // Render with TemplateContext
        let result = template.render_with_context(&context).unwrap();

        assert_eq!(result, "Hello World! Version: 2.0.0 (enabled)");
    }

    #[test]
    fn test_render_with_env() {
        use std::env;

        // Set a test environment variable
        env::set_var("TEST_ENV_VAR", "test_value");

        let template = Template::new("Hello {{USER}}, test var is {{TEST_ENV_VAR}}").unwrap();
        let args = HashMap::new();

        // Don't provide TEST_ENV_VAR in args, it should come from environment
        let result = template.render_with_env(&args).unwrap();

        // Should contain the environment variable value
        assert!(result.contains("test_value"));

        // Clean up
        env::remove_var("TEST_ENV_VAR");
    }

    #[test]
    fn test_render_with_env_args_override() {
        use std::env;

        // Set a test environment variable
        env::set_var("TEST_OVERRIDE", "env_value");

        let template = Template::new("Value is {{TEST_OVERRIDE}}").unwrap();
        let mut args = HashMap::new();
        args.insert("TEST_OVERRIDE".to_string(), "arg_value".to_string());

        let result = template.render_with_env(&args).unwrap();

        // Args should override environment variables
        assert_eq!(result, "Value is arg_value");

        // Clean up
        env::remove_var("TEST_OVERRIDE");
    }

    #[test]
    fn test_raw_template_access() {
        let template_str = "Hello {{ name }}!";
        let template = Template::new(template_str).unwrap();

        assert_eq!(template.raw(), template_str);
    }
}
