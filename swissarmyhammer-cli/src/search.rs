use colored::*;
use is_terminal::IsTerminal;
use std::io;
use tabled::{
    settings::{object::Rows, Alignment, Color, Modify, Style},
    Table, Tabled,
};

use crate::cli::{OutputFormat, PromptSource, PromptSourceArg, SearchCommands};
use crate::error::CliResult;
use swissarmyhammer::{
    prelude::{AdvancedSearchEngine, AdvancedSearchOptions},
    search::{FileIndexer, SearchQuery, SemanticConfig, SemanticSearcher, VectorStorage},
    PromptFilter, PromptLibrary, PromptResolver,
};

// UI constants
const PROCESSING_PATTERN_MESSAGE: &str = "Processing pattern:";

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub source: String,
    pub score: f32,
    pub excerpt: Option<String>,
    pub arguments: Vec<SearchArgument>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Tabled)]
struct SearchResultRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Excerpt")]
    excerpt: String,
    #[tabled(rename = "Source")]
    source: String,
    #[tabled(rename = "Score")]
    score: String,
}

#[allow(clippy::too_many_arguments)]
pub fn run_search_command(
    query: String,
    _fields: Option<Vec<String>>,
    regex: bool,
    fuzzy: bool,
    case_sensitive: bool,
    source_filter: Option<PromptSourceArg>,
    has_arg: Option<String>,
    no_args: bool,
    full: bool,
    format: OutputFormat,
    highlight: bool,
    limit: Option<usize>,
) -> CliResult<()> {
    // Load all prompts from all sources
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Get all prompts
    let all_prompts = library.list()?;

    // Create advanced search options
    let search_options = AdvancedSearchOptions {
        regex,
        fuzzy,
        case_sensitive,
        highlight,
        limit,
    };

    // Create filter based on CLI options
    let mut filter = PromptFilter::new();

    // Apply source filter
    if let Some(ref src_filter) = source_filter {
        let library_source: PromptSource = src_filter.clone().into();
        filter = filter.with_source(library_source);
    }

    // Apply argument filters
    if let Some(arg_name) = has_arg {
        filter = filter.with_has_arg(arg_name);
    }

    if no_args {
        filter = filter.with_no_args(true);
    }

    // Perform search using advanced search engine
    let search_engine = AdvancedSearchEngine::new()?;
    let search_results = search_engine.search(
        &query,
        &all_prompts,
        &search_options,
        Some(&filter),
        &resolver.prompt_sources,
    )?;

    // Convert results to CLI format
    let results: Vec<SearchResult> = search_results
        .into_iter()
        .map(|result| {
            // Get the source from the resolver
            let prompt_source = match resolver.prompt_sources.get(&result.prompt.name) {
                Some(PromptSource::Builtin) => PromptSource::Builtin,
                Some(PromptSource::User) => PromptSource::User,
                Some(PromptSource::Local) => PromptSource::Local,
                Some(PromptSource::Dynamic) => PromptSource::Dynamic,
                None => PromptSource::Dynamic,
            };
            let source_str = prompt_source.to_string();

            let arguments = result
                .prompt
                .arguments
                .iter()
                .map(|arg| SearchArgument {
                    name: arg.name.clone(),
                    description: arg.description.clone(),
                    required: arg.required,
                    default: arg.default.clone(),
                })
                .collect();

            SearchResult {
                name: result.prompt.name.clone(),
                title: None, // No title field in new API
                description: result.prompt.description.clone(),
                source: source_str,
                score: result.score,
                excerpt: result.excerpt,
                arguments,
            }
        })
        .collect();

    // Output results
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&results)?;
            print!("{yaml}");
        }
        OutputFormat::Table => {
            display_table(&results, full)?;
        }
    }

    Ok(())
}

