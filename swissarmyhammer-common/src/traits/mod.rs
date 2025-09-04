//! # Common Trait Definitions
//!
//! This module provides shared trait definitions used throughout the SwissArmyHammer
//! ecosystem. These traits establish common patterns and behaviors that can be
//! implemented across different modules and crates.
//!
//! ## Categories
//!
//! - **Storage Traits**: Common interfaces for storage backends and persistence
//! - **Validation Traits**: Shared validation patterns for input and data integrity
//! - **Serialization Traits**: Extended serialization behaviors beyond serde defaults  
//! - **Context Traits**: Common context and environment patterns
//!
//! ## Design Philosophy
//!
//! - Traits should be minimal and focused on a single responsibility
//! - All traits should have comprehensive documentation with usage examples
//! - Traits should be designed for composition and extensibility
//! - Default implementations should be provided where sensible
//!
//! ## Future Expansion
//!
//! This module will be populated with common traits extracted from the existing
//! codebase during the dependency refactoring process. Traits that are used across
//! multiple crates will be centralized here to avoid duplication and ensure
//! consistent behavior patterns.