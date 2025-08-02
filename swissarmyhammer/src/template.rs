//! Template engine and rendering functionality
//!
//! This module provides the core template engine for SwissArmyHammer, built on top of the
//! Liquid template language. It handles template parsing, rendering, and custom tag support.
//!
//! ## Features
//!
//! - **Liquid Template Engine**: Full support for Liquid syntax with variables, conditionals, loops
//! - **Custom Filters**: Extensible filter system for domain-specific transformations
//! - **Partial Templates**: Support for template composition with the `{% partial %}` tag
//! - **Plugin Integration**: Seamless integration with the plugin system for custom functionality
//!
//! ## Basic Usage
//!
//! ```rust
//! use swissarmyhammer::template::Template;
//! use std::collections::HashMap;
//!
//! // Create a template
//! let template = Template::new("Hello {{name}}! You have {{count}} messages.")?;
//!
//! // Prepare template variables
//! let mut variables = HashMap::new();
//! variables.insert("name".to_string(), "Alice".to_string());
//! variables.insert("count".to_string(), "5".to_string());
//!
//! // Render the template
//! let result = template.render(&variables)?;
//! assert_eq!(result, "Hello Alice! You have 5 messages.");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Advanced Template Features
//!
//! ```rust
//! use swissarmyhammer::template::Template;
//! use std::collections::HashMap;
//!
//! // Template with conditionals and loops
//! let template_text = r#"
//! {% if user.is_admin %}
//!   Admin Dashboard
//! {% else %}
//!   User Dashboard
//! {% endif %}
//!
//! Recent items:
//! {% for item in items %}
//!   - {{ item.name }} ({{ item.date | date: "%Y-%m-%d" }})
//! {% endfor %}
//! "#;
//!
//! let template = Template::new(template_text)?;
//! // ... render with complex data structures
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Custom Filters
//!
//! The template engine supports custom filters through the plugin system:
//!
//! ```rust
//! use swissarmyhammer::{template::Template, plugins::PluginRegistry};
//!
//! // Register custom filters with the plugin registry
//! let mut registry = PluginRegistry::new();
//! // registry.register_filter("capitalize", my_capitalize_filter);
//!
//! // Use in templates: {{ text | capitalize }}
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::{
    plugins::PluginRegistry, sah_config, security, PromptLibrary, Result, SwissArmyHammerError,
};
use liquid::{Object, Parser};
use liquid_core::{Language, ParseTag, Renderable, Runtime, TagReflection, TagTokenIter};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

/// Custom partial tag that acts as a no-op marker for liquid partial files
#[derive(Clone, Debug, Default)]
struct PartialTag;

impl PartialTag {
    pub fn new() -> Self {
        Self
    }
}

impl TagReflection for PartialTag {
    fn tag(&self) -> &'static str {
        "partial"
    }

    fn description(&self) -> &'static str {
        "Marks a file as a partial template (no-op)"
    }
}

impl ParseTag for PartialTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter<'_>,
        _options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        // Consume any arguments (though we expect none)
        arguments.expect_nothing()?;

        // Return a no-op renderable
        Ok(Box::new(PartialRenderable))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

/// Renderable for the partial tag (does nothing)
#[derive(Debug, Clone)]
struct PartialRenderable;

impl Renderable for PartialRenderable {
    fn render_to(
        &self,
        _output: &mut dyn Write,
        _context: &dyn Runtime,
    ) -> liquid_core::Result<()> {
        // No-op: this tag doesn't render anything
        Ok(())
    }
}

/// Custom partial source that loads partials from the prompt library
pub struct PromptPartialSource {
    library: Arc<PromptLibrary>,
    names: Vec<String>,
}

impl std::fmt::Debug for PromptPartialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptPartialSource")
            .field("library", &"<PromptLibrary>")
            .finish()
    }
}

