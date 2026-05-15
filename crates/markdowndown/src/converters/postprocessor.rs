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

    #[test]
    fn test_parse_reference_definition_valid() {
        let result =
            MarkdownPostprocessor::parse_reference_definition("[example]: https://example.com");
        assert!(result.is_some());
        let (reference, url) = result.unwrap();
        assert_eq!(reference, "example");
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_parse_reference_definition_not_starting_with_bracket() {
        let result = MarkdownPostprocessor::parse_reference_definition("not a reference");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_reference_definition_missing_colon_space() {
        let result = MarkdownPostprocessor::parse_reference_definition("[ref] no colon");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_reference_definition_too_short_reference() {
        // Reference name at or below MIN_REFERENCE_LENGTH (1 char) means colon_pos <= 1
        let result = MarkdownPostprocessor::parse_reference_definition("[]: https://example.com");
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_reference_links_to_inline() {
        let postprocessor = setup_postprocessor();

        let input = "Click [example] for more info.\n\n[example]: https://example.com";
        let result = postprocessor.convert_reference_links_to_inline(input);
        assert!(result.contains("(https://example.com)"));
        assert!(!result.contains("[example]: https://example.com"));
    }

    #[test]
    fn test_convert_reference_links_multiple_refs() {
        let postprocessor = setup_postprocessor();

        let input = "See [ref1] and [ref2].\n\n[ref1]: https://one.com\n[ref2]: https://two.com";
        let result = postprocessor.convert_reference_links_to_inline(input);
        assert!(result.contains("(https://one.com)"));
        assert!(result.contains("(https://two.com)"));
    }

    #[test]
    fn test_remove_empty_text_links_with_valid_url() {
        let postprocessor = setup_postprocessor();

        // Valid URL in empty-text link should NOT be removed
        let input = "Text [](https://example.com) more";
        let result = postprocessor.remove_empty_text_links(input.to_string());
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_empty_text_links_no_closing_paren() {
        let postprocessor = setup_postprocessor();

        let input = "Text [](broken link without closing paren";
        let result = postprocessor.remove_empty_text_links(input.to_string());
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_empty_url_links_no_open_bracket() {
        let postprocessor = setup_postprocessor();

        // "](" without preceding "[" should break out
        let input = "plain text ]() more";
        let result = postprocessor.remove_empty_url_links(input.to_string());
        // Should break out of the loop since rfind('[') fails
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_empty_url_links_no_closing_paren() {
        let postprocessor = setup_postprocessor();

        let input = "[text](unclosed";
        let result = postprocessor.remove_empty_url_links(input.to_string());
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_empty_url_links_non_empty_url() {
        let postprocessor = setup_postprocessor();

        // Non-empty URL should break out of the loop (not removed)
        let input = "[text](https://example.com)";
        let result = postprocessor.remove_empty_url_links(input.to_string());
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_with_trailing_space() {
        let mut text = "hello world ".to_string();
        // Remove "hello " (indices 0..6), and the char at index 6 is 'w' not space
        MarkdownPostprocessor::remove_with_trailing_space(&mut text, 0, 6);
        assert_eq!(text, "world ");

        // When there IS a trailing space after the range
        let mut text2 = "abc def".to_string();
        // Remove "abc" (indices 0..3), char at index 3 is ' '
        MarkdownPostprocessor::remove_with_trailing_space(&mut text2, 0, 3);
        assert_eq!(text2, "def");
    }

    #[test]
    fn test_calculate_target_level_first_heading() {
        let postprocessor = setup_postprocessor();
        // First heading (current_level=0) always maps to 1
        assert_eq!(postprocessor.calculate_target_level(0, None, 3), 1);
        assert_eq!(postprocessor.calculate_target_level(0, Some(2), 5), 1);
    }

    #[test]
    fn test_calculate_target_level_no_reference() {
        let postprocessor = setup_postprocessor();
        // current_level > 0 but no reference_level
        assert_eq!(postprocessor.calculate_target_level(1, None, 2), 1);
    }

    #[test]
    fn test_calculate_target_level_below_reference() {
        let postprocessor = setup_postprocessor();
        // hashes <= ref_level maps to 1
        assert_eq!(postprocessor.calculate_target_level(2, Some(3), 2), 1);
        assert_eq!(postprocessor.calculate_target_level(2, Some(3), 3), 1);
    }

    #[test]
    fn test_calculate_target_level_increment() {
        let postprocessor = setup_postprocessor();
        // hashes > current_level maps to current + 1
        assert_eq!(postprocessor.calculate_target_level(1, Some(1), 3), 2);
    }

    #[test]
    fn test_calculate_target_level_relative_to_ref() {
        let postprocessor = setup_postprocessor();
        // Default branch: hashes - ref_level + 1
        // This hits when hashes > ref_level AND hashes <= current_level
        assert_eq!(postprocessor.calculate_target_level(3, Some(1), 3), 3);
    }

    #[test]
    fn test_fix_heading_hierarchy_non_heading_lines() {
        let postprocessor = setup_postprocessor();
        let input = "Normal text\nAnother line\n\nParagraph break";
        let result = postprocessor.fix_heading_hierarchy(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_fix_heading_hierarchy_exceeds_max_level() {
        let postprocessor = setup_postprocessor();
        // 7 hashes exceeds MAX_HEADING_LEVEL (6), should be left unchanged
        let input = "# First\n####### Not a heading";
        let result = postprocessor.fix_heading_hierarchy(input);
        assert!(result.contains("####### Not a heading"));
    }

    #[test]
    fn test_postprocess_full_pipeline() {
        let postprocessor = setup_postprocessor();

        let input = "##  Title  with  spaces\n\n\n\n\nToo many blanks\n\n[](broken) text\n\n[ref]: https://example.com\nClick [ref] please.";
        let result = postprocessor.postprocess(input);

        // Whitespace normalized
        assert!(!result.contains("  "));
        // Excessive blanks removed
        assert!(!result.contains("\n\n\n\n"));
        // Malformed link removed
        assert!(!result.contains("[](broken)"));
        // Reference link converted
        assert!(result.contains("(https://example.com)"));
    }

    #[test]
    fn test_normalize_whitespace_preserves_newlines() {
        let postprocessor = setup_postprocessor();

        let input = "line1\n\nline3\n\n\nline6";
        let result = postprocessor.normalize_whitespace(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_normalize_whitespace_carriage_returns() {
        let postprocessor = setup_postprocessor();

        let input = "text\rwith\rreturns";
        let result = postprocessor.normalize_whitespace(input);
        assert_eq!(result, "text\nwith\nreturns");
    }

    #[test]
    fn test_clean_malformed_links_multiple_empty_text_links() {
        let postprocessor = setup_postprocessor();

        let input = "[](broken1) [](broken2) text";
        let result = postprocessor.clean_malformed_links(input);
        assert_eq!(result, "text");
    }

    #[test]
    fn test_clean_malformed_links_multiple_empty_url_links() {
        let postprocessor = setup_postprocessor();

        let input = "[text1]() [text2]() remaining";
        let result = postprocessor.clean_malformed_links(input);
        assert_eq!(result, "remaining");
    }
}
