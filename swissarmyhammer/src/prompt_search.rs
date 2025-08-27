//! Search functionality for prompts

use crate::common::mcp_errors::McpResultExt;
use crate::{Prompt, Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    doc,
    query::QueryParser,
    schema::{Field, Schema, Value, STORED, TEXT},
    Index, IndexWriter,
};

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The prompt
    pub prompt: Prompt,
    /// Relevance score (higher is better)
    pub score: f32,
}

/// Search engine for prompts
pub struct SearchEngine {
    index: Index,
    writer: IndexWriter,
    name_field: Field,
    description_field: Field,
    category_field: Field,
    tags_field: Field,
    template_field: Field,
    fuzzy_matcher: SkimMatcherV2,
}

impl SearchEngine {
    /// Create a new search engine with in-memory index
    pub fn new() -> Result<Self> {
        let mut schema_builder = Schema::builder();

        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let description_field = schema_builder.add_text_field("description", TEXT | STORED);
        let category_field = schema_builder.add_text_field("category", TEXT | STORED);
        let tags_field = schema_builder.add_text_field("tags", TEXT | STORED);
        let template_field = schema_builder.add_text_field("template", TEXT);

        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);

        let writer = index.writer(50_000_000).with_tantivy_context()?;

        Ok(Self {
            index,
            writer,
            name_field,
            description_field,
            category_field,
            tags_field,
            template_field,
            fuzzy_matcher: SkimMatcherV2::default(),
        })
    }

    /// Create a new search engine with persistent index
    pub fn with_directory(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;

        let mut schema_builder = Schema::builder();

        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let description_field = schema_builder.add_text_field("description", TEXT | STORED);
        let category_field = schema_builder.add_text_field("category", TEXT | STORED);
        let tags_field = schema_builder.add_text_field("tags", TEXT | STORED);
        let template_field = schema_builder.add_text_field("template", TEXT);

        let schema = schema_builder.build();

        let directory = MmapDirectory::open(path).with_tantivy_context()?;

        let index = Index::open_or_create(directory, schema).with_tantivy_context()?;

        let writer = index.writer(50_000_000).with_tantivy_context()?;

        Ok(Self {
            index,
            writer,
            name_field,
            description_field,
            category_field,
            tags_field,
            template_field,
            fuzzy_matcher: SkimMatcherV2::default(),
        })
    }

    /// Index a prompt
    pub fn index_prompt(&mut self, prompt: &Prompt) -> Result<()> {
        let mut document = doc!();

        document.add_text(self.name_field, &prompt.name);

        if let Some(description) = &prompt.description {
            document.add_text(self.description_field, description);
        }

        if let Some(category) = &prompt.category {
            document.add_text(self.category_field, category);
        }

        if !prompt.tags.is_empty() {
            document.add_text(self.tags_field, prompt.tags.join(" "));
        }

        document.add_text(self.template_field, &prompt.template);

        self.writer.add_document(document).with_tantivy_context()?;

        Ok(())
    }

    /// Index multiple prompts
    pub fn index_prompts(&mut self, prompts: &[Prompt]) -> Result<()> {
        for prompt in prompts {
            self.index_prompt(prompt)?;
        }

        self.commit()?;
        Ok(())
    }

    /// Commit changes to the index
    pub fn commit(&mut self) -> Result<()> {
        self.writer.commit().with_tantivy_context()?;
        Ok(())
    }

    /// Search for prompts using full-text search
    pub fn search(&self, query: &str, prompts: &[Prompt]) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader().with_tantivy_context()?;

        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.name_field,
                self.description_field,
                self.category_field,
                self.tags_field,
                self.template_field,
            ],
        );

        let query = query_parser.parse_query(query).with_tantivy_context()?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(100))
            .with_tantivy_context()?;

        let mut results = Vec::new();

        for (score, doc_address) in top_docs {
            // tantivy 0.24+ requires explicit type annotation for doc() method
            // Previous versions: searcher.doc(doc_address)
            // Current version: searcher.doc::<tantivy::TantivyDocument>(doc_address)
            let doc = searcher
                .doc::<tantivy::TantivyDocument>(doc_address)
                .with_tantivy_context()?;

            if let Some(name_value) = doc.get_first(self.name_field) {
                if let Some(name) = name_value.as_str() {
                    // Find the corresponding prompt
                    if let Some(prompt) = prompts.iter().find(|p| p.name == name) {
                        results.push(SearchResult {
                            prompt: prompt.clone(),
                            score,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Search using fuzzy matching
    pub fn fuzzy_search(&self, query: &str, prompts: &[Prompt]) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for prompt in prompts {
            let mut best_score = 0;

            // Score against name
            if let Some(score) = self.fuzzy_matcher.fuzzy_match(&prompt.name, query) {
                best_score = best_score.max(score);
            }

            // Score against description
            if let Some(description) = &prompt.description {
                if let Some(score) = self.fuzzy_matcher.fuzzy_match(description, query) {
                    best_score = best_score.max(score / 2); // Weight description less
                }
            }

            // Score against category
            if let Some(category) = &prompt.category {
                if let Some(score) = self.fuzzy_matcher.fuzzy_match(category, query) {
                    best_score = best_score.max(score / 2);
                }
            }

            // Score against tags
            for tag in &prompt.tags {
                if let Some(score) = self.fuzzy_matcher.fuzzy_match(tag, query) {
                    best_score = best_score.max(score / 2);
                }
            }

            if best_score > 0 {
                results.push(SearchResult {
                    prompt: prompt.clone(),
                    score: best_score as f32,
                });
            }
        }

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        results
    }

    /// Combined search using both full-text and fuzzy matching
    pub fn hybrid_search(&self, query: &str, prompts: &[Prompt]) -> Result<Vec<SearchResult>> {
        let mut results = std::collections::HashMap::new();

        // Get full-text search results
        let text_results = self.search(query, prompts)?;
        for result in text_results {
            results.insert(result.prompt.name.clone(), result);
        }

        // Get fuzzy search results
        let fuzzy_results = self.fuzzy_search(query, prompts);
        for result in fuzzy_results {
            results
                .entry(result.prompt.name.clone())
                .and_modify(|e| e.score = e.score.max(result.score))
                .or_insert(result);
        }

        let mut final_results: Vec<SearchResult> = results.into_values().collect();
        final_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        Ok(final_results)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create search engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{create_temp_dir, create_test_prompts};

    #[test]
    fn test_search_engine_creation() {
        let engine = SearchEngine::new().unwrap();
        assert!(engine.index.schema().fields().count() > 0);
        assert_eq!(engine.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_search_engine_with_directory() {
        let temp_dir = create_temp_dir();
        let engine = SearchEngine::with_directory(temp_dir.path()).unwrap();
        assert!(engine.index.schema().fields().count() > 0);
        assert_eq!(engine.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_search_engine_with_directory_nonexistent_path() {
        let temp_dir = create_temp_dir();
        let nonexistent_path = temp_dir.path().join("nonexistent");
        let engine = SearchEngine::with_directory(&nonexistent_path).unwrap();
        assert!(nonexistent_path.exists());
        assert_eq!(engine.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_default_search_engine() {
        let engine = SearchEngine::default();
        assert_eq!(engine.index.schema().fields().count(), 5);
    }

    #[test]
    fn test_index_single_prompt() {
        let mut engine = SearchEngine::new().unwrap();
        let prompt = Prompt::new("test", "Test template").with_description("Test description");

        engine.index_prompt(&prompt).unwrap();
        engine.commit().unwrap();
    }

    #[test]
    fn test_index_prompt_with_all_fields() {
        let mut engine = SearchEngine::new().unwrap();
        let prompt = Prompt::new("full-test", "Full test template")
            .with_description("Full test description")
            .with_category("testing")
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

        engine.index_prompt(&prompt).unwrap();
        engine.commit().unwrap();
    }

    #[test]
    fn test_index_prompt_minimal_fields() {
        let mut engine = SearchEngine::new().unwrap();
        let prompt = Prompt::new("minimal", "Minimal template");

        engine.index_prompt(&prompt).unwrap();
        engine.commit().unwrap();
    }

    #[test]
    fn test_index_multiple_prompts() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();
    }

    #[test]
    fn test_commit() {
        let mut engine = SearchEngine::new().unwrap();
        let prompt = Prompt::new("test", "Test template");

        engine.index_prompt(&prompt).unwrap();
        engine.commit().unwrap();
    }

    #[test]
    fn test_fuzzy_search() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("cod", &prompts);
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "code-review");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_fuzzy_search_description_match() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("fixing", &prompts);
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "bug-fix");
    }

    #[test]
    fn test_fuzzy_search_category_match() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("debug", &prompts);
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "bug-fix");
    }

    #[test]
    fn test_fuzzy_search_tag_match() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("unit", &prompts);
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "test-generation");
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("nonexistent", &prompts);
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_empty_query() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        let results = engine.fuzzy_search("", &prompts);
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_empty_prompts() {
        let engine = SearchEngine::new().unwrap();
        let prompts = vec![];

        let results = engine.fuzzy_search("test", &prompts);
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_sorting() {
        let engine = SearchEngine::new().unwrap();
        let prompts = vec![
            Prompt::new("code", "Test template"),
            Prompt::new("code-review", "Test template"),
            Prompt::new("review-code", "Test template"),
        ];

        let results = engine.fuzzy_search("code", &prompts);
        assert!(results.len() >= 2);
        // Results should be sorted by score descending
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_full_text_search() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("code", &prompts).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.prompt.name == "code-review"));
    }

    #[test]
    fn test_full_text_search_description() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("reviewing", &prompts).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "code-review");
    }

    #[test]
    fn test_full_text_search_category() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("development", &prompts).unwrap();
        assert!(!results.is_empty());

        // Verify that development category prompts are found
        let development_prompts: Vec<_> = results
            .iter()
            .filter(|r| r.prompt.category.as_ref() == Some(&"development".to_string()))
            .collect();
        assert!(!development_prompts.is_empty());

        // Verify specific development prompts are included
        let prompt_names: Vec<_> = results.iter().map(|r| &r.prompt.name).collect();
        assert!(prompt_names.contains(&&"code-review".to_string()));
    }

    #[test]
    fn test_full_text_search_tags() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("fix", &prompts).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "bug-fix");
    }

    #[test]
    fn test_full_text_search_template() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("error", &prompts).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "bug-fix");
    }

    #[test]
    fn test_full_text_search_no_match() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("nonexistent", &prompts).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_full_text_search_empty_query() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("", &prompts).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_full_text_search_complex_query() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.search("code AND review", &prompts).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].prompt.name, "code-review");
    }

    #[test]
    fn test_hybrid_search() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.hybrid_search("cod", &prompts).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.prompt.name == "code-review"));
    }

    #[test]
    fn test_hybrid_search_combines_results() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        engine.index_prompts(&prompts).unwrap();

        let results = engine.hybrid_search("test", &prompts).unwrap();
        assert!(!results.is_empty());

        // Should include results from both fuzzy and full-text search
        let prompt_names: Vec<&str> = results.iter().map(|r| r.prompt.name.as_str()).collect();
        assert!(prompt_names.contains(&"test-generation"));
    }

    #[test]
    fn test_hybrid_search_score_combination() {
        let mut engine = SearchEngine::new().unwrap();
        let prompts =
            vec![Prompt::new("exact-match", "Template").with_description("Test description")];

        engine.index_prompts(&prompts).unwrap();

        let results = engine.hybrid_search("exact-match", &prompts).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_hybrid_search_empty_index() {
        let engine = SearchEngine::new().unwrap();
        let prompts = create_test_prompts();

        // Don't index any prompts
        let results = engine.hybrid_search("test", &prompts).unwrap();
        assert!(!results.is_empty()); // Should still have fuzzy results
    }

    #[test]
    fn test_search_result_creation() {
        let prompt = Prompt::new("test", "Test template");
        let result = SearchResult {
            prompt: prompt.clone(),
            score: 1.5,
        };

        assert_eq!(result.prompt.name, "test");
        assert_eq!(result.score, 1.5);
    }

    #[test]
    fn test_search_result_clone() {
        let prompt = Prompt::new("test", "Test template");
        let result = SearchResult {
            prompt: prompt.clone(),
            score: 1.5,
        };

        let cloned = result.clone();
        assert_eq!(cloned.prompt.name, result.prompt.name);
        assert_eq!(cloned.score, result.score);
    }

    #[test]
    fn test_score_weighting_in_fuzzy_search() {
        let engine = SearchEngine::new().unwrap();
        let prompts = vec![
            Prompt::new("test", "Template").with_description("This contains test keyword"),
            Prompt::new("other", "Template").with_category("test category"),
        ];

        let results = engine.fuzzy_search("test", &prompts);
        assert_eq!(results.len(), 2);

        // Name matches should score higher than description/category matches
        let test_prompt_result = results.iter().find(|r| r.prompt.name == "test").unwrap();
        let other_prompt_result = results.iter().find(|r| r.prompt.name == "other").unwrap();
        assert!(test_prompt_result.score > other_prompt_result.score);
    }

    #[test]
    fn test_multiple_tag_scoring() {
        let engine = SearchEngine::new().unwrap();
        let prompts = vec![Prompt::new("multi-tag", "Template")
            .with_tags(vec!["test".to_string(), "other".to_string()])];

        let results = engine.fuzzy_search("test", &prompts);
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_prompt_not_found_in_search_results() {
        let mut engine = SearchEngine::new().unwrap();
        let indexed_prompts = vec![Prompt::new("indexed", "Template")];
        let search_prompts = vec![Prompt::new("different", "Template")];

        engine.index_prompts(&indexed_prompts).unwrap();

        // Search with prompts that don't match indexed ones
        let results = engine.search("indexed", &search_prompts).unwrap();
        assert!(results.is_empty());
    }
}
