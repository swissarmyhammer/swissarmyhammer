//! Builtin validators and YAML includes embedded in the AVP binary.
//!
//! This module provides default validators and YAML include files that are
//! always available, regardless of user or project configuration. Files are
//! automatically discovered from the `builtin/` directory at build time.
//!
//! # YAML Includes
//!
//! YAML files from `builtin/` (excluding subdirectories like `validators/`,
//! `prompts/`, etc.) are loaded as includes. These can be referenced in
//! validator frontmatter using `@path/name` syntax:
//!
//! ```yaml
//! match:
//!   files:
//!     - "@file_groups/source_code"
//! ```

use crate::validators::{ValidatorLoader, ValidatorSource};
use std::path::PathBuf;

// Include the generated builtin YAML includes
include!(concat!(env!("OUT_DIR"), "/builtin_includes.rs"));

/// Load all builtin RuleSets into a loader.
///
/// This loads RuleSets from the builtin/validators directory and also loads
/// builtin YAML includes so that `@` references work.
/// Call this method before loading user or project validators to ensure
/// builtins have the lowest precedence.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_validators::builtin::load_builtins;
/// use swissarmyhammer_validators::validators::ValidatorLoader;
///
/// let mut loader = ValidatorLoader::new();
/// load_builtins(&mut loader);
///
/// // Now load user/project validators which will override builtins
/// loader.load_all().ok();
/// ```
pub fn load_builtins(loader: &mut ValidatorLoader) {
    // First load YAML includes so @references work
    for (name, content) in get_builtin_includes() {
        if let Err(e) = loader.add_builtin_include(name, content) {
            tracing::warn!("Failed to load builtin include '{}': {}", name, e);
        }
    }

    // Load RuleSets from builtin/validators directory
    // The path is relative to the crate root where Cargo.toml is located
    let builtin_validators_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");

    if let Err(e) =
        loader.load_rulesets_directory(&builtin_validators_path, ValidatorSource::Builtin)
    {
        tracing::error!("Failed to load builtin RuleSets: {}", e);
    }
}