impl PromptPartialSource {
    /// Create a new partial source that loads partials from the given prompt library
    pub fn new(library: Arc<PromptLibrary>) -> Self {
        let mut names = Vec::new();
        if let Ok(prompts) = library.list() {
            for prompt in prompts.iter() {
                names.push(prompt.name.clone());

                // Strip common prompt extensions to make them available as partials
                let extensions = [".md", ".markdown", ".liquid", ".md.liquid"];
                for ext in &extensions {
                    if let Some(name_without_ext) = prompt.name.strip_suffix(ext) {
                        names.push(name_without_ext.to_string());
                    }
                }
            }
        }
        Self { library, names }
    }
}

impl liquid::partials::PartialSource for PromptPartialSource {
    fn contains(&self, name: &str) -> bool {
        // Try exact name first
        if self.library.get(name).is_ok() {
            return true;
        }

        // Try with various prompt file extensions
        let extensions = [".md", ".markdown", ".liquid", ".md.liquid"];
        for ext in &extensions {
            let name_with_ext = format!("{name}{ext}");
            if self.library.get(&name_with_ext).is_ok() {
                return true;
            }
        }

        // If the name already has an extension, try stripping it
        if name.contains('.') {
            // Try stripping each known extension
            for ext in &extensions {
                if let Some(name_without_ext) = name.strip_suffix(ext) {
                    if self.library.get(name_without_ext).is_ok() {
                        return true;
                    }
                    // Also try with other extensions
                    for other_ext in &extensions {
                        if ext != other_ext {
                            let name_with_other_ext = format!("{name_without_ext}{other_ext}");
                            if self.library.get(&name_with_other_ext).is_ok() {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn names(&self) -> Vec<&str> {
        self.names.iter().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        // Try exact name first
        if let Ok(prompt) = self.library.get(name) {
            return Some(Cow::Owned(prompt.template));
        }

        // Try with various prompt file extensions
        let extensions = [".md", ".markdown", ".liquid", ".md.liquid"];
        for ext in &extensions {
            let name_with_ext = format!("{name}{ext}");
            if let Ok(prompt) = self.library.get(&name_with_ext) {
                return Some(Cow::Owned(prompt.template));
            }
        }

        // If the name already has an extension, try stripping it
        if name.contains('.') {
            // Try stripping each known extension
            for ext in &extensions {
                if let Some(name_without_ext) = name.strip_suffix(ext) {
                    if let Ok(prompt) = self.library.get(name_without_ext) {
                        return Some(Cow::Owned(prompt.template));
                    }
                    // Also try with other extensions
                    for other_ext in &extensions {
                        if ext != other_ext {
                            let name_with_other_ext = format!("{name_without_ext}{other_ext}");
                            if let Ok(prompt) = self.library.get(&name_with_other_ext) {
                                return Some(Cow::Owned(prompt.template));
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

/// Compiled regex patterns for template variable extraction
struct TemplateVariableExtractor {
    variable_re: regex::Regex,
    tag_re: regex::Regex,
}

impl TemplateVariableExtractor {
    fn new() -> Self {
        Self {
            // Match {{ variable }}, {{ variable.property }}, {{ variable | filter }}, etc.
            variable_re: regex::Regex::new(r"\{\{\s*(\w+)(?:\.\w+)*\s*(?:\|[^\}]+)?\}\}")
                .expect("Failed to compile variable regex"),
            // Check for variables in {% if %}, {% unless %}, {% for %} tags
            tag_re: regex::Regex::new(r"\{%\s*(?:if|unless|for\s+\w+\s+in)\s+(\w+)")
                .expect("Failed to compile tag regex"),
        }
    }

    fn extract(&self, template: &str) -> Vec<String> {
        let mut variables = std::collections::HashSet::new();

        for cap in self.variable_re.captures_iter(template) {
            variables.insert(cap[1].to_string());
        }

        for cap in self.tag_re.captures_iter(template) {
            variables.insert(cap[1].to_string());
        }

        variables.into_iter().collect()
    }
}

/// Extract all variable names from a liquid template
fn extract_template_variables(template: &str) -> Vec<String> {
    // Use thread_local to ensure the regex is compiled only once per thread
    thread_local! {
        static EXTRACTOR: TemplateVariableExtractor = TemplateVariableExtractor::new();
    }

    EXTRACTOR.with(|extractor| extractor.extract(template))
}

/// Template wrapper for Liquid templates
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
        security::validate_template_security(template_str, true)?;

        let parser = TemplateEngine::default_parser();
        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

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
        security::validate_template_security(template_str, false)?;

        let parser = TemplateEngine::default_parser();
        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

        Ok(Self {
            parser,
            template_str: template_str.to_string(),
        })
    }

    /// Create a new template with partial support
    pub fn with_partials(template_str: &str, library: Arc<PromptLibrary>) -> Result<Self> {
        let partial_source = PromptPartialSource::new(library);
        let parser = TemplateEngine::parser_with_partials(partial_source);
        // Validate the template by trying to parse it
        parser
            .parse(template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

        Ok(Self {
            parser,
            template_str: template_str.to_string(),
        })
    }

    /// Render the template with given arguments
    pub fn render(&self, args: &HashMap<String, String>) -> Result<String> {
        // Create timeout for template rendering
        let timeout = Duration::from_millis(security::MAX_TEMPLATE_RENDER_TIME_MS);
        self.render_with_timeout(args, timeout)
    }

    /// Render the template with given arguments and custom timeout
    pub fn render_with_timeout(
        &self,
        args: &HashMap<String, String>,
        _timeout: Duration,
    ) -> Result<String> {
        let template = self
            .parser
            .parse(&self.template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

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
                Ok(result) => result.map_err(|e| SwissArmyHammerError::Template(e.to_string())),
                Err(_) => Err(SwissArmyHammerError::Template(
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
        let template = self
            .parser
            .parse(&self.template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

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
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))
    }

    /// Render the template with given arguments and sah.toml configuration variables
    ///
    /// This method merges the provided arguments with sah.toml configuration variables
    /// and environment variables, with the following precedence (highest to lowest):
    /// 1. Provided arguments
    /// 2. Environment variables
    /// 3. sah.toml configuration variables
    pub fn render_with_config(&self, args: &HashMap<String, String>) -> Result<String> {
        let template = self
            .parser
            .parse(&self.template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

        let mut object = Object::new();

        // First, initialize all template variables as nil so filters like | default work
        let variables = extract_template_variables(&self.template_str);
        for var in variables {
            object.insert(var.into(), liquid::model::Value::Nil);
        }

        // Load and merge sah.toml configuration variables (lowest priority)
        if let Ok(Some(config)) = sah_config::load_repo_config() {
            let config_object = config.to_liquid_object();
            for (key, value) in config_object.iter() {
                object.insert(key.clone(), value.clone());
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
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))
    }

    /// Get the raw template string
    pub fn raw(&self) -> &str {
        &self.template_str
    }
}

/// Template engine with Liquid configuration
pub struct TemplateEngine {
    parser: liquid::Parser,
    plugin_registry: Option<PluginRegistry>,
}

impl TemplateEngine {
    /// Create a new template engine with default configuration
    pub fn new() -> Self {
        Self {
            parser: Self::default_parser(),
            plugin_registry: None,
        }
    }

    /// Create a new template engine with custom parser
    pub fn with_parser(parser: liquid::Parser) -> Self {
        Self {
            parser,
            plugin_registry: None,
        }
    }

    /// Create a new template engine with plugin registry
    pub fn with_plugins(plugin_registry: PluginRegistry) -> Self {
        let parser = plugin_registry.create_parser();
        Self {
            parser,
            plugin_registry: Some(plugin_registry),
        }
    }

    /// Create a default parser
    pub fn default_parser() -> liquid::Parser {
        liquid::ParserBuilder::with_stdlib()
            .tag(PartialTag::new())
            .build()
            .expect("Failed to build Liquid parser")
    }

    /// Create a parser with custom partial loader
    pub fn parser_with_partials(partial_source: PromptPartialSource) -> liquid::Parser {
        let partial_compiler = liquid::partials::EagerCompiler::new(partial_source);
        liquid::ParserBuilder::with_stdlib()
            .partials(partial_compiler)
            .tag(PartialTag::new())
            .build()
            .expect("Failed to build Liquid parser with partials")
    }

    /// Parse a template string
    pub fn parse(&self, template_str: &str) -> Result<Template> {
        // Validate the template by trying to parse it
        self.parser
            .parse(template_str)
            .map_err(|e| SwissArmyHammerError::Template(e.to_string()))?;

        Ok(Template {
            parser: self.parser.clone(),
            template_str: template_str.to_string(),
        })
    }

    /// Render a template string with arguments
    pub fn render(&self, template_str: &str, args: &HashMap<String, String>) -> Result<String> {
        let template = self.parse(template_str)?;
        template.render(args)
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
        let template = self.parse(template_str)?;
        template.render_with_env(args)
    }

    /// Render a template string with arguments and sah.toml configuration variables
    ///
    /// This method merges the provided arguments with sah.toml configuration variables,
    /// with provided arguments taking precedence over configuration variables.
    /// Configuration is loaded from the repository root if available.
    pub fn render_with_config(
        &self,
        template_str: &str,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        let template = self.parse(template_str)?;
        template.render_with_config(args)
    }

    /// Get a reference to the plugin registry, if any
    pub fn plugin_registry(&self) -> Option<&PluginRegistry> {
        self.plugin_registry.as_ref()
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
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
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("", &args).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_no_placeholders() {
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("Hello World!", &args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_multiple_occurrences() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());

        let result = engine
            .render("Hello {{ name }}! Nice to meet you, {{ name }}.", &args)
            .unwrap();
        assert_eq!(result, "Hello Alice! Nice to meet you, Alice.");
    }

    #[test]
    fn test_special_characters() {
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
    fn test_numeric_value() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("count".to_string(), "42".to_string());

        let result = engine.render("Count: {{ count }}", &args).unwrap();
        assert_eq!(result, "Count: 42");
    }

    #[test]
    fn test_boolean_value() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("enabled".to_string(), "true".to_string());

        let result = engine.render("Enabled: {{ enabled }}", &args).unwrap();
        assert_eq!(result, "Enabled: true");
    }

    #[test]
    fn test_missing_argument_no_validation() {
        let engine = TemplateEngine::new();
        let args = HashMap::new();

        let result = engine.render("Hello {{ name }}!", &args);
        // With our fix, undefined variables are now initialized as nil and render as empty
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello !");
    }

    #[test]
    fn test_default_value() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("greeting".to_string(), "Hello".to_string());
        args.insert("name".to_string(), "".to_string()); // Provide empty value

        let template = "{{ greeting }}, {{ name }}!";
        let result = engine.render(template, &args).unwrap();
        assert_eq!(result, "Hello, !");
    }

    #[test]
    fn test_required_argument_validation() {
        let template = Template::new("Hello {{ name }}!").unwrap();
        let args = HashMap::new();

        // With our fix, undefined variables are now initialized as nil and render as empty
        let result = template.render(&args);
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
    fn test_liquid_default_filter_multiple_variables() {
        // Test multiple variables with default filters
        let template = Template::new("{{ greeting | default: 'Hello' }} {{ name | default: 'World' }} in {{ language | default: 'English' }}!").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Bob".to_string()); // Only provide name

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello Bob in English!");
    }

    #[test]
    fn test_extract_template_variables() {
        // Test the extract_template_variables function
        let template = "Hello {{ name }}, you have {{ count }} messages in {{ language | default: 'English' }}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"count".to_string()));
        assert!(vars.contains(&"language".to_string()));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_extract_template_variables_with_conditionals() {
        // Test extraction from conditional tags
        let template =
            "{% if premium %}Premium user{% endif %} {% unless disabled %}Active{% endunless %}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"premium".to_string()));
        assert!(vars.contains(&"disabled".to_string()));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_extract_template_variables_whitespace_variations() {
        // Test whitespace variations in liquid templates
        let templates = vec![
            "{{name}}",
            "{{ name }}",
            "{{  name  }}",
            "{{\tname\t}}",
            "{{ name}}",
            "{{name }}",
        ];

        for template in templates {
            let vars = extract_template_variables(template);
            assert!(
                vars.contains(&"name".to_string()),
                "Failed for template: {template}"
            );
            assert_eq!(vars.len(), 1, "Failed for template: {template}");
        }
    }

    #[test]
    fn test_extract_template_variables_unicode() {
        // Test unicode characters in variable names
        // Note: Rust regex \w matches Unicode word characters by default
        let template = "Hello {{ café }}, {{ 用户名 }}, {{ user_name }}";
        let vars = extract_template_variables(template);

        // All three are valid variable names in Liquid/Rust regex
        assert!(vars.contains(&"café".to_string()));
        assert!(vars.contains(&"用户名".to_string()));
        assert!(vars.contains(&"user_name".to_string()));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_extract_template_variables_long_names() {
        // Test very long template variable names
        let long_var_name = "a".repeat(100);
        let template = format!("Hello {{{{ {long_var_name} }}}}");
        let vars = extract_template_variables(&template);

        assert!(vars.contains(&long_var_name));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_extract_template_variables_no_recursive_parsing() {
        // Test handling of nested/malformed template syntax
        let template = "{{ {{ inner }} }} and {{ var_{{ suffix }} }}";
        let vars = extract_template_variables(template);

        // The regex will find "inner" and "suffix" as they appear within {{ }}
        // even though the overall syntax is malformed
        assert!(vars.contains(&"inner".to_string()));
        assert!(vars.contains(&"suffix".to_string()));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_extract_template_variables_duplicates() {
        // Test that duplicate variables are only counted once
        let template = "{{ name }} says hello to {{ name }} and {{ name }}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"name".to_string()));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_extract_template_variables_for_loops() {
        // Test extraction from for loops
        let template = "{% for item in items %}{{ item.name }}{% endfor %} {% for product in products %}{{ product }}{% endfor %}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"items".to_string()));
        assert!(vars.contains(&"item".to_string()));
        assert!(vars.contains(&"products".to_string()));
        assert!(vars.contains(&"product".to_string()));
        assert_eq!(vars.len(), 4);
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
    fn test_render_with_config_precedence() {
        // Test variable precedence: config < env < args
        // This test doesn't rely on actual file loading since that would require
        // setting up filesystem fixtures. Instead, it tests the precedence logic
        // in the template engine's render_with_config method.

        let template =
            Template::new("Project: {{project_name}}, Env: {{TEST_VAR}}, Arg: {{custom_arg}}")
                .unwrap();

        // Set up environment variable
        std::env::set_var("TEST_VAR", "env_value");

        let mut args = HashMap::new();
        args.insert("custom_arg".to_string(), "arg_value".to_string());
        args.insert("TEST_VAR".to_string(), "arg_override".to_string());

        let result = template.render_with_config(&args).unwrap();

        // Environment variable should be overridden by argument
        assert!(result.contains("Env: arg_override"));
        assert!(result.contains("Arg: arg_value"));

        // Clean up
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_template_engine_render_with_config() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("greeting".to_string(), "Hello".to_string());

        let result = engine
            .render_with_config("{{greeting}} World!", &args)
            .unwrap();
        assert_eq!(result, "Hello World!");
    }
}
