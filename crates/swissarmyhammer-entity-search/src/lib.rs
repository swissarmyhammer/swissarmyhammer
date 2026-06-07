//! Hybrid fuzzy + embedding search over Entity objects.
//!
//! Combines fuzzy matching (via `SkimMatcherV2`) for short queries and
//! embedding-based semantic search (via `TextEmbedder` + cosine similarity)
//! for longer queries. Falls back between strategies automatically.

pub mod error;
pub mod fuzzy;
pub mod index;
pub mod rank;
pub mod result;
pub mod semantic;

pub use error::SearchError;
pub use index::EntitySearchIndex;
pub use rank::{top_k_by_cosine, Ranked, RankedTopK};
pub use result::{SearchResult, SearchStrategy};
