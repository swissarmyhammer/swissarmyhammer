//! Test utilities for SwissArmyHammer Search tests
//!
//! This module provides shared testing infrastructure for search-specific tests.

/// Re-export IsolatedTestHome from the common swissarmyhammer crate
pub use swissarmyhammer_common::test_utils::IsolatedTestHome;

/// Re-export acquire_semantic_db_lock from the common swissarmyhammer crate
pub use swissarmyhammer_common::test_utils::acquire_semantic_db_lock;
