//! # Shared Constants
//!
//! This module defines constants that are shared across the SwissArmyHammer
//! ecosystem. These constants help ensure consistency and avoid magic numbers
//! throughout the codebase.
//!
//! ## Categories
//!
//! - **Application Constants**: Version information, application names, and identifiers
//! - **Configuration Constants**: Default values for configuration settings
//! - **Limits and Thresholds**: Size limits, timeout values, and other constraints
//! - **Format Constants**: File extensions, MIME types, and format identifiers
//!
//! ## Design Principles
//!
//! - All constants should have descriptive names and comprehensive documentation
//! - Magic numbers should be avoided in favor of named constants
//! - Constants should be grouped logically and consistently formatted
//! - Values should be reasonable defaults that work across different environments
//!
//! ## Future Expansion
//!
//! This module will be populated with constants extracted from the existing
//! codebase during the dependency refactoring process. Constants that are
//! duplicated across multiple crates will be centralized here to ensure
//! consistency and make updates easier to manage.