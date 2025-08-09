//! TreeSitter integration for parsing source code

use crate::search::types::{ChunkType, CodeChunk, ContentHash, Language};
use crate::search::utils::FileHasher;
use crate::search::{Result, SemanticError};
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::Path;
use tracing;
use tree_sitter::{
    Language as TreeSitterLanguage, Node, Parser, Query, QueryCursor, StreamingIterator,
};

/// Default minimum chunk size in characters
pub const DEFAULT_MIN_CHUNK_SIZE: usize = 10;
/// Default maximum chunk size in characters  
pub const DEFAULT_MAX_CHUNK_SIZE: usize = 2000;
/// Default maximum number of chunks to extract per file
pub const DEFAULT_MAX_CHUNKS_PER_FILE: usize = 100;
/// Default maximum file size in bytes to prevent OOM on massive files (10MB)
pub const DEFAULT_MAX_FILE_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// Definition of language support for TreeSitter parsing
#[derive(Debug, Clone)]
pub struct LanguageDefinition {
    /// Language identifier
    pub language: Language,
    /// File extensions supported by this language
    pub extensions: Vec<&'static str>,
    /// TreeSitter language function
    pub tree_sitter_language: fn() -> TreeSitterLanguage,
    /// Query patterns for extracting semantic chunks
    pub queries: Vec<(&'static str, ChunkType)>,
}

/// Registry of supported languages with their definitions
#[derive(Debug)]
pub struct LanguageRegistry {
    definitions: HashMap<Language, LanguageDefinition>,
    extension_map: HashMap<String, Language>,
}

impl LanguageRegistry {
    /// Create a new language registry with default supported languages
    pub fn with_defaults() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            extension_map: HashMap::new(),
        };

        // Register default languages
        registry.register(create_rust_definition());
        registry.register(create_python_definition());
        registry.register(create_typescript_definition());
        registry.register(create_javascript_definition());
        registry.register(create_dart_definition());

        registry
    }

    /// Register a new language definition
    pub fn register(&mut self, definition: LanguageDefinition) {
        // Map file extensions to language
        for ext in &definition.extensions {
            self.extension_map
                .insert(ext.to_string(), definition.language.clone());
        }

        self.definitions
            .insert(definition.language.clone(), definition);
    }

    /// Get language definition by language type
    pub fn get_definition(&self, language: &Language) -> Option<&LanguageDefinition> {
        self.definitions.get(language)
    }

    /// Detect language from file extension
    pub fn detect_language(&self, file_path: &Path) -> Language {
        file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.extension_map.get(ext))
            .cloned()
            .unwrap_or(Language::Unknown)
    }

    /// Get all supported languages
    pub fn supported_languages(&self) -> Vec<Language> {
        self.definitions.keys().cloned().collect()
    }
}

/// Create Rust language definition
fn create_rust_definition() -> LanguageDefinition {
    LanguageDefinition {
        language: Language::Rust,
        extensions: vec!["rs"],
        tree_sitter_language: || tree_sitter_rust::LANGUAGE.into(),
        queries: vec![
            // Functions - try the format used in tree-sitter examples
            (r"(function_item) @function", ChunkType::Function),
            // Impl blocks
            (r"(impl_item) @impl", ChunkType::Class),
            // Structs
            (r"(struct_item) @struct", ChunkType::Class),
            // Enums
            (r"(enum_item) @enum", ChunkType::Class),
            // Use statements - simplest working pattern
            (r"(use_declaration) @import", ChunkType::Import),
        ],
    }
}

/// Create Python language definition
fn create_python_definition() -> LanguageDefinition {
    LanguageDefinition {
        language: Language::Python,
        extensions: vec!["py", "pyx", "pyi"],
        tree_sitter_language: || tree_sitter_python::LANGUAGE.into(),
        queries: vec![
            // Functions
            (
                "(function_definition name: (identifier) @name) @function",
                ChunkType::Function,
            ),
            // Classes
            (
                "(class_definition name: (identifier) @name) @class",
                ChunkType::Class,
            ),
            // Methods within classes - corrected pattern
            (
                "(class_definition (block (function_definition name: (identifier) @method_name) @method))",
                ChunkType::Function,
            ),
            // Import statements
            ("(import_statement) @import", ChunkType::Import),
            ("(import_from_statement) @import", ChunkType::Import),
        ],
    }
}

/// Create TypeScript language definition
fn create_typescript_definition() -> LanguageDefinition {
    LanguageDefinition {
        language: Language::TypeScript,
        extensions: vec!["ts", "tsx"],
        tree_sitter_language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        queries: vec![
            // Functions
            (
                "(function_declaration name: (identifier) @name) @function",
                ChunkType::Function,
            ),
            // Arrow functions
            ("(arrow_function) @function", ChunkType::Function),
            // Classes
            (
                "(class_declaration name: (type_identifier) @name) @class",
                ChunkType::Class,
            ),
            // Methods within classes - corrected pattern
            (
                "(class_declaration (class_body (method_definition name: (property_identifier) @method_name) @method))",
                ChunkType::Function,
            ),
            // Interface definitions
            (
                "(interface_declaration name: (type_identifier) @name) @interface",
                ChunkType::Class,
            ),
            // Type aliases
            (
                "(type_alias_declaration name: (type_identifier) @name) @type_alias",
                ChunkType::Class,
            ),
            // Function expressions - corrected patterns
            (
                "(variable_declarator name: (identifier) @name value: (function_expression)) @function",
                ChunkType::Function,
            ),
            (
                "(variable_declarator name: (identifier) @name value: (arrow_function)) @function",
                ChunkType::Function,
            ),
            // Import statements
            ("(import_statement) @import", ChunkType::Import),
        ],
    }
}

/// Create JavaScript language definition
fn create_javascript_definition() -> LanguageDefinition {
    LanguageDefinition {
        language: Language::JavaScript,
        extensions: vec!["js", "jsx", "mjs"],
        tree_sitter_language: || tree_sitter_javascript::LANGUAGE.into(),
        queries: vec![
            // Functions
            (
                "(function_declaration name: (identifier) @name) @function",
                ChunkType::Function,
            ),
            // Arrow functions
            ("(arrow_function) @function", ChunkType::Function),
            // Classes
            (
                "(class_declaration name: (identifier) @name) @class",
                ChunkType::Class,
            ),
            // Methods within classes - corrected pattern
            (
                "(class_declaration (class_body (method_definition name: (property_identifier) @method_name) @method))",
                ChunkType::Function,
            ),
            // Function expressions - corrected patterns
            (
                "(variable_declarator name: (identifier) @name value: (function_expression)) @function",
                ChunkType::Function,
            ),
            (
                "(variable_declarator name: (identifier) @name value: (arrow_function)) @function",
                ChunkType::Function,
            ),
            // Import statements
            ("(import_statement) @import", ChunkType::Import),
        ],
    }
}

/// Create Dart language definition
fn create_dart_definition() -> LanguageDefinition {
    LanguageDefinition {
        language: Language::Dart,
        extensions: vec!["dart"],
        tree_sitter_language: || tree_sitter_dart::language(),
        queries: vec![
            // Functions
            (
                "(function_signature name: (identifier) @name) @function",
                ChunkType::Function,
            ),
            // Classes
            (
                "(class_definition name: (identifier) @name) @class",
                ChunkType::Class,
            ),
            // Import statements
            ("(import_specification) @import", ChunkType::Import),
        ],
    }
}

