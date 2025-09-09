//! YAML output formatter for hierarchical outline structures
//!
//! This module provides the functionality to convert hierarchical outline structures
//! into well-formatted YAML output following the specification's structure requirements.
//! The output mirrors the file system hierarchy while providing clean, readable
//! output for code navigation and understanding.

use crate::outline::{
    OutlineDirectory, OutlineFile, OutlineHierarchy, OutlineNode, OutlineNodeType, Result,
    Visibility,
};
use std::fmt::Write;

/// Configuration for YAML formatting options
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Number of spaces for each indentation level
    pub indent_size: usize,
    /// Whether to include empty directories in output
    pub include_empty_dirs: bool,
    /// Sorting order for symbols and directories
    pub sort_order: SortOrder,
    /// Whether to include private symbols
    pub include_private_symbols: bool,
    /// Maximum length for signatures before truncation
    pub max_signature_length: Option<usize>,
    /// Whether to include line numbers
    pub include_line_numbers: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 2,
            include_empty_dirs: false,
            sort_order: SortOrder::SourceOrder,
            include_private_symbols: true,
            max_signature_length: Some(120),
            include_line_numbers: true,
        }
    }
}

/// Sorting strategies for organizing YAML output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Maintain original source order
    SourceOrder,
    /// Sort alphabetically by name  
    Alphabetical,
    /// Group by symbol kind, then alphabetical
    ByKind,
    /// Sort by line number
    ByLine,
}

/// YAML formatter for hierarchical outline structures
pub struct YamlFormatter {
    config: FormatterConfig,
}

impl YamlFormatter {
    /// Create a new YAML formatter with the given configuration
    pub fn new(config: FormatterConfig) -> Self {
        Self { config }
    }

    /// Create a new YAML formatter with default configuration
    pub fn with_defaults() -> Self {
        Self::new(FormatterConfig::default())
    }

    /// Format a complete hierarchical outline structure into YAML
    pub fn format_hierarchy(&self, hierarchy: &OutlineHierarchy) -> Result<String> {
        let mut result = String::new();
        self.format_directory(&hierarchy.root, 0, &mut result)?;
        Ok(result)
    }

    /// Format a directory and its contents recursively
    fn format_directory(
        &self,
        directory: &OutlineDirectory,
        depth: usize,
        result: &mut String,
    ) -> Result<()> {
        let indent = " ".repeat(depth * self.config.indent_size);

        // Skip root directory name if it's "."
        if directory.name != "." {
            writeln!(result, "{}{}:", indent, directory.name).map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        }

        let child_depth = if directory.name == "." {
            depth
        } else {
            depth + 1
        };

        // Format subdirectories first
        for subdir in &directory.subdirectories {
            if !subdir.is_empty() || self.config.include_empty_dirs {
                self.format_directory(subdir, child_depth, result)?;
            }
        }

        // Format files
        for file in &directory.files {
            self.format_file(file, child_depth, result)?;
        }

        Ok(())
    }

    /// Format a file and its symbols
    fn format_file(&self, file: &OutlineFile, depth: usize, result: &mut String) -> Result<()> {
        let indent = " ".repeat(depth * self.config.indent_size);

        // File name as key
        writeln!(result, "{}{}:", indent, file.name).map_err(|e| {
            crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
        })?;

        if file.symbols.is_empty() {
            // Empty file notation
            writeln!(result, "{indent}  children: []").map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        } else {
            // Children array
            writeln!(result, "{indent}  children:").map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;

            let mut symbols = file.symbols.clone();
            self.sort_symbols(&mut symbols);

            for symbol in &symbols {
                if self.should_include_symbol(symbol) {
                    self.format_symbol(symbol, depth + 1, result)?;
                }
            }
        }

        Ok(())
    }

