//! Prompt management and loading functionality
//!
//! This module provides the core types and functionality for managing prompts,
//! including loading from files, rendering with arguments, and organizing in libraries.
//!
//! # Examples
//!
//! Creating and rendering a simple prompt:
//!
//! ```
//! use swissarmyhammer::{Prompt, ArgumentSpec};
//! use std::collections::HashMap;
//!
//! let prompt = Prompt::new("greet", "Hello {{name}}!")
//!     .with_description("A greeting prompt")
//!     .add_argument(ArgumentSpec {
//!         name: "name".to_string(),
//!         description: Some("Name to greet".to_string()),
//!         required: true,
//!         default: None,
//!         type_hint: Some("string".to_string()),
//!     });
//!
//! let mut args = HashMap::new();
//! args.insert("name".to_string(), "World".to_string());
//! let result = prompt.render(&args).unwrap();
//! assert_eq!(result, "Hello World!");
//! ```

use crate::common::{Parameter, ParameterProvider, ParameterType};
use crate::validation::{Validatable, ValidationIssue, ValidationLevel};
use crate::{Result, SwissArmyHammerError, Template};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Represents a single prompt with metadata and template content.
///
/// A [`Prompt`] encapsulates all the information needed to use a template, including
/// its name, description, required arguments, and the template content itself.
/// Prompts are typically loaded from markdown files with YAML front matter.
///
/// # Prompt File Format
///
/// ```markdown
/// ---
/// title: Code Review
/// description: Reviews code for best practices
/// category: development
/// tags: ["code", "review", "quality"]
/// arguments:
///   - name: code
///     description: The code to review
///     required: true
///   - name: language
///     description: Programming language
///     required: false
///     default: "auto-detect"
/// ---
///
/// Please review this {{language}} code:
///
/// \`\`\`
/// {{code}}
/// \`\`\`
///
/// Focus on best practices, potential bugs, and performance.
/// ```
///
/// # Examples
///
/// ```
/// use swissarmyhammer::{Prompt, ArgumentSpec};
/// use std::collections::HashMap;
///
/// // Create a prompt programmatically
/// let prompt = Prompt::new("debug", "Debug this {{language}} error: {{error}}")
///     .with_description("Helps debug programming errors")
///     .with_category("debugging")
///     .add_argument(ArgumentSpec {
///         name: "error".to_string(),
///         description: Some("The error message".to_string()),
///         required: true,
///         default: None,
///         type_hint: Some("string".to_string()),
///     })
///     .add_argument(ArgumentSpec {
///         name: "language".to_string(),
///         description: Some("Programming language".to_string()),
///         required: false,
///         default: Some("unknown".to_string()),
///         type_hint: Some("string".to_string()),
///     });
///
/// // Render with arguments
/// let mut args = HashMap::new();
/// args.insert("error".to_string(), "NullPointerException".to_string());
/// args.insert("language".to_string(), "Java".to_string());
///
/// let rendered = prompt.render(&args).unwrap();
/// assert!(rendered.contains("Java"));
/// assert!(rendered.contains("NullPointerException"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique identifier for the prompt.
    ///
    /// This should be a valid filename without extension (e.g., "code-review", "debug-helper").
    /// Used to reference the prompt from CLI and library code.
    pub name: String,

    /// Human-readable description of what the prompt does.
    ///
    /// This appears in help text and prompt listings to help users understand
    /// the prompt's purpose.
    pub description: Option<String>,

    /// Category for organizing prompts into groups.
    ///
    /// Examples: "development", "writing", "analysis", "debugging".
    /// Used for filtering and organizing prompt collections.
    pub category: Option<String>,

    /// Tags for improved searchability.
    ///
    /// Used by search functionality to find relevant prompts.
    /// Should include relevant keywords and concepts.
    pub tags: Vec<String>,

    /// The template content using Liquid syntax.
    ///
    /// This is the actual prompt template that gets rendered with user arguments.
    /// Supports Liquid template syntax including variables (`{{var}}`), conditionals,
    /// loops, and filters.
    ///
    /// # Template Syntax
    ///
    /// - Variables: `{{variable_name}}`
    /// - Conditionals: `{% if condition %}...{% endif %}`
    /// - Loops: `{% for item in items %}...{% endfor %}`
    /// - Filters: `{{text | upper}}`
    pub template: String,

    /// Specifications for template arguments.
    ///
    /// Defines what arguments the template expects, whether they're required,
    /// default values, and documentation. Used for validation and help generation.
    #[serde(rename = "parameters")]
    pub arguments: Vec<ArgumentSpec>,

    /// Cached shared parameters converted from arguments.
    ///
    /// This field is lazily populated to provide efficient access to the shared
    /// parameter system without breaking backward compatibility.
    #[serde(skip)]
    cached_parameters: std::sync::OnceLock<Vec<Parameter>>,

    /// Path to the source file (if loaded from file).
    ///
    /// Used for debugging and file watching functionality.
    /// `None` for programmatically created prompts.
    pub source: Option<PathBuf>,

    /// Additional metadata from the prompt file.
    ///
    /// Contains any extra fields from the YAML front matter that aren't
    /// part of the core prompt structure. Useful for custom metadata.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Specification for a template argument.
///
/// Defines metadata about an argument that a template expects, including
/// whether it's required, default values, and documentation. Used for
/// validation, help generation, and IDE support.
///
/// # Examples
///
/// ```
/// use swissarmyhammer::ArgumentSpec;
///
/// // Required argument with no default
/// let required_arg = ArgumentSpec {
///     name: "filename".to_string(),
///     description: Some("Path to the file to process".to_string()),
///     required: true,
///     default: None,
///     type_hint: Some("path".to_string()),
/// };
///
/// // Optional argument with default value
/// let optional_arg = ArgumentSpec {
///     name: "format".to_string(),
///     description: Some("Output format".to_string()),
///     required: false,
///     default: Some("markdown".to_string()),
///     type_hint: Some("string".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentSpec {
    /// The name of the argument as used in templates.
    ///
    /// This is how the argument is referenced in the template (e.g., `{{filename}}`).
    /// Should be a valid identifier using letters, numbers, and underscores.
    pub name: String,

    /// Human-readable description of the argument's purpose.
    ///
    /// Used in help text and documentation generation. Should explain
    /// what the argument is for and any constraints or expected formats.
    pub description: Option<String>,

    /// Whether this argument must be provided.
    ///
    /// If `true`, template rendering will fail if this argument is not provided
    /// and no default value is specified.
    pub required: bool,

    /// Default value to use if the argument is not provided.
    ///
    /// Only used when `required` is `false` or when the user doesn't provide
    /// a value for a required argument. The default is used as-is in the template.
    pub default: Option<String>,

    /// Type hint for the argument.
    ///
    /// Helps tools and users understand what kind of value is expected.
    /// Common values: "string", "number", "boolean", "path", "url", "json".
    /// This is primarily for documentation and tooling support.
    pub type_hint: Option<String>,
}

