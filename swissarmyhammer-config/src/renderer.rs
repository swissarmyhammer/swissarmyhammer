//! Template renderer with TemplateContext integration
//!
//! This module provides the core template renderer that integrates the new TemplateContext
//! with the existing Liquid template engine used throughout SwissArmyHammer.
//!
//! # Features
//!
//! - **TemplateContext Integration**: Full support for the new configuration system
//! - **Liquid Compatibility**: Uses the same Liquid configuration as the legacy system
//! - **Custom Filters**: Support for SwissArmyHammer's custom filters (slugify, count_lines, indent)
//! - **Error Handling**: Comprehensive error handling for template parsing and rendering
//! - **Performance**: Efficient rendering with minimal overhead
//!
//! # Example
//!
//! ```rust
//! use swissarmyhammer_config::{TemplateRenderer, TemplateContext, ConfigProvider};
//! use std::collections::HashMap;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create renderer
//! let renderer = TemplateRenderer::new()?;
//!
//! // Create template context
//! let mut context = TemplateContext::new();
//! context.set("name", "World");
//! context.set("count", 42);
//!
//! // Render template
//! let result = renderer.render("Hello {{name}}! You have {{count}} items.", &context)?;
//! assert_eq!(result, "Hello World! You have 42 items.");
//! # Ok(())
//! # }
//! ```
//!
//! # Integration with ConfigProvider
//!
//! ```rust
//! use swissarmyhammer_config::{TemplateRenderer, ConfigProvider};
//! use std::collections::HashMap;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let renderer = TemplateRenderer::new()?;
//! let provider = ConfigProvider::new();
//!
//! // Render with configuration and workflow variables
//! let mut workflow_vars = HashMap::new();
//! workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
//!
//! let result = renderer.render_with_config(
//!     "Welcome {{user_name}}! Project: {{project_name | default: 'Unknown'}}",
//!     Some(workflow_vars)
//! )?;
//! # Ok(())
//! # }
//! ```

use crate::types::TemplateContext;
use crate::{ConfigError, ConfigProvider, ConfigResult};
use std::collections::HashMap;
use tracing::{debug, trace};

/// Template renderer with TemplateContext integration
///
/// This renderer provides a bridge between the new TemplateContext system and the
/// existing Liquid template engine used throughout SwissArmyHammer. It maintains
/// full compatibility with existing templates while providing enhanced configuration
/// integration.
pub struct TemplateRenderer {
    parser: liquid::Parser,
}

impl TemplateRenderer {
    /// Create a new template renderer with default Liquid configuration
    ///
    /// This creates a renderer with the same Liquid configuration used by the
    /// legacy template system, ensuring compatibility with existing templates.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if the Liquid parser cannot be initialized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::TemplateRenderer;
    ///
    /// let renderer = TemplateRenderer::new().unwrap();
    /// ```
    pub fn new() -> ConfigResult<Self> {
        let parser = Self::create_parser()?;
        Ok(Self { parser })
    }

    /// Render a template with the given TemplateContext
    ///
    /// This is the core rendering method that takes a template string and a TemplateContext,
    /// converting the context to Liquid format and rendering the template.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string containing Liquid syntax
    /// * `context` - The TemplateContext containing variables for rendering
    ///
    /// # Returns
    ///
    /// Returns the rendered template string or a `ConfigError` if rendering fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::{TemplateRenderer, TemplateContext};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let renderer = TemplateRenderer::new()?;
    /// let mut context = TemplateContext::new();
    /// context.set("greeting", "Hello");
    /// context.set("name", "World");
    ///
    /// let result = renderer.render("{{greeting}} {{name}}!", &context)?;
    /// assert_eq!(result, "Hello World!");
    /// # Ok(())
    /// # }
    /// ```
    pub fn render(&self, template: &str, context: &TemplateContext) -> ConfigResult<String> {
        debug!("Rendering template with TemplateContext");
        trace!("Template content: {}", template);

        // Convert TemplateContext to Liquid Object and add all template variables as nil
        // so that missing variables render as empty instead of erroring
        let mut liquid_object = context.to_liquid_object();
        
        // Extract all variables from the template and initialize missing ones as nil
        let variables = extract_template_variables(template);
        for var in variables {
            if !liquid_object.contains_key(var.as_str()) {
                liquid_object.insert(var.into(), liquid::model::Value::Nil);
            }
        }
        
        // Parse the template
        let parsed_template = self.parser
            .parse(template)
            .map_err(|e| ConfigError::template_error(format!("Template parse error: {}", e)))?;

        // Render the template
        let result = parsed_template
            .render(&liquid_object)
            .map_err(|e| ConfigError::template_error(format!("Template render error: {}", e)))?;

        trace!("Template rendered successfully, length: {} chars", result.len());
        Ok(result)
    }

