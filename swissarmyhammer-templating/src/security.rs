//! Security validation for template content
//!
//! This module provides security validation for template content to prevent
//! potential security vulnerabilities and resource exhaustion attacks.

use crate::error::{Result, TemplatingError};

/// Maximum allowed template size in bytes for untrusted templates
pub const MAX_TEMPLATE_SIZE: usize = 100_000;

/// Maximum allowed recursion depth for template rendering
pub const MAX_TEMPLATE_RECURSION_DEPTH: usize = 10;

/// Maximum allowed template variables per template
pub const MAX_TEMPLATE_VARIABLES: usize = 1000;

/// Maximum allowed template render time in milliseconds
pub const MAX_TEMPLATE_RENDER_TIME_MS: u64 = 5000;

/// Validates template content for security risks
///
/// This function checks template content for potential security issues
/// including size limits, complexity, and dangerous patterns.
///
/// # Arguments
///
/// * `template_content` - The template content to validate
/// * `is_trusted` - Whether this template comes from a trusted source
///
/// # Returns
///
/// Ok if the template is safe to render, error if it poses security risks
pub fn validate_template_security(template_content: &str, is_trusted: bool) -> Result<()> {
    // For trusted templates (builtin, user-created), apply minimal validation
    if is_trusted {
        // Even trusted templates should have reasonable size limits
        if template_content.len() > MAX_TEMPLATE_SIZE * 10 {
            return Err(TemplatingError::Security(format!(
                "Template too large: {} bytes (max allowed for trusted: {})",
                template_content.len(),
                MAX_TEMPLATE_SIZE * 10
            )));
        }
        return Ok(());
    }

    // Strict validation for untrusted templates

    // Check template size
    if template_content.len() > MAX_TEMPLATE_SIZE {
        return Err(TemplatingError::Security(format!(
            "Template too large: {} bytes (max allowed: {MAX_TEMPLATE_SIZE})",
            template_content.len()
        )));
    }

    // Count template variables and control structures
    let variable_count = count_template_variables(template_content);
    if variable_count > MAX_TEMPLATE_VARIABLES {
        return Err(TemplatingError::Security(format!(
            "Too many template variables: {variable_count} (max allowed: {MAX_TEMPLATE_VARIABLES})"
        )));
    }

    // Check for excessive nesting that could cause stack overflow
    let max_nesting = check_template_nesting_depth(template_content);
    if max_nesting > MAX_TEMPLATE_RECURSION_DEPTH {
        return Err(TemplatingError::Security(format!(
            "Template nesting too deep: {max_nesting} levels (max allowed: {MAX_TEMPLATE_RECURSION_DEPTH})"
        )));
    }

    Ok(())
}

/// Count the number of template variables in a template
fn count_template_variables(template: &str) -> usize {
    use regex::Regex;

    // Match {{ variable }} patterns
    let variable_re = Regex::new(r"\{\{\s*(\w+)").unwrap();
    let mut variables = std::collections::HashSet::new();

    for cap in variable_re.captures_iter(template) {
        variables.insert(cap[1].to_string());
    }

    variables.len()
}

/// Check the maximum nesting depth of template control structures
fn check_template_nesting_depth(template: &str) -> usize {
    use regex::Regex;

    let open_re = Regex::new(r"\{%\s*(if|unless|for|capture|tablerow)\b").unwrap();
    let close_re = Regex::new(r"\{%\s*(endif|endunless|endfor|endcapture|endtablerow)\b").unwrap();

    let mut max_depth = 0;
    let mut current_depth: i32 = 0;

    let mut pos = 0;
    while pos < template.len() {
        if let Some(open_match) = open_re.find_at(template, pos) {
            if let Some(close_match) = close_re.find_at(template, pos) {
                if open_match.start() < close_match.start() {
                    // Opening tag comes first
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                    pos = open_match.end();
                } else {
                    // Closing tag comes first
                    current_depth = current_depth.saturating_sub(1);
                    pos = close_match.end();
                }
            } else {
                // Only opening tag found
                current_depth += 1;
                max_depth = max_depth.max(current_depth);
                pos = open_match.end();
            }
        } else if let Some(close_match) = close_re.find_at(template, pos) {
            // Only closing tag found
            current_depth = current_depth.saturating_sub(1);
            pos = close_match.end();
        } else {
            break;
        }
    }

    max_depth.max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_template_security_trusted() {
        let large_template = "a".repeat(MAX_TEMPLATE_SIZE + 1000);
        // Trusted templates have higher limits
        assert!(validate_template_security(&large_template, true).is_ok());

        let very_large_template = "a".repeat(MAX_TEMPLATE_SIZE * 10 + 1);
        assert!(validate_template_security(&very_large_template, true).is_err());
    }

    #[test]
    fn test_validate_template_security_untrusted_size() {
        let large_template = "a".repeat(MAX_TEMPLATE_SIZE + 1);
        let result = validate_template_security(&large_template, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn test_validate_template_security_excessive_nesting() {
        let deeply_nested = "{% if a %}{% if b %}{% if c %}{% if d %}{% if e %}{% if f %}{% if g %}{% if h %}{% if i %}{% if j %}{% if k %}deep{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}";
        let result = validate_template_security(deeply_nested, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nesting too deep"));
    }

    #[test]
    fn test_validate_template_security_safe_template() {
        let safe_template = "Hello {{ name }}! {% if premium %}You have premium access.{% endif %}";
        assert!(validate_template_security(safe_template, false).is_ok());
    }

    #[test]
    fn test_count_template_variables() {
        let template = "Hello {{ name }}! Your score is {{ score }} and your rank is {{ rank }}.";
        assert_eq!(count_template_variables(template), 3);
    }

    #[test]
    fn test_check_template_nesting_depth() {
        let shallow = "{% if a %}content{% endif %}";
        assert_eq!(check_template_nesting_depth(shallow), 1);

        let nested = "{% if a %}{% for item in items %}{{ item }}{% endfor %}{% endif %}";
        assert_eq!(check_template_nesting_depth(nested), 2);
    }
}
