//! Builtin validator assets embedded in the binary for the profile installer.
//!
//! The build script ([`build.rs`]) embeds every file under `builtin/validators/`
//! — each set's `VALIDATOR.md`, its `rules/*.md`, and its tool-rule fixtures in
//! whatever language the tool lints — as `(name, content)` tuples, where `name`
//! is the path relative to `builtin/validators/` with its real filename
//! preserved (e.g. `dead-code/VALIDATOR.md`, `dead-code/rules/dead-code.md`,
//! `code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl`).
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
        // (or test-file match) and were left whole. `manifests` matches
        // `**/Cargo.toml` rather than source code, which is why it is its own
        // set and not a rule inside code-hygiene.
        for expected in [
            "code-security",
            "code-hygiene",
            "manifests",
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
    /// build script skips it, so the guards below must skip it too.
    const BUILD_ARTIFACT_DIR: &str = "target";

    /// The directory the build script embeds, and the source of truth every
    /// guard below is held against.
    fn builtin_validators_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../builtin/validators")
    }

    /// The filename of every fixture the `set` ships on disk.
    ///
    /// Reads the set's `fixtures/` directory, which is what the build script
    /// embeds, so the answer is the set of fixtures a deployed store receives.
    /// A roster below names files, so this rejects a nested layout it could
    /// never name.
    ///
    /// The name is taken as the directory stores it. The embedded name carries
    /// that spelling through to the store, so a roster entry must match it
    /// character for character; a match that ignored case would pass an entry
    /// the store never writes under that spelling.
    fn fixture_filenames_on_disk(set: &str) -> std::collections::BTreeSet<String> {
        let fixtures_dir = builtin_validators_dir().join(set).join("fixtures");
        let entries = std::fs::read_dir(&fixtures_dir).unwrap_or_else(|error| {
            panic!(
                "cannot read the fixtures directory {}: {error}",
                fixtures_dir.display()
            )
        });

        let mut filenames = std::collections::BTreeSet::new();
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!(
                        "cannot read an entry of {}: {error}",
                        fixtures_dir.display()
                    )
                })
                .path();
            let name = path
                .file_name()
                .expect("every directory entry has a file name")
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                assert_eq!(
                    name, BUILD_ARTIFACT_DIR,
                    "a fixtures directory holds fixture files and the \
                     `{BUILD_ARTIFACT_DIR}` build artifact directory alone; \
                     `{set}/fixtures/{name}` is neither, and a flat roster \
                     cannot name what it holds"
                );
                continue;
            }
            filenames.insert(name);
        }
        filenames
    }

    /// Hold `subject` to naming nothing `reference` lacks.
    ///
    /// Fails with `complaint` and every name the two disagree on, so the
    /// failure names the files rather than a count.
    fn assert_no_names_outside(
        subject: &std::collections::BTreeSet<&str>,
        reference: &std::collections::BTreeSet<&str>,
        complaint: &str,
    ) {
        let deviating: Vec<&str> = subject.difference(reference).copied().collect();
        assert!(deviating.is_empty(), "{complaint}: {deviating:?}");
    }

    /// Every file of a validator set reaches the store, not only its markdown.
    ///
    /// A tool rule's fixtures are source files in whatever language the tool
    /// lints. A fixture left out of the embed makes doctor report the rule as
    /// fixture-less, and the rule silently falls back to its prompt rule.
    #[test]
    fn test_every_builtin_validator_file_is_embedded() {
        let source_dir = builtin_validators_dir();
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

    /// Every shipped fixture is a TEMPLATE.
    ///
    /// A fixture holds the defect its rule reports. Stored under a real source
    /// extension it becomes a file the review engine reviews, and the rule
    /// fires on the fixture built to make it fire. The `.tmpl` suffix keeps
    /// the stored file out of every language and every file group; the doctor
    /// strips it when it materializes the fixture for the tool.
    #[test]
    fn test_every_shipped_fixture_is_a_template() {
        let sets = builtin_validators_by_set();
        let stored: Vec<&str> = sets
            .values()
            .flatten()
            .map(|(name, _)| *name)
            .filter(|name| name.contains("/fixtures/"))
            .filter(|name| !name.ends_with(".tmpl"))
            .collect();

        assert!(
            stored.is_empty(),
            "a fixture stored under a real source extension is reviewed as source, \
             and its own rule reports it; rename each to `<name>.tmpl`: {stored:?}"
        );
    }

    /// The `manifests` fixtures. Two are Cargo manifests, because the rule
    /// reports a manifest; the `lib.rs` support file beside them is the source
    /// whose one named dependency makes the pass fixture pass.
    const MANIFESTS_FIXTURES: &[&str] = &[
        "unused-dependencies-rust.fail.toml.tmpl",
        "unused-dependencies-rust.pass.toml.tmpl",
        "lib.rs.tmpl",
    ];

    /// The `code-hygiene` fixtures: one fail/pass pair for each of its tool
    /// rules, then the shared package files.
    ///
    /// A `workspace`-scope tool reads a package, not a loose file, so the
    /// doctor stages the pair beside the package file of that language before
    /// it runs the tool. The package files carry no defect and belong to no one
    /// rule; every probe of their language needs them, so one copy stands here
    /// for all of them.
    ///
    /// This roster and the directory it names agree in both directions, held by
    /// [`test_fixture_rosters_and_the_fixtures_directory_agree`].
    const CODE_HYGIENE_FIXTURES: &[&str] = &[
        "missing-docs-rust.fail.rs.tmpl",
        "missing-docs-rust.pass.rs.tmpl",
        "missing-docs-python.fail.py.tmpl",
        "missing-docs-python.pass.py.tmpl",
        "missing-docs-typescript.fail.ts.tmpl",
        "missing-docs-typescript.pass.ts.tmpl",
        "missing-docs-go.fail.go.tmpl",
        "missing-docs-go.pass.go.tmpl",
        "missing-docs-swift.fail.swift.tmpl",
        "missing-docs-swift.pass.swift.tmpl",
        "missing-docs-dart.fail.dart.tmpl",
        "missing-docs-dart.pass.dart.tmpl",
        "dead-code-rust.fail.rs.tmpl",
        "dead-code-rust.pass.rs.tmpl",
        "dead-code-go.fail.go.tmpl",
        "dead-code-go.pass.go.tmpl",
        "dead-code-typescript.fail.ts.tmpl",
        "dead-code-typescript.pass.ts.tmpl",
        "dead-code-python.fail.py.tmpl",
        "dead-code-python.pass.py.tmpl",
        "dead-code-dart.fail.dart.tmpl",
        "dead-code-dart.pass.dart.tmpl",
        "dead-code-swift.fail.swift.tmpl",
        "dead-code-swift.pass.swift.tmpl",
        "magic-numbers-python.fail.py.tmpl",
        "magic-numbers-python.pass.py.tmpl",
        "magic-numbers-typescript.fail.ts.tmpl",
        "magic-numbers-typescript.pass.ts.tmpl",
        "magic-numbers-go.fail.go.tmpl",
        "magic-numbers-go.pass.go.tmpl",
        "magic-numbers-swift.fail.swift.tmpl",
        "magic-numbers-swift.pass.swift.tmpl",
        "magic-numbers-dart.fail.dart.tmpl",
        "magic-numbers-dart.pass.dart.tmpl",
        "function-length-rust.fail.rs.tmpl",
        "function-length-rust.pass.rs.tmpl",
        "function-length-python.fail.py.tmpl",
        "function-length-python.pass.py.tmpl",
        "function-length-typescript.fail.ts.tmpl",
        "function-length-typescript.pass.ts.tmpl",
        "function-length-swift.fail.swift.tmpl",
        "function-length-swift.pass.swift.tmpl",
        "function-length-go.fail.go.tmpl",
        "function-length-go.pass.go.tmpl",
        "function-length-dart.fail.dart.tmpl",
        "function-length-dart.pass.dart.tmpl",
        "stuttering-name-go.fail.go.tmpl",
        "stuttering-name-go.pass.go.tmpl",
        "Cargo.toml.tmpl",
        "Cargo.lock.tmpl",
        "lib.rs.tmpl",
        "pyproject.toml.tmpl",
        "tsconfig.json.tmpl",
        "go.mod.tmpl",
        "Package.swift.tmpl",
    ];

    /// Each set that ships tool-rule fixtures, paired with the roster naming
    /// them. Every set that ships a tool rule stands here. A set whose fixtures
    /// never reach the store has every one of its rules reported fixture-less,
    /// and each falls silently back to its prompt rule — or, for a rule that
    /// supersedes nothing, to no rule at all.
    const FIXTURE_ROSTERS: &[(&str, &[&str])] = &[
        ("code-hygiene", CODE_HYGIENE_FIXTURES),
        ("manifests", MANIFESTS_FIXTURES),
    ];

    /// Each fixture roster and the directory it stands for agree in BOTH
    /// directions.
    ///
    /// A roster is written by hand, so it drifts from the directory two ways,
    /// and a guard that only walks the roster sees neither. A fixture added on
    /// disk and left out of the roster stays unheld: every entry the roster
    /// does name exists, so the list is internally consistent and wrong only by
    /// omission. A roster entry whose file was renamed or deleted names
    /// nothing, and the rule it stands for loses its fixtures with no test to
    /// say so.
    #[test]
    fn test_fixture_rosters_and_the_fixtures_directory_agree() {
        for &(set, roster) in FIXTURE_ROSTERS {
            let filenames = fixture_filenames_on_disk(set);
            let on_disk: std::collections::BTreeSet<&str> =
                filenames.iter().map(String::as_str).collect();
            let listed: std::collections::BTreeSet<&str> = roster.iter().copied().collect();

            // Two empty sets agree, so a directory that read as empty would
            // carry this test to a pass having compared nothing.
            assert!(
                !on_disk.is_empty(),
                "`{set}/fixtures/` reads as empty, so the two comparisons below \
                 hold nothing and this test cannot fail"
            );

            assert_no_names_outside(
                &on_disk,
                &listed,
                &format!(
                    "`{set}` ships these fixtures on disk and no roster entry names \
                     them, so nothing holds them to reaching a deployed store"
                ),
            );
            assert_no_names_outside(
                &listed,
                &on_disk,
                &format!(
                    "the `{set}` roster names these fixtures and no file on disk \
                     answers them, so the rule they stand for has lost its fixtures"
                ),
            );
        }
    }

    /// The shipped tool rules' fixtures reach the store, so doctor can prove
    /// each rule healthy in an installed project.
    #[test]
    fn test_tool_rule_fixtures_are_embedded() {
        let sets = builtin_validators_by_set();

        for &(set, fixtures) in FIXTURE_ROSTERS {
            let files = &sets[set];
            for fixture in fixtures {
                assert!(
                    files
                        .iter()
                        .any(|(name, _)| *name == format!("{set}/fixtures/{fixture}")),
                    "{set} must embed the tool-rule fixture `fixtures/{fixture}`, got: {:?}",
                    files.iter().map(|(name, _)| name).collect::<Vec<_>>()
                );
            }
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