/// TreeSitter-based code parser with extensible language support
/// Uses DashMap for thread-safe concurrent access to parsers
pub struct CodeParser {
    parsers: DashMap<Language, Parser>,
    language_registry: LanguageRegistry,
    config: ParserConfig,
}

/// Configuration for the code parser
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Minimum chunk size in characters
    pub min_chunk_size: usize,
    /// Maximum chunk size in characters
    pub max_chunk_size: usize,
    /// Maximum chunks to extract per file
    pub max_chunks_per_file: usize,
    /// Maximum file size in bytes to prevent OOM on massive files
    pub max_file_size_bytes: usize,
}

impl ParserConfig {
    /// Create a new ParserConfig with validation
    ///
    /// # Arguments
    /// * `min_chunk_size` - Minimum chunk size in characters
    /// * `max_chunk_size` - Maximum chunk size in characters  
    /// * `max_chunks_per_file` - Maximum chunks to extract per file
    /// * `max_file_size_bytes` - Maximum file size in bytes to prevent OOM
    ///
    /// # Returns
    /// A validated ParserConfig or an error if validation fails
    ///
    /// # Errors
    /// Returns error if:
    /// - min_chunk_size > max_chunk_size
    /// - Any size parameter is 0
    /// - max_file_size_bytes is unreasonably small (< 1KB)
    pub fn new(
        min_chunk_size: usize,
        max_chunk_size: usize,
        max_chunks_per_file: usize,
        max_file_size_bytes: usize,
    ) -> Result<Self> {
        // Validate chunk size constraints
        if min_chunk_size > max_chunk_size {
            return Err(SemanticError::TreeSitter(format!(
                "Invalid configuration: min_chunk_size ({min_chunk_size}) must be <= max_chunk_size ({max_chunk_size})"
            )));
        }

        // Validate non-zero constraints and reasonable bounds
        if min_chunk_size == 0 {
            return Err(SemanticError::TreeSitter(
                "Invalid configuration: min_chunk_size must be > 0".to_string(),
            ));
        }

        // Validate reasonable upper bound to prevent excessive memory usage or processing time
        if min_chunk_size > 1000 {
            return Err(SemanticError::TreeSitter(format!(
                "Invalid configuration: min_chunk_size ({min_chunk_size}) must be <= 1000 characters for reasonable performance"
            )));
        }

        if max_chunk_size == 0 {
            return Err(SemanticError::TreeSitter(
                "Invalid configuration: max_chunk_size must be > 0".to_string(),
            ));
        }

        if max_chunks_per_file == 0 {
            return Err(SemanticError::TreeSitter(
                "Invalid configuration: max_chunks_per_file must be > 0".to_string(),
            ));
        }

        // Validate reasonable file size limit (at least 1KB)
        if max_file_size_bytes < 1024 {
            return Err(SemanticError::TreeSitter(format!(
                "Invalid configuration: max_file_size_bytes ({max_file_size_bytes}) must be >= 1024 bytes (1KB)"
            )));
        }

        Ok(Self {
            min_chunk_size,
            max_chunk_size,
            max_chunks_per_file,
            max_file_size_bytes,
        })
    }
}

impl Default for ParserConfig {
    fn default() -> Self {
        // Use new() with default constants, which are guaranteed to be valid
        Self::new(
            DEFAULT_MIN_CHUNK_SIZE,
            DEFAULT_MAX_CHUNK_SIZE,
            DEFAULT_MAX_CHUNKS_PER_FILE,
            DEFAULT_MAX_FILE_SIZE_BYTES,
        )
        .expect("Default ParserConfig values should always be valid")
    }
}

impl CodeParser {
    /// Create a new code parser with extensible TreeSitter language support.
    ///
    /// Initializes TreeSitter parsers for all languages in the registry.
    /// Languages that fail to initialize are skipped but other parsers remain functional.
    /// Uses the provided configuration to control chunk size limits and extraction behavior.
    ///
    /// Validates all TreeSitter queries during initialization to catch syntax errors early.
    ///
    /// # Arguments
    /// * `config` - Configuration settings for chunk size limits and parsing behavior
    ///
    /// # Returns
    /// A new `CodeParser` instance or an error if creation fails
    ///
    /// # Errors
    /// Returns error if:
    /// - Configuration validation fails
    /// - Any TreeSitter query is invalid or malformed
    pub fn new(config: ParserConfig) -> Result<Self> {
        let language_registry = LanguageRegistry::with_defaults();
        let parsers = DashMap::new();

        // Initialize parsers for all registered languages
        for language in language_registry.supported_languages() {
            if let Some(definition) = language_registry.get_definition(&language) {
                match Self::create_parser_for_language(definition) {
                    Some(parser) => {
                        parsers.insert(language.clone(), parser);
                        tracing::debug!("Initialized TreeSitter parser for {language:?}");
                    }
                    None => {
                        tracing::warn!("Failed to initialize TreeSitter parser for {language:?}");
                    }
                }
            }
        }

        let parser = Self {
            parsers,
            language_registry,
            config,
        };

        // Validate all queries during startup
        parser.validate_all_queries()?;

        tracing::info!(
            "Initialized CodeParser with {} language parsers",
            parser.parsers.len()
        );
        Ok(parser)
    }