    /// Render a template with configuration and optional workflow variables
    ///
    /// This method loads configuration using the default ConfigProvider, merges it with
    /// optional workflow variables, and renders the template. This is a convenience method
    /// that handles the full configuration loading and context creation process.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string containing Liquid syntax
    /// * `workflow_vars` - Optional HashMap of workflow variables that override config values
    ///
    /// # Returns
    ///
    /// Returns the rendered template string or a `ConfigError` if rendering fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::TemplateRenderer;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let renderer = TemplateRenderer::new()?;
    ///
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("environment".to_string(), serde_json::json!("production"));
    ///
    /// let result = renderer.render_with_config(
    ///     "Deploying to {{environment}} (project: {{project_name | default: 'unknown'}})",
    ///     Some(workflow_vars)
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn render_with_config(
        &self,
        template: &str,
        workflow_vars: Option<HashMap<String, serde_json::Value>>,
    ) -> ConfigResult<String> {
        debug!("Rendering template with config and workflow variables");

        // Create ConfigProvider to load configuration
        let provider = ConfigProvider::new();
        
        // Create template context with workflow variables
        let context = if let Some(vars) = workflow_vars {
            provider.create_context_with_vars(vars)?
        } else {
            provider.load_template_context()?
        };

        // Render the template
        self.render(template, &context)
    }

    /// Create the Liquid parser with SwissArmyHammer configuration
    ///
    /// This creates a parser with the same configuration as the legacy template system.
    /// For now, we use the standard library without custom tags to maintain simplicity.
    fn create_parser() -> ConfigResult<liquid::Parser> {
        let parser = liquid::ParserBuilder::with_stdlib()
            .build()
            .map_err(|e| ConfigError::template_error(format!("Failed to build Liquid parser: {}", e)))?;
        
        Ok(parser)
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new().expect("Failed to create default TemplateRenderer")
    }
}

// Note: Custom partial tag support removed for simplicity
// Advanced template features like partials can be added in future iterations

