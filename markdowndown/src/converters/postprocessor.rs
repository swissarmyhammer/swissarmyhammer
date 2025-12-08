//! Markdown postprocessing utilities for cleaning up formatting and whitespace.
//! This module handles normalization, link cleanup, and heading hierarchy fixes.

use super::config::HtmlConverterConfig;

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
        let result = markdown.to_string();

        // Use a simpler approach with string replacement for common malformed patterns
        let mut cleaned = result;

        // Remove empty links with empty text: [](broken)
        // Match links where text is empty and URL doesn't start with http
        while let Some(start) = cleaned.find("[](") {
            if let Some(end) = cleaned[start + 3..].find(')') {
                let url_part = &cleaned[start + 3..start + 3 + end];
                if !url_part.starts_with("http://") && !url_part.starts_with("https://") {
                    // Remove this malformed link and the space after if any
                    let full_end = start + 3 + end + 1;
                    let mut remove_end = full_end;
                    if cleaned.chars().nth(full_end) == Some(' ') {
                        remove_end += 1;
                    }
                    cleaned.replace_range(start..remove_end, "");
                } else {
                    // Valid empty link, keep it and move past this occurrence
                    break;
                }
            } else {
                break;
            }
        }

        // Remove links with text but empty URL: [text]()
        while let Some(start) = cleaned.find("](") {
            // Find the opening bracket for this link
            if let Some(open_bracket) = cleaned[..start].rfind('[') {
                let _text_part = &cleaned[open_bracket + 1..start];
                if let Some(end) = cleaned[start + 2..].find(')') {
                    let url_part = &cleaned[start + 2..start + 2 + end];
                    if url_part.trim().is_empty() {
                        // Remove this link with empty URL and space after if any
                        let full_end = start + 2 + end + 1;
                        let mut remove_end = full_end;
                        if cleaned.chars().nth(full_end) == Some(' ') {
                            remove_end += 1;
                        }
                        cleaned.replace_range(open_bracket..remove_end, "");
                    } else {
                        // This is a valid link, skip past it
                        let temp = cleaned[start + 2 + end + 1..].to_string();
                        cleaned = cleaned[..start + 2 + end + 1].to_string() + &temp;
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        cleaned
    }

    /// Converts reference-style links to inline links.
    fn convert_reference_links_to_inline(&self, markdown: &str) -> String {
        use std::collections::HashMap;

        let mut result = markdown.to_string();
        let mut reference_definitions = HashMap::new();
        let mut filtered_lines = Vec::new();

        // Process lines to collect reference definitions and filter them out
        for line in result.split('\n') {
            let trimmed = line.trim();

            // Check if this is a reference definition like [1]: https://example.com
            if let Some(colon_pos) = trimmed.find("]: ") {
                if trimmed.starts_with('[') {
                    let bracket_end = colon_pos;
                    if bracket_end > 1 {
                        let reference = &trimmed[1..bracket_end];
                        let url = &trimmed[colon_pos + 3..];
                        reference_definitions.insert(reference.to_string(), url.to_string());
                        // Skip adding this line to filtered_lines (removes the reference definition)
                        continue;
                    }
                }
            }

            // Keep all non-reference-definition lines
            filtered_lines.push(line);
        }

        result = filtered_lines.join("\n");

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
            if trimmed.starts_with('#') {
                // Count the number of # characters
                let hashes = trimmed.chars().take_while(|&c| c == '#').count();
                if hashes > 0 && hashes <= 6 {
                    // Extract the heading text (everything after the hashes and space)
                    let heading_text = trimmed[hashes..].trim_start();

                    // Determine the appropriate level
                    let target_level = if current_level == 0 {
                        // First heading - set reference and use H1
                        reference_level = Some(hashes);
                        1
                    } else if let Some(ref_level) = reference_level {
                        if hashes <= ref_level {
                            // Same or higher level than reference - reset to H1
                            1
                        } else if hashes > current_level {
                            // Going deeper - don't skip levels
                            current_level + 1
                        } else {
                            // Going up but still below reference - use requested level
                            hashes - ref_level + 1
                        }
                    } else {
                        // Fallback
                        1
                    };

                    current_level = target_level;

                    // Create the corrected heading
                    let corrected_heading =
                        format!("{} {}", "#".repeat(target_level), heading_text);
                    result.push(corrected_heading);
                } else {
                    result.push(line.to_string());
                }
            } else {
                result.push(line.to_string());
            }
        }

        result.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace() {
        let config = HtmlConverterConfig::default();
        let postprocessor = MarkdownPostprocessor::new(&config);

        let input = "This  has   multiple\t\tspaces\nAnd\ttabs";
        let result = postprocessor.normalize_whitespace(input);
        let expected = "This has multiple spaces\nAnd tabs";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_remove_excessive_blank_lines() {
        let config = HtmlConverterConfig::default();
        let postprocessor = MarkdownPostprocessor::new(&config);

        let input = "Line 1\n\n\n\nLine 2\n\nLine 3";
        let result = postprocessor.remove_excessive_blank_lines(input);
        // With default max_blank_lines of 2, should keep only 2 consecutive blank lines
        let expected = "Line 1\n\n\nLine 2\n\nLine 3";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_clean_malformed_links() {
        let config = HtmlConverterConfig::default();
        let postprocessor = MarkdownPostprocessor::new(&config);

        let input = "Text [](broken) more text [good text]() end";
        let result = postprocessor.clean_malformed_links(input);
        let expected = "Text more text end";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_fix_heading_hierarchy() {
        let config = HtmlConverterConfig::default();
        let postprocessor = MarkdownPostprocessor::new(&config);

        let input = "### First heading\n##### Skipped level\n## Another";
        let result = postprocessor.fix_heading_hierarchy(input);
        let expected = "# First heading\n## Skipped level\n# Another";
        assert_eq!(result, expected);
    }
}
