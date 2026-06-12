//! Builtin validator assets embedded in the binary for the profile installer.
//!
//! The build script ([`build.rs`]) embeds every file under `builtin/validators/`
//! (each set's `VALIDATOR.md` plus its `rules/*.md`) as `(name, content)` tuples,
//! where `name` is the path relative to `builtin/validators/` with its real
//! filename preserved (e.g. `dead-code/VALIDATOR.md`,
//! `dead-code/rules/dead-code.md`).
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
        // The monolithic security-rules set was split into the focused
        // no-secrets / injection / command-safety validators.
        for expected in ["duplication", "no-secrets", "test-integrity"] {
            assert!(
                sets.contains_key(expected),
                "embedded builtins must include the `{expected}` set, got: {:?}",
                sets.keys().collect::<Vec<_>>()
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
