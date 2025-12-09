//! Common utilities and types shared across parsers
//!
//! This module contains the core building blocks for the mermaid-parser crate.
//! These utilities provide the fundamental operations needed to parse, analyze, and transform Mermaid diagrams.
//!
//! ## Module Overview
//!
//! - [`ast`] - Abstract Syntax Tree definitions for all diagram types
//! - [`lexer`] - Lexical analysis components for tokenizing input
//! - [`metrics`] - Diagram complexity analysis and quality assessment
//! - [`parser_utils`] - Shared parsing utilities and helpers
//! - [`parsing`] - Comprehensive parsing utilities for common patterns
//! - [`pretty_print`] - Pretty-printing utilities for formatting output
//! - [`tokens`] - Token definitions and token stream handling
//! - [`validation`] - Diagram validation and semantic analysis
//! - [`visitor`] - AST visitor pattern for traversal and analysis
//!
//! ## Example
//!
//! ```rust
//! use mermaid_parser::common::visitor::{AstVisitor, NodeCounter};
//! use mermaid_parser::parse_diagram;
//!
//! let input = "flowchart TD\n    A --> B\n    B --> C";
//! let diagram = parse_diagram(input)?;
//!
//! // Use visitor pattern to analyze the diagram
//! let mut counter = NodeCounter::new();
//! diagram.accept(&mut counter);
//! println!("Nodes: {}, edges: {}", counter.nodes(), counter.edges());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod ast;
pub mod constants;
pub mod lexer;
pub mod metrics;
pub mod parser_utils;
pub mod parsing;
pub mod pretty_print;
pub mod tokens;
pub mod validation;
pub mod visitor;
