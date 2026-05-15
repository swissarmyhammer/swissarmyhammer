//! Validation for Agent Skills spec compliance

use crate::skill::SkillName;

/// Maximum allowed length of a skill `description` field, measured in Unicode
/// scalar values (chars). Matches the Anthropic Agent Skills guide requirement.
pub const MAX_DESCRIPTION_CHARS: usize = 1024;

/// Validate a skill name per Agent Skills spec
pub fn validate_skill_name(name: &str) -> Result<SkillName, String> {
    SkillName::new(name)
}

/// Validate a skill `description` against the Anthropic Agent Skills guide:
///
/// - Length must not exceed [`MAX_DESCRIPTION_CHARS`] (1024) characters.
/// - Must not contain XML angle brackets (`<` or `>`), which are reserved for
///   Anthropic's prompt templating and can corrupt skill activation.
///
/// Returns `Ok(())` when the description complies, or `Err(message)` with a
/// human-readable explanation when it does not.
pub fn validate_description(description: &str) -> Result<(), String> {
    let char_count = description.chars().count();
    if char_count > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description is {} chars; must be <= {} chars",
            char_count, MAX_DESCRIPTION_CHARS
        ));
    }

    if let Some(bad) = description.chars().find(|c| *c == '<' || *c == '>') {
        return Err(format!(
            "description must not contain '<' or '>' (found '{}')",
            bad
        ));
    }

    Ok(())
}

/// Validate required SKILL.md frontmatter fields
pub fn validate_frontmatter(
    name: &Option<String>,
    description: &Option<String>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if name.is_none() || name.as_deref() == Some("") {
        errors.push("missing required field: name".to_string());
    }

    if description.is_none() || description.as_deref() == Some("") {
        errors.push("missing required field: description".to_string());
    }

    if let Some(name) = name {
        if let Err(e) = validate_skill_name(name) {
            errors.push(e);
        }
    }

    if let Some(description) = description {
        if !description.is_empty() {
            if let Err(e) = validate_description(description) {
                errors.push(e);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_skill_names() {
        assert!(validate_skill_name("plan").is_ok());
        assert!(validate_skill_name("do").is_ok());
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("skill123").is_ok());
    }

    #[test]
    fn test_invalid_skill_names() {
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("My Skill").is_err());
        assert!(validate_skill_name("UPPERCASE").is_err());
        assert!(validate_skill_name("has_underscore").is_err());
        assert!(validate_skill_name("has.dot").is_err());
    }

    #[test]
    fn test_validate_frontmatter() {
        assert!(
            validate_frontmatter(&Some("plan".to_string()), &Some("description".to_string()),)
                .is_ok()
        );

        assert!(validate_frontmatter(&None, &Some("description".to_string())).is_err());

        assert!(validate_frontmatter(&Some("plan".to_string()), &None).is_err());
    }

    #[test]
    fn test_validate_description_accepts_short_ascii() {
        assert!(validate_description("A short description.").is_ok());
    }

    #[test]
    fn test_validate_description_accepts_exactly_1024_chars() {
        // Boundary: exactly at the limit is allowed.
        let desc = "a".repeat(MAX_DESCRIPTION_CHARS);
        assert!(validate_description(&desc).is_ok());
    }

    #[test]
    fn test_validate_description_rejects_over_length() {
        // Regression: a description longer than 1024 chars must fail.
        let desc = "a".repeat(MAX_DESCRIPTION_CHARS + 1);
        let err = validate_description(&desc).unwrap_err();
        assert!(
            err.contains("1025") && err.contains("<= 1024"),
            "error should report char count and limit, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_description_rejects_left_angle_bracket() {
        // Regression: description containing '<' must fail.
        let err = validate_description("contains a <tag>").unwrap_err();
        assert!(
            err.contains("'<'") || err.contains("found '<'"),
            "error should mention the bad char, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_description_rejects_right_angle_bracket() {
        // Regression companion: description with only '>' must also fail.
        let err = validate_description("trailing bracket >").unwrap_err();
        assert!(
            err.contains("'>'") || err.contains("found '>'"),
            "error should mention the bad char, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_description_counts_chars_not_bytes() {
        // Em-dashes are 3 bytes each in UTF-8. 1024 em-dashes = 3072 bytes but
        // only 1024 chars — still valid. 1025 em-dashes must fail on char count.
        let ok = "—".repeat(MAX_DESCRIPTION_CHARS);
        assert!(validate_description(&ok).is_ok());

        let too_long = "—".repeat(MAX_DESCRIPTION_CHARS + 1);
        assert!(validate_description(&too_long).is_err());
    }

    #[test]
    fn test_validate_frontmatter_propagates_description_errors() {
        // validate_frontmatter should surface description violations alongside
        // the existing name/required checks.
        let bad_desc = "has an <angle bracket>".to_string();
        let result = validate_frontmatter(&Some("plan".to_string()), &Some(bad_desc));
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("'<'")),
            "expected '<' complaint, got: {:?}",
            errors
        );
    }
}
