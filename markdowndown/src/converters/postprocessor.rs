//! Markdown postprocessing utilities for cleaning up formatting and whitespace.
//! This module handles normalization, link cleanup, and heading hierarchy fixes.

use super::config::HtmlConverterConfig;

/// Maximum heading level in markdown (H1 through H6)
const MAX_HEADING_LEVEL: usize = 6;

/// Length of empty text link prefix "[](
const EMPTY_TEXT_LINK_PREFIX_LEN: usize = 3;

/// Length of inline link delimiter "]("
const INLINE_LINK_DELIMITER_LEN: usize = 2;

/// Length of reference link delimiter "]: "
const REFERENCE_LINK_DELIMITER_LEN: usize = 3;

/// Minimum characters in a reference name
const MIN_REFERENCE_LENGTH: usize = 1;

/// Markdown postprocessor that cleans up formatting and whitespace.
pub struct MarkdownPostprocessor<'a> {
    config: &'a HtmlConverterConfig,
}

impl<'a> MarkdownPostprocessor<'a> {
    /// Creates a new markdown postprocessor with the given configuration.
    pub fn new(config: &'a HtmlConverterConfig) -> Self {
        Self { config }
    }

    /// Postprocesses markdown by cleaning up formatting and whitespace.
    pub fn postprocess(&self, markdown: &str) -> String {
        let mut cleaned = markdown.to_string();

        // Normalize whitespace
        cleaned = self.normalize_whitespace(&cleaned);

        // Remove excessive blank lines
        cleaned = self.remove_excessive_blank_lines(&cleaned);

        // Clean up malformed links
        cleaned = self.clean_malformed_links(&cleaned);

        // Convert reference links to inline links
        cleaned = self.convert_reference_links_to_inline(&cleaned);

        // Ensure proper heading hierarchy
        cleaned = self.fix_heading_hierarchy(&cleaned);

        cleaned.trim().to_string()
    }

    /// Normalizes whitespace in markdown content.
    fn normalize_whitespace(&self, markdown: &str) -> String {
        let mut result = String::new();
        let mut in_whitespace = false;

        for ch in markdown.chars() {
            match ch {
                ' ' | '\t' => {
                    if !in_whitespace {
                        result.push(' ');
                        in_whitespace = true;
                    }
                    // Skip additional whitespace
                }
                '\n' | '\r' => {
                    // Preserve line breaks but reset whitespace flag
                    result.push('\n');
                    in_whitespace = false;
                }
                _ => {
                    result.push(ch);
                    in_whitespace = false;
                }
            }
        }

        result
    }

    /// Removes excessive blank lines from markdown.
    fn remove_excessive_blank_lines(&self, markdown: &str) -> String {
        let lines: Vec<&str> = markdown.split('\n').collect();
        let mut result = Vec::new();
        let mut consecutive_blanks = 0;

        for line in lines {
            if line.trim().is_empty() {
                consecutive_blanks += 1;
                // Only allow max_blank_lines consecutive blank lines
                if consecutive_blanks <= self.config.max_blank_lines {
                    result.push(line);
                }
                // Skip additional blank lines beyond max
            } else {
                consecutive_blanks = 0;
                result.push(line);
            }
        }

        result.join("\n")
    }

    /// Cleans up malformed links in markdown.
    fn clean_malformed_links(&self, markdown: &str) -> String {
        let mut cleaned = markdown.to_string();
        cleaned = self.remove_empty_text_links(cleaned);
        cleaned = self.remove_empty_url_links(cleaned);
        cleaned
    }

    /// Removes links with empty text but broken URLs: [](broken)
    fn remove_empty_text_links(&self, mut markdown: String) -> String {
        while let Some(start) = markdown.find("[](") {
            let Some(end) = markdown[start + EMPTY_TEXT_LINK_PREFIX_LEN..].find(')') else {
                break;
            };

            let url_part = &markdown
                [start + EMPTY_TEXT_LINK_PREFIX_LEN..start + EMPTY_TEXT_LINK_PREFIX_LEN + end];
            let is_valid_url = url_part.starts_with("http://") || url_part.starts_with("https://");

            if is_valid_url {
                break;
            }

            let full_end = start + EMPTY_TEXT_LINK_PREFIX_LEN + end + 1;
            Self::remove_with_trailing_space(&mut markdown, start, full_end);
        }
        markdown
    }

    /// Removes links with text but empty URLs: [text]()
    fn remove_empty_url_links(&self, mut markdown: String) -> String {
        while let Some(start) = markdown.find("](") {
            let Some(open_bracket) = markdown[..start].rfind('[') else {
                break;
            };

            let Some(end) = markdown[start + INLINE_LINK_DELIMITER_LEN..].find(')') else {
                break;
            };

            let url_part = &markdown
                [start + INLINE_LINK_DELIMITER_LEN..start + INLINE_LINK_DELIMITER_LEN + end];

            if url_part.trim().is_empty() {
                let full_end = start + INLINE_LINK_DELIMITER_LEN + end + 1;
                Self::remove_with_trailing_space(&mut markdown, open_bracket, full_end);
            } else {
                break;
            }
        }
        markdown
    }

