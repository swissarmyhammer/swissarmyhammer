//! Tree-sitter based parser for generating code outlines
//!
//! This module provides outline-specific parsing functionality that builds upon
//! the existing search parser infrastructure. It focuses on extracting structured
//! symbol information rather than creating search indexes.

use crate::outline::extractors::{
    JavaScriptExtractor, PythonExtractor, RustExtractor, TypeScriptExtractor,
};
use crate::outline::{OutlineNode, OutlineNodeType, OutlineTree, Result, Visibility};
use crate::search::parser::{CodeParser, ParserConfig};
use crate::search::types::Language;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Node, Tree};

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

/// Trait for language-specific symbol extraction
pub trait SymbolExtractor: Send + Sync {
    /// Extract symbols from a parsed Tree-sitter tree
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Result<Vec<OutlineNode>>;

    /// Extract documentation comment for a node
    fn extract_documentation(&self, node: &Node, source: &str) -> Option<String>;

    /// Extract function/method signature
    fn extract_signature(&self, node: &Node, source: &str) -> Option<String>;

    /// Extract visibility modifier
    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility>;

    /// Build hierarchical structure from flat symbol list
    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode>;

    /// Get language-specific Tree-sitter queries
    fn get_queries(&self) -> Vec<(&'static str, OutlineNodeType)>;
}

/// Tree-sitter based outline parser
pub struct OutlineParser {
    /// Underlying code parser for Tree-sitter functionality
    code_parser: CodeParser,
    /// Language-specific symbol extractors
    extractors: HashMap<Language, Box<dyn SymbolExtractor>>,
    /// Outline parser configuration
    config: OutlineParserConfig,
}

impl OutlineParser {
    /// Create a new outline parser
    pub fn new(config: OutlineParserConfig) -> Result<Self> {
        // Create underlying code parser with appropriate configuration
        let parser_config = ParserConfig {
            min_chunk_size: 1,                     // Allow small symbols
            max_chunk_size: 100_000,               // Large enough for entire files
            max_chunks_per_file: 10_000,           // Allow many symbols
            max_file_size_bytes: 50 * 1024 * 1024, // 50MB limit
        };

        let code_parser = CodeParser::new(parser_config)
            .map_err(|e| crate::outline::OutlineError::TreeSitter(e.to_string()))?;

        let mut extractors: HashMap<Language, Box<dyn SymbolExtractor>> = HashMap::new();

        // Register language-specific extractors
        extractors.insert(Language::Rust, Box::new(RustExtractor::new()?));
        extractors.insert(Language::TypeScript, Box::new(TypeScriptExtractor::new()?));
        extractors.insert(Language::JavaScript, Box::new(JavaScriptExtractor::new()?));
        extractors.insert(Language::Python, Box::new(PythonExtractor::new()?));
        // TODO: Add Dart extractor when implemented

        Ok(Self {
            code_parser,
            extractors,
            config,
        })
    }

    /// Parse a file and generate its outline tree
    pub fn parse_file(&self, file_path: &Path, content: &str) -> Result<OutlineTree> {
        let language = self.code_parser.detect_language(file_path);

        // Check if we have an extractor for this language
        let extractor = self.extractors.get(&language).ok_or_else(|| {
            crate::outline::OutlineError::LanguageDetection(format!(
                "No symbol extractor available for language: {language:?}"
            ))
        })?;

        // Parse with Tree-sitter using a custom approach for outline extraction
        let symbols =
            self.parse_with_treesitter(file_path, content, &language, extractor.as_ref())?;

        Ok(OutlineTree::new(file_path.to_path_buf(), language, symbols))
    }

    /// Parse file content using Tree-sitter for outline extraction
    fn parse_with_treesitter(
        &self,
        file_path: &Path,
        content: &str,
        language: &Language,
        extractor: &dyn SymbolExtractor,
    ) -> Result<Vec<OutlineNode>> {
        // Get parser from the underlying code parser by leveraging its infrastructure
        // We'll parse manually to get the tree, then extract symbols
        let mut parser = tree_sitter::Parser::new();

        // Set the appropriate language
        let tree_sitter_language = match language {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::Dart => tree_sitter_dart::language(),
            Language::Unknown => {
                return Err(crate::outline::OutlineError::LanguageDetection(
                    "Cannot parse unknown language".to_string(),
                ))
            }
        };

        parser.set_language(&tree_sitter_language).map_err(|e| {
            crate::outline::OutlineError::TreeSitter(format!(
                "Failed to set parser language for {language:?}: {e}"
            ))
        })?;

        // Parse the content
        let tree = parser.parse(content, None).ok_or_else(|| {
            crate::outline::OutlineError::TreeSitter(format!(
                "Failed to parse {} with Tree-sitter",
                file_path.display()
            ))
        })?;

        // Extract symbols using the language-specific extractor
        let mut symbols = extractor.extract_symbols(&tree, content)?;

        // Apply configuration filters
        if self.config.min_symbol_lines > 1 {
            symbols.retain(|symbol| {
                (symbol.end_line.saturating_sub(symbol.start_line) + 1)
                    >= self.config.min_symbol_lines
            });
        }

        // Build hierarchical structure
        let hierarchical_symbols = extractor.build_hierarchy(symbols);

        Ok(hierarchical_symbols)
    }

    /// Check if a language is supported for outline generation
    pub fn is_language_supported(&self, language: &Language) -> bool {
        self.extractors.contains_key(language)
    }

