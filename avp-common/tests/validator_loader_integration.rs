//! Integration tests for ValidatorLoader to ensure validators are loaded
//! from user (~/.avp/validators) and project (.avp/validators) directories.
//!
//! These tests verify:
//! - User validators are discovered and loaded correctly
//! - Project validators are discovered and loaded correctly
//! - Precedence: Project > User > Builtin
//! - Error handling for missing/invalid validators

mod test_helpers;

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use std::fs;
use tempfile::TempDir;
use test_helpers::{create_validator_dir, minimal_validator, validator_with_settings};

#[test]
fn test_load_validators_from_user_directory() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create user validators
    fs::write(
        validators_dir.join("user-validator-1.md"),
        minimal_validator("user-validator-1", "First user validator"),
    )
    .unwrap();

    fs::write(
        validators_dir.join("user-validator-2.md"),
        minimal_validator("user-validator-2", "Second user validator"),
    )
    .unwrap();

    // Load validators
    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Verify validators were loaded
    assert_eq!(loader.len(), 2, "Should load 2 user validators");

    let v1 = loader.get("user-validator-1");
    assert!(v1.is_some(), "user-validator-1 should be loaded");
    assert_eq!(v1.unwrap().source, ValidatorSource::User);
    assert_eq!(v1.unwrap().description(), "First user validator");

    let v2 = loader.get("user-validator-2");
    assert!(v2.is_some(), "user-validator-2 should be loaded");
    assert_eq!(v2.unwrap().source, ValidatorSource::User);
}

#[test]
fn test_load_validators_from_project_directory() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create project validators
    fs::write(
        validators_dir.join("project-validator.md"),
        minimal_validator("project-validator", "Project-specific validator"),
    )
    .unwrap();

    // Load validators
    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::Project)
        .unwrap();

    // Verify validator was loaded with correct source
    assert_eq!(loader.len(), 1);
    let v = loader.get("project-validator").unwrap();
    assert_eq!(v.source, ValidatorSource::Project);
    assert_eq!(v.description(), "Project-specific validator");
}

#[test]
fn test_project_validators_override_user_validators() {
    let user_temp = TempDir::new().unwrap();
    let user_validators = create_validator_dir(user_temp.path());

    let project_temp = TempDir::new().unwrap();
    let project_validators = create_validator_dir(project_temp.path());

    // Create a validator with same name in both directories
    fs::write(
        user_validators.join("shared-validator.md"),
        validator_with_settings(
            "shared-validator",
            "User version - should be overridden",
            "PostToolUse",
            "warn",
        ),
    )
    .unwrap();

    fs::write(
        project_validators.join("shared-validator.md"),
        validator_with_settings(
            "shared-validator",
            "Project version - should win",
            "PreToolUse",
            "error",
        ),
    )
    .unwrap();

    // Load in precedence order: user first, then project
    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&user_validators, ValidatorSource::User)
        .unwrap();

    // Verify user version loaded first
    assert_eq!(
        loader.get("shared-validator").unwrap().description(),
        "User version - should be overridden"
    );

    // Load project validators (should override)
    loader
        .load_directory(&project_validators, ValidatorSource::Project)
        .unwrap();

    // Verify project version now active
    let v = loader.get("shared-validator").unwrap();
    assert_eq!(
        v.description(),
        "Project version - should win",
        "Project validator should override user validator"
    );
    assert_eq!(
        v.source,
        ValidatorSource::Project,
        "Source should be Project after override"
    );
}

#[test]
fn test_user_validators_override_builtins() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create a user validator that overrides the builtin "no-secrets"
    fs::write(
        validators_dir.join("no-secrets.md"),
        validator_with_settings(
            "no-secrets",
            "Custom no-secrets - user override",
            "PostToolUse",
            "info", // Changed from error to info
        ),
    )
    .unwrap();

    // Load builtins first
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    // Verify builtin is loaded
    let builtin = loader.get("no-secrets");
    assert!(builtin.is_some(), "Builtin no-secrets should exist");
    assert_eq!(builtin.unwrap().source, ValidatorSource::Builtin);

    // Load user validators (should override)
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Verify user version now active
    let v = loader.get("no-secrets").unwrap();
    assert_eq!(
        v.description(),
        "Custom no-secrets - user override",
        "User validator should override builtin"
    );
    assert_eq!(v.source, ValidatorSource::User);
}

