use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("embedding error: {0}")]
    Embedding(#[from] model_embedding::EmbeddingError),
}

pub type Result<T> = std::result::Result<T, SearchError>;
