//! Language-specific symbol extractors for outline generation
//!
//! This module provides Tree-sitter based extractors for different programming
//! languages, implementing the SymbolExtractor trait to generate structured
//! code outlines.

pub mod rust;

pub use rust::RustExtractor;