//! Snapshot of builtin validator sets that have been retired (merged away or
//! deleted) from `builtin/validators/`, retained ONLY so a validators refresh
//! can detect and prune a retired set from the deployed store
//! (`~/.validators/` global or `./.validators/` project) when the user never
//! touched it.
//!
//! This module is deliberately isolated from `builtin/validators/` (the active
//! validator source tree): its snapshot files live under
//! `crates/mirdan/retired-validators/`, a sibling directory neither the
//! `swissarmyhammer-validators` RuleSet loader nor mirdan's own
//! `builtin_validators` build-time embed ever scans. A retired set must never
//! reappear as a loadable or installable validator — only as a fact this
//! module compares the deployed store against.
//!
//! # Reference-copy policy
//!
//! [`install::install_profile_validators`](crate::install) owns builtin-owned
//! files: every embedded *active* file is overwritten on each install/refresh.
//! A *retired* set is different — nothing re-materializes it — so the only
//! honest outcome for a retired set the user left untouched is deletion, and
//! the only honest outcome for one the user edited is to leave it alone
//! entirely. [`prune_unmodified_retired_sets`] is that comparison: exact
//! byte-for-byte match against the snapshot removes the set directory; any
//! difference (edited content, added file, removed file) leaves it in place.

use std::path::{Path, PathBuf};

use crate::store;

/// One file within a retired validator set's shipped snapshot.
#[derive(Debug, Clone, Copy)]
pub struct RetiredFile {
    /// Path relative to the set directory (e.g. `"VALIDATOR.md"`,
    /// `"rules/dead-code.md"`).
    pub relative_path: &'static str,
    /// The exact byte content shipped for this file before the set was
    /// retired.
    pub content: &'static str,
}

/// A retired builtin validator set: its name plus every file it shipped.
#[derive(Debug, Clone, Copy)]
pub struct RetiredSet {
    /// The set's directory name (e.g. `"dead-code"`).
    pub name: &'static str,
    /// Every file the set shipped, relative to the set directory.
    pub files: &'static [RetiredFile],
}

/// The nine single-rule builtin validator sets merged into `code-security` and
/// `code-hygiene`. Their rule text now lives, byte-identical, under the merged
/// sets' `rules/` directories; this table exists only so a stale, unmodified
/// copy of the pre-merge set can be pruned from a deployed store.
pub static RETIRED_VALIDATOR_SETS: &[RetiredSet] = &[
    RetiredSet {
        name: "no-secrets",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/no-secrets/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/no-secrets.md",
                content: include_str!("../retired-validators/no-secrets/rules/no-secrets.md"),
            },
        ],
    },
    RetiredSet {
        name: "injection",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/injection/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/injection.md",
                content: include_str!("../retired-validators/injection/rules/injection.md"),
            },
        ],
    },
    RetiredSet {
        name: "command-safety",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/command-safety/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/command-safety.md",
                content: include_str!(
                    "../retired-validators/command-safety/rules/command-safety.md"
                ),
            },
        ],
    },
    RetiredSet {
        name: "no-commented-code",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/no-commented-code/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/no-commented-code.md",
                content: include_str!(
                    "../retired-validators/no-commented-code/rules/no-commented-code.md"
                ),
            },
        ],
    },
    RetiredSet {
        name: "function-length",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/function-length/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/function-length.md",
                content: include_str!(
                    "../retired-validators/function-length/rules/function-length.md"
                ),
            },
        ],
    },
    RetiredSet {
        name: "complexity",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/complexity/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/cognitive-complexity.md",
                content: include_str!(
                    "../retired-validators/complexity/rules/cognitive-complexity.md"
                ),
            },
        ],
    },
    RetiredSet {
        name: "missing-docs",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/missing-docs/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/missing-docs.md",
                content: include_str!("../retired-validators/missing-docs/rules/missing-docs.md"),
            },
        ],
    },
    RetiredSet {
        name: "data-driven",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/data-driven/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/data-driven.md",
                content: include_str!("../retired-validators/data-driven/rules/data-driven.md"),
            },
        ],
    },
    RetiredSet {
        name: "dead-code",
        files: &[
            RetiredFile {
                relative_path: "VALIDATOR.md",
                content: include_str!("../retired-validators/dead-code/VALIDATOR.md"),
            },
            RetiredFile {
                relative_path: "rules/dead-code.md",
                content: include_str!("../retired-validators/dead-code/rules/dead-code.md"),
            },
        ],
    },
];

/// Remove every retired builtin validator set under `store_root` whose deployed
/// files are byte-identical to what was shipped before it was retired.
///
/// A set that was never deployed (directory absent) is a no-op. A set whose
/// deployed files differ in any way from the snapshot — edited content, an
/// added file, a removed file — is left untouched: that difference is the
/// user's own work, and this function's only job is detecting the *absence* of
/// any such difference.
///
/// Returns the names of the sets actually removed.
pub fn prune_unmodified_retired_sets(store_root: &Path) -> Vec<String> {
    let mut removed = Vec::new();
    for set in RETIRED_VALIDATOR_SETS {
        let set_dir = store_root.join(set.name);
        if !set_dir.is_dir() {
            continue;
        }
        if deployed_set_matches_snapshot(&set_dir, set) && store::remove_if_exists(&set_dir).is_ok()
        {
            removed.push(set.name.to_string());
        }
    }
    removed
}