    /// Format a single symbol and its children
    fn format_symbol(&self, symbol: &OutlineNode, depth: usize, result: &mut String) -> Result<()> {
        let indent = " ".repeat(depth * self.config.indent_size);

        // Start symbol entry
        writeln!(
            result,
            "{}  - name: {}",
            indent,
            Self::escape_yaml_string(&symbol.name)
        )
        .map_err(|e| {
            crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
        })?;
        writeln!(
            result,
            "{}    kind: \"{}\"",
            indent,
            self.node_type_to_string(&symbol.node_type)
        )
        .map_err(|e| {
            crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
        })?;

        if self.config.include_line_numbers {
            writeln!(result, "{}    line: {}", indent, symbol.start_line).map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        }

        // Optional signature
        if let Some(ref signature) = symbol.signature {
            let formatted_sig = self.format_signature(signature)?;
            writeln!(
                result,
                "{}    signature: {}",
                indent,
                Self::escape_yaml_string(&formatted_sig)
            )
            .map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        }

        // Optional type information (from visibility)
        if let Some(ref visibility) = symbol.visibility {
            writeln!(
                result,
                "{}    type: {}",
                indent,
                Self::escape_yaml_string(&format!("{visibility:?}"))
            )
            .map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        }

        // Optional documentation
        if let Some(ref doc) = symbol.documentation {
            let formatted_doc = self.format_documentation(doc)?;
            writeln!(
                result,
                "{}    doc: {}",
                indent,
                Self::escape_yaml_string(&formatted_doc)
            )
            .map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;
        }

        // Optional children
        if !symbol.children.is_empty() {
            writeln!(result, "{indent}    children:").map_err(|e| {
                crate::outline::OutlineError::Generation(format!("Format write error: {e}"))
            })?;

            let mut children: Vec<&OutlineNode> =
                symbol.children.iter().map(|b| b.as_ref()).collect();
            self.sort_symbol_refs(&mut children);

            for child in children {
                if self.should_include_symbol(child) {
                    self.format_symbol(child, depth + 2, result)?;
                }
            }
        }

        Ok(())
    }

