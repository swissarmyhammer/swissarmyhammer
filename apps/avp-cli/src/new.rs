//! AVP New - Scaffold a new RuleSet package.

use std::fs;
use std::path::PathBuf;

use crate::AvpCliError;

/// Run the new command.
///
/// Creates a new RuleSet directory structure with template files.
/// When `global` is true, creates under `~/.avp/validators/`; otherwise `.avp/validators/`.
pub fn run_new(name: &str, global: bool) -> Result<(), AvpCliError> {
    // Validate name: kebab-case, alphanumeric + hyphens
    if !is_valid_package_name(name) {
        return Err(AvpCliError::Validation(format!(
            "Invalid package name '{}'. Must be lowercase, alphanumeric with hyphens, 1-64 chars.",
            name
        )));
    }

    let base_dir = if global {
        dirs::home_dir()
            .ok_or_else(|| AvpCliError::Validation("Could not find home directory".to_string()))?
            .join(".avp")
            .join("validators")
            .join(name)
    } else {
        PathBuf::from(".avp").join("validators").join(name)
    };
    if base_dir.exists() {
        return Err(AvpCliError::Validation(format!(
            "Directory already exists: {}",
            base_dir.display()
        )));
    }

    // Create directory structure
    let rules_dir = base_dir.join("rules");
    fs::create_dir_all(&rules_dir)?;

    // Write VALIDATOR.md
    let validator_md = format!(
        r#"---
name: {name}
description: "TODO: Describe what this RuleSet validates"
metadata:
  version: "0.1.0"
trigger: PostToolUse
match:
  tools: [Write, Edit]
tags: []
---

# {name}

TODO: Describe the purpose and behavior of this RuleSet.
Rules are automatically discovered from the `rules/` directory.
"#,
        name = name
    );
    fs::write(base_dir.join("VALIDATOR.md"), validator_md)?;

    // Write example rule
    let example_rule = r#"---
name: example-rule
description: "An example validation rule"
---

# Example Rule

Check that the code change follows project conventions.

## Validation Criteria

- TODO: Define what this rule checks
- TODO: Define pass/fail conditions

## Instructions

Review the tool input and output. If the change violates the criteria above,
report a failure with a clear explanation.
"#;
    fs::write(rules_dir.join("example.md"), example_rule)?;

    // Write README.md
    let readme = format!(
        r#"# {name}

A validator RuleSet for the Agent Validator Protocol.

## Usage

Install with AVP:

```bash
avp install {name}
```

## Development

Edit `VALIDATOR.md` to configure the RuleSet metadata.
Add rule files to the `rules/` directory.

When ready to publish:

```bash
avp publish
```
"#,
        name = name
    );
    fs::write(base_dir.join("README.md"), readme)?;

    let scope = if global { "global (user)" } else { "project" };
    println!("Created {} RuleSet '{}':\n", scope, name);
    println!("  {}/", base_dir.display());
    println!("  ├── VALIDATOR.md");
    println!("  ├── README.md");
    println!("  └── rules/");
    println!("      └── example.md");
    println!();
    println!("Next steps:");
    println!("  1. Edit VALIDATOR.md to set description, trigger, and match criteria");
    println!("  2. Add rule files to rules/");
    println!("  3. Run 'avp publish' when ready to share");

    Ok(())
}

/// Validate that a package name is valid (kebab-case, 1-64 chars).
fn is_valid_package_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_package_names() {
        assert!(is_valid_package_name("no-secrets"));
        assert!(is_valid_package_name("a"));
        assert!(is_valid_package_name("my-validator-123"));
    }

    #[test]
    fn test_invalid_package_names() {
        assert!(!is_valid_package_name(""));
        assert!(!is_valid_package_name("-starts-with-hyphen"));
        assert!(!is_valid_package_name("ends-with-hyphen-"));
        assert!(!is_valid_package_name("HAS_UPPER"));
        assert!(!is_valid_package_name("has spaces"));
        assert!(!is_valid_package_name("has_underscores"));
        let long = "a".repeat(65);
        assert!(!is_valid_package_name(&long));
    }
}