    /// Helper to remove a range and trailing space if present
    fn remove_with_trailing_space(text: &mut String, start: usize, end: usize) {
        let mut remove_end = end;
        if text.chars().nth(end) == Some(' ') {
            remove_end += 1;
        }
        text.replace_range(start..remove_end, "");
    }

    /// Parses a reference definition line and returns the reference and URL if valid.
    /// Reference definitions are in the format: [reference]: url
    fn parse_reference_definition(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim();

        if !trimmed.starts_with('[') {
            return None;
        }

        let colon_pos = trimmed.find("]: ")?;
        if colon_pos <= MIN_REFERENCE_LENGTH {
            return None;
        }

        let reference = trimmed[1..colon_pos].to_string();
        let url = trimmed[colon_pos + REFERENCE_LINK_DELIMITER_LEN..].to_string();
        Some((reference, url))
    }

    /// Converts reference-style links to inline links.
    fn convert_reference_links_to_inline(&self, markdown: &str) -> String {
        use std::collections::HashMap;

        let mut reference_definitions = HashMap::new();
        let mut filtered_lines = Vec::new();

        // Process lines to collect reference definitions and filter them out
        for line in markdown.split('\n') {
            if let Some((reference, url)) = Self::parse_reference_definition(line) {
                reference_definitions.insert(reference, url);
            } else {
                filtered_lines.push(line);
            }
        }

        let mut result = filtered_lines.join("\n");

        // Convert reference-style links to inline links
        for (reference, url) in reference_definitions {
            let reference_pattern = format!("[{reference}]");
            let inline_replacement = format!("({url})");
            result = result.replace(&reference_pattern, &inline_replacement);
        }

        result
    }

    /// Fixes heading hierarchy to ensure no levels are skipped.
    fn fix_heading_hierarchy(&self, markdown: &str) -> String {
        let lines: Vec<&str> = markdown.split('\n').collect();
        let mut result = Vec::new();
        let mut current_level = 0;
        let mut reference_level = None;

        for line in lines {
            let trimmed = line.trim();
            if !trimmed.starts_with('#') {
                result.push(line.to_string());
                continue;
            }

            let hashes = trimmed.chars().take_while(|&c| c == '#').count();
            if hashes == 0 || hashes > MAX_HEADING_LEVEL {
                result.push(line.to_string());
                continue;
            }

            let heading_text = trimmed[hashes..].trim_start();
            let target_level = self.calculate_target_level(current_level, reference_level, hashes);

            if current_level == 0 {
                reference_level = Some(hashes);
            }
            current_level = target_level;

            let corrected_heading = format!("{} {}", "#".repeat(target_level), heading_text);
            result.push(corrected_heading);
        }

        result.join("\n")
    }

    /// Calculates the target heading level based on current state and input
    fn calculate_target_level(
        &self,
        current_level: usize,
        reference_level: Option<usize>,
        hashes: usize,
    ) -> usize {
        match (current_level, reference_level) {
            (0, _) => 1,
            (_, None) => 1,
            (_, Some(ref_level)) if hashes <= ref_level => 1,
            (curr, Some(_)) if hashes > curr => curr + 1,
            (_, Some(ref_level)) => hashes - ref_level + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_postprocessor() -> MarkdownPostprocessor<'static> {
        let config = Box::leak(Box::new(HtmlConverterConfig::default()));
        MarkdownPostprocessor::new(config)
    }

    #[test]
    fn test_normalize_whitespace() {
        let postprocessor = setup_postprocessor();

        let input = "This  has   multiple\t\tspaces\nAnd\ttabs";
        let result = postprocessor.normalize_whitespace(input);
        let expected = "This has multiple spaces\nAnd tabs";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_remove_excessive_blank_lines() {
        let postprocessor = setup_postprocessor();

        let input = "Line 1\n\n\n\nLine 2\n\nLine 3";
        let result = postprocessor.remove_excessive_blank_lines(input);
        // With default max_blank_lines of 2, should keep only 2 consecutive blank lines
        let expected = "Line 1\n\n\nLine 2\n\nLine 3";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_clean_malformed_links() {
        let postprocessor = setup_postprocessor();

        let input = "Text [](broken) more text [good text]() end";
        let result = postprocessor.clean_malformed_links(input);
        let expected = "Text more text end";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_fix_heading_hierarchy() {
        let postprocessor = setup_postprocessor();

        let input = "### First heading\n##### Skipped level\n## Another";
        let result = postprocessor.fix_heading_hierarchy(input);
        let expected = "# First heading\n## Skipped level\n# Another";
        assert_eq!(result, expected);
    }
}