/// Whether every file under `set_dir` matches `set`'s shipped snapshot
/// byte-for-byte, with no extra or missing files.
fn deployed_set_matches_snapshot(set_dir: &Path, set: &RetiredSet) -> bool {
    let actual_files = collect_relative_files(set_dir);
    if actual_files.len() != set.files.len() {
        return false;
    }

    for file in set.files {
        let expected_relative = PathBuf::from(file.relative_path);
        if !actual_files.contains(&expected_relative) {
            return false;
        }
        match std::fs::read_to_string(set_dir.join(file.relative_path)) {
            Ok(content) if content == file.content => {}
            _ => return false,
        }
    }

    true
}

/// Every file under `dir`, as paths relative to `dir`.
fn collect_relative_files(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(dir).ok().map(Path::to_path_buf))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_retired_sets_are_the_nine_merged_names() {
        let names: Vec<&str> = RETIRED_VALIDATOR_SETS.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            vec![
                "no-secrets",
                "injection",
                "command-safety",
                "no-commented-code",
                "function-length",
                "complexity",
                "missing-docs",
                "data-driven",
                "dead-code",
            ]
        );
    }

    #[test]
    fn test_every_retired_set_has_a_non_empty_manifest_and_rule() {
        for set in RETIRED_VALIDATOR_SETS {
            assert_eq!(
                set.files.len(),
                2,
                "{} should ship exactly VALIDATOR.md + one rule file",
                set.name
            );
            for file in set.files {
                assert!(
                    !file.content.is_empty(),
                    "{}/{} should not be empty",
                    set.name,
                    file.relative_path
                );
            }
        }
    }

    /// Write one retired set's shipped snapshot verbatim under `root/<name>/`.
    fn deploy_snapshot(root: &Path, set: &RetiredSet) {
        for file in set.files {
            let dest = root.join(set.name).join(file.relative_path);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(&dest, file.content).unwrap();
        }
    }

    #[test]
    fn prune_removes_an_unmodified_retired_set() {
        let store = tempdir().unwrap();
        let set = &RETIRED_VALIDATOR_SETS[0];
        deploy_snapshot(store.path(), set);

        let removed = prune_unmodified_retired_sets(store.path());

        assert_eq!(removed, vec![set.name.to_string()]);
        assert!(
            !store.path().join(set.name).exists(),
            "an unmodified retired set must be removed entirely"
        );
    }

    #[test]
    fn prune_leaves_a_user_modified_retired_set_untouched() {
        let store = tempdir().unwrap();
        let set = &RETIRED_VALIDATOR_SETS[0];
        deploy_snapshot(store.path(), set);

        // The user edited the manifest.
        let manifest = store.path().join(set.name).join("VALIDATOR.md");
        std::fs::write(&manifest, "USER EDITED THIS FILE").unwrap();

        let removed = prune_unmodified_retired_sets(store.path());

        assert!(
            removed.is_empty(),
            "a user-modified retired set must never be pruned"
        );
        assert!(
            store.path().join(set.name).is_dir(),
            "the modified set directory must survive"
        );
        assert_eq!(
            std::fs::read_to_string(&manifest).unwrap(),
            "USER EDITED THIS FILE",
            "the user's edit must be preserved exactly"
        );
    }

    #[test]
    fn prune_leaves_a_retired_set_with_an_added_user_file_untouched() {
        let store = tempdir().unwrap();
        let set = &RETIRED_VALIDATOR_SETS[0];
        deploy_snapshot(store.path(), set);

        // The user added a file inside the retired set's rules/ directory.
        let extra = store.path().join(set.name).join("rules/my-extra-rule.md");
        std::fs::write(&extra, "USER RULE").unwrap();

        let removed = prune_unmodified_retired_sets(store.path());

        assert!(
            removed.is_empty(),
            "a retired set with an added file must never be pruned"
        );
        assert!(store.path().join(set.name).is_dir());
    }

    #[test]
    fn prune_is_a_no_op_when_no_retired_set_is_deployed() {
        let store = tempdir().unwrap();
        let removed = prune_unmodified_retired_sets(store.path());
        assert!(removed.is_empty());
    }

    #[test]
    fn prune_never_touches_a_non_retired_set() {
        let store = tempdir().unwrap();
        let user_set = store.path().join("my-team-rules");
        std::fs::create_dir_all(&user_set).unwrap();
        std::fs::write(user_set.join("VALIDATOR.md"), "USER SET").unwrap();

        let removed = prune_unmodified_retired_sets(store.path());

        assert!(removed.is_empty());
        assert!(user_set.is_dir(), "a non-retired set must never be touched");
    }
}