    /// Convert OutlineNodeType to string representation
    fn node_type_to_string(&self, node_type: &OutlineNodeType) -> &'static str {
        match node_type {
            OutlineNodeType::Function => "function",
            OutlineNodeType::Method => "method",
            OutlineNodeType::Class => "class",
            OutlineNodeType::Struct => "struct",
            OutlineNodeType::Enum => "enum",
            OutlineNodeType::Interface => "interface",
            OutlineNodeType::Trait => "trait",
            OutlineNodeType::Impl => "impl",
            OutlineNodeType::Module => "module",
            OutlineNodeType::Property => "property",
            OutlineNodeType::Constant => "constant",
            OutlineNodeType::Variable => "variable",
            OutlineNodeType::TypeAlias => "type_alias",
            OutlineNodeType::Import => "import",
        }
    }

    /// Format a signature string, applying length limits
    fn format_signature(&self, signature: &str) -> Result<String> {
        if let Some(max_len) = self.config.max_signature_length {
            if signature.len() > max_len {
                // Truncate long signatures with ellipsis
                let truncated = &signature[..max_len.saturating_sub(3)];
                return Ok(format!("{truncated}..."));
            }
        }
        Ok(signature.to_string())
    }

    /// Format documentation text, cleaning and limiting length  
    fn format_documentation(&self, doc: &str) -> Result<String> {
        // Clean up documentation text
        let cleaned = doc
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        // Limit documentation length for readability
        if cleaned.len() > 200 {
            let truncated = &cleaned[..197];
            Ok(format!("{truncated}..."))
        } else {
            Ok(cleaned)
        }
    }

    /// Escape a string for safe YAML output
    fn escape_yaml_string(s: &str) -> String {
        // Handle YAML string escaping
        if s.contains('\n') || s.contains('"') || s.contains('\\') || s.contains(' ') {
            format!(
                "\"{}\"",
                s.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
            )
        } else if s.is_empty() {
            "\"\"".to_string()
        } else {
            s.to_string()
        }
    }

    /// Check if a symbol should be included based on configuration
    fn should_include_symbol(&self, symbol: &OutlineNode) -> bool {
        if !self.config.include_private_symbols {
            if let Some(Visibility::Private) = symbol.visibility {
                return false;
            }
        }
        true
    }

    /// Sort symbols according to configuration
    fn sort_symbols(&self, symbols: &mut [OutlineNode]) {
        match self.config.sort_order {
            SortOrder::SourceOrder => {
                // Keep original order
            }
            SortOrder::Alphabetical => {
                symbols.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortOrder::ByKind => {
                symbols.sort_by_key(|s| self.symbol_kind_order(&s.node_type));
            }
            SortOrder::ByLine => {
                symbols.sort_by_key(|s| s.start_line);
            }
        }
    }

    /// Sort symbol references according to configuration  
    fn sort_symbol_refs(&self, symbols: &mut Vec<&OutlineNode>) {
        match self.config.sort_order {
            SortOrder::SourceOrder => {
                // Keep original order
            }
            SortOrder::Alphabetical => {
                symbols.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortOrder::ByKind => {
                symbols.sort_by_key(|s| self.symbol_kind_order(&s.node_type));
            }
            SortOrder::ByLine => {
                symbols.sort_by_key(|s| s.start_line);
            }
        }
    }

    /// Get sorting order value for symbol kinds
    fn symbol_kind_order(&self, node_type: &OutlineNodeType) -> u8 {
        match node_type {
            OutlineNodeType::Module => 0,
            OutlineNodeType::Import => 1,
            OutlineNodeType::Constant => 2,
            OutlineNodeType::TypeAlias => 3,
            OutlineNodeType::Enum => 4,
            OutlineNodeType::Interface => 5,
            OutlineNodeType::Trait => 6,
            OutlineNodeType::Struct => 7,
            OutlineNodeType::Class => 8,
            OutlineNodeType::Impl => 9,
            OutlineNodeType::Function => 10,
            OutlineNodeType::Method => 11,
            OutlineNodeType::Variable => 12,
            OutlineNodeType::Property => 13,
        }
    }
}

/// Extension trait for OutlineDirectory to check if empty
trait DirectoryExt {
    fn is_empty(&self) -> bool;
}

impl DirectoryExt for OutlineDirectory {
    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.subdirectories.iter().all(|d| d.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::{
        HierarchyBuilder, OutlineFile, OutlineNode, OutlineNodeType, OutlineTree, Visibility,
    };
    use swissarmyhammer_search::Language;
    use std::path::PathBuf;

    fn create_test_hierarchy() -> crate::outline::OutlineHierarchy {
        let mut builder = HierarchyBuilder::new();

        // Create a sample Rust file with nested symbols
        let mut class_node = OutlineNode::new(
            "Calculator".to_string(),
            OutlineNodeType::Struct,
            10,
            50,
            (200, 1000),
        )
        .with_signature("pub struct Calculator".to_string())
        .with_documentation("A simple calculator struct".to_string())
        .with_visibility(Visibility::Public);

        // Add fields to the struct
        let field1 = OutlineNode::new(
            "result".to_string(),
            OutlineNodeType::Property,
            12,
            12,
            (250, 280),
        )
        .with_signature("result: f64".to_string())
        .with_documentation("Current calculation result".to_string())
        .with_visibility(Visibility::Public);

        class_node.add_child(field1);

        // Add impl block
        let mut impl_node = OutlineNode::new(
            "impl Calculator".to_string(),
            OutlineNodeType::Impl,
            20,
            45,
            (400, 900),
        );

        // Add methods to impl
        let method1 = OutlineNode::new(
            "new".to_string(),
            OutlineNodeType::Method,
            22,
            25,
            (450, 500),
        )
        .with_signature("pub fn new() -> Self".to_string())
        .with_documentation("Create a new calculator instance".to_string())
        .with_visibility(Visibility::Public);

        let method2 = OutlineNode::new(
            "add".to_string(),
            OutlineNodeType::Method,
            27,
            30,
            (520, 600),
        )
        .with_signature("pub fn add(&mut self, a: f64, b: f64) -> f64".to_string())
        .with_documentation("Add two numbers and return the result".to_string())
        .with_visibility(Visibility::Public);

        impl_node.add_child(method1);
        impl_node.add_child(method2);

        // Add a standalone function
        let function_node = OutlineNode::new(
            "main".to_string(),
            OutlineNodeType::Function,
            60,
            65,
            (1200, 1300),
        )
        .with_signature("fn main()".to_string())
        .with_documentation("Program entry point".to_string())
        .with_visibility(Visibility::Private);

        let symbols = vec![class_node, impl_node, function_node];
        let tree = OutlineTree::new(PathBuf::from("src/calculator.rs"), Language::Rust, symbols);

        builder.add_file_outline(tree).unwrap();
        builder.build_hierarchy().unwrap()
    }

    #[test]
    fn test_yaml_string_escaping() {
        assert_eq!(YamlFormatter::escape_yaml_string("simple"), "simple");
        assert_eq!(
            YamlFormatter::escape_yaml_string("with spaces"),
            "\"with spaces\""
        );
        assert_eq!(
            YamlFormatter::escape_yaml_string("with\nnewline"),
            "\"with\\nnewline\""
        );
        assert_eq!(
            YamlFormatter::escape_yaml_string("with\"quote"),
            "\"with\\\"quote\""
        );
        assert_eq!(YamlFormatter::escape_yaml_string(""), "\"\"");
    }

    #[test]
    fn test_node_type_conversion() {
        let formatter = YamlFormatter::with_defaults();

        assert_eq!(
            formatter.node_type_to_string(&OutlineNodeType::Function),
            "function"
        );
        assert_eq!(
            formatter.node_type_to_string(&OutlineNodeType::Class),
            "class"
        );
        assert_eq!(
            formatter.node_type_to_string(&OutlineNodeType::Method),
            "method"
        );
        assert_eq!(
            formatter.node_type_to_string(&OutlineNodeType::Property),
            "property"
        );
    }

    #[test]
    fn test_signature_formatting() {
        let formatter = YamlFormatter::with_defaults();

        let short_sig = "fn test() -> bool";
        assert_eq!(formatter.format_signature(short_sig).unwrap(), short_sig);

        let long_sig = "a".repeat(150);
        let result = formatter.format_signature(&long_sig).unwrap();
        assert!(result.len() <= 120);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_documentation_formatting() {
        let formatter = YamlFormatter::with_defaults();

        let simple_doc = "Simple documentation";
        assert_eq!(
            formatter.format_documentation(simple_doc).unwrap(),
            simple_doc
        );

        let multiline_doc = "Line 1\n   Line 2   \n\nLine 3";
        let result = formatter.format_documentation(multiline_doc).unwrap();
        assert_eq!(result, "Line 1 Line 2 Line 3");

        let long_doc = "a".repeat(250);
        let result = formatter.format_documentation(&long_doc).unwrap();
        assert!(result.len() <= 200);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_symbol_visibility_filtering() {
        let formatter = YamlFormatter::new(FormatterConfig {
            include_private_symbols: false,
            ..Default::default()
        });

        let public_node = OutlineNode::new(
            "public_func".to_string(),
            OutlineNodeType::Function,
            1,
            5,
            (0, 100),
        )
        .with_visibility(Visibility::Public);
        let private_node = OutlineNode::new(
            "private_func".to_string(),
            OutlineNodeType::Function,
            1,
            5,
            (0, 100),
        )
        .with_visibility(Visibility::Private);

        assert!(formatter.should_include_symbol(&public_node));
        assert!(!formatter.should_include_symbol(&private_node));
    }

    #[test]
    fn test_symbol_kind_ordering() {
        let formatter = YamlFormatter::with_defaults();

        assert!(
            formatter.symbol_kind_order(&OutlineNodeType::Module)
                < formatter.symbol_kind_order(&OutlineNodeType::Function)
        );
        assert!(
            formatter.symbol_kind_order(&OutlineNodeType::Import)
                < formatter.symbol_kind_order(&OutlineNodeType::Class)
        );
        assert!(
            formatter.symbol_kind_order(&OutlineNodeType::Function)
                < formatter.symbol_kind_order(&OutlineNodeType::Property)
        );
    }

    #[test]
    fn test_empty_directory_detection() {
        let empty_dir = OutlineDirectory::new("empty".to_string(), PathBuf::from("empty"));
        assert!(empty_dir.is_empty());

        let mut non_empty_dir =
            OutlineDirectory::new("nonempty".to_string(), PathBuf::from("nonempty"));
        let file = OutlineFile::new(
            "test.rs".to_string(),
            PathBuf::from("test.rs"),
            Language::Rust,
            vec![],
        );
        non_empty_dir.files.push(file);
        assert!(!non_empty_dir.is_empty());
    }

    #[test]
    fn test_complete_hierarchy_yaml_formatting() {
        let hierarchy = create_test_hierarchy();
        let formatter = YamlFormatter::with_defaults();

        let yaml_output = formatter.format_hierarchy(&hierarchy).unwrap();

        // Verify the basic structure is present
        assert!(yaml_output.contains("calculator.rs:"));
        assert!(yaml_output.contains("children:"));

        // Verify symbols are present
        assert!(yaml_output.contains("name: Calculator"));
        assert!(yaml_output.contains("kind: \"struct\""));
        assert!(yaml_output.contains("name: \"impl Calculator\""));
        assert!(yaml_output.contains("kind: \"impl\""));
        assert!(yaml_output.contains("name: main"));
        assert!(yaml_output.contains("kind: \"function\""));

        // Verify nested structure
        assert!(yaml_output.contains("name: result"));
        assert!(yaml_output.contains("kind: \"property\""));
        assert!(yaml_output.contains("name: new"));
        assert!(yaml_output.contains("kind: \"method\""));
        assert!(yaml_output.contains("name: add"));

        // Verify metadata is included
        assert!(yaml_output.contains("line: 10"));
        assert!(yaml_output.contains("signature: \"pub struct Calculator\""));
        assert!(yaml_output.contains("doc: \"A simple calculator struct\""));
        assert!(yaml_output.contains("signature: \"pub fn add(&mut self, a: f64, b: f64) -> f64\""));

        // Print the output for visual inspection during development
        println!("Generated YAML output:\n{yaml_output}");
    }

    #[test]
    fn test_yaml_format_with_different_configs() {
        let hierarchy = create_test_hierarchy();

        // Test with different indent sizes
        let formatter_2_spaces = YamlFormatter::new(FormatterConfig {
            indent_size: 2,
            ..Default::default()
        });
        let yaml_2_spaces = formatter_2_spaces.format_hierarchy(&hierarchy).unwrap();

        let formatter_4_spaces = YamlFormatter::new(FormatterConfig {
            indent_size: 4,
            ..Default::default()
        });
        let yaml_4_spaces = formatter_4_spaces.format_hierarchy(&hierarchy).unwrap();

        // 4-space version should be longer due to more indentation
        assert!(yaml_4_spaces.len() > yaml_2_spaces.len());

        // Test without line numbers
        let formatter_no_lines = YamlFormatter::new(FormatterConfig {
            include_line_numbers: false,
            ..Default::default()
        });
        let yaml_no_lines = formatter_no_lines.format_hierarchy(&hierarchy).unwrap();

        // Should not contain line numbers
        assert!(!yaml_no_lines.contains("line: "));

        // Test without private symbols
        let formatter_no_private = YamlFormatter::new(FormatterConfig {
            include_private_symbols: false,
            ..Default::default()
        });
        let yaml_no_private = formatter_no_private.format_hierarchy(&hierarchy).unwrap();

        // Should not contain the private main function
        assert!(!yaml_no_private.contains("name: main"));
    }

    #[test]
    fn test_yaml_validity_structure() {
        let hierarchy = create_test_hierarchy();
        let formatter = YamlFormatter::with_defaults();

        let yaml_output = formatter.format_hierarchy(&hierarchy).unwrap();

        // Basic YAML structure validation - should have proper indentation
        let lines: Vec<&str> = yaml_output.lines().collect();

        // Find a children array
        let children_line_idx = lines
            .iter()
            .position(|line| line.trim_start().starts_with("children:"))
            .expect("Should have children array");

        // Next line should be properly indented array item
        if children_line_idx + 1 < lines.len() {
            let next_line = lines[children_line_idx + 1];
            assert!(
                next_line.trim_start().starts_with("- name:"),
                "Children should be properly formatted array items"
            );
        }

        // Verify no lines start with invalid YAML
        for line in &lines {
            if !line.trim().is_empty() {
                // Should not start with invalid characters
                assert!(
                    !line.starts_with('-') || line.trim_start().starts_with("- "),
                    "Array items should be properly formatted"
                );
            }
        }
    }

    #[test]
    fn test_yaml_parser_validity() {
        let hierarchy = create_test_hierarchy();
        let formatter = YamlFormatter::with_defaults();

        let yaml_output = formatter.format_hierarchy(&hierarchy).unwrap();

        // Attempt to parse the generated YAML with serde_yaml to ensure it's valid
        let parsed_result: std::result::Result<serde_yaml::Value, serde_yaml::Error> =
            serde_yaml::from_str(&yaml_output);

        match parsed_result {
            Ok(parsed_yaml) => {
                // Verify the parsed structure has the expected top-level keys
                if let serde_yaml::Value::Mapping(map) = parsed_yaml {
                    // Should have the calculator.rs file as a key
                    let has_calculator = map.keys().any(|k| {
                        if let serde_yaml::Value::String(s) = k {
                            s.contains("calculator.rs")
                        } else {
                            false
                        }
                    });
                    assert!(
                        has_calculator,
                        "Parsed YAML should contain calculator.rs file"
                    );
                } else {
                    panic!("Parsed YAML should be a mapping at the top level");
                }
            }
            Err(e) => {
                panic!("Generated YAML should be valid and parseable by serde_yaml: {e}");
            }
        }
    }
}
