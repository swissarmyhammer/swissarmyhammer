//! # SwissArmyHammer Templating
//!
//! A domain-focused templating library built on Liquid templates for SwissArmyHammer.
//!
//! This crate provides a clean, security-focused API for template processing with:
//! - Liquid template engine integration
//! - Custom filters for text processing
//! - Flexible partial template system
//! - Security validation for trusted/untrusted templates
//! - Integration with SwissArmyHammer configuration system
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use swissarmyhammer_templating::{Template, TemplateEngine};
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a simple template
//! let template = Template::new("Hello {{ name }}!")?;
//!
//! // Render with arguments
//! let mut args = HashMap::new();
//! args.insert("name".to_string(), "World".to_string());
//! let result = template.render(&args)?;
//!
//! println!("{}", result); // "Hello World!"
//!
//! // Or use the template engine directly
//! let engine = TemplateEngine::new();
//! let result = engine.render("Hello {{ name }}!", &args)?;
//! println!("{}", result); // "Hello World!"
//! # Ok(())
//! # }
//! ```
//!
//! ## Security
//!
//! Templates are automatically validated for security risks:
//!
//! ```rust,no_run
//! use swissarmyhammer_templating::Template;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Trusted templates (from your application)
//! let trusted = Template::new_trusted("Hello {{ name }}!")?;
//!
//! // Untrusted templates (from user input) - strict validation
//! let untrusted = Template::new_untrusted("Hello {{ name }}!")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Partial Templates
//!
//! Extend templates with partial loading:
//!
//! ```rust,no_run
//! use swissarmyhammer_templating::{Template, PartialLoader};
//! use std::borrow::Cow;
//! use std::collections::HashMap;
//!
//! #[derive(Debug)]
//! struct MyPartialLoader {
//!     partials: HashMap<String, String>,
//! }
//!
//! impl PartialLoader for MyPartialLoader {
//!     fn contains(&self, name: &str) -> bool {
//!         self.partials.contains_key(name)
//!     }
//!     
//!     fn names(&self) -> Vec<String> {
//!         self.partials.keys().cloned().collect()
//!     }
//!     
//!     fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
//!         self.partials.get(name).map(|s| Cow::Borrowed(s.as_str()))
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut loader = MyPartialLoader { partials: HashMap::new() };
//! loader.partials.insert("header".to_string(), "# Header".to_string());
//!
//! let template = Template::with_partials("{% include 'header' %}", loader)?;
//! let args = HashMap::new();
//! let result = template.render(&args)?;
//! println!("{}", result); // "# Header"
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

/// Error types for template operations
pub mod error;

/// Security validation for template content
pub mod security;

/// Custom template filters for text processing
pub mod filters;

/// Template variable extraction utilities
pub mod variables;

/// Partial template loading system
pub mod partials;

/// Core template functionality
pub mod template;

/// Template engine for processing and rendering templates
pub mod engine;

// Re-export core types for convenience
pub use engine::TemplateEngine;
pub use error::{Result, TemplatingError};
pub use partials::{
    HashMapPartialLoader, LibraryPartialAdapter, PartialLoader, PartialLoaderAdapter, PartialTag,
    TemplateContentProvider,
};
pub use security::{
    validate_template_security, MAX_TEMPLATE_RECURSION_DEPTH, MAX_TEMPLATE_RENDER_TIME_MS,
    MAX_TEMPLATE_SIZE, MAX_TEMPLATE_VARIABLES,
};
pub use template::{create_default_parser, create_parser_with_partials, Template};
pub use variables::{create_well_known_variables, extract_template_variables};

// Re-export filter functions for convenience
pub use filters::{
    count_lines_in_string, indent_string, preprocess_custom_filters, slugify_string,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        engine::TemplateEngine,
        error::{Result, TemplatingError},
        filters::{count_lines_in_string, indent_string, slugify_string},
        partials::{PartialLoader, PartialLoaderAdapter},
        security::validate_template_security,
        template::Template,
        variables::extract_template_variables,
    };
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_basic_template_workflow() {
        let template = Template::new("Hello {{ name }}!").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Integration Test".to_string());

        let result = template.render(&args).unwrap();
        assert_eq!(result, "Hello Integration Test!");
    }

    #[test]
    fn test_engine_workflow() {
        let engine = TemplateEngine::new();
        let mut args = HashMap::new();
        args.insert("message".to_string(), "Engine Test".to_string());

        let result = engine.render("Message: {{ message }}", &args).unwrap();
        assert_eq!(result, "Message: Engine Test");
    }

    #[test]
    fn test_custom_filters() {
        let mut args = HashMap::new();
        args.insert("title".to_string(), "Hello World!".to_string());
        args.insert("text".to_string(), "line1\nline2".to_string());

        let slug = slugify_string("Hello World!");
        assert_eq!(slug, "hello-world");

        let lines = count_lines_in_string("line1\nline2");
        assert_eq!(lines, 2);

        let indented = indent_string("test", 2);
        assert_eq!(indented, "  test");
    }

    #[test]
    fn test_variable_extraction() {
        let template = "Hello {{ name }}! You have {{ count }} items.";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"count".to_string()));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_security_validation() {
        // Safe template should pass
        assert!(validate_template_security("Hello {{ name }}!", false).is_ok());

        // Templates with include should now pass (no longer blocked)
        assert!(validate_template_security("{% include 'header' %}", false).is_ok());

        // Large template should fail for untrusted
        let large_template = "a".repeat(MAX_TEMPLATE_SIZE + 1);
        assert!(validate_template_security(&large_template, false).is_err());
    }

    #[test]
    fn test_well_known_variables() {
        let vars = create_well_known_variables();
        assert!(vars.contains_key("issues_directory"));
    }
}
