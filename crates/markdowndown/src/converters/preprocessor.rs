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
            if let Some((start, end)) = self.find_and_remove_tag(&result, class_pos) {
                result.replace_range(start..end, "");
            } else {
                break;
            }
        }

        result
    }

    /// Finds the boundaries of a tag containing the class at the given position.
    /// Returns (tag_start, tag_end) or None if the tag cannot be found.
    fn find_and_remove_tag(&self, html: &str, class_pos: usize) -> Option<(usize, usize)> {
        let tag_start = html[..class_pos].rfind('<')?;
        let tag_end_offset = html[tag_start..].find('>')?;
        let tag_end_pos = tag_start + tag_end_offset + 1;

        let tag_name = self.extract_tag_name(&html[tag_start..tag_end_pos])?;
        let closing_tag = format!("</{tag_name}>");

        if let Some(close_offset) = html[tag_end_pos..].find(&closing_tag) {
            let close_end = tag_end_pos + close_offset + closing_tag.len();
            Some((tag_start, close_end))
        } else {
            Some((tag_start, tag_end_pos))
        }
    }

    /// Extracts the tag name from the opening tag content.
    /// Returns None if the tag name cannot be extracted.
    fn extract_tag_name(&self, tag_content: &str) -> Option<String> {
        let content = tag_content.strip_prefix('<')?.trim();
        let space_pos = content.find(' ').unwrap_or(content.len());
        let tag_name = &content[..space_pos];
        let tag_name = tag_name.trim_end_matches('>');

        if tag_name.is_empty() {
            None
        } else {
            Some(tag_name.to_string())
        }
    }

    /// Removes script and style tags and their content.
    fn remove_scripts_and_styles(&self, html: &str) -> String {
        let mut result = self.remove_elements_by_tag(html, "script");
        result = self.remove_elements_by_tag(&result, "style");
        result
    }

    /// Generic helper to remove elements by optional tag name and list of class names.
    fn remove_elements_by_tag_and_classes(
        &self,
        html: &str,
        tag: Option<&str>,
        classes: &[&str],
    ) -> String {
        let mut result = if let Some(tag_name) = tag {
            self.remove_elements_by_tag(html, tag_name)
        } else {
            html.to_string()
        };

        for class in classes {
            result = self.remove_elements_by_class(&result, class);
        }

        result
    }

    /// Removes navigation elements.
    fn remove_navigation_elements(&self, html: &str) -> String {
        self.remove_elements_by_tag_and_classes(html, Some("nav"), &["nav", "navigation"])
    }

    /// Removes sidebar elements.
    fn remove_sidebar_elements(&self, html: &str) -> String {
        self.remove_elements_by_tag_and_classes(html, Some("aside"), &["sidebar", "side-bar"])
    }

    /// Removes advertisement elements.
    fn remove_advertisement_elements(&self, html: &str) -> String {
        self.remove_elements_by_tag_and_classes(html, None, &["ad", "ads", "advertisement"])
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
