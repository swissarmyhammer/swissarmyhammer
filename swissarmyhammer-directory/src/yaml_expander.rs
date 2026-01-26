//! YAML include expansion for configuration files.
//!
//! This module provides functionality to expand `@path/to/file` references
//! in YAML content. Referenced files are loaded from the standard directory
//! hierarchy (builtin → user → local) with proper precedence.
//!
//! # Example
//!
//! Given a file `file_groups/source_code.yaml`:
//! ```yaml
//! - "*.js"
//! - "*.ts"
//! - "*.py"
//! ```
//!
//! You can reference it in any YAML frontmatter or config:
//! ```yaml
//! match:
//!   files:
//!     - "@file_groups/source_code"
//!     - "*.custom"
//! ```
//!
//! The `@file_groups/source_code` will be expanded to the contents of the
//! referenced file.

use std::collections::HashMap;
use std::marker::PhantomData;

use crate::config::DirectoryConfig;
use crate::error::{DirectoryError, Result};
use crate::file_loader::VirtualFileSystem;

/// Expands `@path/to/file` references in YAML values.
///
/// The expander loads all `.yaml` and `.yml` files from the managed directory
/// hierarchy and makes them available for inclusion via `@` references.
///
/// # Type Parameters
///
/// * `C` - A type implementing `DirectoryConfig` that specifies the directory
///   configuration (e.g., `SwissarmyhammerConfig` or `AvpConfig`).
///
/// # Precedence
///
/// Files are loaded with standard precedence (later overrides earlier):
/// 1. Builtin files (added via `add_builtin`)
/// 2. User files (~/.swissarmyhammer/ or ~/.avp/)
/// 3. Local files (./.swissarmyhammer/ or ./.avp/)
#[derive(Debug)]
pub struct YamlExpander<C: DirectoryConfig> {
    /// Loaded YAML files indexed by their relative path (without extension).
    includes: HashMap<String, serde_yaml::Value>,
    /// Phantom data for the configuration type.
    _phantom: PhantomData<C>,
}

