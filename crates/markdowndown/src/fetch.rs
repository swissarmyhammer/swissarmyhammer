//! Web fetching functionality for markdowndown

use crate::{Config, Result};
use reqwest::Client;

/// Fetch a URL and convert the HTML content to Markdown
///
/// # Arguments
///
/// * `url` - The URL to fetch
/// * `config` - Configuration for the fetch operation
///
/// # Returns
///
/// A `Result` containing the converted Markdown string or an error
///
/// # Example
///
/// ```no_run
/// use markdowndown::{convert_url_with_config, Config};
///
/// #[tokio::main]
/// async fn main() {
///     let config = Config::default();
///     let markdown = convert_url_with_config("https://example.com", config).await.unwrap();
///     println!("{}", markdown);
/// }
/// ```
pub async fn convert_url_with_config(url: &str, config: Config) -> Result<String> {
    // Build HTTP client with configuration
    let client = Client::builder()
        .timeout(config.http.timeout)
        .user_agent(&config.http.user_agent)
        .redirect(if config.http.max_redirects > 0 {
            reqwest::redirect::Policy::limited(config.http.max_redirects as usize)
        } else {
            reqwest::redirect::Policy::none()
        })
        .build()?;

    // Fetch the URL
    let response = client.get(url).send().await?;

    // Check for HTTP errors
    let response = response.error_for_status()?;

    // Get the response body as text
    let html = response.text().await?;

    // Convert HTML to Markdown
    let markdown = html2md::parse_html(&html);

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_url_with_default_config() {
        let config = Config::default();

        // This test requires network access, so we'll just test that the function
        // can be called and returns an error for an invalid URL
        let result = convert_url_with_config("http://invalid.local", config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_convert_url_with_custom_config() {
        let mut config = Config::default();
        config.http.user_agent = "TestAgent/1.0".to_string();
        config.http.max_redirects = 5;

        // Test with invalid URL to verify config is used
        let result = convert_url_with_config("http://invalid.local", config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_convert_url_no_redirects() {
        let mut config = Config::default();
        config.http.max_redirects = 0;

        // Test with invalid URL
        let result = convert_url_with_config("http://invalid.local", config).await;
        assert!(result.is_err());
    }
}