#[test]
fn test_full_precedence_chain_builtin_user_project() {
    let user_temp = TempDir::new().unwrap();
    let user_validators = create_validator_dir(user_temp.path());

    let project_temp = TempDir::new().unwrap();
    let project_validators = create_validator_dir(project_temp.path());

    // Create validators at each level
    // User-only validator
    fs::write(
        user_validators.join("user-only.md"),
        minimal_validator("user-only", "Only in user dir"),
    )
    .unwrap();

    // Project-only validator
    fs::write(
        project_validators.join("project-only.md"),
        minimal_validator("project-only", "Only in project dir"),
    )
    .unwrap();

    // Override chain: builtin -> user -> project
    fs::write(
        user_validators.join("no-secrets.md"),
        validator_with_settings("no-secrets", "User no-secrets", "PostToolUse", "warn"),
    )
    .unwrap();

    fs::write(
        project_validators.join("no-secrets.md"),
        validator_with_settings("no-secrets", "Project no-secrets", "PostToolUse", "info"),
    )
    .unwrap();

    // Load in precedence order
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    let builtin_count = loader.len();
    assert!(builtin_count > 0, "Should have loaded builtins");

    loader
        .load_directory(&user_validators, ValidatorSource::User)
        .unwrap();
    loader
        .load_directory(&project_validators, ValidatorSource::Project)
        .unwrap();

    // Verify final state
    // Builtin-only validators should still exist
    assert!(
        loader.get("safe-commands").is_some(),
        "Builtin safe-commands should exist"
    );
    assert_eq!(
        loader.get("safe-commands").unwrap().source,
        ValidatorSource::Builtin
    );

    // User-only validator should exist
    assert!(loader.get("user-only").is_some(), "User-only should exist");
    assert_eq!(
        loader.get("user-only").unwrap().source,
        ValidatorSource::User
    );

    // Project-only validator should exist
    assert!(
        loader.get("project-only").is_some(),
        "Project-only should exist"
    );
    assert_eq!(
        loader.get("project-only").unwrap().source,
        ValidatorSource::Project
    );

    // Override chain: project should win
    let no_secrets = loader.get("no-secrets").unwrap();
    assert_eq!(
        no_secrets.description(),
        "Project no-secrets",
        "Project version should be active"
    );
    assert_eq!(no_secrets.source, ValidatorSource::Project);
}

#[test]
fn test_load_directory_handles_nested_directories() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());
    let nested_dir = validators_dir.join("subdirectory");
    fs::create_dir_all(&nested_dir).unwrap();

    // Create validators at different levels
    fs::write(
        validators_dir.join("root-validator.md"),
        minimal_validator("root-validator", "At root level"),
    )
    .unwrap();

    fs::write(
        nested_dir.join("nested-validator.md"),
        minimal_validator("nested-validator", "In subdirectory"),
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Both should be loaded
    assert!(
        loader.get("root-validator").is_some(),
        "Root validator should load"
    );
    assert!(
        loader.get("nested-validator").is_some(),
        "Nested validator should load"
    );
}

#[test]
fn test_load_directory_skips_partials_directory() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());
    let partials_dir = validators_dir.join("_partials");
    fs::create_dir_all(&partials_dir).unwrap();

    // Create a regular validator
    fs::write(
        validators_dir.join("regular.md"),
        minimal_validator("regular", "Regular validator"),
    )
    .unwrap();

    // Create a partial in _partials (should be skipped)
    fs::write(
        partials_dir.join("shared.md"),
        "{% partial %}\n\nShared content.",
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Only regular validator should be loaded
    assert_eq!(loader.len(), 1);
    assert!(loader.get("regular").is_some());
    assert!(loader.get("shared").is_none());
}

#[test]
fn test_load_directory_skips_non_markdown_files() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create files with different extensions
    fs::write(
        validators_dir.join("valid.md"),
        minimal_validator("valid", "Valid markdown"),
    )
    .unwrap();

    fs::write(validators_dir.join("readme.txt"), "This is a readme").unwrap();

    fs::write(validators_dir.join("config.yaml"), "key: value").unwrap();

    fs::write(validators_dir.join("script.js"), "console.log('hi')").unwrap();

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Only .md file should be loaded
    assert_eq!(loader.len(), 1);
    assert!(loader.get("valid").is_some());
}