impl<C: DirectoryConfig> Default for YamlExpander<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: DirectoryConfig> YamlExpander<C> {
    /// Create a new empty YAML expander.
    pub fn new() -> Self {
        Self {
            includes: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Load all YAML files from the standard directory hierarchy.
    ///
    /// This loads `.yaml` and `.yml` files from:
    /// 1. User directory (~/<DIR_NAME>/)
    /// 2. Local directory (./<DIR_NAME>/)
    ///
    /// Call `add_builtin` before this to include builtin files.
    pub fn load_all(&mut self) -> Result<()> {
        // Use VirtualFileSystem with empty subdirectory to load from root
        let mut vfs = VirtualFileSystem::<C>::new("");

        // Load from all directories
        if let Err(e) = vfs.load_all() {
            tracing::warn!("Failed to load YAML includes from some directories: {}", e);
        }

        // Parse each YAML file
        for file_entry in vfs.list() {
            // Only process .yaml and .yml files
            let path_str = file_entry.path.to_string_lossy();
            if !path_str.ends_with(".yaml") && !path_str.ends_with(".yml") {
                continue;
            }

            match serde_yaml::from_str::<serde_yaml::Value>(&file_entry.content) {
                Ok(value) => {
                    tracing::debug!(
                        "Loaded YAML include '{}' from {} ({})",
                        file_entry.name,
                        file_entry.source,
                        file_entry.path.display()
                    );
                    self.includes.insert(file_entry.name.clone(), value);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse YAML file '{}': {}",
                        file_entry.path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Add a builtin YAML include.
    ///
    /// # Arguments
    ///
    /// * `name` - The include name (e.g., "file_groups/source_code")
    /// * `content` - The YAML content as a string
    pub fn add_builtin(&mut self, name: &str, content: &str) -> Result<()> {
        let value = serde_yaml::from_str(content).map_err(|e| DirectoryError::Other {
            message: format!("Failed to parse builtin YAML '{}': {}", name, e),
        })?;
        self.includes.insert(name.to_string(), value);
        Ok(())
    }

    /// Get a loaded include by name.
    pub fn get(&self, name: &str) -> Option<&serde_yaml::Value> {
        self.includes.get(name)
    }

    /// List all loaded include names.
    pub fn list_names(&self) -> Vec<&String> {
        self.includes.keys().collect()
    }

    /// Expand `@` references in a YAML value.
    ///
    /// This recursively walks the value tree and replaces string values
    /// starting with `@` with the contents of the referenced include.
    ///
    /// # Expansion Rules
    ///
    /// - `@path/to/file` in a sequence: The referenced value (must be a sequence)
    ///   is spliced into the parent sequence at that position.
    /// - `@path/to/file` as a standalone value: Replaced with the referenced value.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A referenced include is not found
    /// - A sequence expansion references a non-sequence value
    pub fn expand(&self, value: serde_yaml::Value) -> Result<serde_yaml::Value> {
        self.expand_value(value, &mut Vec::new())
    }

    /// Internal recursive expansion with cycle detection.
    fn expand_value(
        &self,
        value: serde_yaml::Value,
        visited: &mut Vec<String>,
    ) -> Result<serde_yaml::Value> {
        match value {
            serde_yaml::Value::String(s) if s.starts_with('@') => {
                let include_name = &s[1..]; // Strip the @

                // Check for cycles
                if visited.contains(&include_name.to_string()) {
                    return Err(DirectoryError::Other {
                        message: format!(
                            "Circular include detected: {} -> {}",
                            visited.join(" -> "),
                            include_name
                        ),
                    });
                }

                // Look up the include
                let included =
                    self.includes
                        .get(include_name)
                        .ok_or_else(|| DirectoryError::Other {
                            message: format!("Include not found: @{}", include_name),
                        })?;

                // Recursively expand the included value
                visited.push(include_name.to_string());
                let expanded = self.expand_value(included.clone(), visited)?;
                visited.pop();

                Ok(expanded)
            }
            serde_yaml::Value::Sequence(seq) => {
                let mut expanded_seq = Vec::new();

                for item in seq {
                    // Check if this is an @include that should be spliced
                    if let serde_yaml::Value::String(s) = &item {
                        if s.starts_with('@') {
                            let include_name = &s[1..];

                            // Check for cycles
                            if visited.contains(&include_name.to_string()) {
                                return Err(DirectoryError::Other {
                                    message: format!(
                                        "Circular include detected: {} -> {}",
                                        visited.join(" -> "),
                                        include_name
                                    ),
                                });
                            }

                            // Look up and expand
                            let included = self.includes.get(include_name).ok_or_else(|| {
                                DirectoryError::Other {
                                    message: format!("Include not found: @{}", include_name),
                                }
                            })?;

                            visited.push(include_name.to_string());
                            let expanded = self.expand_value(included.clone(), visited)?;
                            visited.pop();

                            // Splice sequences, otherwise just add the value
                            if let serde_yaml::Value::Sequence(included_seq) = expanded {
                                expanded_seq.extend(included_seq);
                            } else {
                                expanded_seq.push(expanded);
                            }
                            continue;
                        }
                    }

                    // Regular item, just expand recursively
                    expanded_seq.push(self.expand_value(item, visited)?);
                }

                Ok(serde_yaml::Value::Sequence(expanded_seq))
            }
            serde_yaml::Value::Mapping(map) => {
                let mut expanded_map = serde_yaml::Mapping::new();

                for (key, val) in map {
                    let expanded_key = self.expand_value(key, visited)?;
                    let expanded_val = self.expand_value(val, visited)?;
                    expanded_map.insert(expanded_key, expanded_val);
                }

                Ok(serde_yaml::Value::Mapping(expanded_map))
            }
            // Other value types pass through unchanged
            other => Ok(other),
        }
    }

    /// Parse a YAML string and expand includes.
    ///
    /// This is a convenience method that combines parsing and expansion.
    pub fn parse_and_expand(&self, yaml: &str) -> Result<serde_yaml::Value> {
        let value = serde_yaml::from_str(yaml).map_err(|e| DirectoryError::Other {
            message: format!("Failed to parse YAML: {}", e),
        })?;
        self.expand(value)
    }

    /// Parse a YAML string, expand includes, and deserialize to a type.
    ///
    /// This is a convenience method for the common case of parsing YAML
    /// config into a typed struct.
    pub fn parse_yaml<T: serde::de::DeserializeOwned>(&self, yaml: &str) -> Result<T> {
        let expanded = self.parse_and_expand(yaml)?;
        serde_yaml::from_value(expanded).map_err(|e| DirectoryError::Other {
            message: format!("Failed to deserialize YAML: {}", e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SwissarmyhammerConfig;

    #[test]
    fn test_expander_new() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();
        assert!(expander.includes.is_empty());
    }

    #[test]
    fn test_add_builtin() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.js"
- "*.ts"
- "*.py"
"#,
            )
            .unwrap();

        assert!(expander.get("file_groups/source_code").is_some());
    }

    #[test]
    fn test_expand_simple_reference() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.js"
- "*.ts"
"#,
            )
            .unwrap();

        // Note: @ must be quoted in YAML
        let input: serde_yaml::Value =
            serde_yaml::from_str("\"@file_groups/source_code\"").unwrap();
        let expanded = expander.expand(input).unwrap();

        assert!(expanded.is_sequence());
        let seq = expanded.as_sequence().unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].as_str(), Some("*.js"));
        assert_eq!(seq[1].as_str(), Some("*.ts"));
    }

    #[test]
    fn test_expand_in_sequence() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.js"
- "*.ts"
"#,
            )
            .unwrap();

        let input: serde_yaml::Value = serde_yaml::from_str(
            r#"
- "@file_groups/source_code"
- "*.custom"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();

        assert!(expanded.is_sequence());
        let seq = expanded.as_sequence().unwrap();
        assert_eq!(seq.len(), 3); // 2 from include + 1 custom
        assert_eq!(seq[0].as_str(), Some("*.js"));
        assert_eq!(seq[1].as_str(), Some("*.ts"));
        assert_eq!(seq[2].as_str(), Some("*.custom"));
    }

    #[test]
    fn test_expand_in_mapping() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.js"
- "*.ts"
"#,
            )
            .unwrap();

        let input: serde_yaml::Value = serde_yaml::from_str(
            r#"
match:
  files:
    - "@file_groups/source_code"
    - "*.custom"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();

        let files = expanded
            .get("match")
            .and_then(|m| m.get("files"))
            .and_then(|f| f.as_sequence())
            .unwrap();

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].as_str(), Some("*.js"));
        assert_eq!(files[1].as_str(), Some("*.ts"));
        assert_eq!(files[2].as_str(), Some("*.custom"));
    }

    #[test]
    fn test_expand_nested_includes() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin(
                "base/js",
                r#"
- "*.js"
- "*.mjs"
"#,
            )
            .unwrap();
        expander
            .add_builtin(
                "file_groups/frontend",
                r#"
- "@base/js"
- "*.css"
"#,
            )
            .unwrap();

        // Note: @ must be quoted in YAML
        let input: serde_yaml::Value = serde_yaml::from_str("\"@file_groups/frontend\"").unwrap();
        let expanded = expander.expand(input).unwrap();

        let seq = expanded.as_sequence().unwrap();
        assert_eq!(seq.len(), 3);
        assert_eq!(seq[0].as_str(), Some("*.js"));
        assert_eq!(seq[1].as_str(), Some("*.mjs"));
        assert_eq!(seq[2].as_str(), Some("*.css"));
    }

    #[test]
    fn test_expand_circular_reference_detected() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        // Note: @ must be quoted in YAML
        expander.add_builtin("a", "\"@b\"").unwrap();
        expander.add_builtin("b", "\"@a\"").unwrap();

        let input: serde_yaml::Value = serde_yaml::from_str("\"@a\"").unwrap();
        let result = expander.expand(input);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular include"));
    }

    #[test]
    fn test_expand_not_found() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        // Note: @ must be quoted in YAML
        let input: serde_yaml::Value = serde_yaml::from_str("\"@nonexistent/file\"").unwrap();
        let result = expander.expand(input);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Include not found"));
    }

    #[test]
    fn test_parse_yaml_with_expansion() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("patterns", r#"["*.js", "*.ts"]"#)
            .unwrap();

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            files: Vec<String>,
        }

        let config: Config = expander
            .parse_yaml(
                r#"
files:
  - "@patterns"
  - "*.custom"
"#,
            )
            .unwrap();

        assert_eq!(config.files, vec!["*.js", "*.ts", "*.custom"]);
    }

    #[test]
    fn test_non_reference_strings_unchanged() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        let input: serde_yaml::Value = serde_yaml::from_str(
            r#"
- "regular string"
- "email@example.com"
- "not @a reference"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();
        let seq = expanded.as_sequence().unwrap();

        assert_eq!(seq[0].as_str(), Some("regular string"));
        assert_eq!(seq[1].as_str(), Some("email@example.com"));
        assert_eq!(seq[2].as_str(), Some("not @a reference"));
    }
}
