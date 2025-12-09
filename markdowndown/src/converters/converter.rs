//! Base converter trait and registry for URL-to-markdown conversion.
//!
//! This module defines the core `Converter` trait that all converter implementations
//! must implement, and the `ConverterRegistry` that manages converter routing based
//! on URL type detection.

use crate::types::{Markdown, MarkdownError, UrlType};
use async_trait::async_trait;

/// Trait for converting URLs to markdown.
///
/// All converter implementations must implement this trait to participate
/// in the unified conversion system.
#[async_trait]
pub trait Converter: Send + Sync {
    /// Converts content from a URL to markdown.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch and convert
    ///
    /// # Returns
    ///
    /// Returns the converted markdown content or an error.
    async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError>;

    /// Returns the human-readable name of this converter.
    fn name(&self) -> &'static str;
}

/// Registry for managing converters based on URL types.
///
/// The registry maps URL types to specific converter implementations,
/// allowing the main API to route URLs to appropriate handlers.
pub struct ConverterRegistry {
    converters: std::collections::HashMap<UrlType, Box<dyn Converter>>,
}

impl ConverterRegistry {
    /// Creates an empty converter registry with no converters.
    pub fn empty() -> Self {
        Self {
            converters: std::collections::HashMap::new(),
        }
    }

    /// Creates a new converter registry with default converters.
    pub fn new() -> Self {
        let mut registry = Self {
            converters: std::collections::HashMap::new(),
        };

        // Register default converters
        registry.register(UrlType::Html, Box::new(super::HtmlConverter::new()));
        registry.register(
            UrlType::GoogleDocs,
            Box::new(super::GoogleDocsConverter::new()),
        );
        registry.register(
            UrlType::GitHubIssue,
            Box::new(super::GitHubConverter::from_env()),
        );
        registry.register(
            UrlType::LocalFile,
            Box::new(super::local::LocalFileConverter::new()),
        );

        registry
    }

    /// Creates a new converter registry with configured converters.
    pub fn with_config(
        http_client: crate::client::HttpClient,
        html_config: super::config::HtmlConverterConfig,
        output_config: &crate::config::OutputConfig,
    ) -> Self {
        let mut registry = Self {
            converters: std::collections::HashMap::new(),
        };

        // Register configured converters
        registry.register(
            UrlType::Html,
            Box::new(super::HtmlConverter::with_config(
                http_client.clone(),
                html_config,
                output_config.clone(),
            )),
        );
        registry.register(
            UrlType::GoogleDocs,
            Box::new(super::GoogleDocsConverter::new()), // GoogleDocs converter manages its own HttpClient
        );
        registry.register(
            UrlType::GitHubIssue,
            Box::new(super::GitHubConverter::from_env()),
        );
        registry.register(
            UrlType::LocalFile,
            Box::new(super::local::LocalFileConverter::new()),
        );

        registry
    }

    /// Registers a converter for a specific URL type.
    ///
    /// # Arguments
    ///
    /// * `url_type` - The URL type this converter handles
    /// * `converter` - The converter implementation
    pub fn register(&mut self, url_type: UrlType, converter: Box<dyn Converter>) {
        self.converters.insert(url_type, converter);
    }

    /// Gets a converter for the specified URL type.
    ///
    /// # Arguments
    ///
    /// * `url_type` - The URL type to get a converter for
    ///
    /// # Returns
    ///
    /// Returns a reference to the converter if available, or None if no converter
    /// is registered for the given URL type.
    pub fn get_converter(&self, url_type: &UrlType) -> Option<&dyn Converter> {
        self.converters.get(url_type).map(|c| c.as_ref())
    }

    /// Returns a list of all supported URL types.
    pub fn supported_types(&self) -> Vec<UrlType> {
        self.converters.keys().cloned().collect()
    }
}

impl Default for ConverterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
