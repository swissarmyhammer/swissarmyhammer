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
/// use std::path::Path;
///
/// let mut loader = ValidatorLoader::new();
/// load_builtins(&mut loader);
///
/// // Now load user/project validators for one workspace, which override builtins
/// loader.load_all(Some(Path::new("/path/to/workspace"))).ok();
/// ```
pub fn load_builtins(loader: &mut ValidatorLoader) {
    // First load YAML includes so @references work
    for (name, content) in get_builtin_includes() {
        if let Err(e) = loader.add_builtin_include(name, content) {
            tracing::warn!("Failed to load builtin include '{}': {}", name, e);
        }
    }

    if let Err(e) =
        loader.load_rulesets_directory(&builtin_validators_dir(), ValidatorSource::Builtin)
    {
        tracing::error!("Failed to load builtin RuleSets: {}", e);
    }
}

/// The directory the BUILTIN validator sets are loaded from at runtime, and so
/// the root every builtin `fixtures/` directory stands under.
///
/// It is `<repository>/builtin/validators`, resolved from `CARGO_MANIFEST_DIR`
/// at COMPILE time — an absolute path into the source checkout this engine was
/// built from, not a copy beside the binary and not a `sah init` snapshot under
/// the user's home. Two consequences are worth stating, because the review
/// scope stage compares changed files against this root
/// ([`ValidatorLoader::fixture_dirs`](crate::validators::ValidatorLoader::fixture_dirs)):
///
/// - Reviewing THIS repository, a changed `builtin/validators/*/fixtures/*`
///   file resolves under this root, so it is excluded as fixture data.
/// - Reviewing any other repository, no file of it can stand under this root,
///   so the builtin roots exclude nothing there.
pub fn builtin_validators_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators")
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

    /// The probe-bearing focused validators that kept their own dedicated
    /// probe and stayed standalone (folding either into a merged set would
    /// force its probe onto every other rule in that set). Probe names must be
    /// real catalog entries (`duplicates` / `similar` / `callers` /
    /// `complexity`); never `search_symbol` or `get_blastradius`.
    const PROBE_VALIDATORS: &[(&str, &str)] =
        &[("duplication", "duplicates"), ("reuse", "similar")];

    /// The two sets the nine single-rule builtin validators were merged into:
    /// `code-security` (no probes) and `code-hygiene` (`probes: [callers,
    /// complexity]`, needed by the `dead-code` and `cognitive-complexity` rules
    /// bundled inside it). See [`test_code_security_loads_with_three_rules_and_no_probes`]
    /// and [`test_code_hygiene_loads_its_rule_roster_and_callers_and_complexity_probes`].
    const MERGED_VALIDATORS: &[&str] = &["code-security", "code-hygiene"];

    /// The nine single-rule builtin validators retired by the code-security /
    /// code-hygiene merge. The loader must no longer report any of these as a
    /// standalone RuleSet from the builtin layer.
    const RETIRED_VALIDATOR_NAMES: &[&str] = &[
        "no-secrets",
        "injection",
        "command-safety",
        "no-commented-code",
        "function-length",
        "complexity",
        "missing-docs",
        "data-driven",
        "dead-code",
    ];

    /// The focused review-time integrity validator migrated from the old
    /// multi-rule `security-rules` and `test-integrity` sets, with no
    /// `trigger`. Its `no-secrets` / `injection` / `command-safety` siblings
    /// were later merged into `code-security` (see [`MERGED_VALIDATORS`]);
    /// `test-integrity` also matches `@file_groups/test_files`, so it cannot
    /// merge with a source-only set and stayed whole.
    const SAFETY_VALIDATORS: &[&str] = &["test-integrity"];

    /// Language-scoped review validators migrated from the skill's
    /// `references/*_REVIEW.md` files. Each entry is
    /// `(validator name, a file it MUST match, a file it MUST NOT match)`.
    /// Every one is in-file (no probes) and file-triggered (no tool match).
    const LANGUAGE_VALIDATORS: &[(&str, &str, &str)] = &[
        ("rust", "src/main.rs", "src/main.py"),
        ("python", "src/app.py", "src/app.rs"),
        ("js-ts", "src/index.ts", "src/index.rs"),
        ("dart", "lib/widget.dart", "lib/widget.rs"),
        (
            "swift",
            "Sources/App/Feature.swift",
            "Sources/App/Feature.rs",
        ),
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
    fn test_focused_validators_have_clean_manifest_frontmatter() {
        use crate::validators::parser::check_manifest_frontmatter;

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");

        let names = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(MERGED_VALIDATORS.iter().copied())
            .chain(std::iter::once(MANIFESTS_VALIDATOR));

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
    // The code-security / code-hygiene merge
    // ========================================================================

    /// `code-security` carries exactly the three merged security rules and
    /// declares no probes — each of `no-secrets`, `injection`, and
    /// `command-safety` was already an in-file judgment.
    #[test]
    fn test_code_security_loads_with_three_rules_and_no_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("code-security")
            .expect("code-security should be loaded");

        assert!(
            ruleset.manifest.probes.is_empty(),
            "code-security must declare no probes, got: {:?}",
            ruleset.manifest.probes
        );

        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            ruleset.rules.len(),
            3,
            "code-security should carry exactly 3 rules, got: {rule_names:?}"
        );
        for expected in ["no-secrets", "injection", "command-safety"] {
            assert!(
                rule_names.contains(&expected),
                "code-security should carry the {expected} rule, got: {rule_names:?}"
            );
        }
    }

    /// The prompt rules `code-hygiene` carries — the merged hygiene set.
    const CODE_HYGIENE_PROMPT_RULES: &[&str] = &[
        "no-commented-code",
        "function-length",
        "cognitive-complexity",
        "missing-docs",
        "data-driven",
        "magic-numbers",
        "dead-code",
    ];

    /// The documentation tool rules `code-hygiene` carries. Each supersedes the
    /// `missing-docs` prompt rule for the language it serves.
    const CODE_HYGIENE_MISSING_DOCS_TOOL_RULES: &[&str] = &[
        "missing-docs-rust",
        "missing-docs-python",
        "missing-docs-typescript",
        "missing-docs-go",
        "missing-docs-swift",
        "missing-docs-dart",
    ];

    /// The dead-code tool rules `code-hygiene` carries. Each supersedes the
    /// `dead-code` prompt rule for the language it serves.
    ///
    /// Three of that rule's four carve-outs are compiler behavior — an exported
    /// item, a test, and an entry point are exempt because the compiler can see
    /// which callers exist and which cannot. The fourth, work-in-process
    /// scaffolding, is an annotation contract: staged code carries the
    /// language's own suppression marker with a reason, or it is dead.
    const CODE_HYGIENE_DEAD_CODE_TOOL_RULES: &[&str] = &[
        "dead-code-rust",
        "dead-code-go",
        "dead-code-typescript",
        "dead-code-python",
        "dead-code-dart",
        "dead-code-swift",
    ];

    /// The magic-number tool rules `code-hygiene` carries. Each supersedes the
    /// `magic-numbers` prompt rule for the language it serves. Rust has no rule
    /// here: its one lint is an unpublished dylint example crate that builds
    /// from a git checkout against a pinned nightly toolchain, so Rust keeps
    /// the prompt rule.
    const CODE_HYGIENE_MAGIC_NUMBERS_TOOL_RULES: &[&str] = &[
        "magic-numbers-python",
        "magic-numbers-typescript",
        "magic-numbers-go",
        "magic-numbers-swift",
        "magic-numbers-dart",
    ];

    /// The naming tool rules `code-hygiene` carries. Each supersedes nothing:
    /// no shipped prompt rule reads a Go NAME, so there is no prompt rule to
    /// replace and no fallback to degrade to.
    ///
    /// `stuttering-name-go` runs the same revive `exported` rule
    /// `missing-docs-go` runs and owns the other half of it — the `naming`
    /// category, an exported name that opens with the name of its own package.
    const CODE_HYGIENE_NAMING_TOOL_RULES: &[&str] = &["stuttering-name-go"];

    /// The complexity tool rules `code-hygiene` carries, each paired with the
    /// prompt rules it supersedes.
    ///
    /// This is the one group whose `supersedes` differs per rule, so each row
    /// carries its own list. One run decides both gates for Rust, TypeScript
    /// and Swift, so those rules replace both prompt rules; Python and Go name
    /// one tool for each gate, so each takes one rule for each. Dart keeps the
    /// `complexity` probe and both prompt rules, because its only metrics tool
    /// is commercial.
    const CODE_HYGIENE_COMPLEXITY_TOOL_RULES: &[(&str, &[&str])] = &[
        (
            "complexity-rust",
            &["cognitive-complexity", "function-length"],
        ),
        ("complexity-python", &["cognitive-complexity"]),
        ("function-length-python", &["function-length"]),
        (
            "complexity-typescript",
            &["cognitive-complexity", "function-length"],
        ),
        (
            "complexity-swift",
            &["cognitive-complexity", "function-length"],
        ),
        ("complexity-go", &["cognitive-complexity"]),
        ("function-length-go", &["function-length"]),
    ];

    /// `code-hygiene` carries exactly its prompt rules plus its tool rules, and
    /// declares `probes: [callers, complexity]` — `callers` for `dead-code`,
    /// `complexity` for `cognitive-complexity` (the rest are in-file
    /// judgments that need no probe).
    #[test]
    fn test_code_hygiene_loads_its_rule_roster_and_callers_and_complexity_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("code-hygiene")
            .expect("code-hygiene should be loaded");

        assert_eq!(
            ruleset.manifest.probes,
            vec!["callers".to_string(), "complexity".to_string()],
            "code-hygiene should declare exactly [callers, complexity], got: {:?}",
            ruleset.manifest.probes
        );
        for declared in &ruleset.manifest.probes {
            assert!(
                crate::review::probe_exists(declared),
                "code-hygiene declares probe '{declared}' which is not in the catalog"
            );
        }

        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        let expected_rules = CODE_HYGIENE_PROMPT_RULES
            .iter()
            .chain(CODE_HYGIENE_MISSING_DOCS_TOOL_RULES.iter())
            .chain(CODE_HYGIENE_DEAD_CODE_TOOL_RULES.iter())
            .chain(CODE_HYGIENE_MAGIC_NUMBERS_TOOL_RULES.iter())
            .chain(CODE_HYGIENE_NAMING_TOOL_RULES.iter())
            .chain(
                CODE_HYGIENE_COMPLEXITY_TOOL_RULES
                    .iter()
                    .map(|(name, _)| name),
            );
        assert_eq!(
            ruleset.rules.len(),
            CODE_HYGIENE_PROMPT_RULES.len()
                + CODE_HYGIENE_MISSING_DOCS_TOOL_RULES.len()
                + CODE_HYGIENE_DEAD_CODE_TOOL_RULES.len()
                + CODE_HYGIENE_MAGIC_NUMBERS_TOOL_RULES.len()
                + CODE_HYGIENE_NAMING_TOOL_RULES.len()
                + CODE_HYGIENE_COMPLEXITY_TOOL_RULES.len(),
            "code-hygiene should carry exactly its prompt and tool rules, got: {rule_names:?}"
        );
        for expected in expected_rules {
            assert!(
                rule_names.contains(expected),
                "code-hygiene should carry the {expected} rule, got: {rule_names:?}"
            );
        }

        // Each tool rule carries a tool block, and supersedes exactly what its
        // group promises: the documentation tools replace the `missing-docs`
        // prompt rule, the magic-number tools replace the `magic-numbers`
        // prompt rule, the dead-code tools replace the `dead-code` prompt rule,
        // and each complexity tool replaces the gates its own tool decides.
        let expected_supersedes = CODE_HYGIENE_MISSING_DOCS_TOOL_RULES
            .iter()
            .map(|name| (name, ["missing-docs"].as_slice()))
            .chain(
                CODE_HYGIENE_DEAD_CODE_TOOL_RULES
                    .iter()
                    .map(|name| (name, ["dead-code"].as_slice())),
            )
            .chain(
                CODE_HYGIENE_MAGIC_NUMBERS_TOOL_RULES
                    .iter()
                    .map(|name| (name, ["magic-numbers"].as_slice())),
            )
            .chain(
                CODE_HYGIENE_NAMING_TOOL_RULES
                    .iter()
                    .map(|name| (name, [].as_slice())),
            )
            .chain(
                CODE_HYGIENE_COMPLEXITY_TOOL_RULES
                    .iter()
                    .map(|(name, superseded)| (name, *superseded)),
            );
        for (tool_rule_name, superseded) in expected_supersedes {
            let tool_rule = ruleset
                .rules
                .iter()
                .find(|rule| rule.name == **tool_rule_name)
                .unwrap_or_else(|| panic!("{tool_rule_name} should be loaded"));
            assert!(
                tool_rule.tool.is_some(),
                "{tool_rule_name} must carry a tool block, or it is a prompt rule"
            );
            assert_eq!(
                tool_rule.supersedes.names(),
                superseded,
                "{tool_rule_name} must supersede {superseded:?}"
            );
        }
    }

    // ========================================================================
    // The manifests set
    // ========================================================================

    /// The builtin set that matches dependency manifests instead of source
    /// code.
    ///
    /// `code-hygiene` matches `@file_groups/source_code`, which declares no
    /// manifest pattern, so a finding naming a `Cargo.toml` is dropped there on
    /// every path through the engine. This set is where such a rule reports.
    const MANIFESTS_VALIDATOR: &str = "manifests";

    /// The tool rules `manifests` carries. Each supersedes nothing: no shipped
    /// prompt rule asks whether a declared dependency is used, so there is no
    /// prompt rule to replace and no fallback to degrade to.
    const MANIFESTS_TOOL_RULES: &[&str] = &["unused-dependencies-rust"];

    /// A manifest path the set must match: the one at the repository root.
    const ROOT_MANIFEST_PATH: &str = "Cargo.toml";

    /// A manifest path the set must match: a workspace member's.
    const MEMBER_MANIFEST_PATH: &str = "crates/swissarmyhammer-fields/Cargo.toml";

    /// `manifests` carries exactly its tool rules, declares no probes, and each
    /// rule carries a tool block and supersedes nothing.
    ///
    /// The empty `supersedes` is the load-bearing assertion. A tool rule that
    /// names a prompt rule silences that rule for the files it matches; this
    /// one replaces nothing, so a machine without `cargo machete` simply gets no
    /// answer to the dependency question rather than a degraded one.
    #[test]
    fn test_manifests_loads_its_tool_rules_with_no_probes_and_no_supersedes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset(MANIFESTS_VALIDATOR)
            .expect("manifests should be loaded");

        assert!(
            ruleset.manifest.probes.is_empty(),
            "manifests must declare no probes, got: {:?}",
            ruleset.manifest.probes
        );

        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            ruleset.rules.len(),
            MANIFESTS_TOOL_RULES.len(),
            "manifests should carry exactly its tool rules, got: {rule_names:?}"
        );
        for expected in MANIFESTS_TOOL_RULES {
            let rule = ruleset
                .rules
                .iter()
                .find(|rule| rule.name == *expected)
                .unwrap_or_else(|| {
                    panic!("manifests should carry the {expected} rule, got: {rule_names:?}")
                });
            assert!(
                rule.tool.is_some(),
                "{expected} must carry a tool block, or it is a prompt rule"
            );
            assert!(
                rule.supersedes.names().is_empty(),
                "{expected} must supersede nothing, got: {:?}",
                rule.supersedes.names()
            );
        }
    }

    /// The set fires on the manifest at the repository root and on a workspace
    /// member's manifest, and on nothing else.
    ///
    /// One pattern, `**/Cargo.toml`, carries both. The engine compiles file
    /// patterns under `require_literal_separator: false`, so a leading `**/`
    /// matches no directory as readily as several. A bare `Cargo.toml` literal
    /// would reach the root manifest alone, which is why it is not the pattern.
    #[test]
    fn test_manifests_matches_root_and_member_manifests_only() {
        use crate::validators::types::MatchContext;

        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset(MANIFESTS_VALIDATOR)
            .expect("manifests should be loaded");

        for manifest in [ROOT_MANIFEST_PATH, MEMBER_MANIFEST_PATH] {
            let yes = MatchContext::new().with_file(manifest);
            assert!(
                ruleset.matches(&yes),
                "manifests should match the changed manifest '{manifest}'"
            );
        }

        for other in ["src/app.py", "crates/swissarmyhammer-fields/src/lib.rs"] {
            let no = MatchContext::new().with_file(other);
            assert!(
                !ruleset.matches(&no),
                "manifests should NOT match the non-manifest file '{other}'"
            );
        }
    }

    /// The nine single-rule sets folded into code-security/code-hygiene must
    /// no longer load standalone from the builtin layer.
    #[test]
    fn test_retired_single_rule_validators_no_longer_load() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for retired in RETIRED_VALIDATOR_NAMES {
            assert!(
                loader.get_ruleset(retired).is_none(),
                "the retired `{retired}` set must no longer load standalone from the builtin \
                 layer; its rule was merged into code-security or code-hygiene"
            );
        }
    }

    /// Both merged sets are file-triggered over source code, exactly like the
    /// nine single-rule sets they replaced.
    #[test]
    fn test_code_security_and_code_hygiene_match_expected_paths() {
        use crate::validators::types::MatchContext;

        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for name in MERGED_VALIDATORS {
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

    /// The merge must not disturb the three sets that were deliberately left
    /// out of it: `test-integrity` (also matches `@file_groups/test_files`),
    /// and `reuse`/`duplication` (each carries its own dedicated probe).
    #[test]
    fn test_test_integrity_reuse_and_duplication_are_unaffected_by_the_merge() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let test_integrity = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should still load unchanged");
        assert_eq!(
            test_integrity.rules.len(),
            2,
            "test-integrity should still carry its no-hard-code + no-test-cheating rules"
        );

        let reuse = loader
            .get_ruleset("reuse")
            .expect("reuse should still load unchanged");
        assert_eq!(reuse.manifest.probes, vec!["similar".to_string()]);
        assert_eq!(reuse.rules.len(), 1);

        let duplication = loader
            .get_ruleset("duplication")
            .expect("duplication should still load unchanged");
        assert_eq!(duplication.manifest.probes, vec!["duplicates".to_string()]);
        // `duplication`, `rust` and `swift`.
        assert_eq!(duplication.rules.len(), 3);
    }

    /// Every probe the `completeness` ruleset declares, beside the rule that
    /// reads its rows.
    ///
    /// One row per probe, so declaring a probe and reading it stay one edit.
    const COMPLETENESS_PROBE_READERS: &[(&str, &str)] = &[
        ("inverse-pairs", "inverse-operation-coverage"),
        ("public-surface", "public-output-contract"),
        ("clone-siblings", "invariant-propagation"),
    ];

    /// `completeness` declares exactly the probes [`COMPLETENESS_PROBE_READERS`]
    /// names, each of them is in the catalog, and the rule that reads each one
    /// is in the ruleset. The three halves are asserted together because each is
    /// useless without the others: a declaration no probe answers to, or a probe
    /// no rule reads, leaves the rule judging from prose alone.
    #[test]
    fn test_completeness_declares_exactly_the_probes_its_rules_read() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let completeness = loader
            .get_ruleset("completeness")
            .expect("completeness should be loaded");

        let declared: Vec<String> = COMPLETENESS_PROBE_READERS
            .iter()
            .map(|(probe, _reader)| (*probe).to_string())
            .collect();
        assert_eq!(
            completeness.manifest.probes, declared,
            "completeness should declare exactly {declared:?}, got: {:?}",
            completeness.manifest.probes
        );

        let rule_names: Vec<&str> = completeness
            .rules
            .iter()
            .map(|rule| rule.name.as_str())
            .collect();
        for (probe, reader) in COMPLETENESS_PROBE_READERS {
            assert!(
                crate::review::probe_exists(probe),
                "the probe completeness declares must be in the catalog: {probe}"
            );
            assert!(
                rule_names.contains(reader),
                "the rule that reads `{probe}`'s rows must be in the ruleset, got: {rule_names:?}"
            );
        }
    }

    // ========================================================================
    // Focused safety/integrity validators (migrated from security-rules /
    // test-integrity multi-rule sets into focused review-time validators)
    // ========================================================================

    #[test]
    fn test_safety_validators_load_with_catalog_probes() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        for name in SAFETY_VALIDATORS {
            let ruleset = loader
                .get_ruleset(name)
                .unwrap_or_else(|| panic!("safety validator '{name}' should be loaded"));
            assert_eq!(ruleset.name(), *name);

            // Whatever it declares must be a real catalog name.
            for declared in &ruleset.manifest.probes {
                assert!(
                    crate::review::probe_exists(declared),
                    "{name} declares probe '{declared}' which is not in the catalog"
                );
            }

            // Each carries at least one rule.
            assert!(
                !ruleset.rules.is_empty(),
                "{name} should carry at least one rule"
            );
        }
    }

    /// `test-integrity` declares the `assertion-census` probe, and its
    /// `no-test-cheating` rule is what reads those rows. The three halves are
    /// asserted together because each is useless without the others: a
    /// declaration no probe answers to, or a probe no rule reads, leaves the
    /// rule counting assertions from prose alone.
    #[test]
    fn test_test_integrity_declares_the_assertion_census_probe_its_rule_reads() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let test_integrity = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        assert_eq!(
            test_integrity.manifest.probes,
            vec!["assertion-census".to_string()],
            "test-integrity should declare exactly [assertion-census], got: {:?}",
            test_integrity.manifest.probes
        );
        assert!(
            crate::review::probe_exists("assertion-census"),
            "the probe test-integrity declares must be in the catalog"
        );
        let census_rule = test_integrity
            .rules
            .iter()
            .find(|rule| rule.name == "no-test-cheating")
            .expect("the rule that reads the probe's rows must be in the ruleset");
        assert!(
            census_rule.body.contains("assertion-census"),
            "no-test-cheating must read the probe's rows by name, got: {}",
            census_rule.body
        );
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
    fn test_swift_casing_accepts_both_acronym_spellings() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("swift")
            .expect("swift validator should be loaded");

        let casing_rule = ruleset
            .rules
            .iter()
            .find(|r| r.name == "casing")
            .expect("swift validator should carry a casing rule");

        assert!(
            casing_rule.body.contains("BOTH accepted"),
            "casing rule should state both acronym spellings are accepted, got: {}",
            casing_rule.body
        );
        assert!(
            !casing_rule.body.contains("are all wrong"),
            "casing rule should no longer flag Url/Json spellings as wrong, got: {}",
            casing_rule.body
        );
        assert!(
            !casing_rule.body.contains("DON'T: `entryId`"),
            "casing rule should no longer retain the retired entryId DON'T bullet, got: {}",
            casing_rule.body
        );
        assert!(
            !casing_rule.body.contains("flag toward the uniform form"),
            "casing rule should no longer retain the flag-toward-uniform tiebreaker, got: {}",
            casing_rule.body
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
    fn test_builtin_rulesets_carry_their_validator_md_body() {
        // The VALIDATOR.md prose body is authored validator-wide guidance and
        // must survive the builtin load path (not just the on-disk loader).
        // Assert on the permanent `# <Name> Validator` heading of each body so
        // the test stays hermetic against user revisions to the prose.
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let (name, heading) = ("duplication", "Duplication Validator");
        let ruleset = loader
            .get_ruleset(name)
            .unwrap_or_else(|| panic!("{name} should be loaded"));
        assert!(
            !ruleset.manifest_body().is_empty(),
            "{name} should carry a non-empty VALIDATOR.md body, got: {:?}",
            ruleset.manifest_body()
        );
        assert!(
            ruleset.manifest_body().contains(heading),
            "{name} VALIDATOR.md body should carry its {heading:?} heading, got: {:?}",
            ruleset.manifest_body()
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

        // Check for expected RuleSets. The nine single-rule sets (no-secrets,
        // injection, command-safety, no-commented-code, function-length,
        // complexity, missing-docs, data-driven, dead-code) were merged into
        // code-security / code-hygiene.
        assert!(
            loader.get_ruleset("code-security").is_some(),
            "Should have the merged code-security validator"
        );
        // The monolithic code-quality set was split into focused validators.
        assert!(
            loader.get_ruleset("duplication").is_some(),
            "Should have the focused duplication validator"
        );
    }

    #[test]
    fn test_rehomed_quality_validators_load() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // The nine code-quality concerns were re-homed/split into focused
        // validators; two (duplication, reuse) kept their own probe and
        // stayed standalone, and the other seven were merged into
        // code-security/code-hygiene. Each loads as its own RuleSet with at
        // least one rule.
        let focused = PROBE_VALIDATORS
            .iter()
            .map(|(name, _)| *name)
            .chain(MERGED_VALIDATORS.iter().copied());
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
    fn test_code_security_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("code-security")
            .expect("code-security should be loaded");

        // The @file_groups/source_code should have been expanded in the manifest
        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("code-security should have match criteria");

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
            .chain(MERGED_VALIDATORS.iter().copied());
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
            .chain(MERGED_VALIDATORS.iter().copied());
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