impl ArgumentSpec {
    /// Convert this ArgumentSpec to the shared Parameter type
    pub fn to_parameter(&self) -> Parameter {
        let parameter_type = if let Some(type_hint) = &self.type_hint {
            type_hint.parse().unwrap_or(ParameterType::String)
        } else {
            ParameterType::String // Default to string
        };

        let mut param = Parameter::new(
            &self.name,
            self.description.clone().unwrap_or_default(),
            parameter_type,
        )
        .required(self.required);

        if let Some(default) = &self.default {
            param = param.with_default(serde_json::Value::String(default.clone()));
        }

        param
    }
}

impl From<Parameter> for ArgumentSpec {
    /// Convert a shared Parameter back to ArgumentSpec for backward compatibility
    fn from(param: Parameter) -> Self {
        Self {
            name: param.name,
            description: Some(param.description),
            required: param.required,
            default: param.default.and_then(|v| v.as_str().map(String::from)),
            type_hint: Some(param.parameter_type.as_str().to_string()),
        }
    }
}

impl ParameterProvider for Prompt {
    /// Get the parameters for this prompt by converting from ArgumentSpec
    fn get_parameters(&self) -> &[Parameter] {
        self.cached_parameters.get_or_init(|| {
            self.arguments
                .iter()
                .map(|arg_spec| arg_spec.to_parameter())
                .collect()
        })
    }
}

