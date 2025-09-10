//! Test utilities for SwissArmyHammer Search tests
//!
//! This module provides shared testing infrastructure for search-specific tests.

use std::sync::Mutex;

/// Global mutex to serialize access to the semantic database during tests
static SEMANTIC_DB_LOCK: Mutex<()> = Mutex::new(());

/// Acquire a lock on the semantic database to prevent concurrent test access
pub fn acquire_semantic_db_lock() -> std::sync::MutexGuard<'static, ()> {
    SEMANTIC_DB_LOCK
        .lock()
        .expect("Failed to acquire semantic database lock")
}
