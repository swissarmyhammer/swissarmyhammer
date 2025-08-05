//! Language-specific symbol extractors for outline generation
//!
//! This module provides Tree-sitter based extractors for different programming
//! languages, implementing the SymbolExtractor trait to generate structured
//! code outlines.

pub mod javascript;
pub mod python;
pub mod rust;
pub mod typescript;

pub use javascript::JavaScriptExtractor;
pub use python::PythonExtractor;
pub use rust::RustExtractor;
pub use typescript::TypeScriptExtractor;