impl Prompt {
    /// Creates a new prompt with the given name and template.
    ///
    /// This is the minimal constructor for a prompt. Additional metadata can be added
    /// using the builder methods like [`with_description`](Self::with_description),
    /// [`with_category`](Self::with_category), and [`add_argument`](Self::add_argument).
    ///
    /// # Arguments
    ///
    /// * `name` - Unique identifier for the prompt
    /// * `template` - Template content using Liquid syntax
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::Prompt;
    ///
    /// let prompt = Prompt::new("hello", "Hello {{name}}!");
    /// assert_eq!(prompt.name, "hello");
    /// assert_eq!(prompt.template, "Hello {{name}}!");
    /// ```
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            category: None,
            tags: Vec::new(),
            template: template.into(),
            arguments: Vec::new(),
            cached_parameters: std::sync::OnceLock::new(),
            source: None,
            metadata: HashMap::new(),
        }
    }

    /// Renders the prompt template with the provided arguments.
    ///
    /// This method validates that all required arguments are provided, applies
    /// default values for missing optional arguments, and renders the template
    /// using the Liquid template engine.
    ///
    /// # Arguments
    ///
    /// * `args` - Map of argument names to values
    ///
    /// # Returns
    ///
    /// The rendered template as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template parsing fails due to invalid Liquid syntax
    /// - Required arguments are missing from the provided arguments map
    /// - Template rendering fails during execution
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{Prompt, ArgumentSpec};
    /// use std::collections::HashMap;
    ///
    /// let prompt = Prompt::new("greet", "Hello {{name}}!")
    ///     .add_argument(ArgumentSpec {
    ///         name: "name".to_string(),
    ///         description: None,
    ///         required: true,
    ///         default: None,
    ///         type_hint: None,
    ///     });
    ///
    /// let mut args = HashMap::new();
    /// args.insert("name".to_string(), "Alice".to_string());
    ///
    /// let result = prompt.render(&args).unwrap();
    /// assert_eq!(result, "Hello Alice!");
    /// ```
    pub fn render(&self, args: &HashMap<String, String>) -> Result<String> {
        let template = Template::new(&self.template)?;

        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(SwissArmyHammerError::Template(format!(
                    "Required argument '{}' not provided",
                    arg.name
                )));
            }
        }

        // Start with all provided arguments
        let mut render_args = args.clone();

        // Add defaults for missing arguments
        for arg in &self.arguments {
            if !render_args.contains_key(&arg.name) {
                if let Some(default) = &arg.default {
                    render_args.insert(arg.name.clone(), default.clone());
                }
            }
        }

        template.render(&render_args)
    }

    /// Renders the prompt template with environment variables included
    ///
    /// This method merges the provided arguments with environment variables,
    /// with provided arguments taking precedence over environment variables.
    /// This is useful for templates that need access to system configuration
    /// or environment-specific values.
    ///
    /// # Arguments
    ///
    /// * `args` - Template variables as key-value pairs
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::Prompt;
    /// use std::collections::HashMap;
    ///
    /// let prompt = Prompt::new("deploy", "Deploying to {{ENV}} by {{USER}}");
    /// let mut args = HashMap::new();
    /// args.insert("ENV".to_string(), "production".to_string());
    /// // The USER env var will be picked up automatically
    ///
    /// let result = prompt.render_with_env(&args).unwrap();
    /// ```
    pub fn render_with_env(&self, args: &HashMap<String, String>) -> Result<String> {
        let template = Template::new(&self.template)?;

        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(SwissArmyHammerError::Template(format!(
                    "Required argument '{}' not provided",
                    arg.name
                )));
            }
        }

        // Start with all provided arguments
        let mut render_args = args.clone();

        // Add defaults for missing arguments
        for arg in &self.arguments {
            if !render_args.contains_key(&arg.name) {
                if let Some(default) = &arg.default {
                    render_args.insert(arg.name.clone(), default.clone());
                }
            }
        }

        template.render_with_env(&render_args)
    }

    /// Renders the prompt template with partial support
    ///
    /// This method enables the use of `{% render %}` tags within the template
    /// to include other prompts as partials.
    ///
    /// # Arguments
    ///
    /// * `args` - Template variables as key-value pairs
    /// * `library` - The prompt library to use for resolving partials
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer::{Prompt, PromptLibrary};
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// let mut library = PromptLibrary::new();
    /// // Add partials to library...
    ///
    /// let prompt = Prompt::new("main", "{% render \"header\" %}\nContent here");
    /// let mut args = HashMap::new();
    /// args.insert("name".to_string(), "World".to_string());
    ///
    /// let result = prompt.render_with_partials(&args, Arc::new(library)).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template parsing fails due to invalid Liquid syntax or partial resolution
    /// - Required arguments are missing from the provided arguments map
    /// - Template rendering fails during execution
    /// - Referenced partials cannot be found in the provided library
    pub fn render_with_partials(
        &self,
        args: &HashMap<String, String>,
        library: Arc<PromptLibrary>,
    ) -> Result<String> {
        let template = crate::Template::with_partials(&self.template, library)?;

        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(SwissArmyHammerError::Template(format!(
                    "Required argument '{}' not provided",
                    arg.name
                )));
            }
        }

        // Start with all provided arguments
        let mut render_args = args.clone();

        // Add defaults for missing arguments
        for arg in &self.arguments {
            if !render_args.contains_key(&arg.name) {
                if let Some(default) = &arg.default {
                    render_args.insert(arg.name.clone(), default.clone());
                }
            }
        }

        template.render_with_config(&render_args)
    }

    /// Renders the prompt template with partial support and environment variables
    ///
    /// This method combines the features of partial rendering and environment variable
    /// inclusion. It enables the use of `{% render %}` tags within the template
    /// to include other prompts as partials, while also making environment variables
    /// available in the template context.
    ///
    /// # Arguments
    ///
    /// * `args` - Template variables as key-value pairs
    /// * `library` - The prompt library to use for resolving partials
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer::{Prompt, PromptLibrary};
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// let mut library = PromptLibrary::new();
    /// // Add partials to library...
    ///
    /// let prompt = Prompt::new("deploy", "{% render \"header\" %}\nDeploying to {{ENV}}");
    /// let mut args = HashMap::new();
    /// args.insert("app".to_string(), "myapp".to_string());
    /// // ENV var from environment will be available
    ///
    /// let result = prompt.render_with_partials_and_env(&args, Arc::new(library)).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template parsing fails due to invalid Liquid syntax or partial resolution
    /// - Required arguments are missing from the provided arguments map
    /// - Template rendering fails during execution
    /// - Referenced partials cannot be found in the provided library
    pub fn render_with_partials_and_env(
        &self,
        args: &HashMap<String, String>,
        library: Arc<PromptLibrary>,
    ) -> Result<String> {
        let template = crate::Template::with_partials(&self.template, library)?;

        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(SwissArmyHammerError::Template(format!(
                    "Required argument '{}' not provided",
                    arg.name
                )));
            }
        }

        // Start with all provided arguments
        let mut render_args = args.clone();

        // Add defaults for missing arguments
        for arg in &self.arguments {
            if !render_args.contains_key(&arg.name) {
                if let Some(default) = &arg.default {
                    render_args.insert(arg.name.clone(), default.clone());
                }
            }
        }

        template.render_with_config(&render_args)
    }

    /// Adds an argument specification to the prompt.
    ///
    /// Arguments define what inputs the template expects, whether they're required,
    /// and provide documentation for users of the prompt.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument specification to add
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{Prompt, ArgumentSpec};
    ///
    /// let prompt = Prompt::new("example", "Processing {{file}}")
    ///     .add_argument(ArgumentSpec {
    ///         name: "file".to_string(),
    ///         description: Some("Path to input file".to_string()),
    ///         required: true,
    ///         default: None,
    ///         type_hint: Some("path".to_string()),
    ///     });
    ///
    /// assert_eq!(prompt.arguments.len(), 1);
    /// assert_eq!(prompt.arguments[0].name, "file");
    /// ```
    #[must_use]
    pub fn add_argument(mut self, arg: ArgumentSpec) -> Self {
        self.arguments.push(arg);
        // Clear the cached parameters since arguments have changed
        self.cached_parameters = std::sync::OnceLock::new();
        self
    }

    /// Sets the description for the prompt.
    ///
    /// The description helps users understand what the prompt does and when to use it.
    /// It appears in help text and prompt listings.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable description of the prompt's purpose
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::Prompt;
    ///
    /// let prompt = Prompt::new("debug", "Debug this error: {{error}}")
    ///     .with_description("Helps analyze and debug programming errors");
    ///
    /// assert_eq!(prompt.description, Some("Helps analyze and debug programming errors".to_string()));
    /// ```
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the category for the prompt.
    ///
    /// Categories help organize prompts into logical groups. Common categories
    /// include "development", "writing", "analysis", and "debugging".
    ///
    /// # Arguments
    ///
    /// * `category` - Category name for organizing the prompt
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::Prompt;
    ///
    /// let prompt = Prompt::new("code-review", "Review this code: {{code}}")
    ///     .with_category("development");
    ///
    /// assert_eq!(prompt.category, Some("development".to_string()));
    /// ```
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Sets the tags for the prompt.
    ///
    /// Tags improve searchability by providing keywords that describe the prompt's
    /// functionality and use cases. They're used by the search system to find
    /// relevant prompts.
    ///
    /// # Arguments
    ///
    /// * `tags` - Vector of tag strings
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::Prompt;
    ///
    /// let prompt = Prompt::new("sql-gen", "Generate SQL: {{description}}")
    ///     .with_tags(vec![
    ///         "sql".to_string(),
    ///         "database".to_string(),
    ///         "generation".to_string()
    ///     ]);
    ///
    /// assert_eq!(prompt.tags.len(), 3);
    /// assert!(prompt.tags.contains(&"sql".to_string()));
    /// ```
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

impl Validatable for Prompt {
    fn validate(&self, source_path: Option<&Path>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let file_path = source_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(format!("prompt:{}", self.name)));

        // Check if this is a partial template by looking at the description
        let is_partial = self
            .description
            .as_ref()
            .map(|desc| desc == "Partial template for reuse in other prompts")
            .unwrap_or(false);

