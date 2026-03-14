/// How a match was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStrategy {
    Fuzzy,
    Semantic,
}

/// A single search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub entity_id: String,
    pub score: f64,
    pub strategy: SearchStrategy,
    pub matched_field: Option<String>,
}
