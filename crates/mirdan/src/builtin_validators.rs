//! Builtin validator assets embedded in the binary for the profile installer.
//!
//! The build script ([`build.rs`]) embeds every file under `builtin/validators/`
//! — each set's `VALIDATOR.md`, its `rules/*.md`, and its tool-rule fixtures in
//! whatever language the tool lints — as `(name, content)` tuples, where `name`
//! is the path relative to `builtin/validators/` with its real filename
//! preserved (e.g. `dead-code/VALIDATOR.md`, `dead-code/rules/dead-code.md`,
//! `code-hygiene/fixtures/missing-docs-rust.fail.rs`).
//!
//! This mirrors how `swissarmyhammer-skills` embeds `builtin/skills/`: the
//! profile installer materializes these onto disk in the validators store
//! (`~/.validators/` global or `./.validators/` project) so users can read,
//! learn from, and copy them. The validator *loader* still
//! reads the embedded set at lowest precedence; this on-disk copy is the
//! read-only reference, refreshed on every install.

// Include the generated `get_builtin_validators()` accessor.
include!(concat!(env!("OUT_DIR"), "/builtin_validators.rs"));

/// The top-level set name for an embedded validator file path.
///
/// Embedded names are `<set>/...` (e.g. `dead-code/VALIDATOR.md`); the set is
/// the first path segment. A name with no `/` is its own set name.
pub fn set_name(embedded_name: &str) -> &str {
    embedded_name
        .split_once('/')
        .map_or(embedded_name, |(set, _)| set)
}

/// Group the embedded builtin validators by set name.
///
/// Returns a `set → [(relative_path, content)]` map where `relative_path` is the
/// embedded name (still set-prefixed). The set ordering is by name; this is the
/// shape the installer's [`Selector`](crate::install::Selector) resolves against
/// (set name → membership tags, validators carry none).
pub fn builtin_validators_by_set(
) -> std::collections::BTreeMap<&'static str, Vec<(&'static str, &'static str)>> {
    let mut sets: std::collections::BTreeMap<&'static str, Vec<(&'static str, &'static str)>> =
        std::collections::BTreeMap::new();
    for (name, content) in get_builtin_validators() {
        // Skip top-level files that are not part of a set subdirectory — a real
        // set member is always `<set>/...`. The store's own `README.md` (a
        // discovery doc deployed to `.validators/`, not a validator) lives at the
        // top level, so this keeps it from forming a phantom, manifest-less set.
        if !name.contains('/') {
            continue;
        }
        sets.entry(set_name(name))
            .or_default()
            .push((name, content));
    }
    sets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_name_extracts_first_segment() {
        assert_eq!(set_name("duplication/VALIDATOR.md"), "duplication");
        assert_eq!(set_name("dead-code/rules/dead-code.md"), "dead-code");
        assert_eq!(set_name("loose.md"), "loose.md");
    }

    #[test]
    fn test_builtin_validators_embed_expected_sets() {
        let sets = builtin_validators_by_set();
        // The nine single-rule sets (no-secrets, injection, command-safety,
        // no-commented-code, function-length, complexity, missing-docs,
        // data-driven, dead-code) were merged into code-security and
        // code-hygiene. duplication/reuse/test-integrity keep their own probe
        // (or test-file match) and were left whole.
        for expected in [
            "code-security",
            "code-hygiene",
            "duplication",
            "reuse",
            "test-integrity",
        ] {
            assert!(
                sets.contains_key(expected),
                "embedded builtins must include the `{expected}` set, got: {:?}",
                sets.keys().collect::<Vec<_>>()
            );
        }

        for retired in [
            "no-secrets",
            "injection",
            "command-safety",
            "no-commented-code",
            "function-length",
            "complexity",
            "missing-docs",
            "data-driven",
            "dead-code",
        ] {
            assert!(
                !sets.contains_key(retired),
                "embedded builtins must no longer include the retired `{retired}` set, got: {:?}",
                sets.keys().collect::<Vec<_>>()
            );
        }

        let code_security_files = &sets["code-security"];
        for expected_rule in ["no-secrets.md", "injection.md", "command-safety.md"] {
            assert!(
                code_security_files
                    .iter()
                    .any(|(name, _)| *name == format!("code-security/rules/{expected_rule}")),
                "code-security must embed the moved rule `rules/{expected_rule}`, got: {:?}",
                code_security_files
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            );
        }

        let code_hygiene_files = &sets["code-hygiene"];
        for expected_rule in [
            "no-commented-code.md",
            "function-length.md",
            "cognitive-complexity.md",
            "missing-docs.md",
            "data-driven.md",
            "dead-code.md",
        ] {
            assert!(
                code_hygiene_files
                    .iter()
                    .any(|(name, _)| *name == format!("code-hygiene/rules/{expected_rule}")),
                "code-hygiene must embed the moved rule `rules/{expected_rule}`, got: {:?}",
                code_hygiene_files
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            );
        }
    }

    /// The build artifact directory a fixture's own tool writes beside it. The
    /// build script skips it, so the guard below must skip it too.
    const BUILD_ARTIFACT_DIR: &str = "target";

    /// Every file of a validator set reaches the store, not only its markdown.
    ///
    /// A tool rule's fixtures are source files in whatever language the tool
    /// lints. A fixture left out of the embed makes doctor report the rule as
    /// fixture-less, and the rule silently falls back to its prompt rule.
    #[test]
    fn test_every_builtin_validator_file_is_embedded() {
        let source_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators");
        let embedded: std::collections::BTreeSet<&str> = get_builtin_validators()
            .into_iter()
            .map(|(name, _)| name)
            .collect();

        let mut missing = Vec::new();
        let mut pending = vec![source_dir.clone()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).expect("read validators dir") {
                let path = entry.expect("read dir entry").path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n == BUILD_ARTIFACT_DIR) {
                        continue;
                    }
                    pending.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&source_dir)
                    .expect("every file is under the source dir")
                    .to_string_lossy()
                    .to_string();
                if !embedded.contains(relative.as_str()) {
                    missing.push(relative);
                }
            }
        }

        assert!(
            missing.is_empty(),
            "these builtin validator files are not embedded, so `sah init` never writes them: {missing:?}"
        );
    }

    /// The shipped tool rules' fixtures reach the store, so doctor can prove
    /// each rule healthy in an installed project.
    #[test]
    fn test_tool_rule_fixtures_are_embedded() {
        let sets = builtin_validators_by_set();
        let code_hygiene_files = &sets["code-hygiene"];

        for fixture in [
            "missing-docs-rust.fail.rs",
            "missing-docs-rust.pass.rs",
            "missing-docs-python.fail.py",
            "missing-docs-python.pass.py",
            "missing-docs-typescript.fail.ts",
            "missing-docs-typescript.pass.ts",
            "missing-docs-go.fail.go",
            "missing-docs-go.pass.go",
            "missing-docs-swift.fail.swift",
            "missing-docs-swift.pass.swift",
            "missing-docs-dart.fail.dart",
            "missing-docs-dart.pass.dart",
        ] {
            assert!(
                code_hygiene_files
                    .iter()
                    .any(|(name, _)| *name == format!("code-hygiene/fixtures/{fixture}")),
                "code-hygiene must embed the tool-rule fixture `fixtures/{fixture}`, got: {:?}",
                code_hygiene_files
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_each_set_has_a_manifest() {
        for (set, files) in builtin_validators_by_set() {
            assert!(
                files
                    .iter()
                    .any(|(name, _)| name.ends_with("/VALIDATOR.md")),
                "set `{set}` must embed a VALIDATOR.md"
            );
        }
    }
}