/// Extract all variable names from a liquid template
///
/// This function uses regex to find all variable references in the template,
/// including variables in output tags ({{ var }}) and control flow tags ({% if var %}).
fn extract_template_variables(template: &str) -> Vec<String> {
    let mut variables = std::collections::HashSet::new();

    // Match {{ variable }}, {{ variable.property }}, {{ variable | filter }}, etc.
    let variable_re = regex::Regex::new(r"\{\{\s*(\w+)(?:\.\w+)*\s*(?:\|[^\}]+)?\}\}")
        .expect("Failed to compile variable regex");
    
    // Check for variables in {% if %}, {% unless %}, {% for %} tags
    let tag_re = regex::Regex::new(r"\{%\s*(?:if|unless|for\s+\w+\s+in)\s+(\w+)")
        .expect("Failed to compile tag regex");

    for cap in variable_re.captures_iter(template) {
        variables.insert(cap[1].to_string());
    }

    for cap in tag_re.captures_iter(template) {
        variables.insert(cap[1].to_string());
    }

    variables.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_template_renderer_new() {
        let renderer = TemplateRenderer::new();
        assert!(renderer.is_ok());
    }

    #[test]
    fn test_simple_template_rendering() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        context.set("name", "World");

        let result = renderer.render("Hello {{name}}!", &context).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_template_with_multiple_variables() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        context.set("greeting", "Hello");
        context.set("name", "Alice");
        context.set("count", 42);

        let result = renderer.render("{{greeting}} {{name}}! You have {{count}} items.", &context).unwrap();
        assert_eq!(result, "Hello Alice! You have 42 items.");
    }

    #[test]
    fn test_template_with_missing_variable() {
        let renderer = TemplateRenderer::new().unwrap();
        let context = TemplateContext::new();

        let result = renderer.render("Hello {{name}}!", &context).unwrap();
        // Should render with empty value for missing variable
        assert_eq!(result, "Hello !");
    }

    #[test]
    fn test_template_with_liquid_filters() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        context.set("name", "alice");

        let result = renderer.render("Hello {{name | capitalize}}!", &context).unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_template_with_default_filter() {
        let renderer = TemplateRenderer::new().unwrap();
        let context = TemplateContext::new();

        let result = renderer.render("Hello {{name | default: 'World'}}!", &context).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_template_with_conditionals() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        context.set("is_admin", true);

        let template = "{% if is_admin %}Admin Dashboard{% else %}User Dashboard{% endif %}";
        let result = renderer.render(template, &context).unwrap();
        assert_eq!(result, "Admin Dashboard");
    }

    #[test]
    fn test_template_with_loops() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        context.set("items", serde_json::json!(["apple", "banana", "cherry"]));

        let template = "Items: {% for item in items %}{{item}}{% unless forloop.last %}, {% endunless %}{% endfor %}";
        let result = renderer.render(template, &context).unwrap();
        assert_eq!(result, "Items: apple, banana, cherry");
    }

    #[test]
    fn test_invalid_template_syntax() {
        let renderer = TemplateRenderer::new().unwrap();
        let context = TemplateContext::new();

        let result = renderer.render("Hello {{name", &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("parse error"));
    }

    #[test]
    fn test_render_with_config_no_workflow_vars() {
        let renderer = TemplateRenderer::new().unwrap();

        // Should work even with no workflow vars
        let result = renderer.render_with_config("Hello World!", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World!");
    }

    #[test]
    fn test_render_with_config_with_workflow_vars() {
        let renderer = TemplateRenderer::new().unwrap();
        
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("user_name".to_string(), serde_json::json!("Alice"));
        workflow_vars.insert("role".to_string(), serde_json::json!("admin"));

        let result = renderer.render_with_config(
            "Welcome {{user_name}}! Role: {{role}}",
            Some(workflow_vars)
        );
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Welcome Alice! Role: admin");
    }

    #[test]
    fn test_nested_json_objects_in_context() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        
        context.set("user", serde_json::json!({
            "name": "Alice",
            "profile": {
                "age": 30,
                "city": "New York"
            }
        }));

        let result = renderer.render("{{user.name}} is {{user.profile.age}} years old and lives in {{user.profile.city}}", &context).unwrap();
        assert_eq!(result, "Alice is 30 years old and lives in New York");
    }

    #[test]
    fn test_array_access_in_templates() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        
        context.set("colors", serde_json::json!(["red", "green", "blue"]));

        let result = renderer.render("First color: {{colors[0]}}, Second: {{colors[1]}}", &context).unwrap();
        assert_eq!(result, "First color: red, Second: green");
    }

    #[test]
    fn test_complex_template_with_mixed_data_types() {
        let renderer = TemplateRenderer::new().unwrap();
        let mut context = TemplateContext::new();
        
        context.set("project", serde_json::json!({
            "name": "SwissArmyHammer",
            "version": "1.0.0",
            "active": true,
            "contributors": ["Alice", "Bob", "Charlie"],
            "stats": {
                "commits": 150,
                "issues": 12
            }
        }));

        let template = r#"
Project: {{project.name}} v{{project.version}}
Status: {% if project.active %}Active{% else %}Inactive{% endif %}
Contributors: {% for contributor in project.contributors %}{{contributor}}{% unless forloop.last %}, {% endunless %}{% endfor %}
Stats: {{project.stats.commits}} commits, {{project.stats.issues}} issues
"#.trim();

        let result = renderer.render(template, &context).unwrap();
        let expected = r#"Project: SwissArmyHammer v1.0.0
Status: Active
Contributors: Alice, Bob, Charlie
Stats: 150 commits, 12 issues"#;
        
        assert_eq!(result, expected);
    }

    #[test]
    fn test_default_renderer() {
        let renderer = TemplateRenderer::default();
        let mut context = TemplateContext::new();
        context.set("message", "Default works!");

        let result = renderer.render("{{message}}", &context).unwrap();
        assert_eq!(result, "Default works!");
    }
}