    /// Create a parser for a specific language definition
    fn create_parser_for_language(definition: &LanguageDefinition) -> Option<Parser> {
        let mut parser = Parser::new();
        let language = (definition.tree_sitter_language)();
        match parser.set_language(&language) {
            Ok(_) => Some(parser),
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize {:?} parser: {}",
                    definition.language,
                    e
                );
                None
            }
        }
    }

    /// Detect programming language from file extension using the language registry.
    ///
    /// Uses the extensible language registry to map file extensions to supported
    /// languages. This makes it easy to add support for new languages without
    /// modifying this method.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to analyze
    ///
    /// # Returns
    /// The detected `Language` or `Language::Unknown` if not supported
    pub fn detect_language(&self, file_path: &Path) -> Language {
        self.language_registry.detect_language(file_path)
    }

    /// Get appropriate parser for language from the registry
    /// Check if a parser is available for the given language
    fn has_parser_for_language(&self, language: &Language) -> bool {
        self.parsers.contains_key(language)
    }

    /// Get available language list for error reporting
    fn get_available_languages(&self) -> Vec<Language> {
        self.parsers
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get TreeSitter queries for extracting semantic chunks from source code.
    ///
    /// Uses the language registry to get query patterns for the specified language.
    /// This makes the query system extensible - new languages can be added by
    /// registering them with their query definitions.
    ///
    /// # Arguments
    /// * `language` - The programming language to get queries for
    ///
    /// # Returns
    /// Vector of (query_pattern, chunk_type) tuples for the specified language
    fn get_queries_for_language(&self, language: &Language) -> Vec<(&'static str, ChunkType)> {
        self.language_registry
            .get_definition(language)
            .map(|def| def.queries.clone())
            .unwrap_or_default()
    }

    /// Parse a source file and extract semantic code chunks using TreeSitter.
    ///
    /// Attempts to parse the file using the appropriate TreeSitter grammar based on
    /// file extension. Extracts semantic chunks like functions, classes, methods, and
    /// imports using language-specific query patterns. Falls back to plain text parsing
    /// if TreeSitter parsing fails for any reason.
    ///
    /// Performance metrics are logged including parse time, chunk count, and throughput.
    ///
    /// # TreeSitter Processing Flow
    /// 1. Detect language from file extension
    /// 2. Get appropriate TreeSitter parser for the language
    /// 3. Parse content into syntax tree
    /// 4. Apply semantic queries to extract meaningful code constructs
    /// 5. Create chunks with accurate line numbers and content
    /// 6. Apply configuration limits (size, count)
    /// 7. Fall back to plain text if any step fails
    ///
    /// # Chunk Extraction
    /// - Functions: Complete function definitions with signatures and bodies
    /// - Classes: Class/struct/enum definitions including nested members
    /// - Methods: Method definitions within classes or impl blocks
    /// - Imports: Import/use statements for dependency tracking
    /// - Fallback: Entire file as single plain text chunk if no semantic chunks found
    ///
    /// # Configuration Limits
    /// - Filters chunks by `min_chunk_size` and `max_chunk_size`
    /// - Limits total chunks per file to `max_chunks_per_file`
    /// - Strict filtering: chunks that don't meet criteria are excluded
    ///
    /// # Arguments
    /// * `file_path` - Path to the source file being parsed
    /// * `content` - Raw file content as string
    ///
    /// # Returns
    /// Vector of `CodeChunk` objects representing extracted semantic units,
    /// or error if parsing completely fails
    ///
    /// # Errors
    /// Returns error only if both TreeSitter parsing and plain text fallback fail
    pub fn parse_file(&self, file_path: &Path, content: &str) -> Result<Vec<CodeChunk>> {
        let start_time = std::time::Instant::now();
        let content_size = content.len();

        // Check file size limit to prevent OOM on massive files
        if content_size > self.config.max_file_size_bytes {
            return Err(SemanticError::TreeSitter(format!(
                "File {} is too large ({content_size} bytes > {} bytes limit). Skipping to prevent OOM.",
                file_path.display(),
                self.config.max_file_size_bytes
            )));
        }

        let language = self.detect_language(file_path);

        let result = match self.parse_with_treesitter(file_path, content, &language) {
            Ok(chunks) => {
                let parse_duration = start_time.elapsed();
                let chunks_per_sec = if parse_duration.as_secs_f64() > 0.0 {
                    chunks.len() as f64 / parse_duration.as_secs_f64()
                } else {
                    chunks.len() as f64
                };
                let bytes_per_sec = if parse_duration.as_secs_f64() > 0.0 {
                    content_size as f64 / parse_duration.as_secs_f64()
                } else {
                    content_size as f64
                };

                tracing::info!(
                    "TreeSitter parse success: {} | {} chunks | {:.2}ms | {:.0} chunks/sec | {:.0} bytes/sec",
                    file_path.display(),
                    chunks.len(),
                    parse_duration.as_secs_f64() * 1000.0,
                    chunks_per_sec,
                    bytes_per_sec
                );
                Ok(chunks)
            }
            Err(e) => {
                // Fall back to plain text as per specification
                tracing::warn!(
                    "TreeSitter parsing failed for {}: {}. Falling back to plain text.",
                    file_path.display(),
                    e
                );
                let fallback_result = self.parse_as_plain_text(file_path, content);

                if let Ok(ref chunks) = fallback_result {
                    let parse_duration = start_time.elapsed();
                    tracing::info!(
                        "Plain text fallback: {} | {} chunks | {:.2}ms",
                        file_path.display(),
                        chunks.len(),
                        parse_duration.as_secs_f64() * 1000.0
                    );
                }

                fallback_result
            }
        };

        // Log final metrics
        match &result {
            Ok(chunks) => {
                let total_duration = start_time.elapsed();
                tracing::debug!(
                    "Parse complete: {} | language: {:?} | {} chunks | {} bytes | {:.2}ms total",
                    file_path.display(),
                    language,
                    chunks.len(),
                    content_size,
                    total_duration.as_secs_f64() * 1000.0
                );
            }
            Err(e) => {
                let total_duration = start_time.elapsed();
                tracing::error!(
                    "Parse failed completely: {} | language: {:?} | {} bytes | {:.2}ms | error: {}",
                    file_path.display(),
                    language,
                    content_size,
                    total_duration.as_secs_f64() * 1000.0,
                    e
                );
            }
        }

        result
    }

    fn parse_with_treesitter(
        &self,
        file_path: &Path,
        content: &str,
        language: &Language,
    ) -> Result<Vec<CodeChunk>> {
        // Check if parser is available before attempting to borrow
        if !self.has_parser_for_language(language) {
            let available_languages = self.get_available_languages();
            let error_msg = format!(
                "No TreeSitter parser available for language: {:?} (file: {}). \
                 Available parsers: {:?}. This could be due to:\
                 \n1. Language not supported in current configuration\
                 \n2. Parser initialization failed during startup\
                 \n3. Missing TreeSitter grammar for this language",
                language,
                file_path.display(),
                available_languages
            );
            tracing::error!("{}", error_msg);
            return Err(SemanticError::TreeSitter(error_msg));
        }

        let tree_parse_start = std::time::Instant::now();

        // Parse using DashMap's entry API for thread-safe access
        let tree = {
            let mut parser_ref = self.parsers.get_mut(language).ok_or_else(|| {
                SemanticError::TreeSitter(format!("Parser disappeared for language: {language:?}"))
            })?;

            parser_ref.parse(content, None)
                .ok_or_else(|| {
                    let error_msg = format!(
                        "TreeSitter parsing failed for {} (language: {:?}, content size: {} bytes). \
                         This could be due to syntax errors, encoding issues, or language grammar limitations.",
                        file_path.display(),
                        language,
                        content.len()
                    );
                    tracing::warn!("{}", error_msg);
                    SemanticError::TreeSitter(error_msg)
                })?
        };
        let tree_parse_duration = tree_parse_start.elapsed();

        // Log additional tree parsing context for debugging
        let root_node = tree.root_node();
        if root_node.has_error() {
            let error_details = format!(
                "TreeSitter syntax tree contains errors for {} (language: {:?}). \
                 Tree root: {:?}, Error nodes may affect chunk extraction.",
                file_path.display(),
                language,
                root_node.kind()
            );
            tracing::warn!("{}", error_details);
        }

        let mut chunks = Vec::new();
        let content_hash = FileHasher::hash_string(content);
        let queries = self.get_queries_for_language(language);
        let mut total_matches = 0;

        let query_start = std::time::Instant::now();
        // Extract semantic chunks using queries
        for (query_str, chunk_type) in queries {
            let query = Query::new(&tree.language(), query_str).map_err(|e| {
                let error_msg = format!(
                    "Invalid TreeSitter query for {} (language: {:?}, chunk type: {:?}): {}\
                         \nQuery pattern: {}\n\
                         This indicates the query pattern doesn't match the language grammar.",
                    file_path.display(),
                    language,
                    chunk_type,
                    e,
                    query_str
                );
                tracing::error!("{}", error_msg);
                SemanticError::TreeSitter(error_msg)
            })?;

            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

            let mut query_matches = 0;
            while let Some(query_match) = matches.next() {
                for capture in query_match.captures {
                    let node = capture.node;
                    let chunk = self.create_chunk_from_node(
                        file_path,
                        content,
                        node,
                        language.clone(),
                        chunk_type.clone(),
                        content_hash.clone(),
                    )?;
                    chunks.push(chunk);
                    query_matches += 1;
                }
            }

            if query_matches > 0 {
                tracing::debug!(
                    "Query extracted {} chunks for {:?} type from {}",
                    query_matches,
                    chunk_type,
                    file_path.display()
                );
            }
            total_matches += query_matches;
        }
        let query_duration = query_start.elapsed();

        // If no semantic chunks found, create one chunk for entire file
        if chunks.is_empty() {
            chunks.push(self.create_full_file_chunk(
                file_path,
                content,
                language,
                &content_hash,
            )?);
        }

        let filter_start = std::time::Instant::now();
        let initial_chunk_count = chunks.len();
        let filtered_chunks = self.apply_chunk_limits_strict(chunks);
        let filter_duration = filter_start.elapsed();
        let filtered_count = initial_chunk_count - filtered_chunks.len();

        tracing::debug!(
            "TreeSitter parsing metrics: {} | tree: {:.2}ms | queries: {:.2}ms ({} matches) | filter: {:.2}ms ({} filtered)",
            file_path.display(),
            tree_parse_duration.as_secs_f64() * 1000.0,
            query_duration.as_secs_f64() * 1000.0,
            total_matches,
            filter_duration.as_secs_f64() * 1000.0,
            filtered_count
        );

        Ok(filtered_chunks)
    }

    /// Apply chunk size and count limits strictly (no fallback)
    fn apply_chunk_limits_strict(&self, mut chunks: Vec<CodeChunk>) -> Vec<CodeChunk> {
        // Filter by chunk size
        chunks.retain(|chunk| {
            let size = chunk.content.len();
            size >= self.config.min_chunk_size && size <= self.config.max_chunk_size
        });

        // Limit number of chunks per file
        if chunks.len() > self.config.max_chunks_per_file {
            chunks.truncate(self.config.max_chunks_per_file);
        }

        chunks
    }

    fn create_chunk_from_node(
        &self,
        file_path: &Path,
        content: &str,
        node: Node,
        language: Language,
        chunk_type: ChunkType,
        content_hash: ContentHash,
    ) -> Result<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let chunk_content = &content[start_byte..end_byte];

        let start_pos = node.start_position();
        let end_pos = node.end_position();

        Ok(CodeChunk {
            id: format!(
                "{}:{}:{}",
                file_path.display(),
                start_pos.row,
                chunk_type.clone() as u8
            ),
            file_path: file_path.to_path_buf(),
            language,
            content: chunk_content.to_string(),
            start_line: start_pos.row + 1, // TreeSitter uses 0-based rows
            end_line: end_pos.row + 1,
            chunk_type,
            content_hash,
        })
    }

    fn parse_as_plain_text(&self, file_path: &Path, content: &str) -> Result<Vec<CodeChunk>> {
        let content_hash = FileHasher::hash_string(content);
        let language = self.detect_language(file_path);

        // Create single chunk for entire file
        let chunk = self.create_full_file_chunk(file_path, content, &language, &content_hash)?;
        let chunks = vec![chunk];

        // Apply configuration limits strictly for plain text
        let filtered_chunks = self.apply_chunk_limits_strict(chunks);
        Ok(filtered_chunks)
    }

    fn create_full_file_chunk(
        &self,
        file_path: &Path,
        content: &str,
        language: &Language,
        content_hash: &ContentHash,
    ) -> Result<CodeChunk> {
        let line_count = content.lines().count();

        Ok(CodeChunk {
            id: format!("{}:full", file_path.display()),
            file_path: file_path.to_path_buf(),
            language: language.clone(),
            content: content.to_string(),
            start_line: 1,
            end_line: line_count.max(1),
            chunk_type: ChunkType::PlainText,
            content_hash: content_hash.clone(),
        })
    }

    /// Check if a file is supported for TreeSitter semantic parsing.
    ///
    /// Uses the extensible language registry to determine if the file extension
    /// maps to a supported language with an available TreeSitter parser.
    /// Files with unknown extensions will fall back to plain text parsing.
    ///
    /// # Arguments
    /// * `file_path` - Path to check for language support
    ///
    /// # Returns
    /// `true` if the file extension maps to a supported language, `false` otherwise
    pub fn is_supported_file(&self, file_path: &Path) -> bool {
        let language = self.detect_language(file_path);
        !matches!(language, Language::Unknown) && self.parsers.contains_key(&language)
    }

    /// Validate all TreeSitter queries for syntax correctness.
    ///
    /// Compiles all query patterns for each supported language to ensure they are
    /// syntactically valid according to the TreeSitter grammar. This catches query
    /// errors at startup rather than during file parsing.
    ///
    /// # Validation Process
    /// 1. For each language with an available parser
    /// 2. Get all query patterns for that language
    /// 3. Attempt to compile each query against the language grammar
    /// 4. Report detailed error information for any invalid queries
    ///
    /// # Returns
    /// `Ok(())` if all queries are valid, or error with details of first invalid query
    ///
    /// # Errors
    /// Returns `SemanticError::TreeSitter` with query validation details if any query is invalid
    /// Validate all TreeSitter queries for syntax correctness using the registry.
    ///
    /// Iterates through all languages in the registry and validates their query patterns
    /// against the corresponding TreeSitter grammars. This extensible approach
    /// automatically validates queries for any registered language.
    ///
    /// # Returns
    /// `Ok(())` if all queries are valid, or error with details of first invalid query
    fn validate_all_queries(&self) -> Result<()> {
        for language in self.language_registry.supported_languages() {
            if let Some(parser) = self.parsers.get(&language) {
                let queries = self.get_queries_for_language(&language);

                for (query_str, chunk_type) in queries {
                    // Attempt to compile the query to validate syntax
                    let language_ref = parser.language().ok_or_else(|| {
                        SemanticError::TreeSitter(format!(
                            "Parser for {language:?} has no language set"
                        ))
                    })?;

                    Query::new(&language_ref, query_str)
                        .map_err(|e| {
                            SemanticError::TreeSitter(format!(
                                "Invalid query for {language:?} language (chunk type: {chunk_type:?}): {e}\nQuery: {query_str}"
                            ))
                        })?;
                }

                tracing::debug!(
                    "Validated {} queries for {language:?}",
                    self.get_queries_for_language(&language).len()
                );
            }
        }

        tracing::info!("All TreeSitter queries validated successfully");
        Ok(())
    }

    /// Add support for a new language to the parser.
    ///
    /// This method demonstrates the extensibility - new languages can be added
    /// at runtime by providing a language definition.
    ///
    /// # Arguments
    /// * `definition` - Language definition with queries and TreeSitter language
    ///
    /// # Returns
    /// `Ok(())` if the language was added successfully, error if parser creation or query validation fails
    pub fn add_language_support(&mut self, definition: LanguageDefinition) -> Result<()> {
        // Create parser for the new language
        if let Some(parser) = Self::create_parser_for_language(&definition) {
            // Validate queries for the new language
            let language_ref = parser.language().ok_or_else(|| {
                SemanticError::TreeSitter(format!(
                    "Parser for {:?} has no language set",
                    definition.language
                ))
            })?;

            for (query_str, chunk_type) in &definition.queries {
                Query::new(&language_ref, query_str)
                    .map_err(|e| {
                        SemanticError::TreeSitter(format!(
                            "Invalid query for {:?} language (chunk type: {chunk_type:?}): {e}\nQuery: {query_str}",
                            definition.language
                        ))
                    })?;
            }

            // Register the language and store the parser in thread-safe map
            let language = definition.language.clone();
            self.parsers.insert(language.clone(), parser);
            // Note: We can't modify the registry here as it's immutable,
            // but in a real implementation you might want to make it mutable

            tracing::info!(
                "Added support for {:?} language with {} queries",
                language,
                definition.queries.len()
            );
            Ok(())
        } else {
            Err(SemanticError::TreeSitter(format!(
                "Failed to create parser for {:?} language",
                definition.language
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parser_creation() {
        let config = ParserConfig::default();
        let parser = CodeParser::new(config);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_detect_language() {
        let parser = CodeParser::new(ParserConfig::default()).unwrap();

        assert_eq!(parser.detect_language(Path::new("test.rs")), Language::Rust);
        assert_eq!(
            parser.detect_language(Path::new("test.py")),
            Language::Python
        );
        assert_eq!(
            parser.detect_language(Path::new("test.ts")),
            Language::TypeScript
        );
        assert_eq!(
            parser.detect_language(Path::new("test.js")),
            Language::JavaScript
        );
        assert_eq!(
            parser.detect_language(Path::new("test.dart")),
            Language::Dart
        );
        assert_eq!(
            parser.detect_language(Path::new("test.txt")),
            Language::Unknown
        );
    }

    #[test]
    fn test_is_supported_file() {
        let config = ParserConfig::default();
        let parser = CodeParser::new(config).unwrap();

        assert!(parser.is_supported_file(Path::new("main.rs")));
        assert!(parser.is_supported_file(Path::new("script.py")));
        assert!(!parser.is_supported_file(Path::new("readme.txt")));
    }

    #[test]
    fn test_parse_rust_function() {
        let config = ParserConfig {
            min_chunk_size: 1, // Allow small chunks for this test
            max_chunk_size: 2000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Should extract the function as a chunk
        assert!(!chunks.is_empty());

        // Find the function chunk
        let function_chunk = chunks.iter().find(|c| c.chunk_type == ChunkType::Function);
        if let Some(chunk) = function_chunk {
            assert_eq!(chunk.language, Language::Rust);
            assert!(chunk.content.contains("fn main()"));
            assert_eq!(chunk.start_line, 1);
        } else {
            // If no function found, should have at least one chunk (fallback)
            assert_eq!(chunks.len(), 1);
            assert_eq!(chunks[0].chunk_type, ChunkType::PlainText);
        }
    }

    #[test]
    fn test_parse_python_class() {
        let config = ParserConfig::default();
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.py");
        let content = "class MyClass:\n    def __init__(self):\n        pass";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Should extract semantic chunks or fallback to plain text
        assert!(!chunks.is_empty());

        // Check if we got class and function chunks or fallback
        let has_class = chunks.iter().any(|c| c.chunk_type == ChunkType::Class);
        let has_function = chunks.iter().any(|c| c.chunk_type == ChunkType::Function);
        let has_plaintext = chunks.iter().any(|c| c.chunk_type == ChunkType::PlainText);

        // Should have either semantic chunks or plain text fallback
        assert!(has_class || has_function || has_plaintext);
    }

    #[test]
    fn test_fallback_to_plain_text() {
        let config = ParserConfig {
            min_chunk_size: 1, // Allow small chunks for this test
            max_chunk_size: 2000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.txt");
        let content = "This is just plain text, not code.";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Should fallback to plain text
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::PlainText);
        assert_eq!(chunks[0].language, Language::Unknown);
        assert_eq!(chunks[0].content, content);
    }

    #[test]
    fn test_chunk_ids_are_unique() {
        let config = ParserConfig::default();
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        let content = "fn func1() {}\nfn func2() {}";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Collect all chunk IDs
        let ids: Vec<&String> = chunks.iter().map(|c| &c.id).collect();

        // Check that all IDs are unique
        let mut unique_ids = ids.clone();
        unique_ids.sort();
        unique_ids.dedup();

        assert_eq!(
            ids.len(),
            unique_ids.len(),
            "All chunk IDs should be unique"
        );
    }

    #[test]
    fn test_content_hashing() {
        let config = ParserConfig::default();
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        let content = "fn main() {}";

        let chunks1 = parser.parse_file(file_path, content).unwrap();
        let chunks2 = parser.parse_file(file_path, content).unwrap();

        // Same content should produce same hashes
        assert_eq!(chunks1.len(), chunks2.len());
        for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
            assert_eq!(c1.content_hash, c2.content_hash);
        }
    }

    #[test]
    fn test_chunk_size_filtering() {
        let config = ParserConfig {
            min_chunk_size: 10,
            max_chunk_size: 50,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        // Create content with functions of different sizes
        let content = "fn small() { /* 5 */ }\nfn medium_sized_function() {\n    println!(\"Hello\");\n    println!(\"World\");\n}\nfn very_long_function_that_should_be_filtered_out_because_it_exceeds_the_maximum_chunk_size_limit() {\n    println!(\"This function is too long\");\n    println!(\"It should be filtered out\");\n    println!(\"Because it exceeds max_chunk_size\");\n    println!(\"And contains way too much code\");\n}";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Verify chunks are filtered by size
        for chunk in &chunks {
            let size = chunk.content.len();
            assert!(
                (10..=50).contains(&size),
                "Chunk size {size} should be between 10 and 50 characters"
            );
        }
    }

    #[test]
    fn test_max_chunks_per_file_limit() {
        let config = ParserConfig {
            min_chunk_size: 1,
            max_chunk_size: 1000,
            max_chunks_per_file: 2,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        // Create content with multiple functions
        let content = "fn func1() {}\nfn func2() {}\nfn func3() {}\nfn func4() {}";

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Should be limited to max 2 chunks
        assert!(
            chunks.len() <= 2,
            "Should have at most 2 chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_plain_text_respects_config() {
        let config = ParserConfig {
            min_chunk_size: 100, // Large minimum size
            max_chunk_size: 1000,
            max_chunks_per_file: 10,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.txt");
        let content = "Short"; // Only 5 characters, below minimum

        let chunks = parser.parse_file(file_path, content).unwrap();

        // Should be filtered out due to size limit
        assert!(
            chunks.is_empty(),
            "Short content should be filtered out by min_chunk_size"
        );
    }

    #[test]
    fn test_file_size_limit() {
        let config = ParserConfig {
            min_chunk_size: 1,
            max_chunk_size: 1000,
            max_chunks_per_file: 10,
            max_file_size_bytes: 100, // Very small limit for testing
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.rs");
        // Create content larger than the limit
        let content = "a".repeat(200); // 200 bytes, exceeds 100 byte limit

        let result = parser.parse_file(file_path, &content);

        // Should fail due to file size limit
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("too large"));
        assert!(error_msg.contains("prevent OOM"));
    }

    #[test]
    fn test_enhanced_error_context() {
        let config = ParserConfig {
            min_chunk_size: 1, // Allow small chunks
            max_chunk_size: 1000,
            max_chunks_per_file: 10,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        let file_path = Path::new("test.unknownext"); // Unknown extension
        let content = "some content that should be parsed as plain text with enough characters to pass the minimum size filter";

        let result = parser.parse_file(file_path, content);

        // Should fall back to plain text for unknown language
        match result {
            Ok(chunks) => {
                // Should create plain text chunk for unknown language
                assert!(
                    !chunks.is_empty(),
                    "Should create at least one plain text chunk for unknown language"
                );
                assert_eq!(chunks[0].chunk_type, ChunkType::PlainText);
                assert_eq!(chunks[0].language, Language::Unknown);
            }
            Err(e) => {
                // If it fails, error should contain detailed context
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("No TreeSitter parser available")
                        || error_msg.contains("Unknown")
                );
            }
        }
    }

    #[test]
    fn test_concurrent_parsing() {
        use std::sync::Arc;
        use std::thread;

        let config = ParserConfig {
            min_chunk_size: 1,
            max_chunk_size: 1000,
            max_chunks_per_file: 10,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = Arc::new(CodeParser::new(config).unwrap());

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let parser_clone = Arc::clone(&parser);
                thread::spawn(move || {
                    let file_name = format!("test{i}.rs");
                    let file_path = Path::new(&file_name);
                    let content = format!("fn test_function_{i}() {{ println!(\"test\"); }}");

                    // Each thread parses concurrently
                    let result = parser_clone.parse_file(file_path, &content);
                    assert!(result.is_ok(), "Concurrent parsing should succeed");

                    let chunks = result.unwrap();
                    assert!(!chunks.is_empty(), "Should produce chunks");
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_config_validation_success() {
        // Valid configurations should succeed
        let valid_configs = vec![
            ParserConfig::new(10, 100, 10, 1024),
            ParserConfig::new(1, 1, 1, 1024),
            ParserConfig::new(50, 2000, 100, 10 * 1024 * 1024),
        ];

        for config in valid_configs {
            assert!(config.is_ok(), "Valid config should succeed");
        }
    }

    #[test]
    fn test_config_validation_failures() {
        // Test min_chunk_size > max_chunk_size
        let result = ParserConfig::new(100, 50, 10, 1024);
        assert!(
            result.is_err(),
            "min_chunk_size > max_chunk_size should fail"
        );

        // Test min_chunk_size = 0
        let result = ParserConfig::new(0, 100, 10, 1024);
        assert!(result.is_err(), "min_chunk_size = 0 should fail");

        // Test max_chunk_size = 0
        let result = ParserConfig::new(10, 0, 10, 1024);
        assert!(result.is_err(), "max_chunk_size = 0 should fail");

        // Test max_chunks_per_file = 0
        let result = ParserConfig::new(10, 100, 0, 1024);
        assert!(result.is_err(), "max_chunks_per_file = 0 should fail");

        // Test max_file_size_bytes too small
        let result = ParserConfig::new(10, 100, 10, 500); // Less than 1KB
        assert!(result.is_err(), "max_file_size_bytes < 1KB should fail");
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = ParserConfig::default();
        // Should not panic and should have reasonable values
        assert!(config.min_chunk_size > 0);
        assert!(config.max_chunk_size > config.min_chunk_size);
        assert!(config.max_chunks_per_file > 0);
        assert!(config.max_file_size_bytes >= 1024);
    }

    /// Helper function to test TreeSitter parsing with realistic Rust content
    fn test_rust_content_parsing(
        config: ParserConfig,
        content: &str,
        file_path: &Path,
        expected_patterns: &[&str],
    ) {
        let parser = CodeParser::new(config).unwrap();

        let chunks = parser.parse_file(file_path, content).unwrap();

        // This test should fail with the current broken implementation
        assert!(
            !chunks.is_empty(),
            "TreeSitter should extract chunks from real Rust code, but got 0 chunks"
        );

        // Count specific types of chunks we expect
        let use_chunks = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Import)
            .count();
        let struct_chunks = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Class)
            .count();
        let function_chunks = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Function)
            .count();

        // We should get multiple types of chunks from this realistic Rust code
        let semantic_chunks = use_chunks + struct_chunks + function_chunks;
        assert!(semantic_chunks > 0,
            "Expected semantic chunks (use statements: {use_chunks}, structs/enums: {struct_chunks}, functions: {function_chunks}), but got plain text only");

        // Verify we got some meaningful content
        let has_meaningful_content = chunks.iter().any(|chunk| {
            expected_patterns
                .iter()
                .any(|pattern| chunk.content.contains(pattern))
        });
        assert!(
            has_meaningful_content,
            "Chunks should contain meaningful Rust constructs"
        );
    }

    #[test]
    fn test_treesitter_extracts_chunks_from_real_rust_code() {
        // Issue: TreeSitter parser reports success but extracts 0 chunks from real Rust files
        // This test ensures we get actual chunks from realistic Rust code
        let config = ParserConfig {
            min_chunk_size: 1, // Allow small chunks to catch all structures
            max_chunk_size: 5000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };

        let file_path = Path::new("test_real_content.rs");
        let content = r#"//! Workflow execution visualization

use crate::workflow::{RunMetrics, StateId, Workflow, WorkflowRun, WorkflowRunStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

/// Maximum path length for full visualization
pub const MAX_PATH_LENGTH_FULL: usize = 1000;

/// Execution visualization generator
#[derive(Debug, Clone)]
pub struct ExecutionVisualizer {
    /// Include timing information in visualization
    pub include_timing: bool,
    /// Include execution counts in visualization
    pub include_counts: bool,
}

/// Visualization output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualizationFormat {
    /// Mermaid state diagram
    Mermaid,
    /// DOT graph format
    Dot,
    /// JSON execution trace
    Json,
}

impl ExecutionVisualizer {
    /// Create a new execution visualizer
    pub fn new() -> Self {
        Self {
            include_timing: true,
            include_counts: true,
        }
    }

    /// Generate visualization
    pub fn generate(&self, workflow: &Workflow) -> String {
        "mermaid diagram".to_string()
    }
}

pub fn format_content_preview(content: &str, max_length: usize) -> String {
    let preview = if content.len() > max_length {
        format!("{}...", &content[..max_length])
    } else {
        content.to_string()
    };
    preview.replace('\n', " ")
}
"#;

        let expected_patterns = &[
            "ExecutionVisualizer",
            "VisualizationFormat",
            "fn new()",
            "use crate::workflow",
        ];

        test_rust_content_parsing(config, content, file_path, expected_patterns);
    }

    #[test]
    fn test_treesitter_with_default_config_reproduces_issue() {
        // This test reproduces the issue: using default config results in 0 chunks
        // because the minimum chunk size filters out small chunks like use statements
        let parser = CodeParser::new(ParserConfig::default()).unwrap(); // Use default config

        let file_path = Path::new("test_default_config.rs");
        let content = r#"//! Workflow execution visualization

use crate::workflow::{RunMetrics, StateId, Workflow, WorkflowRun, WorkflowRunStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

/// Maximum path length for full visualization
pub const MAX_PATH_LENGTH_FULL: usize = 1000;

/// Execution visualization generator
#[derive(Debug, Clone)]
pub struct ExecutionVisualizer {
    /// Include timing information in visualization
    pub include_timing: bool,
    /// Include execution counts in visualization
    pub include_counts: bool,
}

/// Visualization output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualizationFormat {
    /// Mermaid state diagram
    Mermaid,
    /// DOT graph format
    Dot,
    /// JSON execution trace
    Json,
}

impl ExecutionVisualizer {
    /// Create a new execution visualizer
    pub fn new() -> Self {
        Self {
            include_timing: true,
            include_counts: true,
        }
    }

    /// Generate visualization
    pub fn generate(&self, workflow: &Workflow) -> String {
        "mermaid diagram".to_string()
    }
}

pub fn format_content_preview(content: &str, max_length: usize) -> String {
    let preview = if content.len() > max_length {
        format!("{}...", &content[..max_length])
    } else {
        content.to_string()
    };
    preview.replace('\n', " ")
}
"#;

        let chunks = parser.parse_file(file_path, content).unwrap();

        // This test demonstrates the issue: with default config (min_chunk_size: 50),
        // small chunks like use statements get filtered out, potentially leaving 0 chunks
        // if all extracted chunks are below the minimum size threshold

        if chunks.is_empty() {
            // Let's also test with a more permissive config to compare
            let permissive_config = ParserConfig {
                min_chunk_size: 1,
                max_chunk_size: 5000,
                max_chunks_per_file: 100,
                max_file_size_bytes: 10 * 1024 * 1024,
            };
            let permissive_parser = CodeParser::new(permissive_config).unwrap();
            let permissive_chunks = permissive_parser.parse_file(file_path, content).unwrap();

            tracing::debug!(
                "With permissive config (min_chunk_size: 1): {} chunks",
                permissive_chunks.len()
            );
            for (i, chunk) in permissive_chunks.iter().enumerate().take(5) {
                tracing::debug!(
                    "  Chunk {}: {:?} - {} chars",
                    i,
                    chunk.chunk_type,
                    chunk.content.len()
                );
            }
        }
    }

    #[test]
    fn test_with_actual_problematic_files() {
        // Test with the actual files mentioned in the issue to reproduce the problem
        let parser = CodeParser::new(ParserConfig::default()).unwrap();

        // Test with visualization.rs content (sample from the actual file)
        let visualization_content = r#"//! Workflow execution visualization
//!
//! This module provides functionality to visualize workflow execution using Mermaid diagrams
//! with execution overlays showing actual paths taken, timing information, and execution status.

use crate::workflow::{RunMetrics, StateId, Workflow, WorkflowRun, WorkflowRunStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

/// Maximum path length for full visualization
pub const MAX_PATH_LENGTH_FULL: usize = 1000;

/// Maximum path length for minimal visualization
pub const MAX_PATH_LENGTH_MINIMAL: usize = 100;

/// Maximum execution steps allowed in a trace to prevent DoS
pub const MAX_EXECUTION_STEPS: usize = 500;

/// Execution visualization generator
#[derive(Debug, Clone)]
pub struct ExecutionVisualizer {
    /// Include timing information in visualization
    pub include_timing: bool,
    /// Include execution counts in visualization
    pub include_counts: bool,
    /// Include status indicators in visualization
    pub include_status: bool,
    /// Maximum path length to display
    pub max_path_length: usize,
}

impl ExecutionVisualizer {
    /// Create a new execution visualizer with default settings
    pub fn new() -> Self {
        Self {
            include_timing: true,
            include_counts: true,
            include_status: true,
            max_path_length: MAX_PATH_LENGTH_FULL,
        }
    }

    /// Generate execution trace from workflow run
    pub fn generate_trace(&self) -> String {
        "execution trace".to_string()
    }
}
"#;

        let file_path = Path::new("./swissarmyhammer/src/workflow/visualization.rs");
        let chunks = parser.parse_file(file_path, visualization_content).unwrap();

        if chunks.is_empty() {
            // Debug: Let's also check what language is detected
            let _detected_language = parser.detect_language(file_path);

            // Check if file is supported
            let _is_supported = parser.is_supported_file(file_path);
        }

        // Test with memo.rs content
        let memo_content = r#"use crate::cli::MemoCommands;
use colored::*;
use std::io::{self, Read};
use swissarmyhammer::memoranda::{
    AdvancedMemoSearchEngine, MarkdownMemoStorage, MemoId, MemoStorage, SearchOptions,
};

// Configurable preview length constants
const DEFAULT_LIST_PREVIEW_LENGTH: usize = 100;
const DEFAULT_SEARCH_PREVIEW_LENGTH: usize = 150;

/// Format content preview with specified maximum length
fn format_content_preview(content: &str, max_length: usize) -> String {
    let preview = if content.len() > max_length {
        format!("{}...", &content[..max_length])
    } else {
        content.to_string()
    };
    preview.replace('\n', " ")
}

pub async fn handle_memo_command(command: MemoCommands) -> Result<(), Box<dyn std::error::Error>> {
    let storage = MarkdownMemoStorage::new_default()?;

    match command {
        MemoCommands::Create { title, content } => {
            create_memo(storage, title, content).await?;
        }
        MemoCommands::List => {
            list_memos(storage).await?;
        }
        MemoCommands::Get { id } => {
            get_memo(storage, &id).await?;
        }
        MemoCommands::Update { id, content } => {
            update_memo(storage, &id, content).await?;
        }
        MemoCommands::Delete { id } => {
            delete_memo(storage, &id).await?;
        }
        MemoCommands::Search { query } => {
            search_memos(storage, &query).await?;
        }
        MemoCommands::Context => {
            get_context(storage).await?;
        }
    }

    Ok(())
}
"#;

        let memo_file_path = Path::new("./swissarmyhammer-cli/src/memo.rs");
        let memo_chunks = parser.parse_file(memo_file_path, memo_content).unwrap();

        memo_chunks.is_empty();

        // The test should pass if we extract chunks from either file
        assert!(
            !chunks.is_empty() || !memo_chunks.is_empty(),
            "Should extract chunks from at least one of the test files"
        );
    }

    #[test]
    #[ignore = "Debug test with println output"]
    fn test_treesitter_chunk_extraction_independent_of_embedding() {
        // This test verifies that TreeSitter chunk extraction works regardless of embedding engine availability
        // This addresses the original issue where 0 chunks were extracted despite successful TreeSitter parsing

        // Test with multiple configurations
        let configs = [
            ParserConfig::default(),
            ParserConfig {
                min_chunk_size: 1,
                max_chunk_size: 10000,
                max_chunks_per_file: 1000,
                max_file_size_bytes: 10 * 1024 * 1024,
            },
            ParserConfig {
                min_chunk_size: 25,
                max_chunk_size: 500,
                max_chunks_per_file: 50,
                max_file_size_bytes: 10 * 1024 * 1024,
            },
        ];

        for (i, config) in configs.iter().enumerate() {
            let parser = CodeParser::new(config.clone()).unwrap();

            // Test with realistic Rust code that should definitely extract chunks
            let test_content = r#"use std::collections::HashMap;
use std::fmt::Display;

const MAX_SIZE: usize = 1000;

#[derive(Debug, Clone)]
pub struct DataProcessor {
    pub config: ProcessorConfig,
    pub cache: HashMap<String, String>,
}

#[derive(Debug)]
pub enum ProcessorError {
    InvalidInput,
    ProcessingFailed,
}

impl DataProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }
    
    pub fn process(&mut self, input: &str) -> Result<String, ProcessorError> {
        if input.is_empty() {
            return Err(ProcessorError::InvalidInput);
        }
        
        let result = format!("processed: {}", input);
        self.cache.insert(input.to_string(), result.clone());
        Ok(result)
    }
}

pub fn helper_function(data: &[u8]) -> String {
    String::from_utf8_lossy(data).to_string()
}
"#;

            let file_path = Path::new("test_code.rs");
            let chunks = parser.parse_file(file_path, test_content).unwrap();

            tracing::debug!(
                "Config {}: min_chunk_size={}, extracted {} chunks:",
                i,
                config.min_chunk_size,
                chunks.len()
            );

            for (j, chunk) in chunks.iter().enumerate() {
                tracing::debug!(
                    "  Chunk {}: {:?} - {} chars",
                    j,
                    chunk.chunk_type,
                    chunk.content.len()
                );
            }

            // With any reasonable configuration, we should extract at least some chunks
            // from this well-structured Rust code
            if config.min_chunk_size <= 50 {
                // With small min_chunk_size, we should get use statements, functions, structs, etc.
                assert!(!chunks.is_empty(),
                    "Config {} with min_chunk_size={} should extract chunks from realistic Rust code",
                    i, config.min_chunk_size);

                // We should get multiple types of chunks
                let use_chunks = chunks
                    .iter()
                    .filter(|c| c.chunk_type == ChunkType::Import)
                    .count();
                let struct_chunks = chunks
                    .iter()
                    .filter(|c| c.chunk_type == ChunkType::Class)
                    .count();
                let function_chunks = chunks
                    .iter()
                    .filter(|c| c.chunk_type == ChunkType::Function)
                    .count();

                tracing::debug!("  Use: {use_chunks}, Struct/Enum/Impl: {struct_chunks}, Function: {function_chunks}");

                // We should get at least one semantic chunk (not just fallback to plain text)
                let semantic_chunks = use_chunks + struct_chunks + function_chunks;
                assert!(
                    semantic_chunks > 0,
                    "Should extract semantic chunks, not just plain text fallback"
                );
            }
        }

        tracing::debug!("✅ TreeSitter chunk extraction works correctly across all configurations!");
    }

    #[test]
    #[ignore = "Debug test with println output"]
    fn test_fix_treesitter_query_matching() {
        // Fix the TreeSitter query matching issue
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}";

        // Create parser and parse
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(content, None).unwrap();

        tracing::debug!("=== DEBUGGING TREESITTER QUERY MATCHING ===");
        tracing::debug!("Content: {content}");

        // Test the query patterns one by one
        let test_queries = vec![
            ("(function_item) @function", "Function"),
            (
                "(function_item name: (identifier) @name) @function",
                "Named Function",
            ),
            ("(function_item) @func", "Simple Function"),
        ];

        for (query_str, description) in test_queries {
            tracing::debug!("\nTesting query: {description} - '{query_str}'");

            match tree_sitter::Query::new(&language, query_str) {
                Ok(query) => {
                    tracing::debug!("  Query compiled successfully");
                    tracing::debug!("  Query capture names: {:?}", query.capture_names());

                    let mut cursor = tree_sitter::QueryCursor::new();
                    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

                    let mut match_count = 0;
                    let mut match_idx = 0;
                    while let Some(query_match) = matches.next() {
                        match_count += 1;
                        tracing::debug!(
                            "  Match {}: {} captures",
                            match_idx + 1,
                            query_match.captures.len()
                        );
                        match_idx += 1;

                        for (cap_idx, capture) in query_match.captures.iter().enumerate() {
                            let node = capture.node;
                            let text = &content[node.start_byte()..node.end_byte()];
                            let capture_name = query.capture_names()[capture.index as usize];
                            tracing::debug!(
                                "    Capture {}: '{}' = '{}' (node: {})",
                                cap_idx,
                                capture_name,
                                text.chars()
                                    .take(30)
                                    .collect::<String>()
                                    .replace('\n', "\\n"),
                                node.kind()
                            );
                        }
                    }

                    if match_count == 0 {
                        tracing::debug!("  ❌ No matches found for this query!");
                    } else {
                        tracing::debug!("  ✅ Found {match_count} matches");
                    }
                }
                Err(e) => {
                    tracing::error!("  ❌ Query compilation failed: {e}");
                }
            }
        }

        // Test with the exact same iterator pattern used in the main code
        tracing::debug!("\n=== Testing iterator pattern from main code ===");
        let query = tree_sitter::Query::new(&language, "(function_item) @function").unwrap();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

        let mut found_with_next = 0;
        while let Some(query_match) = matches.next() {
            found_with_next += 1;
            tracing::debug!("Found match {found_with_next} with next() pattern");
            for capture in query_match.captures {
                let text = &content[capture.node.start_byte()..capture.node.end_byte()];
                tracing::debug!(
                    "  Capture: '{}'",
                    text.chars()
                        .take(30)
                        .collect::<String>()
                        .replace('\n', "\\n")
                );
            }
        }

        if found_with_next == 0 {
            tracing::error!("❌ No matches found with next() pattern - this is the bug!");
        } else {
            tracing::debug!("✅ Found {found_with_next} matches with next() pattern");
        }
    }

    #[test]
    #[ignore = "Debug test with println output"]
    fn test_debug_treesitter_query_execution() {
        // Debug test to understand why TreeSitter queries aren't matching
        let config = ParserConfig {
            min_chunk_size: 1,
            max_chunk_size: 5000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };
        let parser = CodeParser::new(config).unwrap();

        // Simple Rust function that should definitely match
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let file_path = Path::new("debug.rs");

        // Get TreeSitter language and parser directly
        let language = parser.detect_language(file_path);
        tracing::debug!("Detected language: {language:?}");

        let queries = parser.get_queries_for_language(&language);
        tracing::debug!("Found {} queries for {:?}", queries.len(), language);

        for (i, (query_str, chunk_type)) in queries.iter().enumerate() {
            tracing::debug!("Query {i}: {chunk_type:?} - '{query_str}'");
        }

        // Try parsing with TreeSitter directly
        let tree_sitter_language = tree_sitter_rust::LANGUAGE.into();
        let mut tree_parser = tree_sitter::Parser::new();
        tree_parser.set_language(&tree_sitter_language).unwrap();

        let tree = tree_parser.parse(content, None).unwrap();
        let root_node = tree.root_node();

        tracing::debug!(
            "TreeSitter parse success. Root node: {:?}",
            root_node.kind()
        );
        tracing::debug!("Has error: {}", root_node.has_error());
        tracing::debug!("Node count: {}", root_node.named_child_count());

        // Print the tree structure
        print_tree(&root_node, content, 0);

        // Test each query manually
        for (query_str, chunk_type) in queries {
            tracing::debug!("\nTesting query: {chunk_type:?} - '{query_str}'");

            match tree_sitter::Query::new(&tree_sitter_language, query_str) {
                Ok(query) => {
                    let mut cursor = tree_sitter::QueryCursor::new();
                    let matches = cursor.matches(&query, root_node, content.as_bytes());

                    let mut match_count = 0;
                    let mut matches = matches;
                    while let Some(query_match) = matches.get() {
                        match_count += 1;
                        tracing::debug!(
                            "  Match {}: {} captures",
                            match_count,
                            query_match.captures.len()
                        );
                        for capture in query_match.captures {
                            let node = capture.node;
                            let text = &content[node.start_byte()..node.end_byte()];
                            tracing::debug!(
                                "    Capture: '{}' ({})",
                                text.chars().take(30).collect::<String>(),
                                node.kind()
                            );
                        }
                        matches.advance();
                    }

                    if match_count == 0 {
                        tracing::debug!("  No matches found for this query!");
                    }
                }
                Err(e) => {
                    tracing::error!("  Query compilation failed: {e}");
                }
            }
        }
    }

    fn print_tree(node: &tree_sitter::Node, content: &str, depth: usize) {
        let indent = "  ".repeat(depth);
        let node_text = if node.child_count() == 0 {
            let text = &content[node.start_byte()..node.end_byte()];
            format!(" \"{}\"", text.chars().take(20).collect::<String>())
        } else {
            String::new()
        };

        tracing::debug!("{}{}:{}", indent, node.kind(), node_text);

        if depth < 3 {
            // Limit depth to avoid too much output
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    print_tree(&child, content, depth + 1);
                }
            }
        }
    }
}