#[test]
fn test_load_directory_handles_invalid_validator_gracefully() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create a valid validator
    fs::write(
        validators_dir.join("valid.md"),
        minimal_validator("valid", "Valid validator"),
    )
    .unwrap();

    // Create an invalid validator (malformed YAML)
    fs::write(
        validators_dir.join("invalid.md"),
        r#"---
name: [broken yaml
description: this won't parse
---

Body.
"#,
    )
    .unwrap();

    // Create another valid validator
    fs::write(
        validators_dir.join("also-valid.md"),
        minimal_validator("also-valid", "Another valid one"),
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    // Should not panic, should log warning for invalid file
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Valid validators should still be loaded
    assert_eq!(loader.len(), 2);
    assert!(loader.get("valid").is_some());
    assert!(loader.get("also-valid").is_some());
}

#[test]
fn test_load_directory_empty_directory() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    assert_eq!(loader.len(), 0);
}

#[test]
fn test_load_directory_nonexistent_directory() {
    let temp = TempDir::new().unwrap();
    let validators_dir = temp.path().join("nonexistent");

    let mut loader = ValidatorLoader::new();
    // Should not error, just skip
    let result = loader.load_directory(&validators_dir, ValidatorSource::User);
    assert!(result.is_ok());
    assert_eq!(loader.len(), 0);
}

#[test]
fn test_validator_with_defaults_from_filename() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    // Create a minimal validator - no name, no trigger
    fs::write(
        validators_dir.join("my-custom-check.md"),
        r#"---
---

Check that the code is correct.
"#,
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::User)
        .unwrap();

    // Should be loaded with name derived from filename
    let v = loader.get("my-custom-check");
    assert!(
        v.is_some(),
        "Validator should be loaded with name from filename"
    );
    let v = v.unwrap();
    assert_eq!(v.name(), "my-custom-check");
    assert_eq!(v.description(), "Validator: my-custom-check");
    assert_eq!(
        v.trigger(),
        avp_common::types::HookType::PostToolUse,
        "Default trigger should be PostToolUse"
    );
}

#[test]
fn test_get_directories_returns_valid_paths() {
    // This tests the directory discovery mechanism
    // Note: This test may find real directories if they exist on the system
    let dirs = ValidatorLoader::get_directories();

    // Verify all returned directories actually exist
    for dir in &dirs {
        assert!(
            dir.exists(),
            "get_directories should only return existing directories: {}",
            dir.display()
        );
        assert!(
            dir.is_dir(),
            "get_directories should only return directories, not files: {}",
            dir.display()
        );
    }
}

#[test]
fn test_load_all_uses_vfs_correctly() {
    // This test verifies that load_all() integrates with VirtualFileSystem
    // It can't fully test user home loading without mocking, but verifies the method runs
    let mut loader = ValidatorLoader::new();

    // Should not panic and should complete successfully
    let result = loader.load_all();
    assert!(result.is_ok(), "load_all should not error");

    // Note: The actual validators loaded depend on what's on the filesystem
    // This test ensures the code path works, not specific validator counts
}

#[test]
fn test_validator_without_any_frontmatter() {
    let temp = TempDir::new().unwrap();
    let validators_dir = create_validator_dir(temp.path());

    fs::write(
        validators_dir.join("comfy-table-rule.md"),
        "# Comfy-Table Rule\n\nUse Cell API for colored text.",
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    loader
        .load_directory(&validators_dir, ValidatorSource::Project)
        .unwrap();

    let v = loader
        .get("comfy-table-rule")
        .expect("should load validator without frontmatter");
    assert_eq!(v.name(), "comfy-table-rule");
    assert_eq!(v.description(), "Validator: comfy-table-rule");
    assert_eq!(v.trigger(), avp_common::types::HookType::PostToolUse);
    assert_eq!(v.severity(), avp_common::validator::Severity::Warn);
    assert_eq!(v.source, ValidatorSource::Project);
    assert!(v.body.contains("Comfy-Table Rule"));
}

#[test]
fn test_list_validators_shows_all_sources() {
    let user_temp = TempDir::new().unwrap();
    let user_validators = create_validator_dir(user_temp.path());

    fs::write(
        user_validators.join("user-check.md"),
        minimal_validator("user-check", "User check"),
    )
    .unwrap();

    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);
    loader
        .load_directory(&user_validators, ValidatorSource::User)
        .unwrap();

    let all_validators = loader.list();

    // Should have both builtin and user validators
    let sources: Vec<_> = all_validators.iter().map(|v| &v.source).collect();
    assert!(
        sources.contains(&&ValidatorSource::Builtin),
        "Should have builtin validators"
    );
    assert!(
        sources.contains(&&ValidatorSource::User),
        "Should have user validators"
    );

    // Verify user-check is in the list
    let user_check = all_validators.iter().find(|v| v.name() == "user-check");
    assert!(user_check.is_some(), "user-check should be in list");
}
