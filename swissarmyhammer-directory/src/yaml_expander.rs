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
/// 2. User files (~/.sah/ or ~/.avp/)
/// 3. Local files (./.sah/ or ./.avp/)
#[derive(Debug)]
pub struct YamlExpander<C: DirectoryConfig> {
    /// Loaded YAML files indexed by their relative path (without extension).
    includes: HashMap<String, serde_yaml_ng::Value>,
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

            match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&file_entry.content) {
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
        let value = serde_yaml_ng::from_str(content).map_err(|e| DirectoryError::Other {
            message: format!("Failed to parse builtin YAML '{}': {}", name, e),
        })?;
        self.includes.insert(name.to_string(), value);
        Ok(())
    }

    /// Get a loaded include by name.
    pub fn get(&self, name: &str) -> Option<&serde_yaml_ng::Value> {
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
    pub fn expand(&self, value: serde_yaml_ng::Value) -> Result<serde_yaml_ng::Value> {
        self.expand_value(value, &mut Vec::new())
    }

    /// Internal recursive expansion with cycle detection.
    fn expand_value(
        &self,
        value: serde_yaml_ng::Value,
        visited: &mut Vec<String>,
    ) -> Result<serde_yaml_ng::Value> {
        match value {
            serde_yaml_ng::Value::String(s) if s.starts_with('@') => {
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
            serde_yaml_ng::Value::Sequence(seq) => {
                let mut expanded_seq = Vec::new();

                for item in seq {
                    // Check if this is an @include that should be spliced
                    if let serde_yaml_ng::Value::String(s) = &item {
                        if let Some(include_name) = s.strip_prefix('@') {
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
                            if let serde_yaml_ng::Value::Sequence(included_seq) = expanded {
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

                Ok(serde_yaml_ng::Value::Sequence(expanded_seq))
            }
            serde_yaml_ng::Value::Mapping(map) => {
                let mut expanded_map = serde_yaml_ng::Mapping::new();

                for (key, val) in map {
                    let expanded_key = self.expand_value(key, visited)?;
                    let expanded_val = self.expand_value(val, visited)?;
                    expanded_map.insert(expanded_key, expanded_val);
                }

                Ok(serde_yaml_ng::Value::Mapping(expanded_map))
            }
            // Other value types pass through unchanged
            other => Ok(other),
        }
    }

    /// Parse a YAML string and expand includes.
    ///
    /// This is a convenience method that combines parsing and expansion.
    pub fn parse_and_expand(&self, yaml: &str) -> Result<serde_yaml_ng::Value> {
        let value = serde_yaml_ng::from_str(yaml).map_err(|e| DirectoryError::Other {
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
        serde_yaml_ng::from_value(expanded).map_err(|e| DirectoryError::Other {
            message: format!("Failed to deserialize YAML: {}", e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SwissarmyhammerConfig;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

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
        let input: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("\"@file_groups/source_code\"").unwrap();
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

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
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

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
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
        let input: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("\"@file_groups/frontend\"").unwrap();
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

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str("\"@a\"").unwrap();
        let result = expander.expand(input);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular include"));
    }

    #[test]
    fn test_expand_not_found() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        // Note: @ must be quoted in YAML
        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str("\"@nonexistent/file\"").unwrap();
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
    fn test_default_creates_empty_expander() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::default();
        assert!(expander.includes.is_empty());
        assert_eq!(expander.list_names().len(), 0);
    }

    #[test]
    fn test_list_names_returns_added_builtins() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("file_groups/source_code", r#"["*.js"]"#)
            .unwrap();
        expander
            .add_builtin("patterns/ignore", r#"["node_modules"]"#)
            .unwrap();

        let mut names: Vec<&String> = expander.list_names();
        names.sort();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "file_groups/source_code");
        assert_eq!(names[1], "patterns/ignore");
    }

    #[test]
    fn test_parse_and_expand_success() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("patterns/js", r#"["*.js", "*.mjs"]"#)
            .unwrap();

        let result = expander
            .parse_and_expand(
                r#"
files:
  - "@patterns/js"
  - "*.custom"
"#,
            )
            .unwrap();

        let files = result.get("files").and_then(|f| f.as_sequence()).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].as_str(), Some("*.js"));
        assert_eq!(files[1].as_str(), Some("*.mjs"));
        assert_eq!(files[2].as_str(), Some("*.custom"));
    }

    #[test]
    fn test_parse_and_expand_invalid_yaml() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        let result = expander.parse_and_expand("{{invalid: yaml: [}}}");

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse YAML"));
    }

    #[test]
    fn test_non_reference_strings_unchanged() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
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

    #[test]
    fn test_circular_reference_in_sequence() {
        // Tests cycle detection inside the sequence branch (lines 208-213).
        // Include "a" contains a sequence with "@b", and "b" contains a
        // sequence with "@a", forming a cycle when expanded inside a sequence.
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander.add_builtin("a", r#"["@b", "x"]"#).unwrap();
        expander.add_builtin("b", r#"["@a", "y"]"#).unwrap();

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
- "@a"
- "other"
"#,
        )
        .unwrap();

        let result = expander.expand(input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Circular include"),
            "Expected 'Circular include' error, got: {}",
            err
        );
    }

    #[test]
    fn test_non_sequence_splice_in_sequence() {
        // Tests that when an @include inside a sequence resolves to a scalar
        // (not a sequence), the scalar is pushed as a single element rather
        // than spliced. This exercises line 233.
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("greeting", r#""hello world""#)
            .unwrap();

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
- "@greeting"
- "other"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();
        let seq = expanded.as_sequence().unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].as_str(), Some("hello world"));
        assert_eq!(seq[1].as_str(), Some("other"));
    }

    /// Calling load_all on a fresh expander should succeed even when the
    /// XDG data directory and local project directory do not exist.
    #[test]
    #[serial]
    fn test_load_all_succeeds_with_empty_directories() {
        let temp_dir = TempDir::new().unwrap();
        // Point XDG_DATA_HOME at an empty temp dir so the VFS finds no files.
        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.load_all();

        // Restore env
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "load_all should succeed: {:?}", result);
        // No YAML files means no includes loaded
        assert!(
            expander.includes.is_empty(),
            "includes should be empty when no yaml files exist"
        );
    }

    /// Verifies that load_all discovers .yaml files placed in the XDG data
    /// directory and parses them into the includes map.
    #[test]
    #[serial]
    fn test_load_all_loads_yaml_files_from_xdg_data() {
        let temp_dir = TempDir::new().unwrap();
        // VFS uses VirtualFileSystem::<C>::new("") with empty subdirectory,
        // then calls ManagedDirectory::<C>::xdg_data() which resolves to
        // $XDG_DATA_HOME/sah/. With empty subdirectory, load_directory
        // joins base_path with "" so it reads from $XDG_DATA_HOME/sah/ directly.
        let sah_dir = temp_dir.path().join("sah");
        fs::create_dir_all(&sah_dir).unwrap();

        // Write a valid YAML file
        fs::write(sah_dir.join("colors.yaml"), "- red\n- green\n- blue\n").unwrap();

        // Write a second YAML file using .yml extension
        fs::write(sah_dir.join("sizes.yml"), "small: 1\nlarge: 10\n").unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.load_all();

        // Restore env
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "load_all should succeed: {:?}", result);

        // Check that both files were loaded
        let colors = expander.get("colors");
        assert!(colors.is_some(), "colors.yaml should be loaded");
        let colors_seq = colors.unwrap().as_sequence().unwrap();
        assert_eq!(colors_seq.len(), 3);
        assert_eq!(colors_seq[0].as_str(), Some("red"));

        let sizes = expander.get("sizes");
        assert!(sizes.is_some(), "sizes.yml should be loaded");
        let sizes_map = sizes.unwrap().as_mapping().unwrap();
        assert_eq!(
            sizes_map
                .get(serde_yaml_ng::Value::String("small".into()))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    /// Non-YAML files (e.g. .txt) in the data directory should be skipped
    /// by load_all, leaving only .yaml/.yml files in the includes map.
    #[test]
    #[serial]
    fn test_load_all_skips_non_yaml_files() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join("sah");
        fs::create_dir_all(&sah_dir).unwrap();

        fs::write(sah_dir.join("valid.yaml"), "- item\n").unwrap();
        fs::write(sah_dir.join("readme.txt"), "not yaml").unwrap();
        fs::write(sah_dir.join("data.json"), r#"{"key": "value"}"#).unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.load_all();

        // Restore env
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok());
        // Only the .yaml file should be loaded, not .txt or .json
        assert!(
            expander.get("valid").is_some(),
            "valid.yaml should be loaded"
        );
        assert_eq!(
            expander.includes.len(),
            1,
            "only .yaml files should be in includes, got: {:?}",
            expander.list_names()
        );
    }

    /// When load_all encounters a .yaml file with invalid YAML content, it
    /// should warn and skip the file rather than returning an error.
    #[test]
    #[serial]
    fn test_load_all_skips_invalid_yaml_files() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join("sah");
        fs::create_dir_all(&sah_dir).unwrap();

        // Valid YAML
        fs::write(sah_dir.join("good.yaml"), "- one\n- two\n").unwrap();
        // Invalid YAML (unclosed bracket)
        fs::write(sah_dir.join("bad.yaml"), "{{invalid: yaml: [}}}").unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.load_all();

        // Restore env
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        // load_all should succeed even with invalid YAML files
        assert!(
            result.is_ok(),
            "load_all should not fail on bad yaml: {:?}",
            result
        );
        // The good file should still be loaded
        assert!(expander.get("good").is_some(), "good.yaml should be loaded");
        // The bad file should be skipped
        assert!(expander.get("bad").is_none(), "bad.yaml should be skipped");
    }

    /// Builtins added before load_all should be preserved alongside files
    /// discovered from the filesystem.
    #[test]
    #[serial]
    fn test_load_all_preserves_existing_builtins() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join("sah");
        fs::create_dir_all(&sah_dir).unwrap();

        fs::write(sah_dir.join("from_disk.yaml"), "- disk_item\n").unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("pre_existing", r#"["builtin_item"]"#)
            .unwrap();
        let result = expander.load_all();

        // Restore env
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok());
        // Both the builtin and disk file should be present
        assert!(
            expander.get("pre_existing").is_some(),
            "builtin added before load_all should still be present"
        );
        assert!(
            expander.get("from_disk").is_some(),
            "file from disk should be loaded"
        );
    }

    /// add_builtin should return an error when the YAML content is invalid,
    /// covering the error path in the add_builtin method.
    #[test]
    fn test_add_builtin_invalid_yaml_returns_error() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.add_builtin("bad", "{{invalid: yaml: [}}}");
        assert!(result.is_err(), "add_builtin with invalid YAML should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to parse builtin YAML"),
            "error should mention parse failure, got: {err}"
        );
    }

    /// When a sequence contains an @include that references a nonexistent name,
    /// expand_value should return an error (covers the include-not-found path
    /// inside sequence processing).
    #[test]
    fn test_expand_sequence_with_missing_include_errors() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        // A sequence that references a name that was never registered
        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
- "@missing_group"
- "*.custom"
"#,
        )
        .unwrap();

        let result = expander.expand(input);
        assert!(
            result.is_err(),
            "expanding sequence with missing include should error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Include not found"),
            "error should mention include not found, got: {err}"
        );
    }

    /// parse_yaml should return an error when the YAML structure does not match
    /// the expected deserialization type.
    #[test]
    fn test_parse_yaml_type_mismatch_returns_error() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        #[derive(serde::Deserialize, Debug)]
        #[allow(dead_code)]
        struct Specific {
            name: String,
        }

        // YAML is a sequence, not a mapping — can't deserialize into Specific
        let result: Result<Specific> = expander.parse_yaml("- item1\n- item2\n");
        assert!(
            result.is_err(),
            "parse_yaml with wrong type should return error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to deserialize YAML"),
            "error should mention deserialization failure, got: {err}"
        );
    }

    /// load_all should skip non-yaml files (like .md) even when they are loaded
    /// by the VFS (which loads .md files). This covers the `continue` branch
    /// that checks for .yaml/.yml extension.
    #[test]
    #[serial]
    fn test_load_all_skips_md_files() {
        let temp_dir = TempDir::new().unwrap();
        // VFS uses empty subdirectory, so it loads from $XDG_DATA_HOME/sah/
        let sah_dir = temp_dir.path().join("sah");
        fs::create_dir_all(&sah_dir).unwrap();

        // Write a .md file (loaded by VFS but should be skipped by yaml filter)
        fs::write(sah_dir.join("readme.md"), "# Hello World").unwrap();
        // Write a valid .yaml file so we can confirm load_all ran
        fs::write(sah_dir.join("valid.yaml"), "- item\n").unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());

        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.load_all();

        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "load_all should succeed: {:?}", result);
        // .md file should not be in includes (was skipped by yaml extension check)
        assert!(
            expander.get("readme").is_none(),
            ".md files should not be loaded into includes"
        );
        // .yaml file should be loaded
        assert!(
            expander.get("valid").is_some(),
            ".yaml files should be loaded into includes"
        );
    }

    // ── Additional coverage tests ──────────────────────────────────────

    /// Expand a mapping where the value is an @reference that resolves to a
    /// non-sequence value. This exercises the mapping expansion branch.
    #[test]
    fn test_expand_mapping_value_reference() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander.add_builtin("greeting", r#""hello""#).unwrap();

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
message: "@greeting"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();
        let msg = expanded.get("message").and_then(|v| v.as_str());
        assert_eq!(msg, Some("hello"));
    }

    /// Expand a mapping where the key is an @reference. This exercises
    /// the expand_value call on the key side of mapping iteration.
    #[test]
    fn test_expand_mapping_key_reference() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander
            .add_builtin("key_name", r#""resolved_key""#)
            .unwrap();

        let mut map = serde_yaml_ng::Mapping::new();
        map.insert(
            serde_yaml_ng::Value::String("@key_name".to_string()),
            serde_yaml_ng::Value::String("value".to_string()),
        );
        let input = serde_yaml_ng::Value::Mapping(map);

        let expanded = expander.expand(input).unwrap();
        let val = expanded.get("resolved_key").and_then(|v| v.as_str());
        assert_eq!(val, Some("value"));
    }

    /// Expand passes through non-string, non-sequence, non-mapping values
    /// unchanged (numbers, booleans, null).
    #[test]
    fn test_expand_passthrough_values() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        // Number
        let num = serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(42));
        let expanded = expander.expand(num.clone()).unwrap();
        assert_eq!(expanded, num);

        // Boolean
        let b = serde_yaml_ng::Value::Bool(true);
        let expanded = expander.expand(b.clone()).unwrap();
        assert_eq!(expanded, b);

        // Null
        let n = serde_yaml_ng::Value::Null;
        let expanded = expander.expand(n.clone()).unwrap();
        assert_eq!(expanded, n);
    }

    /// parse_and_expand with a string that is not an @reference passes through.
    #[test]
    fn test_parse_and_expand_plain_string() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();
        let result = expander.parse_and_expand(r#""just a string""#).unwrap();
        assert_eq!(result.as_str(), Some("just a string"));
    }

    /// Nested includes through a mapping value exercise recursive expansion
    /// across value types.
    #[test]
    fn test_expand_nested_mapping_with_reference() {
        let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
        expander.add_builtin("inner", r#"["a", "b"]"#).unwrap();

        let input: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
outer:
  items: "@inner"
"#,
        )
        .unwrap();

        let expanded = expander.expand(input).unwrap();
        let items = expanded
            .get("outer")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_sequence())
            .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_str(), Some("a"));
    }

    /// get returns None for a name that was never added.
    #[test]
    fn test_get_returns_none_for_missing() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();
        assert!(expander.get("nonexistent").is_none());
    }

    /// parse_yaml successfully deserializes into a type when YAML matches.
    #[test]
    fn test_parse_yaml_success_no_includes() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Simple {
            name: String,
            count: u32,
        }

        let result: Simple = expander.parse_yaml("name: test\ncount: 5\n").unwrap();
        assert_eq!(result.name, "test");
        assert_eq!(result.count, 5);
    }

    /// parse_yaml returns error when the YAML is invalid (not parseable).
    #[test]
    fn test_parse_yaml_invalid_yaml_returns_error() {
        let expander = YamlExpander::<SwissarmyhammerConfig>::new();

        #[derive(serde::Deserialize, Debug)]
        #[allow(dead_code)]
        struct Dummy {
            x: String,
        }

        let result: crate::error::Result<Dummy> = expander.parse_yaml("{{invalid yaml}}}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse YAML"));
    }
}