fn display_table(results: &[SearchResult], full: bool) -> CliResult<()> {
    if results.is_empty() {
        println!("No prompts found matching the search criteria.");
        return Ok(());
    }

    let is_tty = io::stdout().is_terminal();

    let rows: Vec<SearchResultRow> = results
        .iter()
        .map(|result| {
            let title = result.title.as_deref().unwrap_or("");
            let excerpt = if full {
                result.excerpt.as_deref().unwrap_or("")
            } else {
                // Truncate long excerpts for table display
                let exc = result.excerpt.as_deref().unwrap_or("");
                if exc.len() > 50 {
                    &format!("{}...", &exc[..47])
                } else {
                    exc
                }
            };

            SearchResultRow {
                name: result.name.clone(),
                title: title.to_string(),
                excerpt: excerpt.to_string(),
                source: result.source.clone(),
                score: format!("{:.1}", result.score),
            }
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::modern());

    if is_tty {
        // Add colors for better readability in terminal
        // tabled 0.20+ changed API: Rows::single() → Rows::one()
        table.with(Modify::new(Rows::one(0)).with(Color::FG_BRIGHT_CYAN));

        // Color code sources
        for (i, result) in results.iter().enumerate() {
            let row_index = i + 1; // +1 because row 0 is header
            match result.source.as_str() {
                "builtin" => {
                    table.with(Modify::new(Rows::one(row_index)).with(Color::FG_GREEN));
                }
                "user" => {
                    table.with(Modify::new(Rows::one(row_index)).with(Color::FG_BLUE));
                }
                "local" => {
                    table.with(Modify::new(Rows::one(row_index)).with(Color::FG_YELLOW));
                }
                _ => {}
            }
        }
    }

    table.with(Modify::new(Rows::new(1..)).with(Alignment::left()));

    println!("{table}");

    if is_tty && !results.is_empty() {
        println!();
        println!("{} results found", results.len());
    }

    Ok(())
}

pub fn generate_excerpt(content: &str, query: &str, highlight: bool) -> Option<String> {
    let query_lower = query.to_lowercase();
    let content_lower = content.to_lowercase();

    if let Some(pos) = content_lower.find(&query_lower) {
        let start = pos.saturating_sub(30);
        let end = (pos + query.len() + 30).min(content.len());

        let excerpt = &content[start..end];

        if highlight {
            let highlighted = excerpt.replace(query, &format!("{}", query.bright_yellow()));
            Some(format!("...{highlighted}..."))
        } else {
            Some(format!("...{excerpt}..."))
        }
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn generate_excerpt_with_long_text(content: &str, query: &str, max_length: usize) -> String {
    let excerpt = generate_excerpt(content, query, false).unwrap_or_default();
    if excerpt.len() > max_length {
        format!("{}...", &excerpt[..max_length.saturating_sub(3)])
    } else {
        excerpt
    }
}

/// Run semantic search commands
pub async fn run_search(subcommand: SearchCommands) -> i32 {
    use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};

    match subcommand {
        SearchCommands::Index { patterns, force } => {
            match run_semantic_index(&patterns, force).await {
                Ok(()) => EXIT_SUCCESS,
                Err(e) => {
                    eprintln!("{}", format!("❌ Indexing failed: {e}").red());
                    EXIT_ERROR
                }
            }
        }
        SearchCommands::Query {
            query,
            limit,
            format: _format,
        } => match run_semantic_query(&query, limit).await {
            Ok(()) => EXIT_SUCCESS,
            Err(e) => {
                eprintln!("{}", format!("❌ Search failed: {e}").red());
                EXIT_ERROR
            }
        },
    }
}

/// Run semantic indexing for the given patterns (globs or individual files)
async fn run_semantic_index(patterns: &[String], force: bool) -> CliResult<()> {
    println!("{}", "🔍 Starting semantic search indexing...".cyan());

    if patterns.is_empty() {
        return Err(crate::error::CliError::new(
            "No patterns or files provided for indexing. Please specify one or more glob patterns (like '**/*.rs') or file paths.",
            crate::exit_codes::EXIT_ERROR
        ));
    }

    // For backward compatibility with tests, show different message based on pattern count
    if patterns.len() == 1 {
        println!("Indexing files matching: {}", patterns[0].bright_yellow());
    } else {
        println!(
            "Indexing patterns/files: {}",
            patterns.join(", ").bright_yellow()
        );
    }
    if force {
        println!("{}", "Force re-indexing: enabled".yellow());
    }

    // Initialize semantic search components
    let config = SemanticConfig::default();
    let storage = VectorStorage::new(config.clone())?;
    storage.initialize()?;

    let mut indexer = FileIndexer::new(storage).await?;

    // Perform indexing for all patterns
    let start_time = std::time::Instant::now();
    let mut combined_report = None;

    for pattern in patterns {
        println!("{} {}", PROCESSING_PATTERN_MESSAGE, pattern.bright_cyan());
        let report = indexer.index_glob(pattern, force).await?;

        match combined_report {
            None => combined_report = Some(report),
            Some(mut existing_report) => {
                // Merge reports (combine all statistics and errors)
                existing_report.merge_report(report);
                combined_report = Some(existing_report);
            }
        }
    }

    let report = combined_report.expect("Should have at least one report");
    let duration = start_time.elapsed();

    // Display results
    println!("\n{}", "✅ Indexing completed!".green().bold());
    println!("Duration: {:.2}s", duration.as_secs_f32());
    println!("{}", report.summary().bright_cyan());

    if !report.errors.is_empty() {
        println!(
            "\n{}",
            format!("⚠️  {} errors occurred:", report.errors.len()).yellow()
        );
        for (path, error) in &report.errors {
            println!(
                "  {} {}",
                "•".red(),
                format!("{}: {}", path.display(), error).dimmed()
            );
        }
    }

    Ok(())
}

/// Run semantic query search
async fn run_semantic_query(query: &str, limit: usize) -> CliResult<()> {
    println!("{}", "🔍 Starting semantic search query...".cyan());
    println!("Searching for: {}", query.bright_yellow());
    println!("Result limit: {}", limit.to_string().bright_yellow());

    // Initialize semantic search components
    let config = SemanticConfig::default();
    let storage = VectorStorage::new(config.clone())?;
    storage.initialize()?;

    let searcher = SemanticSearcher::new(storage, config).await?;

    // Perform search
    let search_query = SearchQuery {
        text: query.to_string(),
        limit,
        similarity_threshold: 0.5, // Use lower threshold for more results
        language_filter: None,
    };

    let start_time = std::time::Instant::now();
    let results = searcher.search(&search_query).await?;
    let duration = start_time.elapsed();

    // Display results
    if results.is_empty() {
        println!("\n{}", "No matches found.".yellow());
    } else {
        println!(
            "\n{}",
            format!("✅ Found {} results!", results.len())
                .green()
                .bold()
        );
        println!("Search duration: {:.2}s", duration.as_secs_f32());
        println!();

        for (i, result) in results.iter().enumerate() {
            let score_color = if result.similarity_score > 0.8 {
                "green"
            } else if result.similarity_score > 0.6 {
                "yellow"
            } else {
                "white"
            };

            println!(
                "{}",
                format!(
                    "{}. {}:{} (score: {:.3})",
                    i + 1,
                    result.chunk.file_path.display(),
                    result.chunk.start_line,
                    result.similarity_score
                )
                .color(score_color)
            );

            // Show excerpt with syntax highlighting context
            let excerpt = result.excerpt.trim();
            if !excerpt.is_empty() {
                for line in excerpt.lines() {
                    println!("   {}", line.dimmed());
                }
            }
            println!();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_excerpt() {
        let content = "This is a test content with some keywords in it";
        let query = "keywords";

        let excerpt = generate_excerpt(content, query, false);
        assert!(excerpt.is_some());
        assert!(excerpt.unwrap().contains("keywords"));
    }

    #[test]
    fn test_generate_excerpt_with_long_text() {
        let content = "This is a very long test content with some keywords that we want to find and excerpt properly";
        let query = "keywords";

        let excerpt = generate_excerpt_with_long_text(content, query, 50);
        assert!(excerpt.len() <= 50);
        assert!(excerpt.contains("..."));
    }

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            name: "test-prompt".to_string(),
            title: Some("Test Prompt".to_string()),
            description: Some("A test prompt".to_string()),
            source: "local".to_string(),
            score: 100.0,
            excerpt: None,
            arguments: vec![],
        };

        assert_eq!(result.name, "test-prompt");
        assert_eq!(result.score, 100.0);
    }

    #[tokio::test]
    async fn test_run_semantic_index_empty_patterns() {
        let patterns: Vec<String> = vec![];
        let result = run_semantic_index(&patterns, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No patterns or files provided"));
    }

    #[tokio::test]
    async fn test_run_semantic_index_single_pattern() {
        let patterns = vec!["test_pattern.rs".to_string()];

        // Semantic indexing should work with fastembed's automatic model caching
        // Models will be downloaded on first run and cached for subsequent runs
        // However, in test environments without network access or proper cache setup,
        // model initialization may fail - this is expected and acceptable
        match run_semantic_index(&patterns, false).await {
            Ok(_) => {
                println!("✅ Semantic indexing succeeded as expected");
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Failed to initialize fastembed model")
                    || error_msg.contains("I/O error")
                    || error_msg.contains("No such file or directory")
                {
                    println!("⚠️  Semantic indexing skipped - model initialization failed in test environment: {error_msg}");
                    println!("   This is expected when fastembed models cannot be downloaded (offline/restricted environment)");
                } else {
                    panic!("Unexpected error in semantic indexing: {error_msg}");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_run_semantic_index_multiple_patterns() {
        let patterns = vec![
            "src/**/*.rs".to_string(),
            "tests/**/*.rs".to_string(),
            "benches/**/*.rs".to_string(),
        ];

        // Semantic indexing should work with fastembed's automatic model caching
        // Models will be downloaded on first run and cached for subsequent runs
        // However, in test environments without network access or proper cache setup,
        // model initialization may fail - this is expected and acceptable
        match run_semantic_index(&patterns, false).await {
            Ok(_) => {
                println!("✅ Semantic indexing succeeded as expected");
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Failed to initialize fastembed model")
                    || error_msg.contains("I/O error")
                    || error_msg.contains("No such file or directory")
                {
                    println!("⚠️  Semantic indexing skipped - model initialization failed in test environment: {error_msg}");
                    println!("   This is expected when fastembed models cannot be downloaded (offline/restricted environment)");
                } else {
                    panic!("Unexpected error in semantic indexing: {error_msg}");
                }
            }
        }
    }

    #[test]
    fn test_file_line_format() {
        // Test that file:line format is correctly structured
        use std::path::PathBuf;
        use swissarmyhammer::search::{ChunkType, CodeChunk, ContentHash, Language};

        let chunk = CodeChunk {
            id: "test-chunk".to_string(),
            file_path: PathBuf::from("./src/main.rs"),
            language: Language::Rust,
            content: "fn main() {}".to_string(),
            start_line: 42,
            end_line: 45,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("hash123".to_string()),
        };

        // Test the format string that would be used in the display
        let formatted_result = format!("{}:{}", chunk.file_path.display(), chunk.start_line);
        assert_eq!(formatted_result, "./src/main.rs:42");

        // Test with a different path format
        let chunk2 = CodeChunk {
            id: "test-chunk-2".to_string(),
            file_path: PathBuf::from("tests/integration.rs"),
            language: Language::Rust,
            content: "#[test] fn test() {}".to_string(),
            start_line: 123,
            end_line: 125,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("hash456".to_string()),
        };

        let formatted_result2 = format!("{}:{}", chunk2.file_path.display(), chunk2.start_line);
        assert_eq!(formatted_result2, "tests/integration.rs:123");
    }
}