/// Get all builtin YAML includes as (name, content) tuples.
pub fn includes_raw() -> Vec<(&'static str, &'static str)> {
    get_builtin_includes()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Focused review validators (split out from the monolithic code-quality set)
    // ========================================================================

    /// The four probe-bearing focused validators and the catalog probe each one
    /// declares. Probe names must be real catalog entries (`duplicates` /
    /// `similar` / `callers`); never `search_symbol` or `get_blastradius`.
    const PROBE_VALIDATORS: &[(&str, &str)] = &[
        ("duplication", "duplicates"),
        ("reuse", "similar"),
        ("dead-code", "callers"),
    ];

    /// In-file focused validators that declare NO probes — every judgment is made
    /// by reading the diff, with no engine-run code_context probe.
    const IN_FILE_VALIDATORS: &[&str] = &[
        "data-driven",
        "complexity",
        "function-length",
        "naming",
        "no-commented-code",
        "magic-numbers",
        "missing-docs",
    ];

    /// The focused review-time safety/integrity validators migrated from the old
    /// multi-rule `security-rules` and `test-integrity` sets. Each is a blocker
    /// (`severity: error`), in-file (no probes), with no `trigger`.
    const SAFETY_VALIDATORS: &[&str] = &[
        "no-secrets",
        "injection",
        "command-safety",
        "test-integrity",
    ];

    /// Language-scoped review validators migrated from the skill's
    /// `references/*_REVIEW.md` files. Each entry is
    /// `(validator name, a file it MUST match, a file it MUST NOT match)`.
    /// Every one is in-file (no probes) and file-triggered (no tool match).
    const LANGUAGE_VALIDATORS: &[(&str, &str, &str)] = &[
        ("rust", "src/main.rs", "src/main.py"),
        ("python", "src/app.py", "src/app.rs"),
        ("js-ts", "src/index.ts", "src/index.rs"),
        ("dart", "lib/widget.dart", "lib/widget.rs"),
    ];

    #[test]
    fn test_focused_validators_load_with_their_catalog_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for (name, probe) in PROBE_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("focused validator '{name}' should be loaded"));

            assert_eq!(
                ruleset.manifest.probes,
                vec![probe.to_string()],
                "{name} should declare exactly the catalog probe [{probe}]"
            );

            // Every declared probe must be a real catalog name.
            for declared in &ruleset.manifest.probes {
                assert!(
                    crate::review::probe_exists(declared),
                    "{name} declares probe '{declared}' which is not in the catalog"
                );
            }

            // A probe-bearing validator must still carry at least one rule.
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should have at least one rule"
            );
        }
    }

    #[test]
    fn test_in_file_validators_declare_no_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for name in IN_FILE_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("in-file validator '{name}' should be loaded"));
            assert!(
                ruleset.manifest.probes.is_empty(),
                "in-file validator '{name}' must declare no probes, got: {:?}",
                ruleset.manifest.probes
            );
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should have at least one rule"
            );
        }
    }

    #[test]
    fn test_focused_validators_have_clean_manifest_frontmatter() {
        use crate::validators::parser::check_manifest_frontmatter;

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");

        let names = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(IN_FILE_VALIDATORS.iter().copied());

        for name in names {
            let dir = base.join(name);
            let manifest = dir.join("VALIDATOR.md");
            let content = std::fs::read_to_string(&manifest)
                .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
            let issues = check_manifest_frontmatter(&content, &dir);
            assert!(
                issues.is_empty(),
                "{name} VALIDATOR.md should have no stray frontmatter (e.g. `trigger`), got: {issues:?}"
            );
        }
    }

    // ========================================================================
    // Focused safety/integrity validators (migrated from security-rules /
    // test-integrity multi-rule sets into focused review-time validators)
    // ========================================================================

    #[test]
    fn test_safety_validators_load_as_blockers_with_no_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for name in SAFETY_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("safety validator '{name}' should be loaded"));
            assert_eq!(ruleset.name(), *name);

            // Every safety/integrity concern is a blocker.
            assert_eq!(
                ruleset.manifest.severity,
                crate::validators::types::Severity::Error,
                "{name} should be a blocker (severity: error)"
            );

            // All four are in-file judgments — no engine probes.
            assert!(
                ruleset.manifest.probes.is_empty(),
                "safety validator '{name}' must declare no probes, got: {:?}",
                ruleset.manifest.probes
            );

            // Each carries at least one rule.
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should carry at least one rule"
            );
        }
    }

    #[test]
    fn test_test_integrity_homes_no_hard_code() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        // `no-hard-code` ("return 42 to pass a test") moved here from the
        // deleted code-quality set, alongside the original test-cheating rule.
        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        assert!(
            rule_names.contains(&"no-hard-code"),
            "test-integrity should home the no-hard-code rule, got: {rule_names:?}"
        );
        assert!(
            rule_names.contains(&"no-test-cheating"),
            "test-integrity should keep the no-test-cheating rule, got: {rule_names:?}"
        );
    }

    #[test]
    fn test_old_multi_rule_safety_sets_are_rehomed() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // The old multi-rule `security-rules` set was split into the focused
        // `no-secrets` and `injection` validators and must no longer load.
        assert!(
            loader.get_ruleset("security-rules").is_none(),
            "the multi-rule security-rules set must be re-homed into no-secrets + injection"
        );
    }

    #[test]
    fn test_safety_validators_match_expected_paths() {
        use crate::validators::types::MatchContext;

        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Each safety validator is file-triggered over source code: it matches a
        // changed source file by glob and does not match a non-source path.
        for name in SAFETY_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("{name} should be loaded"));

            let yes = MatchContext::new().with_file("src/app.py");
            assert!(
                ruleset.matches(&yes),
                "{name} should match a changed source file 'src/app.py'"
            );

            let no = MatchContext::new().with_file("README.md");
            assert!(
                !ruleset.matches(&no),
                "{name} should NOT match a non-source file 'README.md'"
            );
        }
    }

    #[test]
    fn test_safety_validators_have_clean_manifest_frontmatter() {
        use crate::validators::parser::check_manifest_frontmatter;

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");

        for name in SAFETY_VALIDATORS {
            let dir = base.join(name);
            let manifest = dir.join("VALIDATOR.md");
            let content = std::fs::read_to_string(&manifest)
                .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
            let issues = check_manifest_frontmatter(&content, &dir);
            assert!(
                issues.is_empty(),
                "{name} VALIDATOR.md should have no stray frontmatter (e.g. `trigger`), got: {issues:?}"
            );
        }
    }

    #[test]
    fn test_monolithic_code_quality_set_is_gone() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        assert!(
            loader.get_ruleset("code-quality").is_none(),
            "the monolithic code-quality set must be deleted once its rules are re-homed"
        );
    }

    #[test]
    fn test_load_builtins() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Should have loaded at least 3 RuleSets (the focused safety validators
        // plus the focused validators split out of code-quality)
        assert!(
            loader.ruleset_count() >= 3,
            "Should have loaded at least 3 RuleSets, got {}",
            loader.ruleset_count()
        );

        // Check for expected RuleSets. The monolithic security-rules set was
        // split into the focused no-secrets + injection validators.
        assert!(
            loader.get_ruleset("no-secrets").is_some(),
            "Should have the focused no-secrets validator"
        );
        // The monolithic code-quality set was split into focused validators.
        assert!(
            loader.get_ruleset("duplication").is_some(),
            "Should have the focused duplication validator"
        );
    }

    #[test]
    fn test_no_secrets_and_injection_rulesets_load() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // The old multi-rule security-rules set was split into two focused
        // blocker validators, each carrying its single rule.
        let no_secrets = loader
            .get_ruleset("no-secrets")
            .expect("no-secrets RuleSet should exist");
        assert_eq!(no_secrets.name(), "no-secrets");
        assert!(
            no_secrets.rules.iter().any(|r| r.name == "no-secrets"),
            "no-secrets validator should carry the no-secrets rule"
        );

        let injection = loader
            .get_ruleset("injection")
            .expect("injection RuleSet should exist");
        assert_eq!(injection.name(), "injection");
        assert!(
            injection.rules.iter().any(|r| r.name == "injection"),
            "injection validator should carry the injection rule"
        );
    }

    #[test]
    fn test_rehomed_quality_validators_load() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // The nine code-quality concerns were re-homed/split into ten focused,
        // one-concern validators. Each loads as its own RuleSet with at least
        // one rule.
        let focused = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(IN_FILE_VALIDATORS.iter().copied());
        for name in focused {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("focused validator '{name}' should exist"));
            assert_eq!(ruleset.name(), name);
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should carry at least one rule"
            );
        }
    }

    #[test]
    fn test_builtin_includes_loaded() {
        let includes = get_builtin_includes();
        assert!(
            !includes.is_empty(),
            "Should have at least one builtin include"
        );

        // Should have file_groups
        let names: Vec<&str> = includes.iter().map(|(name, _)| *name).collect();
        assert!(
            names.iter().any(|n| n.contains("file_groups")),
            "Should have file_groups includes"
        );
    }

    #[test]
    fn test_no_secrets_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("no-secrets")
            .expect("no-secrets should be loaded");

        // The @file_groups/source_code should have been expanded in the manifest
        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("no-secrets should have match criteria");

        // Should have actual file patterns, not the @reference
        assert!(
            !match_criteria.files.is_empty(),
            "files should not be empty after expansion"
        );
        assert!(
            !match_criteria.files.iter().any(|f| f.starts_with('@')),
            "@ references should be expanded, but found: {:?}",
            match_criteria.files
        );
        // Should contain some expected patterns from source_code.yaml
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain common source file patterns after expansion"
        );
    }

    #[test]
    fn test_test_integrity_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        // The @file_groups/source_code and @file_groups/test_files should have been expanded
        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("test-integrity should have match criteria");

        // Should have actual file patterns, not the @reference
        assert!(
            !match_criteria.files.is_empty(),
            "files should not be empty after expansion"
        );
        assert!(
            !match_criteria.files.iter().any(|f| f.starts_with('@')),
            "@ references should be expanded, but found: {:?}",
            match_criteria.files
        );
        // Should contain patterns from both source_code.yaml and test_files.yaml
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain source file patterns after expansion"
        );
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f.contains("test") || f.contains("spec")),
            "Should contain test file patterns after expansion"
        );
    }

    // ========================================================================
    // Match-criteria assertions (hook-free)
    // ========================================================================

    #[test]
    fn test_focused_validators_have_no_tool_match() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Review-time validators are file-triggered, never tool-triggered: they
        // match a changed file by glob, with no tool pattern.
        let focused = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(IN_FILE_VALIDATORS.iter().copied());
        for name in focused {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("{name} should be loaded"));
            if let Some(match_criteria) = &ruleset.manifest.match_criteria {
                assert!(
                    match_criteria.tools.is_empty(),
                    "{name} should not have tool match patterns, but has: {:?}",
                    match_criteria.tools
                );
            }
        }
    }

    #[test]
    fn test_test_integrity_has_no_tool_match() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        // Stop validators should not have tool patterns (Stop hooks have no tool_name)
        if let Some(match_criteria) = &ruleset.manifest.match_criteria {
            assert!(
                match_criteria.tools.is_empty(),
                "test-integrity (Stop trigger) should not have tool match patterns, but has: {:?}",
                match_criteria.tools
            );
        }
    }

    #[test]
    fn test_focused_validators_retain_file_patterns() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Each focused validator must carry expanded file globs so the fleet can
        // scope it to the changed files.
        let focused = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(IN_FILE_VALIDATORS.iter().copied());
        for name in focused {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("{name} should be loaded"));
            let match_criteria =
                ruleset.manifest.match_criteria.as_ref().unwrap_or_else(|| {
                    panic!("{name} should have match criteria with file patterns")
                });
            assert!(
                !match_criteria.files.is_empty(),
                "{name} should retain file patterns for filtering changed files"
            );
            assert!(
                !match_criteria.files.iter().any(|f| f.starts_with('@')),
                "{name} @file_groups references should be expanded, got: {:?}",
                match_criteria.files
            );
        }
    }

    #[test]
    fn test_test_integrity_retains_file_patterns() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("test-integrity should have match criteria with file patterns");

        assert!(
            !match_criteria.files.is_empty(),
            "test-integrity should retain file patterns for filtering changed files"
        );
    }

    // ========================================================================
    // Language review validators (migrated from references/*_REVIEW.md)
    // ========================================================================

    #[test]
    fn test_language_validators_load_with_rules_and_no_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for (name, _, _) in LANGUAGE_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("language validator '{name}' should be loaded"));
            assert_eq!(ruleset.name(), *name);
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should carry at least one rule derived from its *_REVIEW.md"
            );
            // These are in-file idiom judgments — no engine probes.
            assert!(
                ruleset.manifest.probes.is_empty(),
                "language validator '{name}' must declare no probes, got: {:?}",
                ruleset.manifest.probes
            );
        }
    }

    #[test]
    fn test_language_validators_match_only_their_glob() {
        use crate::validators::types::MatchContext;

        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for (name, should_match, should_not_match) in LANGUAGE_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("language validator '{name}' should be loaded"));

            let yes = MatchContext::new().with_file(*should_match);
            assert!(
                ruleset.matches(&yes),
                "{name} should match its own language file '{should_match}'"
            );

            let no = MatchContext::new().with_file(*should_not_match);
            assert!(
                !ruleset.matches(&no),
                "{name} should NOT match foreign file '{should_not_match}'"
            );
        }
    }

    #[test]
    fn test_js_ts_validator_matches_all_four_extensions() {
        use crate::validators::types::MatchContext;

        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("js-ts")
            .expect("js-ts validator should be loaded");

        // `**/*.{js,jsx,ts,tsx}` is expressed as four literal globs because the
        // glob engine does not expand brace alternation. All four must match.
        for file in ["src/a.js", "src/b.jsx", "src/c.ts", "src/d.tsx"] {
            let ctx = MatchContext::new().with_file(file);
            assert!(ruleset.matches(&ctx), "js-ts should match '{file}'");
        }

        // And foreign extensions must not match.
        for file in ["src/e.py", "src/f.rs", "src/g.dart", "src/h.json"] {
            let ctx = MatchContext::new().with_file(file);
            assert!(!ruleset.matches(&ctx), "js-ts should NOT match '{file}'");
        }
    }

    #[test]
    fn test_language_validators_are_file_triggered_not_tool_triggered() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Review-time language validators match changed files by glob, never a
        // tool pattern, and carry expanded (non-`@`) file globs.
        for (name, _, _) in LANGUAGE_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("{name} should be loaded"));
            let match_criteria = ruleset
                .manifest
                .match_criteria
                .as_ref()
                .unwrap_or_else(|| panic!("{name} should have match criteria with file globs"));
            assert!(
                match_criteria.tools.is_empty(),
                "{name} should not have tool match patterns, but has: {:?}",
                match_criteria.tools
            );
            assert!(
                !match_criteria.files.is_empty(),
                "{name} should carry file globs to scope it to changed files"
            );
            assert!(
                !match_criteria.files.iter().any(|f| f.starts_with('@')),
                "{name} file globs should be literal, not `@` references, got: {:?}",
                match_criteria.files
            );
        }
    }

    #[test]
    fn test_language_validator_manifests_have_clean_frontmatter() {
        use crate::validators::parser::check_manifest_frontmatter;

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");

        for (name, _, _) in LANGUAGE_VALIDATORS {
            let dir = base.join(name);
            let manifest = dir.join("VALIDATOR.md");
            let content = std::fs::read_to_string(&manifest)
                .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
            let issues = check_manifest_frontmatter(&content, &dir);
            assert!(
                issues.is_empty(),
                "{name} VALIDATOR.md should have no stray frontmatter (e.g. `trigger`), got: {issues:?}"
            );
        }
    }

    #[test]
    fn test_review_reference_files_are_removed() {
        // The language guidance was migrated into builtin/validators/<lang>; the
        // source reference files must no longer exist.
        let refs = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../builtin/skills/review/references");
        for file in [
            "RUST_REVIEW.md",
            "PYTHON_REVIEW.md",
            "JS_TS_REVIEW.md",
            "DART_FLUTTER_REVIEW.md",
        ] {
            let path = refs.join(file);
            assert!(
                !path.exists(),
                "{} should have been removed after migration",
                path.display()
            );
        }
    }
}
