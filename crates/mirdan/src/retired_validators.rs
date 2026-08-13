//! Snapshot of builtin validator content that has been retired (merged away or
//! deleted) from `builtin/validators/`, retained ONLY so a validators refresh
//! can detect and prune the retired content from the deployed store
//! (`~/.validators/` global or `./.validators/` project) when the user never
//! touched it.
//!
//! Retirement happens at two grains, and each has its own table and its own
//! prune:
//!
//! - A whole SET leaves the builtin lineup — [`RETIRED_VALIDATOR_SETS`] and
//!   [`prune_unmodified_retired_sets`], which remove the set directory.
//! - A single RULE FILE is deleted from a set that still ships —
//!   [`RETIRED_VALIDATOR_FILES`] and [`prune_unmodified_retired_files`], which
//!   remove that one file and nothing else around it.
//!
//! This module is deliberately isolated from `builtin/validators/` (the active
//! validator source tree): its snapshot files live under
//! `crates/mirdan/retired-validators/`, a sibling directory neither the
//! `swissarmyhammer-validators` RuleSet loader nor mirdan's own
//! `builtin_validators` build-time embed ever scans. Retired content must never
//! reappear as a loadable or installable validator — only as a fact this
//! module compares the deployed store against. Every snapshot file here is
//! byte-frozen for the same reason: editing one breaks the comparison below,
//! and the prune it drives silently stops firing.
//!
//! # Reference-copy policy
//!
//! [`install::install_profile_validators`](crate::install) owns builtin-owned
//! files: every embedded *active* file is overwritten on each install/refresh.
//! *Retired* content is different — nothing re-materializes it — so the only
//! honest outcome for retired content the user left untouched is deletion, and
//! the only honest outcome for content the user edited is to leave it alone
//! entirely.
//!
//! Both prunes are that same comparison, differing only in what they compare
//! and what they remove:
//!
//! - [`prune_unmodified_retired_sets`] compares the whole set directory. An
//!   exact byte-for-byte match removes the directory; any difference (edited
//!   content, an added file, a removed file) leaves it in place.
//! - [`prune_unmodified_retired_files`] compares one file. An exact
//!   byte-for-byte match removes that file; any difference leaves it in place.
//!   Nothing else in the surrounding set is read or touched, because that set
//!   still ships and the installer still owns it.

use std::path::{Path, PathBuf};

use crate::store;

/// One file within a retired validator set's shipped snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredFile {
    /// Path relative to the set directory (e.g. `"VALIDATOR.md"`,
    /// `"rules/dead-code.md"`).
    pub relative_path: &'static str,
    /// The exact byte content shipped for this file before the set was
    /// retired.
    pub content: &'static str,
}

/// A retired builtin validator set: its name plus every file it shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredSet {
    /// The set's directory name (e.g. `"dead-code"`).
    pub name: &'static str,
    /// Every file the set shipped, relative to the set directory.
    pub files: &'static [RetiredFile],
}

/// A retired rule file inside a builtin validator set that still ships.
///
/// This is the file-level counterpart of [`RetiredSet`]. The set itself is
/// still part of the builtin lineup, so the installer keeps refreshing it and
/// never removes it; only one rule inside it was deleted, and nothing on the
/// install path takes that one file back out of a deployed store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredSetFile {
    /// The still-shipping set's directory name (e.g. `"duplication"`).
    pub set_name: &'static str,
    /// Path relative to the set directory (e.g.
    /// `"rules/duplication-parsed.md"`).
    pub relative_path: &'static str,
    /// The exact byte content shipped for this file before the rule was
    /// retired.
    pub content: &'static str,
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