        // Skip field validation for partial templates
        if !is_partial {
            // Check required fields
            if !self.metadata.contains_key("title")
                || self
                    .metadata
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: file_path.to_path_buf(),
                    content_title: Some(self.name.clone()),
                    line: None,
                    column: None,
                    message: "Missing required field: title".to_string(),
                    suggestion: Some("Add a title field to the YAML front matter".to_string()),
                });
            }

            if self.description.is_none() || self.description.as_ref().unwrap().is_empty() {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: file_path.to_path_buf(),
                    content_title: Some(self.name.clone()),
                    line: None,
                    column: None,
                    message: "Missing required field: description".to_string(),
                    suggestion: Some(
                        "Add a description field to the YAML front matter".to_string(),
                    ),
                });
            }
        }

        // Skip variable validation for partial templates
        if !is_partial {
            issues.extend(self.validate_template_variables(&file_path));
        }

        issues
    }
}

impl Prompt {
    /// Validate template variables against defined arguments
    fn validate_template_variables(&self, file_path: &Path) -> Vec<ValidationIssue> {
        use regex::Regex;

        let mut issues = Vec::new();

        // Remove {% raw %} blocks from content before validation
        let raw_regex = Regex::new(r"(?s)\{%\s*raw\s*%\}.*?\{%\s*endraw\s*%\}").unwrap();
        let content_without_raw = raw_regex.replace_all(&self.template, "");

        // Enhanced regex to match various Liquid variable patterns
        let patterns = [
            // Simple variables: {{ variable }}
            r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}",
            // Variables with filters: {{ variable | filter }}
            r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\|",
            // Variables as filter arguments: {{ "value" | filter: variable }}
            r"\|\s*[a-zA-Z_][a-zA-Z0-9_]*\s*:\s*([a-zA-Z_][a-zA-Z0-9_]*)",
            // Object properties: {{ object.property }}
            r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\.[a-zA-Z_][a-zA-Z0-9_]*",
            // Array access: {{ array[0] }}
            r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\[",
            // Case statements: {% case variable %}
            r"\{\%\s*case\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\%\}",
            // If statements: {% if variable %}
            r"\{\%\s*if\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[%}=<>!]",
            // Unless statements: {% unless variable %}
            r"\{\%\s*unless\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[%}=<>!]",
            // Elsif statements: {% elsif variable %}
            r"\{\%\s*elsif\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[%}=<>!]",
            // Variable comparisons: {% if variable == "value" %}
            r"\{\%\s*(?:if|elsif|unless)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[=<>!]",
            // Assignment statements: {% assign var = variable %}
            r"\{\%\s*assign\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*([a-zA-Z_][a-zA-Z0-9_]*)",
        ];

        let mut used_variables = std::collections::HashSet::new();

        for pattern in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for captures in regex.captures_iter(&content_without_raw) {
                    if let Some(var_match) = captures.get(1) {
                        let var_name = var_match.as_str().trim();
                        // Skip built-in Liquid objects and variables
                        let builtin_vars = ["env", "forloop", "tablerow", "paginate"];
                        if !builtin_vars.contains(&var_name) {
                            used_variables.insert(var_name.to_string());
                        }
                    }
                }
            }
        }

        // Find assigned variables with {% assign %} statements
        let assign_regex = Regex::new(r"\{\%\s*assign\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=").unwrap();
        let mut assigned_variables = std::collections::HashSet::new();
        for captures in assign_regex.captures_iter(&content_without_raw) {
            if let Some(var_match) = captures.get(1) {
                assigned_variables.insert(var_match.as_str().trim().to_string());
            }
        }

        // Also check for loop variables in {% for %} statements
        let for_regex =
            Regex::new(r"\{\%\s*for\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                .unwrap();
        for captures in for_regex.captures_iter(&content_without_raw) {
            if let Some(loop_var) = captures.get(1) {
                // The loop variable is defined by the for loop
                assigned_variables.insert(loop_var.as_str().trim().to_string());
            }
            if let Some(collection_match) = captures.get(2) {
                let collection_name = collection_match.as_str().trim();
                used_variables.insert(collection_name.to_string());
            }
        }

        // Also find variables from {% capture %} blocks
        let capture_regex =
            Regex::new(r"\{\%\s*capture\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\%\}").unwrap();
        for captures in capture_regex.captures_iter(&content_without_raw) {
            if let Some(var_match) = captures.get(1) {
                assigned_variables.insert(var_match.as_str().trim().to_string());
            }
        }

        // Check if all used variables are defined in arguments or sah.toml
        let defined_args: std::collections::HashSet<String> =
            self.arguments.iter().map(|arg| arg.name.clone()).collect();

        // Also include variables from sah.toml configuration
        let mut defined_config_vars = std::collections::HashSet::new();
        if let Ok(Some(config)) = crate::sah_config::load_repo_config_for_cli() {
            for key in config.values().keys() {
                defined_config_vars.insert(key.clone());
            }
        }


        for used_var in &used_variables {
            // Skip if this variable is defined within the template
            if assigned_variables.contains(used_var) {
                continue;
            }

            // Check if it's defined in arguments or sah.toml configuration
            if !defined_args.contains(used_var) && !defined_config_vars.contains(used_var) {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: file_path.to_path_buf(),
                    content_title: Some(self.name.clone()),
                    line: None,
                    column: None,
                    message: format!("Undefined template variable: '{used_var}'"),
                    suggestion: Some(format!(
                        "Add '{used_var}' to the arguments list, define it in sah.toml, or remove the template variable"
                    )),
                });
            }
        }

        // Check for unused arguments (warning)
        for arg in &self.arguments {
            if !used_variables.contains(&arg.name) {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Warning,
                    file_path: file_path.to_path_buf(),
                    content_title: Some(self.name.clone()),
                    line: None,
                    column: None,
                    message: format!("Unused argument: '{}'", arg.name),
                    suggestion: Some(format!(
                        "Remove '{}' from arguments or use it in the template",
                        arg.name
                    )),
                });
            }
        }

        // Check if template has variables but no arguments defined
        if !used_variables.is_empty() && self.arguments.is_empty() {
            issues.push(ValidationIssue {
                level: ValidationLevel::Warning,
                file_path: file_path.to_path_buf(),
                content_title: Some(self.name.clone()),
                line: None,
                column: None,
                message: "Template uses variables but no arguments are defined".to_string(),
                suggestion: Some("Define arguments for the template variables".to_string()),
            });
        }

        issues
    }
}

