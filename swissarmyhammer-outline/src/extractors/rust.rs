//! Rust-specific symbol extractor (placeholder)

/// Rust symbol extractor
pub struct RustExtractor;

impl RustExtractor {
    /// Create a new Rust extractor
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustExtractor {
    fn default() -> Self {
        Self::new()
    }
}