/// The retired rule files whose sets still ship.
///
/// `duplication-parsed` and `no-commented-code-parsed` were deleted from
/// `builtin/validators/` while `duplication` and `code-hygiene` stayed in the
/// builtin lineup. A store an earlier install wrote still holds both rule
/// files; the validator loader reads them at user or project precedence, so
/// each keeps running, and `sah doctor` keeps reporting it as a tool rule
/// whose tool it can no longer reach.
pub static RETIRED_VALIDATOR_FILES: &[RetiredSetFile] = &[
    RetiredSetFile {
        set_name: "duplication",
        relative_path: "rules/duplication-parsed.md",
        content: include_str!("../retired-validators/duplication/rules/duplication-parsed.md"),
    },
    RetiredSetFile {
        set_name: "code-hygiene",
        relative_path: "rules/no-commented-code-parsed.md",
        content: include_str!(
            "../retired-validators/code-hygiene/rules/no-commented-code-parsed.md"
        ),
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

/// Remove every retired rule file under `store_root` whose deployed bytes are
/// identical to what was shipped before the rule was retired.
///
/// A file that was never deployed is a no-op. A file whose deployed bytes
/// differ in any way from the snapshot is left untouched: that difference is
/// the user's own work. Only the named file is ever removed — the set around
/// it still ships, so every sibling in it stays where the installer put it.
///
/// Returns the `<set>/<relative path>` of each file actually removed.
pub fn prune_unmodified_retired_files(store_root: &Path) -> Vec<String> {
    let mut removed = Vec::new();
    for file in RETIRED_VALIDATOR_FILES {
        let deployed = store_root.join(file.set_name).join(file.relative_path);
        if !deployed.is_file() {
            continue;
        }
        let matches_snapshot = matches!(
            std::fs::read_to_string(&deployed),
            Ok(content) if content == file.content
        );
        if matches_snapshot && store::remove_if_exists(&deployed).is_ok() {
            removed.push(format!("{}/{}", file.set_name, file.relative_path));
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

    #[test]
    fn test_retired_files_are_the_two_parsed_rules() {
        let names: Vec<String> = RETIRED_VALIDATOR_FILES
            .iter()
            .map(|f| format!("{}/{}", f.set_name, f.relative_path))
            .collect();
        assert_eq!(
            names,
            vec![
                "duplication/rules/duplication-parsed.md".to_string(),
                "code-hygiene/rules/no-commented-code-parsed.md".to_string(),
            ]
        );
    }

    #[test]
    fn test_every_retired_file_snapshot_is_non_empty() {
        for file in RETIRED_VALIDATOR_FILES {
            assert!(
                !file.content.is_empty(),
                "{}/{} should not be empty",
                file.set_name,
                file.relative_path
            );
        }
    }

    /// A retired file's own set must still be a current builtin. If the whole
    /// set has gone, the entry belongs in [`RETIRED_VALIDATOR_SETS`] instead —
    /// this table only removes a file from a directory the installer still
    /// owns and refreshes.
    #[test]
    fn test_every_retired_file_names_a_set_that_still_ships() {
        let sets = crate::builtin_validators::builtin_validators_by_set();
        for file in RETIRED_VALIDATOR_FILES {
            assert!(
                sets.contains_key(file.set_name),
                "{} no longer ships as a builtin set; retire the whole set instead",
                file.set_name
            );
        }
    }

    /// A retired file must not also be a shipped builtin. Were it both, the
    /// prune would delete a file the installer re-materializes on the same
    /// run, and the snapshot would be a claim about content that still ships.
    #[test]
    fn test_no_retired_file_is_still_shipped_by_a_builtin_set() {
        let shipped: Vec<&str> = crate::builtin_validators::get_builtin_validators()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for file in RETIRED_VALIDATOR_FILES {
            let embedded_name = format!("{}/{}", file.set_name, file.relative_path);
            assert!(
                !shipped.contains(&embedded_name.as_str()),
                "{embedded_name} still ships as a builtin: pruning it would delete a file the \
                 installer just wrote"
            );
        }
    }

    /// Write one retired rule file's shipped snapshot verbatim under
    /// `root/<set>/<relative path>`, and return where it landed.
    fn deploy_file_snapshot(root: &Path, file: &RetiredSetFile) -> PathBuf {
        let dest = root.join(file.set_name).join(file.relative_path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, file.content).unwrap();
        dest
    }

    #[test]
    fn prune_removes_an_unmodified_retired_file() {
        let store = tempdir().unwrap();
        let file = &RETIRED_VALIDATOR_FILES[0];
        let deployed = deploy_file_snapshot(store.path(), file);

        let removed = prune_unmodified_retired_files(store.path());

        assert_eq!(
            removed,
            vec![format!("{}/{}", file.set_name, file.relative_path)]
        );
        assert!(
            !deployed.exists(),
            "an unmodified retired rule file must be removed"
        );
    }

    #[test]
    fn prune_leaves_a_user_modified_retired_file_untouched() {
        let store = tempdir().unwrap();
        let file = &RETIRED_VALIDATOR_FILES[0];
        let deployed = deploy_file_snapshot(store.path(), file);
        std::fs::write(&deployed, "USER EDITED THIS FILE").unwrap();

        let removed = prune_unmodified_retired_files(store.path());

        assert!(
            removed.is_empty(),
            "a user-modified retired rule file must never be pruned"
        );
        assert_eq!(
            std::fs::read_to_string(&deployed).unwrap(),
            "USER EDITED THIS FILE",
            "the user's edit must be preserved exactly"
        );
    }

    #[test]
    fn prune_removes_every_unmodified_retired_file_at_once() {
        let store = tempdir().unwrap();
        for file in RETIRED_VALIDATOR_FILES {
            deploy_file_snapshot(store.path(), file);
        }

        let removed = prune_unmodified_retired_files(store.path());

        assert_eq!(removed.len(), RETIRED_VALIDATOR_FILES.len());
        for file in RETIRED_VALIDATOR_FILES {
            assert!(
                !store
                    .path()
                    .join(file.set_name)
                    .join(file.relative_path)
                    .exists(),
                "{}/{} must be removed",
                file.set_name,
                file.relative_path
            );
        }
    }

    #[test]
    fn prune_is_a_no_op_when_no_retired_file_is_deployed() {
        let store = tempdir().unwrap();
        let removed = prune_unmodified_retired_files(store.path());
        assert!(removed.is_empty());
    }

    #[test]
    fn prune_removes_only_the_named_file_from_a_still_shipping_set() {
        let store = tempdir().unwrap();
        let file = &RETIRED_VALIDATOR_FILES[0];
        let deployed = deploy_file_snapshot(store.path(), file);

        let set_dir = store.path().join(file.set_name);
        let manifest = set_dir.join("VALIDATOR.md");
        std::fs::write(&manifest, "SHIPPED MANIFEST").unwrap();
        let sibling = set_dir.join("rules/a-rule-that-still-ships.md");
        std::fs::write(&sibling, "SHIPPED RULE").unwrap();

        prune_unmodified_retired_files(store.path());

        assert!(
            !deployed.exists(),
            "the retired rule file must be removed, or \"only\" proves nothing"
        );
        assert!(
            set_dir.is_dir(),
            "the still-shipping set directory must survive"
        );
        assert_eq!(
            std::fs::read_to_string(&manifest).unwrap(),
            "SHIPPED MANIFEST",
            "the set manifest must never be touched"
        );
        assert_eq!(
            std::fs::read_to_string(&sibling).unwrap(),
            "SHIPPED RULE",
            "a sibling rule must never be touched"
        );
    }
}