/// Manages a collection of prompts with storage and retrieval capabilities.
///
/// The [`PromptLibrary`] is the main interface for working with collections of prompts.
/// It provides methods to load prompts from directories, search through them, and
/// manage them programmatically. The library uses a pluggable storage backend
/// system to support different storage strategies.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer::PromptLibrary;
///
/// // Create a new library with default in-memory storage
/// let mut library = PromptLibrary::new();
///
/// // Load prompts from a directory
/// let count = library.add_directory("./.swissarmyhammer/prompts").unwrap();
/// println!("Loaded {} prompts", count);
///
/// // Get a specific prompt
/// let prompt = library.get("code-review").unwrap();
///
/// // Search for prompts
/// let debug_prompts = library.search("debug").unwrap();
/// ```
pub struct PromptLibrary {
    storage: Box<dyn crate::StorageBackend>,
}

impl PromptLibrary {
    /// Creates a new prompt library with default in-memory storage.
    ///
    /// The default storage backend stores prompts in memory, which is suitable
    /// for testing and temporary use. For persistent storage, use
    /// [`with_storage`](Self::with_storage) with a file-based backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::PromptLibrary;
    ///
    /// let library = PromptLibrary::new();
    /// // Library is ready to use with in-memory storage
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: Box::new(crate::storage::MemoryStorage::new()),
        }
    }

    /// Creates a prompt library with a custom storage backend.
    ///
    /// This allows you to use different storage strategies such as file-based
    /// storage, database storage, or custom implementations.
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage backend to use
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, storage::MemoryStorage};
    ///
    /// let storage = Box::new(MemoryStorage::new());
    /// let library = PromptLibrary::with_storage(storage);
    /// ```
    #[must_use]
    pub fn with_storage(storage: Box<dyn crate::StorageBackend>) -> Self {
        Self { storage }
    }

    /// Loads all prompts from a directory and adds them to the library.
    ///
    /// Recursively scans the directory for markdown files (`.md` and `.markdown`)
    /// and loads them as prompts. Files should have YAML front matter with prompt
    /// metadata.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the directory containing prompt files
    ///
    /// # Returns
    ///
    /// The number of prompts successfully loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory does not exist
    /// - I/O errors occur while reading the directory or files
    /// - Storage backend fails to store loaded prompts
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer::PromptLibrary;
    ///
    /// let mut library = PromptLibrary::new();
    /// let count = library.add_directory("./.swissarmyhammer/prompts").unwrap();
    /// println!("Loaded {} prompts from directory", count);
    /// ```
    pub fn add_directory(&mut self, path: impl AsRef<Path>) -> Result<usize> {
        let loader = PromptLoader::new();
        let prompts = loader.load_directory(path)?;
        let count = prompts.len();

        for prompt in prompts {
            self.storage.store(prompt)?;
        }

        Ok(count)
    }

    /// Retrieves a prompt by its name.
    ///
    /// # Arguments
    ///
    /// * `name` - The unique name of the prompt to retrieve
    ///
    /// # Returns
    ///
    /// The prompt if found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The prompt with the specified name is not found
    /// - Storage backend fails to retrieve the prompt
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, Prompt};
    ///
    /// let mut library = PromptLibrary::new();
    ///
    /// // Add a prompt first
    /// let prompt = Prompt::new("test", "Hello {{name}}!");
    /// library.add(prompt).unwrap();
    ///
    /// // Retrieve it
    /// let retrieved = library.get("test").unwrap();
    /// assert_eq!(retrieved.name, "test");
    /// ```
    pub fn get(&self, name: &str) -> Result<Prompt> {
        self.storage.get(name)
    }

    /// Lists all prompts in the library.
    ///
    /// # Returns
    ///
    /// A vector of all prompts currently stored in the library.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Storage backend fails to list prompts
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, Prompt};
    ///
    /// let mut library = PromptLibrary::new();
    /// library.add(Prompt::new("test1", "Template 1")).unwrap();
    /// library.add(Prompt::new("test2", "Template 2")).unwrap();
    ///
    /// let prompts = library.list().unwrap();
    /// assert_eq!(prompts.len(), 2);
    /// ```
    pub fn list(&self) -> Result<Vec<Prompt>> {
        self.storage.list()
    }

    /// Renders a prompt with partial support
    ///
    /// This method renders the specified prompt with access to all prompts in the library
    /// as partials, enabling the use of `{% render %}` tags.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to render
    /// * `args` - Template variables as key-value pairs
    ///
    /// # Returns
    ///
    /// The rendered prompt content, or an error if the prompt is not found or rendering fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer::PromptLibrary;
    /// use std::collections::HashMap;
    ///
    /// let library = PromptLibrary::new();
    /// let mut args = HashMap::new();
    /// args.insert("name".to_string(), "World".to_string());
    ///
    /// let result = library.render_prompt("greeting", &args).unwrap();
    /// ```
    pub fn render_prompt(&self, name: &str, args: &HashMap<String, String>) -> Result<String> {
        let prompt = self.get(name)?;
        prompt.render_with_partials(
            args,
            Arc::new(Self {
                storage: self.storage.clone_box(),
            }),
        )
    }

    /// Renders a prompt with the given arguments and environment variables.
    ///
    /// Retrieves the prompt by name and renders it with both the provided arguments
    /// and environment variables. The provided arguments take precedence over
    /// environment variables with the same name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to render
    /// * `args` - Template variables as key-value pairs
    ///
    /// # Returns
    ///
    /// The rendered template string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template parsing fails due to invalid Liquid syntax
    /// - Required arguments are missing from the provided arguments map
    /// - Template rendering fails during execution
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer::PromptLibrary;
    /// use std::collections::HashMap;
    ///
    /// let library = PromptLibrary::new();
    /// let mut args = HashMap::new();
    /// args.insert("app".to_string(), "myapp".to_string());
    /// // Environment variables like USER will be available automatically
    ///
    /// let result = library.render_prompt_with_env("deploy", &args).unwrap();
    /// ```
    pub fn render_prompt_with_env(
        &self,
        name: &str,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        let prompt = self.get(name)?;
        prompt.render_with_partials_and_env(
            args,
            Arc::new(Self {
                storage: self.storage.clone_box(),
            }),
        )
    }

    /// Searches for prompts matching the given query.
    ///
    /// The search implementation depends on the storage backend. Basic implementations
    /// search through prompt names, descriptions, and content. Advanced backends
    /// may provide full-text search capabilities.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    ///
    /// # Returns
    ///
    /// A vector of prompts matching the search query.
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, Prompt};
    ///
    /// let mut library = PromptLibrary::new();
    /// library.add(Prompt::new("debug-js", "Debug JavaScript code")
    ///     .with_description("Helps debug JavaScript errors")).unwrap();
    /// library.add(Prompt::new("format-py", "Format Python code")).unwrap();
    ///
    /// let results = library.search("debug").unwrap();
    /// assert_eq!(results.len(), 1);
    /// assert_eq!(results[0].name, "debug-js");
    /// ```
    pub fn search(&self, query: &str) -> Result<Vec<Prompt>> {
        self.storage.search(query)
    }

    /// Lists prompts filtered by the given criteria.
    ///
    /// This method provides a flexible way to filter prompts based on various criteria
    /// such as source, category, search terms, and argument requirements. It works
    /// with a `PromptResolver` to determine prompt sources.
    ///
    /// # Arguments
    ///
    /// * `filter` - A `PromptFilter` specifying the filtering criteria
    /// * `sources` - A `HashMap` mapping prompt names to their sources
    ///
    /// # Returns
    ///
    /// A vector of prompts matching all the specified filter criteria.
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, PromptFilter, PromptSource, Prompt};
    /// use std::collections::HashMap;
    ///
    /// let mut library = PromptLibrary::new();
    /// library.add(Prompt::new("code-review", "Review code")
    ///     .with_category("development")).unwrap();
    /// library.add(Prompt::new("write-essay", "Write essay")
    ///     .with_category("writing")).unwrap();
    ///
    /// let filter = PromptFilter::new().with_category("development");
    /// let sources = HashMap::new(); // Empty sources for this example
    /// let results = library.list_filtered(&filter, &sources).unwrap();
    /// assert_eq!(results.len(), 1);
    /// assert_eq!(results[0].name, "code-review");
    /// ```
    pub fn list_filtered(
        &self,
        filter: &crate::prompt_filter::PromptFilter,
        sources: &HashMap<String, crate::PromptSource>,
    ) -> Result<Vec<Prompt>> {
        let all_prompts = self.list()?;
        Ok(filter.apply(all_prompts, sources))
    }

    /// Adds a single prompt to the library.
    ///
    /// If a prompt with the same name already exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to add
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, Prompt};
    ///
    /// let mut library = PromptLibrary::new();
    /// let prompt = Prompt::new("example", "Example template");
    /// library.add(prompt).unwrap();
    ///
    /// assert!(library.get("example").is_ok());
    /// ```
    pub fn add(&mut self, prompt: Prompt) -> Result<()> {
        self.storage.store(prompt)
    }

    /// Removes a prompt from the library.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the prompt to remove
    ///
    /// # Returns
    ///
    /// Ok(()) if the prompt was removed, or an error if it doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer::{PromptLibrary, Prompt};
    ///
    /// let mut library = PromptLibrary::new();
    /// library.add(Prompt::new("temp", "Temporary prompt")).unwrap();
    ///
    /// library.remove("temp").unwrap();
    /// assert!(library.get("temp").is_err());
    /// ```
    pub fn remove(&mut self, name: &str) -> Result<()> {
        self.storage.remove(name)
    }
}

