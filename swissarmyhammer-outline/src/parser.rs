//! Tree-sitter based parser for generating code outlines

use crate::{
    types::{FileOutline, Language, OutlineNode, OutlineNodeType, SymbolVisibility},
    OutlineError, Result,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Language as TSLanguage, Node, Parser, Tree};

/// Configuration for outline parsing
#[derive(Debug, Clone)]
pub struct OutlineParserConfig {
    /// Enable extraction of documentation comments
    pub extract_documentation: bool,
    /// Enable extraction of function signatures
    pub extract_signatures: bool,
    /// Enable extraction of visibility modifiers
    pub extract_visibility: bool,
    /// Maximum nesting depth for hierarchical symbols
    pub max_nesting_depth: usize,
    /// Minimum line count for symbols to be included
    pub min_symbol_lines: usize,
}

impl Default for OutlineParserConfig {
    fn default() -> Self {
        Self {
            extract_documentation: true,
            extract_signatures: true,
            extract_visibility: true,
            max_nesting_depth: 10,
            min_symbol_lines: 1,
        }
    }
}

/// Tree-sitter based outline parser
pub struct OutlineParser {
    /// Tree-sitter parser instance
    parser: Parser,
    /// Language-specific Tree-sitter languages
    languages: HashMap<Language, TSLanguage>,
    /// Parser configuration
    config: OutlineParserConfig,
}

impl OutlineParser {
    /// Create a new outline parser
    pub fn new(config: OutlineParserConfig) -> Result<Self> {
        let parser = Parser::new();
        let mut languages = HashMap::new();

        // Register Tree-sitter languages
        languages.insert(Language::Rust, tree_sitter_rust::LANGUAGE.into());
        languages.insert(Language::Python, tree_sitter_python::LANGUAGE.into());
        languages.insert(Language::TypeScript, tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
        languages.insert(Language::JavaScript, tree_sitter_javascript::LANGUAGE.into());
        languages.insert(Language::Dart, tree_sitter_dart::language());

        Ok(Self {
            parser,
            languages,
            config,
        })
    }

    /// Parse a file and generate its outline
    pub fn parse_file(&mut self, file_path: &Path, content: &str) -> Result<FileOutline> {
        // Detect language from file extension
        let language = if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            Language::from_extension(ext)
        } else {
            Language::Unknown
        };

        if !language.is_supported() {
            return Err(OutlineError::LanguageDetection(format!(
                "Unsupported language for file: {}",
                file_path.display()
            )));
        }

        // Set the appropriate Tree-sitter language
        let ts_language = self.languages.get(&language).ok_or_else(|| {
            OutlineError::TreeSitter(format!("No Tree-sitter language for {:?}", language))
        })?;

        self.parser.set_language(ts_language).map_err(|e| {
            OutlineError::TreeSitter(format!("Failed to set Tree-sitter language: {}", e))
        })?;

        // Parse the source code
        let tree = self
            .parser
            .parse(content, None)
            .ok_or_else(|| OutlineError::TreeSitter("Failed to parse source code".to_string()))?;

        // Extract symbols from the parse tree
        let symbols = self.extract_symbols(&tree, content, &language)?;

        // Create file outline
        let file_outline = FileOutline::new(file_path.to_path_buf(), language, symbols);

        Ok(file_outline)
    }

    /// Extract symbols from a parsed Tree-sitter tree
    fn extract_symbols(&self, tree: &Tree, source: &str, language: &Language) -> Result<Vec<OutlineNode>> {
        let root_node = tree.root_node();
        let mut symbols = Vec::new();

        self.extract_symbols_recursive(&root_node, source, language, &mut symbols, 0)?;

        Ok(symbols)
    }

    /// Recursively extract symbols from Tree-sitter nodes
    fn extract_symbols_recursive(
        &self,
        node: &Node,
        source: &str,
        language: &Language,
        symbols: &mut Vec<OutlineNode>,
        depth: usize,
    ) -> Result<()> {
        if depth > self.config.max_nesting_depth {
            return Ok(());
        }

        // Check if this node represents a symbol we're interested in
        if let Some(symbol) = self.node_to_symbol(node, source, language)? {
            symbols.push(symbol);
        }

        // Recursively process child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols_recursive(&child, source, language, symbols, depth + 1)?;
        }

