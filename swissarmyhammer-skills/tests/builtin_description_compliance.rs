//! Enforces the Anthropic Agent Skills guide requirements on every builtin
//! `SKILL.md` description:
//!
//! - description length must be <= 1024 chars
//! - description must not contain `<` or `>`
//!
//! Failing this test means a builtin skill drifted past the guide limits and
//! must be shortened or reworded before it ships.

use swissarmyhammer_skills::{validate_description, SkillResolver, MAX_DESCRIPTION_CHARS};

/// Assert every builtin skill description obeys the Anthropic guide limits.
///
/// This is a belt-and-suspenders check. The skill loader also calls
/// `validate_frontmatter` (which now includes description checks), but a
/// loader that silently skips a failing skill would still let a bad builtin
/// ship. Iterating the resolved map here guarantees an explicit crate-level
/// assertion for every builtin that actually loaded.
#[test]
fn all_builtin_skill_descriptions_comply_with_anthropic_guide() {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();

    assert!(
        !builtins.is_empty(),
        "expected at least one builtin skill to be loaded"
    );

    let mut failures: Vec<String> = Vec::new();
    for (name, skill) in &builtins {
        if let Err(msg) = validate_description(&skill.description) {
            failures.push(format!("skill '{name}': {msg}"));
        }
    }

    assert!(
        failures.is_empty(),
        "builtin skills violate Anthropic guide description rules (limit = {} chars, no '<'/'>'):\n  {}",
        MAX_DESCRIPTION_CHARS,
        failures.join("\n  ")
    );
}

/// Assert the loader-level validator (`validate_all_sources`) also reports no
/// issues for builtins. This catches the case where a skill fails validation
/// hard enough to be dropped entirely by `resolve_builtins`, so the previous
/// test would not see it.
#[test]
fn validate_all_sources_reports_no_errors_for_builtin_descriptions() {
    use swissarmyhammer_common::validation::ValidationLevel;

    let resolver = SkillResolver::new();
    let issues = resolver.validate_all_sources();

    let builtin_errors: Vec<_> = issues
        .iter()
        .filter(|i| {
            i.level == ValidationLevel::Error && i.file_path.to_string_lossy().contains("builtin")
        })
        .collect();

    assert!(
        builtin_errors.is_empty(),
        "validate_all_sources returned errors for builtin skills: {:?}",
        builtin_errors
            .iter()
            .map(|i| format!("{}: {}", i.file_path.display(), i.message))
            .collect::<Vec<_>>()
    );
}
