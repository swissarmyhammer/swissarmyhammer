//! Validation for agent definitions

use crate::agent::AgentName;

/// Validate an agent name
pub fn validate_agent_name(name: &str) -> Result<AgentName, String> {
    AgentName::new(name)
}

/// Validate required AGENT.md frontmatter fields
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
        if let Err(e) = validate_agent_name(name) {
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
    fn test_valid_agent_names() {
        assert!(validate_agent_name("default").is_ok());
        assert!(validate_agent_name("test").is_ok());
        assert!(validate_agent_name("general-purpose").is_ok());
        assert!(validate_agent_name("agent123").is_ok());
    }

    #[test]
    fn test_invalid_agent_names() {
        assert!(validate_agent_name("").is_err());
        assert!(validate_agent_name("My Agent").is_err());
        assert!(validate_agent_name("UPPERCASE").is_err());
        assert!(validate_agent_name("has_underscore").is_err());
        assert!(validate_agent_name("has.dot").is_err());
    }

    #[test]
    fn test_validate_frontmatter() {
        assert!(
            validate_frontmatter(&Some("test".to_string()), &Some("description".to_string()),)
                .is_ok()
        );

        assert!(validate_frontmatter(&None, &Some("description".to_string())).is_err());
        assert!(validate_frontmatter(&Some("test".to_string()), &None).is_err());
    }
}
