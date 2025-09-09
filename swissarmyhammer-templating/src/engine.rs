//! Template engine for processing and rendering templates
//!
//! This module provides the main TemplateEngine struct for creating and
//! configuring template parsers with various features and extensions.

use crate::error::{Result, TemplatingError};
use crate::filters::preprocess_custom_filters;
use crate::partials::{PartialLoader, PartialLoaderAdapter};
use crate::template::{create_default_parser, create_parser_with_partials, Template};

use std::collections::HashMap;
use swissarmyhammer_config::TemplateContext;

/// Template engine with Liquid configuration
pub struct TemplateEngine {
    parser: liquid::Parser,
}

impl TemplateEngine {
    /// Create a new template engine with default configuration
    pub fn new() -> Self {
        Self {
            parser: create_default_parser(),
        }
    }

    /// Create a new template engine with custom parser
    pub fn with_parser(parser: liquid::Parser) -> Self {
        Self { parser }
    }

    /// Create a new template engine with partial loader
    pub fn with_partials<T: PartialLoader + 'static>(loader: T) -> Self {
        let adapter = PartialLoaderAdapter::new(loader);
        let parser = create_parser_with_partials(adapter);
        Self { parser }
    }

    /// Create a template engine with plugins (stub - plugins managed at main crate level)
    /// This method exists for API compatibility but returns a basic engine.
    /// Plugin functionality is handled by the main swissarmyhammer crate.
    pub fn with_plugins<T>(_plugin_registry: T) -> Self {
        Self::new()
    }

    /// Get plugin registry (stub - plugins managed at main crate level)
    /// This method exists for API compatibility but always returns None.
    /// Plugin functionality is handled by the main swissarmyhammer crate.
    pub fn plugin_registry(&self) -> Option<()> {
        None
    }

    /// Parse a template string
    pub fn parse(&self, template_str: &str) -> Result<Template> {
        // Validate the template by trying to parse it
        self.parser
            .parse(template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        Template::new(template_str)
    }

    /// Render a template string with arguments
    pub fn render(&self, template_str: &str, args: &HashMap<String, String>) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(template_str, args);
        let template = self.parse(&processed_template_str)?;
        template.render(args)
    }

    /// Render a template string with TemplateContext
    pub fn render_with_context(
        &self,
        template_str: &str,
        context: &TemplateContext,
    ) -> Result<String> {
        let template = self
            .parser
            .parse(template_str)
            .map_err(|e| TemplatingError::Parse(e.to_string()))?;

        // Convert TemplateContext to liquid::Object
        let liquid_context = context.to_liquid_context();

        template
            .render(&liquid_context)
            .map_err(|e| TemplatingError::Render(e.to_string()))
    }

    /// Render a template string with arguments and environment variables
    ///
    /// This method merges the provided arguments with environment variables,
    /// with provided arguments taking precedence over environment variables.
    pub fn render_with_env(
        &self,
        template_str: &str,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(template_str, args);
        let template = self.parse(&processed_template_str)?;
        template.render_with_env(args)
    }

    /// Render a template string with arguments and configuration variables
    ///
    /// This method merges the provided arguments with configuration variables,
    /// environment variables, and well-known system variables with the following precedence:
    /// 1. Provided arguments (highest)
    /// 2. Environment variables
    /// 3. Configuration variables
    /// 4. Well-known system variables (lowest)
    ///    Configuration is loaded from the repository root if available.
    pub fn render_with_config(
        &self,
        template_str: &str,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        // Preprocess template to handle custom filters
        let processed_template_str = preprocess_custom_filters(template_str, args);
        let template = self.parse(&processed_template_str)?;
        template.render_with_config(args)
    }

    /// Get a reference to the underlying parser
    pub fn parser(&self) -> &liquid::Parser {
        &self.parser
    }

    /// Create a template using this engine's parser
    pub fn create_template(&self, template_str: &str) -> Result<Template> {
        // Use the engine's parser to create a template
        Template::with_partials(template_str, DummyPartialLoader::new())
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Dummy partial loader for basic template creation
#[derive(Debug)]
struct DummyPartialLoader;

impl DummyPartialLoader {
    fn new() -> Self {
        Self
    }
}

impl PartialLoader for DummyPartialLoader {
    fn contains(&self, _name: &str) -> bool {
        false
    }

    fn names(&self) -> Vec<String> {
        Vec::new()
    }

    fn try_get(&self, _name: &str) -> Option<std::borrow::Cow<'_, str>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_engine_creation() {
        let engine = TemplateEngine::new();
        // Parser exists and is ready to use
        let _parser = engine.parser();

        let default_engine = TemplateEngine::default();
        // Parser exists and is ready to use
        let _parser = default_engine.parser();
    }

    #[test]
    fn test_engine_render() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("greeting".to_string(), "Hello".to_string());

        let result = engine.render("{{greeting}} World!", &args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_engine_render_empty() {
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("", &args).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_engine_render_no_placeholders() {
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("Hello World!", &args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_engine_render_multiple_occurrences() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());

        let result = engine
            .render("Hello {{ name }}! Nice to meet you, {{ name }}.", &args)
            .unwrap();
        assert_eq!(result, "Hello Alice! Nice to meet you, Alice.");
    }

    #[test]
    fn test_engine_render_special_characters() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert(
            "code".to_string(),
            "<script>alert('XSS')</script>".to_string(),
        );

        let result = engine.render("Code: {{ code }}", &args).unwrap();
        assert_eq!(result, "Code: <script>alert('XSS')</script>");
    }

    #[test]
    fn test_engine_render_numeric_value() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("count".to_string(), "42".to_string());

        let result = engine.render("Count: {{ count }}", &args).unwrap();
        assert_eq!(result, "Count: 42");
    }

    #[test]
    fn test_engine_render_missing_argument() {
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("Hello {{ name }}!", &args);
        // With our fix, undefined variables are now initialized as nil and render as empty
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello !");
    }

    #[test]
    fn test_engine_render_with_context() {
        use serde_json::json;
        use std::collections::HashMap;

        // Create a TemplateContext with test data, handle config loading failures gracefully
        let mut template_vars = HashMap::new();
        template_vars.insert("user".to_string(), json!("Alice"));
        template_vars.insert("count".to_string(), json!(42));

        let context = match TemplateContext::with_template_vars(template_vars) {
            Ok(ctx) => ctx,
            Err(_) => {
                // If config loading fails, create a simple context
                let mut ctx = TemplateContext::new();
                ctx.set("user".to_string(), json!("Alice"));
                ctx.set("count".to_string(), json!(42));
                ctx
            }
        };

        // Create template engine
        let engine = TemplateEngine::new();

        // Render with TemplateContext
        let result = engine
            .render_with_context("{{user}} has {{count}} items", &context)
            .unwrap();

        assert_eq!(result, "Alice has 42 items");
    }

    #[test]
    fn test_engine_render_with_config() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("greeting".to_string(), "Hello".to_string());

        let result = engine
            .render_with_config("{{greeting}} World!", &args)
            .unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_engine_parse() {
        let engine = TemplateEngine::new();
        let template = engine.parse("Hello {{ name }}!").unwrap();

        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_engine_parse_invalid_template() {
        let engine = TemplateEngine::new();
        // This should cause a parsing error
        let result = engine.parse("Hello {{ unclosed");
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_render_with_env() {
        use std::env;

        // Set a test environment variable
        env::set_var("TEST_ENGINE_VAR", "test_value");

        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine
            .render_with_env("Test: {{TEST_ENGINE_VAR}}", &args)
            .unwrap();
        assert!(result.contains("test_value"));

        // Clean up
        env::remove_var("TEST_ENGINE_VAR");
    }
}
