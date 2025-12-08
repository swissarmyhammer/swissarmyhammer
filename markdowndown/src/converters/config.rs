//! Configuration options for HTML to markdown conversion.

/// Configuration options for HTML to markdown conversion.
#[derive(Debug, Clone)]
pub struct HtmlConverterConfig {
    /// Maximum line width for markdown output
    pub max_line_width: usize,
    /// Whether to remove script and style tags
    pub remove_scripts_styles: bool,
    /// Whether to remove navigation elements
    pub remove_navigation: bool,
    /// Whether to remove sidebar elements
    pub remove_sidebars: bool,
    /// Whether to remove advertisement elements
    pub remove_ads: bool,
    /// Maximum consecutive blank lines allowed
    pub max_blank_lines: usize,
}

impl Default for HtmlConverterConfig {
    fn default() -> Self {
        Self {
            max_line_width: 120,
            remove_scripts_styles: true,
            remove_navigation: true,
            remove_sidebars: true,
            remove_ads: true,
            max_blank_lines: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = HtmlConverterConfig::default();
        assert_eq!(config.max_line_width, 120);
        assert!(config.remove_scripts_styles);
        assert!(config.remove_navigation);
        assert!(config.remove_sidebars);
        assert!(config.remove_ads);
        assert_eq!(config.max_blank_lines, 2);
    }
}
