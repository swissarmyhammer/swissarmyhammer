//! Language-specific symbol extractors for outline generation
//!
//! This module provides Tree-sitter based extractors for different programming
//! languages, implementing symbol extraction for code outlines.

// Note: These extractors are placeholders and need full implementation
// The functionality is currently integrated into the parser module

pub mod rust;
pub mod python;
pub mod typescript;
pub mod javascript;
pub mod dart;

pub use rust::RustExtractor;
pub use python::PythonExtractor; 
pub use typescript::TypeScriptExtractor;
pub use javascript::JavaScriptExtractor;
pub use dart::DartExtractor;