//! HTML preprocessing utilities for removing unwanted elements.
//! This module handles the removal of scripts, styles, navigation, sidebars, and advertisements.

use super::config::HtmlConverterConfig;
use regex::Regex;

/// HTML preprocessor that removes unwanted elements based on configuration.
pub struct HtmlPreprocessor<'a> {
    config: &'a HtmlConverterConfig,
}

impl<'a> HtmlPreprocessor<'a> {
    /// Creates a new HTML preprocessor with the given configuration.
    pub fn new(config: &'a HtmlConverterConfig) -> Self {
        Self { config }
    }

    /// Preprocesses HTML by removing unwanted elements.
    pub fn preprocess(&self, html: &str) -> String {
        let mut cleaned = html.to_string();

        if self.config.remove_scripts_styles {
            cleaned = self.remove_scripts_and_styles(&cleaned);
        }

        if self.config.remove_navigation {
            cleaned = self.remove_navigation_elements(&cleaned);
        }

        if self.config.remove_sidebars {
            cleaned = self.remove_sidebar_elements(&cleaned);
        }

        if self.config.remove_ads {
            cleaned = self.remove_advertisement_elements(&cleaned);
        }

        cleaned
    }

    /// Helper function to remove HTML elements by tag name using regex.
    fn remove_elements_by_tag(&self, html: &str, tag_name: &str) -> String {
        // Create regex pattern to match opening tag, content, and closing tag
        // Pattern handles attributes and self-closing tags
        let pattern = format!(
            r"(?i)<{tag_name}(?:\s[^>]*)?>.*?</{tag_name}>|<{tag_name}(?:\s[^>]*)?/>",
            tag_name = regex::escape(tag_name)
        );

        match Regex::new(&pattern) {
            Ok(re) => re.replace_all(html, "").to_string(),
            Err(_) => {
                // Fallback to simple string replacement if regex fails
                html.to_string()
            }
        }
    }

    /// Helper function to remove HTML elements by class name using regex.
    fn remove_elements_by_class(&self, html: &str, class_name: &str) -> String {
        // Simpler approach: match elements containing the class attribute
        // Pattern: <tag ...class="...classname..."...>content</tag>
        let pattern = format!(
            r#"(?is)<(\w+)[^>]*class\s*=\s*["'][^"']*\b{class_name}\b[^"']*["'][^>]*>.*?</\1>"#,
            class_name = regex::escape(class_name)
        );

        match Regex::new(&pattern) {
            Ok(re) => re.replace_all(html, "").to_string(),
            Err(_) => {
                // Fallback: use original string-based method if regex fails
                self.remove_elements_by_class_fallback(html, class_name)
            }
        }
    }

    /// Fallback method for class removal using string operations.
    fn remove_elements_by_class_fallback(&self, html: &str, class_name: &str) -> String {
        let pattern = format!("class=\"{class_name}\"");
        let mut result = html.to_string();

        while let Some(class_pos) = result.find(&pattern) {
            // Find the start of the tag containing this class
            let tag_start = result[..class_pos].rfind('<').unwrap_or(0);

            // Find the end of the opening tag
            if let Some(tag_end) = result[tag_start..].find('>') {
                let tag_end_pos = tag_start + tag_end + 1;

                // Extract tag name to find closing tag
                let tag_content = &result[tag_start..tag_end_pos];
                if let Some(space_pos) = tag_content[1..].find(' ') {
                    let tag_name = &tag_content[1..space_pos + 1];
                    let closing_tag = format!("</{tag_name}>");

                    if let Some(close_pos) = result[tag_end_pos..].find(&closing_tag) {
                        let close_end = tag_end_pos + close_pos + closing_tag.len();
                        result.replace_range(tag_start..close_end, "");
                    } else {
                        // Just remove the opening tag if no closing tag
                        result.replace_range(tag_start..tag_end_pos, "");
                    }
                } else {
                    // Remove just the opening tag
                    result.replace_range(tag_start..tag_end_pos, "");
                }
            } else {
                break;
            }
        }

        result
    }

    /// Removes script and style tags and their content.
    fn remove_scripts_and_styles(&self, html: &str) -> String {
        let mut result = self.remove_elements_by_tag(html, "script");
        result = self.remove_elements_by_tag(&result, "style");
        result
    }

    /// Removes navigation elements.
    fn remove_navigation_elements(&self, html: &str) -> String {
        let mut result = self.remove_elements_by_tag(html, "nav");

        // Remove elements with nav-related classes
        let nav_classes = ["nav", "navigation"];
        for class in nav_classes {
            result = self.remove_elements_by_class(&result, class);
        }

        result
    }

    /// Removes sidebar elements.
    fn remove_sidebar_elements(&self, html: &str) -> String {
        let mut result = self.remove_elements_by_tag(html, "aside");

        // Remove elements with sidebar-related classes
        let sidebar_classes = ["sidebar", "side-bar"];
        for class in sidebar_classes {
            result = self.remove_elements_by_class(&result, class);
        }

        result
    }

    /// Removes advertisement elements.
    fn remove_advertisement_elements(&self, html: &str) -> String {
        let mut result = html.to_string();

        // Remove elements with advertisement-related classes
        let ad_classes = ["ad", "ads", "advertisement"];
        for class in ad_classes {
            result = self.remove_elements_by_class(&result, class);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_scripts_and_styles() {
        let config = HtmlConverterConfig::default();
        let preprocessor = HtmlPreprocessor::new(&config);

        let html = r#"
            <html>
                <head><script>alert('test');</script></head>
                <body>
                    <p>Content</p>
                    <style>body { color: red; }</style>
                </body>
            </html>
        "#;

        let result = preprocessor.remove_scripts_and_styles(html);
        assert!(!result.contains("<script>"));
        assert!(!result.contains("<style>"));
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn test_remove_navigation_elements() {
        let config = HtmlConverterConfig::default();
        let preprocessor = HtmlPreprocessor::new(&config);

        let html = r#"<nav>Menu</nav><p>Content</p><div class="nav">Nav</div>"#;
        let result = preprocessor.remove_navigation_elements(html);

        assert!(!result.contains("<nav>"));
        assert!(!result.contains("class=\"nav\""));
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn test_remove_sidebar_elements() {
        let config = HtmlConverterConfig::default();
        let preprocessor = HtmlPreprocessor::new(&config);

        let html = r#"<aside>Sidebar</aside><p>Content</p><div class="sidebar">Side</div>"#;
        let result = preprocessor.remove_sidebar_elements(html);

        assert!(!result.contains("<aside>"));
        assert!(!result.contains("class=\"sidebar\""));
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn test_remove_advertisement_elements() {
        let config = HtmlConverterConfig::default();
        let preprocessor = HtmlPreprocessor::new(&config);

        let html =
            r#"<p>Content</p><div class="ad">Ad content</div><span class="ads">More ads</span>"#;
        let result = preprocessor.remove_advertisement_elements(html);

        assert!(!result.contains("class=\"ad\""));
        assert!(!result.contains("class=\"ads\""));
        assert!(result.contains("<p>Content</p>"));
    }
}