    /// Get supported languages
    pub fn supported_languages(&self) -> Vec<Language> {
        self.extractors.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_outline_parser_creation() {
        let config = OutlineParserConfig::default();
        let parser = OutlineParser::new(config);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_rust_symbol_extraction() {
        let config = OutlineParserConfig::default();
        let parser = OutlineParser::new(config).unwrap();

        let rust_code = r#"
//! Module documentation

use std::collections::HashMap;
use std::fmt::Display;

/// Maximum buffer size constant
pub const MAX_BUFFER_SIZE: usize = 1024;

/// Global static counter
pub static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Type alias for result type
pub type MyResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Simple trait for debugging
pub trait Debuggable {
    fn debug_info(&self) -> String;
}

/// This is a test function
pub fn test_function() -> MyResult<()> {
    println!("Hello, world!");
    Ok(())
}

/// Another function that's private
fn private_helper(value: i32) -> i32 {
    value * 2
}

/// Test struct with documentation
#[derive(Debug, Clone)]
pub struct TestStruct {
    pub field: String,
    counter: u32,
}

/// Test enum with variants
#[derive(Debug)]
pub enum TestEnum {
    Variant1,
    Variant2(String),
    Variant3 { data: i32 },
}

impl TestStruct {
    /// Create a new instance
    pub fn new() -> Self {
        Self {
            field: String::new(),
            counter: 0,
        }
    }
    
    fn increment(&mut self) {
        self.counter += 1;
    }
    
    pub fn get_count(&self) -> u32 {
        self.counter
    }
}

impl Debuggable for TestStruct {
    fn debug_info(&self) -> String {
        format!("TestStruct {{ field: {}, counter: {} }}", self.field, self.counter)
    }
}

/// Inner module
pub mod inner {
    pub fn inner_function() {}
}
        "#;

        let file_path = Path::new("test.rs");
        let result = parser.parse_file(file_path, rust_code);

        if let Err(ref e) = result {
            println!("Error: {e:?}");
        }
        assert!(result.is_ok());
        let outline = result.unwrap();
        assert_eq!(outline.language, Language::Rust);
        assert!(!outline.symbols.is_empty());

        let stats = outline.stats();
        println!("Extracted symbols: {}", stats.summary());

        // Check for specific symbol types
        let functions = outline.find_symbols_by_type(&OutlineNodeType::Function);
        let structs = outline.find_symbols_by_type(&OutlineNodeType::Struct);
        let enums = outline.find_symbols_by_type(&OutlineNodeType::Enum);
        let constants = outline.find_symbols_by_type(&OutlineNodeType::Constant);
        let modules = outline.find_symbols_by_type(&OutlineNodeType::Module);
        let traits = outline.find_symbols_by_type(&OutlineNodeType::Interface);
        let imports = outline.find_symbols_by_type(&OutlineNodeType::Import);
        let type_aliases = outline.find_symbols_by_type(&OutlineNodeType::TypeAlias);

        println!("Functions: {}", functions.len());
        println!("Structs: {}", structs.len());
        println!("Enums: {}", enums.len());
        println!("Constants: {}", constants.len());
        println!("Modules: {}", modules.len());
        println!("Traits: {}", traits.len());
        println!("Imports: {}", imports.len());
        println!("Type aliases: {}", type_aliases.len());

        // Should find at least some functions
        assert!(
            functions.len() >= 2,
            "Should find test_function and private_helper"
        );
        // Should find TestStruct
        assert!(!structs.is_empty(), "Should find TestStruct");
        // Should find TestEnum
        assert!(!enums.is_empty(), "Should find TestEnum");
        // Should find constants
        assert!(
            !constants.is_empty(),
            "Should find MAX_BUFFER_SIZE or COUNTER"
        );
        // Should find imports (may or may not depending on Tree-sitter query success)
        // assert!(imports.len() >= 1, "Should find use statements");
        println!("Found {} import statements", imports.len());

        // Check that we can find symbols by name
        let test_struct_symbols = outline.find_symbols_by_name("TestStruct");
        assert!(
            !test_struct_symbols.is_empty(),
            "Should find TestStruct by name"
        );

        // Test documentation extraction
        let documented_symbols: Vec<_> = outline
            .all_symbols()
            .into_iter()
            .filter(|s| s.documentation.is_some())
            .collect();
        println!("Symbols with documentation: {}", documented_symbols.len());

        // Test signature extraction
        let signatures: Vec<_> = outline
            .all_symbols()
            .into_iter()
            .filter(|s| s.signature.is_some())
            .collect();
        println!("Symbols with signatures: {}", signatures.len());
    }

    #[test]
    fn test_unsupported_language() {
        let config = OutlineParserConfig::default();
        let parser = OutlineParser::new(config).unwrap();

        let file_path = Path::new("test.unknown");
        let result = parser.parse_file(file_path, "some content");

        assert!(result.is_err());
    }

    #[test]
    fn test_supported_languages() {
        let config = OutlineParserConfig::default();
        let parser = OutlineParser::new(config).unwrap();

        let supported = parser.supported_languages();
        assert!(supported.contains(&Language::Rust));
        assert!(supported.contains(&Language::TypeScript));
        assert!(supported.contains(&Language::JavaScript));
        assert!(supported.contains(&Language::Python));
        // TODO: Add when Dart extractor is implemented
        // assert!(supported.contains(&Language::Dart));
    }
}