        Ok(())
    }

    /// Convert a Tree-sitter node to an outline symbol if applicable
    fn node_to_symbol(&self, node: &Node, source: &str, language: &Language) -> Result<Option<OutlineNode>> {
        let node_type = match (language, node.kind()) {
            // Rust symbols
            (Language::Rust, "function_item") => Some(OutlineNodeType::Function),
            (Language::Rust, "struct_item") => Some(OutlineNodeType::Struct),
            (Language::Rust, "enum_item") => Some(OutlineNodeType::Enum),
            (Language::Rust, "impl_item") => Some(OutlineNodeType::Impl),
            (Language::Rust, "trait_item") => Some(OutlineNodeType::Trait),
            (Language::Rust, "mod_item") => Some(OutlineNodeType::Module),
            (Language::Rust, "const_item") => Some(OutlineNodeType::Constant),
            (Language::Rust, "static_item") => Some(OutlineNodeType::Variable),
            (Language::Rust, "type_item") => Some(OutlineNodeType::TypeAlias),
            (Language::Rust, "use_declaration") => Some(OutlineNodeType::Import),

            // Python symbols
            (Language::Python, "function_definition") => Some(OutlineNodeType::Function),
            (Language::Python, "class_definition") => Some(OutlineNodeType::Class),
            (Language::Python, "import_statement") => Some(OutlineNodeType::Import),
            (Language::Python, "import_from_statement") => Some(OutlineNodeType::Import),

            // TypeScript/JavaScript symbols
            (Language::TypeScript | Language::JavaScript, "function_declaration") => Some(OutlineNodeType::Function),
            (Language::TypeScript | Language::JavaScript, "method_definition") => Some(OutlineNodeType::Method),
            (Language::TypeScript | Language::JavaScript, "class_declaration") => Some(OutlineNodeType::Class),
            (Language::TypeScript, "interface_declaration") => Some(OutlineNodeType::Interface),
            (Language::TypeScript, "type_alias_declaration") => Some(OutlineNodeType::TypeAlias),
            (Language::TypeScript | Language::JavaScript, "import_statement") => Some(OutlineNodeType::Import),

            // Dart symbols
            (Language::Dart, "function_signature") => Some(OutlineNodeType::Function),
            (Language::Dart, "method_signature") => Some(OutlineNodeType::Method),
            (Language::Dart, "class_definition") => Some(OutlineNodeType::Class),
            (Language::Dart, "enum_declaration") => Some(OutlineNodeType::Enum),
            (Language::Dart, "import_specification") => Some(OutlineNodeType::Import),

            _ => None,
        };

        let Some(symbol_type) = node_type else {
            return Ok(None);
        };

        // Extract symbol name
        let name = self.extract_symbol_name(node, source, language)?;
        if name.is_empty() {
            return Ok(None);
        }

        // Create outline node
        let mut outline_node = OutlineNode::new(
            name,
            symbol_type,
            node.start_position().row + 1, // Convert to 1-based line numbers
            node.end_position().row + 1,
        );

        outline_node.start_column = node.start_position().column;
        outline_node.end_column = node.end_position().column;

        // Extract additional information if enabled
        if self.config.extract_signatures {
            outline_node.signature = self.extract_signature(node, source, language);
        }

        if self.config.extract_documentation {
            outline_node.documentation = self.extract_documentation(node, source, language);
        }

        if self.config.extract_visibility {
            outline_node.visibility = self.extract_visibility(node, source, language);
        }

        Ok(Some(outline_node))
    }

    /// Extract the name of a symbol from a Tree-sitter node
    fn extract_symbol_name(&self, node: &Node, source: &str, _language: &Language) -> Result<String> {
        // Look for identifier nodes within the symbol node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "identifier" | "type_identifier") {
                let name = child.utf8_text(source.as_bytes()).map_err(|e| {
                    OutlineError::TreeSitter(format!("Failed to extract symbol name: {}", e))
                })?;
                return Ok(name.to_string());
            }
        }

        // Fallback: try to extract from node text directly
        let text = node.utf8_text(source.as_bytes()).map_err(|e| {
            OutlineError::TreeSitter(format!("Failed to extract node text: {}", e))
        })?;

        // Extract the first word as a simple fallback
        Ok(text.split_whitespace().next().unwrap_or("unnamed").to_string())
    }

    /// Extract function/method signature
    fn extract_signature(&self, node: &Node, source: &str, _language: &Language) -> Option<String> {
        // For now, just return the first line of the node
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            Some(text.lines().next()?.trim().to_string())
        } else {
            None
        }
    }

    /// Extract documentation comment for a symbol
    fn extract_documentation(&self, node: &Node, source: &str, _language: &Language) -> Option<String> {
        // Look for comment nodes immediately before this symbol
        let parent = node.parent()?;
        let mut cursor = parent.walk();
        
        for child in parent.children(&mut cursor) {
            if child.id() == node.id() {
                break;
            }
            if child.kind().contains("comment") {
                if let Ok(comment_text) = child.utf8_text(source.as_bytes()) {
                    return Some(comment_text.trim().to_string());
                }
            }
        }
        
        None
    }

    /// Extract visibility modifier for a symbol
    fn extract_visibility(&self, node: &Node, _source: &str, language: &Language) -> Option<SymbolVisibility> {
        match language {
            Language::Rust => {
                // Look for 'pub' keyword
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "visibility_modifier" {
                        return Some(SymbolVisibility::Public);
                    }
                }
                Some(SymbolVisibility::Private)
            }
            _ => Some(SymbolVisibility::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_outline_parser_creation() {
        let config = OutlineParserConfig::default();
        let parser = OutlineParser::new(config);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_rust_function() {
        let config = OutlineParserConfig::default();
        let mut parser = OutlineParser::new(config).unwrap();
        
        let rust_code = r#"
            pub fn main() {
                println!("Hello, world!");
            }
        "#;

        let file_path = PathBuf::from("test.rs");
        let result = parser.parse_file(&file_path, rust_code);
        
        assert!(result.is_ok());
        let outline = result.unwrap();
        assert_eq!(outline.language, Language::Rust);
        assert!(!outline.symbols.is_empty());
        
        let main_fn = &outline.symbols[0];
        assert_eq!(main_fn.name, "main");
        assert_eq!(main_fn.node_type, OutlineNodeType::Function);
    }

    #[test]
    fn test_parse_python_class() {
        let config = OutlineParserConfig::default();
        let mut parser = OutlineParser::new(config).unwrap();
        
        let python_code = r#"
            class TestClass:
                def method(self):
                    pass
        "#;

        let file_path = PathBuf::from("test.py");
        let result = parser.parse_file(&file_path, python_code);
        
        assert!(result.is_ok());
        let outline = result.unwrap();
        assert_eq!(outline.language, Language::Python);
        assert!(!outline.symbols.is_empty());
        
        // Should find the class
        let class_symbol = outline.symbols.iter()
            .find(|s| s.node_type == OutlineNodeType::Class);
        assert!(class_symbol.is_some());
        assert_eq!(class_symbol.unwrap().name, "TestClass");
    }
}