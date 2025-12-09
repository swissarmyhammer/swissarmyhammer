//! Compaction test modules
//!
//! This module organizes all compaction-related tests into focused areas:
//! - token_usage_tests: Tests for token counting and usage calculation
//! - prompt_tests: Tests for CompactionPrompt validation and rendering  
//! - session_compaction_tests: Tests for core session compaction functionality
//! - manager_integration_tests: Tests for SessionManager compaction operations
//! - agent_integration_tests: Tests for Agent-level compaction integration
//! - performance_tests: Performance and stress tests for compaction

#[cfg(test)]
pub mod agent_integration_tests;
#[cfg(test)]
pub mod manager_integration_tests;
#[cfg(test)]
pub mod performance_tests;
#[cfg(test)]
pub mod prompt_tests;
#[cfg(test)]
pub mod session_compaction_tests;
#[cfg(test)]
pub mod token_usage_tests;
