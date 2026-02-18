//! Validation for Agent Skills spec compliance

use crate::skill::SkillName;

/// Validate a skill name per Agent Skills spec
pub fn validate_skill_name(name: &str) -> Result<SkillName, String> {
    SkillName::new(name)
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
        assert!(validate_frontmatter(
            &Some("plan".to_string()),
            &Some("description".to_string()),
        )
        .is_ok());

        assert!(validate_frontmatter(&None, &Some("description".to_string())).is_err());

        assert!(validate_frontmatter(&Some("plan".to_string()), &None).is_err());
    }
}
