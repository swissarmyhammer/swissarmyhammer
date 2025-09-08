//! Language-specific symbol extractors for outline generation
//!
//! This module provides Tree-sitter based extractors for different programming
//! languages, implementing symbol extraction for code outlines.

// Note: These extractors are placeholders and need full implementation
// The functionality is currently integrated into the parser module

pub mod dart;
pub mod javascript;
pub mod python;
pub mod rust;
pub mod typescript;

pub use dart::DartExtractor;
pub use javascript::JavaScriptExtractor;
pub use python::PythonExtractor;
pub use rust::RustExtractor;
pub use typescript::TypeScriptExtractor;