impl Default for PromptLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Loads prompts from various sources
pub struct PromptLoader {
    /// File extensions to consider
    extensions: Vec<String>,
}

impl PromptLoader {
    /// Create a new prompt loader
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: vec![
                "md".to_string(),
                "md.liquid".to_string(),
                "markdown".to_string(),
                "markdown.liquid".to_string(),
                "liquid".to_string(),
                "liquid.md".to_string(),
                "liquid.markdown".to_string(),
            ],
        }
    }

    /// Load prompts from a directory
    pub fn load_directory(&self, path: impl AsRef<Path>) -> Result<Vec<Prompt>> {
        let path = path.as_ref();
        let mut prompts = Vec::new();

        if !path.exists() {
            return Err(SwissArmyHammerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {}", path.display()),
            )));
        }

        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let entry_path = entry.path();
            if entry_path.is_file() && self.is_prompt_file(entry_path) {
                if let Ok(prompt) = self.load_file_with_base(entry_path, path) {
                    prompts.push(prompt);
                }
            }
        }

        Ok(prompts)
    }

    /// Load a single prompt file
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<Prompt> {
        self.load_file_with_base(
            path.as_ref(),
            path.as_ref().parent().unwrap_or_else(|| path.as_ref()),
        )
    }

    /// Load a single prompt file with base path for relative naming
    fn load_file_with_base(&self, path: &Path, base_path: &Path) -> Result<Prompt> {
        let content = std::fs::read_to_string(path)?;

        let (metadata, template) = Self::parse_front_matter(&content)?;

        let name = self.extract_prompt_name_with_base(path, base_path);

        let mut prompt = Prompt::new(name, template);
        prompt.source = Some(path.to_path_buf());

        // Check if this is a partial template before processing metadata
        let has_partial_marker = content.trim_start().starts_with("{% partial %}");

        // Parse metadata
        if let Some(ref metadata_value) = metadata {
            if let Some(title) = metadata_value
                .get("title")
                .and_then(serde_json::Value::as_str)
            {
                prompt.metadata.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }
            if let Some(desc) = metadata_value
                .get("description")
                .and_then(serde_json::Value::as_str)
            {
                prompt.description = Some(desc.to_string());
            }
            if let Some(cat) = metadata_value
                .get("category")
                .and_then(serde_json::Value::as_str)
            {
                prompt.category = Some(cat.to_string());
            }
            if let Some(tags) = metadata_value.get("tags").and_then(|v| v.as_array()) {
                prompt.tags = tags
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect();
            }
            // Check both "parameters" (standard) and "arguments" (legacy) field names
            let args_array = metadata_value.get("parameters")
                .or_else(|| metadata_value.get("arguments"))
                .and_then(|v| v.as_array());
            
            if let Some(args) = args_array {
                for arg in args {
                    if let Some(arg_obj) = arg.as_object() {
                        let name = arg_obj
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_string();

                        let arg_spec = ArgumentSpec {
                            name,
                            description: arg_obj
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                            required: arg_obj
                                .get("required")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                            default: arg_obj
                                .get("default")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                            type_hint: arg_obj
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                        };

                        prompt.arguments.push(arg_spec);
                    }
                }
            }
        }

        // If this is a partial template (no metadata), set appropriate description
        if prompt.description.is_none()
            && (has_partial_marker || Self::is_likely_partial(&prompt.name, &content))
        {
            prompt.description = Some("Partial template for reuse in other prompts".to_string());
        }

        Ok(prompt)
    }

    /// Determine if a prompt is likely a partial template
    fn is_likely_partial(name: &str, content: &str) -> bool {
        // Check if the name suggests it's a partial (common naming patterns)
        let name_lower = name.to_lowercase();
        if name_lower.contains("partial") || name_lower.starts_with('_') {
            return true;
        }

        // Check if it has no YAML front matter (partials often don't)
        let has_front_matter = content.starts_with("---\n");
        if !has_front_matter {
            return true;
        }

        // Check for typical partial characteristics:
        // - Short content that looks like a fragment
        // - Contains mostly template variables
        // - Doesn't have typical prompt structure
        let lines: Vec<&str> = content.lines().collect();
        let content_lines: Vec<&str> = if has_front_matter {
            // Skip YAML front matter
            lines
                .iter()
                .skip_while(|line| **line != "---")
                .skip(1)
                .skip_while(|line| **line != "---")
                .skip(1)
                .copied()
                .collect()
        } else {
            lines
        };

        // If it's very short and has no headers, it might be a partial
        if content_lines.len() <= 5 && !content_lines.iter().any(|line| line.starts_with('#')) {
            return true;
        }

        false
    }

    /// Load a prompt from a string
    pub fn load_from_string(&self, name: &str, content: &str) -> Result<Prompt> {
        let (metadata, template) = Self::parse_front_matter(content)?;

        let mut prompt = Prompt::new(name, template);

        // Check if this is a partial template before processing metadata
        let has_partial_marker = content.trim_start().starts_with("{% partial %}");

        // Parse metadata
        if let Some(ref metadata_value) = metadata {
            if let Some(title) = metadata_value
                .get("title")
                .and_then(serde_json::Value::as_str)
            {
                prompt.metadata.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }
            if let Some(desc) = metadata_value
                .get("description")
                .and_then(serde_json::Value::as_str)
            {
                prompt.description = Some(desc.to_string());
            }
            if let Some(cat) = metadata_value
                .get("category")
                .and_then(serde_json::Value::as_str)
            {
                prompt.category = Some(cat.to_string());
            }
            if let Some(tags) = metadata_value.get("tags").and_then(|v| v.as_array()) {
                prompt.tags = tags
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }

            // Parse arguments
            // Check both "parameters" (standard) and "arguments" (legacy) field names
            let args_array = metadata_value.get("parameters")
                .or_else(|| metadata_value.get("arguments"))
                .and_then(|v| v.as_array());
            
            if let Some(args) = args_array {
                for arg in args {
                    if let Some(arg_obj) = arg.as_object() {
                        let arg_spec = ArgumentSpec {
                            name: arg_obj
                                .get("name")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            description: arg_obj
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                            required: arg_obj
                                .get("required")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                            default: arg_obj
                                .get("default")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                            type_hint: arg_obj
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .map(String::from),
                        };

                        prompt.arguments.push(arg_spec);
                    }
                }
            }
        }

        // If this is a partial template (no metadata), set appropriate description
        if prompt.description.is_none()
            && (has_partial_marker || Self::is_likely_partial(&prompt.name, content))
        {
            prompt.description = Some("Partial template for reuse in other prompts".to_string());
        }

        Ok(prompt)
    }

    /// Check if a path is a prompt file
    fn is_prompt_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        self.extensions
            .iter()
            .any(|ext| path_str.ends_with(&format!(".{ext}")))
    }

    /// Parse front matter from content
    fn parse_front_matter(content: &str) -> Result<(Option<serde_json::Value>, String)> {
        // Check for partial marker first
        if content.trim_start().starts_with("{% partial %}") {
            // This is a partial template, no front matter expected
            return Ok((None, content.to_string()));
        }

        if content.starts_with("---\n") {
            let parts: Vec<&str> = content.splitn(3, "---\n").collect();
            if parts.len() >= 3 {
                let yaml_content = parts[1];
                let template = parts[2].trim_start().to_string();

                let metadata: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
                let json_value = serde_json::to_value(metadata)
                    .map_err(|e| SwissArmyHammerError::Other(e.to_string()))?;

                return Ok((Some(json_value), template));
            }
        }

        Ok((None, content.to_string()))
    }

    /// Extract prompt name from file path, handling compound extensions
    fn extract_prompt_name(&self, path: &Path) -> String {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        // Sort extensions by length descending to match longest first
        let mut sorted_extensions = self.extensions.clone();
        sorted_extensions.sort_by_key(|b| std::cmp::Reverse(b.len()));

        // Remove supported extensions, checking longest first
        for ext in &sorted_extensions {
            let extension = format!(".{ext}");
            if filename.ends_with(&extension) {
                return filename[..filename.len() - extension.len()].to_string();
            }
        }

        // Fallback to file_stem behavior
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Extract prompt name with relative path from base directory
    fn extract_prompt_name_with_base(&self, path: &Path, base_path: &Path) -> String {
        // Get relative path from base
        let relative_path = path.strip_prefix(base_path).unwrap_or(path);

        // Get the path without the filename
        let mut name_path = String::new();
        if let Some(parent) = relative_path.parent() {
            if parent != Path::new("") {
                name_path = parent.to_string_lossy().replace('\\', "/");
                name_path.push('/');
            }
        }

        // Extract filename without extension
        let filename = self.extract_prompt_name(path);
        name_path.push_str(&filename);

        name_path
    }
}

