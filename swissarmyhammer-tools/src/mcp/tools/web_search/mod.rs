//! Web search tools for MCP operations
//!
//! This module provides web search tools that enable LLMs to perform web searches using DuckDuckGo.
//! The tools provide privacy-respecting search capabilities with automatic result fetching and content processing.

pub mod chrome_detection;
pub mod content_fetcher;
pub mod duckduckgo_client;
pub mod search;
pub mod types;

// Registration is handled by the unified `web` tool module.
// This module is kept as an internal utility providing search pipeline components.
