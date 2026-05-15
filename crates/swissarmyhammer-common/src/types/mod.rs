//! # Common Type Definitions
//!
//! This module provides common type definitions used throughout the SwissArmyHammer
//! ecosystem. It includes:
//!
//! - **Newtypes**: Wrapper types that provide domain safety and prevent mixing
//!   of different identifier types
//! - **Domain Types**: Core types that represent business concepts across modules
//! - **ID Types**: Strongly-typed identifier wrappers using ULID for uniqueness
//!
//! ## Design Principles
//!
//! - Use newtypes to prevent mixing different kinds of identifiers
//! - All types implement `serde::Serialize` and `serde::Deserialize` by default
//! - ID types use ULID for sortable, unique identifiers
//! - Types follow the SwissArmyHammer naming conventions
//!
//! ## Usage Notes
//!
//! Types that are used across multiple crates should be moved here to establish a
//! single source of truth. New types should follow the design principles and
//! naming conventions outlined above.