impl Default for PromptLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_creation() {
        let prompt = Prompt::new("test", "Hello {{ name }}!");
        assert_eq!(prompt.name, "test");
        assert_eq!(prompt.template, "Hello {{ name }}!");
    }

    #[test]
    fn test_prompt_render() {
        let prompt = Prompt::new("test", "Hello {{ name }}!").add_argument(ArgumentSpec {
            name: "name".to_string(),
            description: None,
            required: true,
            default: None,
            type_hint: None,
        });

        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());

        let result = prompt.render(&args).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_extension_stripping() {
        let loader = PromptLoader::new();

        // Test various extensions
        let test_cases = vec![
            ("test.md", "test"),
            ("test.liquid.md", "test"),
            ("test.md.liquid", "test"),
            ("test.liquid", "test"),
            ("partials/header.liquid.md", "header"),
        ];

        for (filename, expected) in test_cases {
            let path = std::path::Path::new(filename);
            let result = loader.extract_prompt_name(path);
            println!("File: {filename} -> Name: {result} (expected: {expected})");
            assert_eq!(result, expected, "Failed for {filename}");
        }
    }

    #[test]
    fn test_prompt_loader_loads_only_valid_prompts() {
        use std::fs;
        use tempfile::TempDir;

        // This test verifies that PromptLoader only successfully loads files
        // that are valid prompts (with proper YAML front matter)
        let temp_dir = TempDir::new().unwrap();

        // Create some directories with invalid markdown files
        let test_dirs = ["issues", "doc", "examples"];

        for dir_name in &test_dirs {
            let dir_path = temp_dir.path().join(dir_name);
            fs::create_dir_all(&dir_path).unwrap();

            // Create a markdown file without YAML front matter (will be skipped during loading)
            let file_path = dir_path.join("invalid.md");
            fs::write(
                &file_path,
                "# Just a regular markdown file\n\nNo YAML front matter here.",
            )
            .unwrap();
        }

        // Create a valid prompt that SHOULD be loaded
        let valid_prompt = temp_dir.path().join("valid.md");
        let valid_content = r"---
title: Valid Prompt
description: A valid prompt for testing
arguments:
  - name: topic
    description: The topic
    required: true
---

# Valid Prompt

Discuss {{topic}}.
";
        fs::write(&valid_prompt, valid_content).unwrap();

        // Create another valid prompt in a subdirectory
        let sub_dir = temp_dir.path().join("prompts");
        fs::create_dir_all(&sub_dir).unwrap();
        let sub_prompt = sub_dir.join("another.md");
        let sub_content = r"---
title: Another Prompt
description: Another valid prompt
---

This is another prompt.
";
        fs::write(&sub_prompt, sub_content).unwrap();

        let loader = PromptLoader::new();
        let prompts = loader.load_directory(temp_dir.path()).unwrap();

        // Should load all markdown files (5 total: 3 invalid + 2 valid)
        // But only the valid ones will have proper metadata
        assert_eq!(
            prompts.len(),
            5,
            "Should load 5 prompts total, but loaded: {}",
            prompts.len()
        );

        // All prompts should now have descriptions (either from metadata or default for partials)
        let prompts_with_descriptions: Vec<&Prompt> =
            prompts.iter().filter(|p| p.description.is_some()).collect();

        assert_eq!(
            prompts_with_descriptions.len(),
            5,
            "All 5 prompts should have descriptions (2 from metadata, 3 default for partials)"
        );

        // Check that the invalid ones (now treated as partials) have the default description
        let partials: Vec<&Prompt> = prompts
            .iter()
            .filter(|p| {
                p.description.as_deref() == Some("Partial template for reuse in other prompts")
            })
            .collect();
        assert_eq!(
            partials.len(),
            3,
            "Should have 3 partials with default description"
        );

        // Check that the valid ones have their original descriptions
        let prompts_with_custom_desc: Vec<&Prompt> = prompts
            .iter()
            .filter(|p| {
                p.description.is_some()
                    && p.description.as_deref()
                        != Some("Partial template for reuse in other prompts")
            })
            .collect();
        assert_eq!(
            prompts_with_custom_desc.len(),
            2,
            "Should have 2 prompts with custom descriptions"
        );

        let prompt_names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();
        assert!(prompt_names.contains(&"valid".to_string()));
        assert!(prompt_names.contains(&"prompts/another".to_string()));
    }

    #[test]
    fn test_shared_parameter_system_integration() {
        use crate::common::ParameterProvider;

        let prompt = Prompt::new("test", "Hello {{name}}!").add_argument(ArgumentSpec {
            name: "name".to_string(),
            description: Some("Name to greet".to_string()),
            required: true,
            default: None,
            type_hint: Some("string".to_string()),
        });

        // Test that ParameterProvider trait works
        let parameters = prompt.get_parameters();
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[0].description, "Name to greet");
        assert!(parameters[0].required);
        assert_eq!(parameters[0].parameter_type.as_str(), "string");
    }

    #[test]
    fn test_partial_template_without_description() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create a partial template without front matter (common for partials)
        let partial_path = temp_dir.path().join("_header.liquid.md");
        let partial_content = r#"<div class="header">
  <h1>{{title}}</h1>
  <p>{{subtitle}}</p>
</div>"#;
        fs::write(&partial_path, partial_content).unwrap();

        // Create another partial with underscore naming pattern
        let partial2_path = temp_dir.path().join("_footer.md");
        let partial2_content = r"<footer>
  Copyright {{year}} {{company}}
</footer>";
        fs::write(&partial2_path, partial2_content).unwrap();

        // Create a partial with "partial" in the name
        let partial3_path = temp_dir.path().join("header-partial.md");
        let partial3_content = r"## {{section_title}}
{{section_content}}";
        fs::write(&partial3_path, partial3_content).unwrap();

        let loader = PromptLoader::new();
        let prompts = loader.load_directory(temp_dir.path()).unwrap();

        assert_eq!(prompts.len(), 3, "Should load 3 partial templates");

        // Check that partials now have default descriptions
        for prompt in &prompts {
            assert_eq!(
                prompt.description.as_deref(),
                Some("Partial template for reuse in other prompts"),
                "Partial '{}' should have default description",
                prompt.name
            );
        }
    }
}
