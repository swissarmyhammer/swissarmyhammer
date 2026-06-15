//! Incremental-review tracking: per-file context hashes under `.validators/.hashes/`.
//!
//! `review working` baselines against `HEAD`, but in a `/finish` fix-loop `HEAD`
//! never moves between passes, so every re-review re-scopes the *whole*
//! uncommitted change set even though the latest fix touched one file. This
//! module makes the working scope **incremental**: after a review completes it
//! records, per reviewed file, a content+rules **context hash**; on the next pass
//! [`subtract_unchanged`] drops every candidate whose context hash is unchanged
//! since it was last reviewed, so only genuinely-edited files are re-reviewed.
//!
//! # Directory layout
//!
//! `.validators/` is the project validators directory and MAY hold *committed*
//! project rule config, so it is never blanket-ignored. The ephemeral tracking
//! lives under the `.hashes/` subdirectory, the source tree mirrored beneath it,
//! and a `.gitignore` *inside* `.validators/` ignores only that subdirectory:
//!
//! ```text
//! .validators/
//!   .gitignore            # ignores `.hashes/`
//!   .hashes/              # ephemeral per-file context-hash tracking (gitignored)
//!     src/error.rs.yaml
//!     src/parser.rs.yaml
//!   <committed project rule files stay tracked>
//! ```
//!
//! # Tracking entry
//!
//! One [`TrackingEntry`] YAML file per reviewed source file, path-mirrored for
//! debuggability (`src/error.rs` → `.validators/.hashes/src/error.rs.yaml`). The
//! entry carries the plaintext path plus each component hash so "why did this
//! re-review?" is eyeball-able:
//!
//! ```yaml
//! path: src/error.rs
//! context_hash: sha256:<combined>
//! content_hash: sha256:<file content>
//! rules_hash: sha256:<rules>
//! reviewed_at: 2026-06-14T18:40:00Z
//! ```
//!
//! `context_hash = H(relative_path ‖ file_content ‖ rules_hash)` — changing the
//! path, the content, OR the rules changes it, forcing a re-review. `rules_hash`
//! is **global** for v1: a hash of every loaded validator rule body, so any rule
//! edit invalidates every entry and forces a full re-sweep.
//!
//! # Concurrency
//!
//! Entries are written atomically (write a temp file, then rename) so concurrent
//! `sah` processes (parallel review subagents) never tear each other's state.
//! Two writers computing a hash for the same file compute the *same* value and
//! write the same bytes — the operation is idempotent, and a race only ever
//! causes a redundant re-review, never incorrectness.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AvpError;
use crate::validators::ValidatorLoader;

/// The project validators directory name (`<repo>/.validators`).
const VALIDATORS_DIR: &str = ".validators";

/// The ephemeral hash-tracking subdirectory inside `.validators/`.
const HASHES_DIR: &str = ".hashes";

/// The `.gitignore` filename written inside `.validators/`.
const GITIGNORE_NAME: &str = ".gitignore";

/// The single line `.validators/.gitignore` carries: ignore the `.hashes/`
/// subtree while leaving committed project rule config tracked.
const GITIGNORE_LINE: &str = ".hashes/";

/// The algorithm prefix on every stored hash, so the entry self-describes which
/// digest produced the value (and a future algorithm change is detectable).
const HASH_PREFIX: &str = "sha256:";

/// The extension on a path-mirrored tracking entry file.
const ENTRY_EXT: &str = "yaml";

/// One reviewed file's tracking entry, serialized as a path-mirrored YAML file
/// under `.validators/.hashes/`.
///
/// The component hashes are stored alongside the combined `context_hash` so a
/// human can see *which* input changed (content vs rules) when a file re-reviews.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackingEntry {
    /// The reviewed file's repo-relative path, in plaintext.
    pub path: String,
    /// `H(path ‖ content ‖ rules_hash)` — the inclusion key. Equal across two
    /// passes ⇒ the file is subtracted from the second.
    pub context_hash: String,
    /// `H(file content)` — recorded for debuggability.
    pub content_hash: String,
    /// `H(all loaded rule bodies)` — the global v1 rules hash.
    pub rules_hash: String,
    /// When the file was last reviewed, RFC 3339 (`2026-06-14T18:40:00Z`).
    pub reviewed_at: String,
}

impl TrackingEntry {
    /// Build a fresh entry for `path` with `content` against `rules_hash`, stamped
    /// `reviewed_at`.
    ///
    /// `reviewed_at` is supplied by the caller (the tool boundary owns the clock)
    /// so the engine stays clock-free and the entry is deterministic in tests.
    pub fn new(path: &str, content: &str, rules_hash: &str, reviewed_at: &str) -> Self {
        Self {
            path: path.to_string(),
            context_hash: context_hash(path, content, rules_hash),
            content_hash: hash(content.as_bytes()),
            rules_hash: rules_hash.to_string(),
            reviewed_at: reviewed_at.to_string(),
        }
    }
}

/// Hash `bytes` into the prefixed, hex digest form stored in entries.
fn hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{HASH_PREFIX}{:x}", hasher.finalize())
}

/// The current local timestamp formatted RFC 3339 for a [`TrackingEntry`].
///
/// Provided so a caller without its own clock value (the tracking write path is
/// reached from the tool, which has `now`, but a direct caller may not) can stamp
/// entries; the engine itself never calls this.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// The combined inclusion hash for a file: `H(relative_path ‖ content ‖ rules_hash)`.
///
/// Domain-separated by length-prefixing each component so two different
/// `(path, content)` splits can never collide into the same digest. Changing the
/// path, the content, OR the rules changes the result, which is exactly the
/// re-review trigger.
pub fn context_hash(rel_path: &str, content: &str, rules_hash: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [
        rel_path.as_bytes(),
        content.as_bytes(),
        rules_hash.as_bytes(),
    ] {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    format!("{HASH_PREFIX}{:x}", hasher.finalize())
}

/// The global v1 rules hash: `H(all loaded rule bodies)`.
///
/// Deterministic regardless of loader insertion order — RuleSets and their rules
/// are gathered into a sorted set keyed by `(ruleset, rule, body)` before
/// hashing, so the same loaded rules always produce the same hash. Any rule edit
/// (a changed body, an added or removed rule) changes the hash and so invalidates
/// every tracking entry, forcing the next pass to review the full set.
pub fn rules_hash(loader: &ValidatorLoader) -> String {
    let mut keyed: BTreeSet<String> = BTreeSet::new();
    for ruleset in loader.list_rulesets() {
        for rule in &ruleset.rules {
            keyed.insert(format!(
                "{}\u{0}{}\u{0}{}",
                ruleset.name(),
                rule.name,
                rule.body
            ));
        }
    }
    let mut hasher = Sha256::new();
    for entry in &keyed {
        hasher.update((entry.len() as u64).to_le_bytes());
        hasher.update(entry.as_bytes());
    }
    format!("{HASH_PREFIX}{:x}", hasher.finalize())
}

/// The `.validators` directory for a repo root.
pub fn validators_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(VALIDATORS_DIR)
}

/// The path-mirrored tracking-entry file for a repo-relative source path:
/// `<repo>/.validators/.hashes/<rel_path>.yaml`.
fn entry_path(repo_path: &Path, rel_path: &str) -> PathBuf {
    validators_dir(repo_path)
        .join(HASHES_DIR)
        .join(format!("{rel_path}.{ENTRY_EXT}"))
}

/// Ensure `.validators/.gitignore` exists and ignores `.hashes/`.
///
/// Create-if-missing and idempotent: when the file is absent it is created with
/// the `.hashes/` ignore line (also creating `.validators/` if needed); when it
/// already exists it is left untouched unless it lacks the ignore line, in which
/// case the line is appended so the hash dir is never accidentally committed.
/// Committed project rule config under `.validators/` stays tracked — only the
/// `.hashes/` subtree is ignored.
///
/// This is the lazy guard the tracking writer runs before the first entry so the
/// hash dir is gitignored even when no separate `review` init step ran.
///
/// # Errors
///
/// Returns [`AvpError::Io`] when the `.validators/` directory or the `.gitignore`
/// file cannot be created or read.
pub fn ensure_gitignore(repo_path: &Path) -> Result<(), AvpError> {
    let dir = validators_dir(repo_path);
    std::fs::create_dir_all(&dir)?;
    let gitignore = dir.join(GITIGNORE_NAME);
    let existing = match std::fs::read_to_string(&gitignore) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(AvpError::Io(e)),
    };
    if existing.lines().any(|l| l.trim() == GITIGNORE_LINE) {
        return Ok(());
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(GITIGNORE_LINE);
    content.push('\n');
    write_atomic(&gitignore, content.as_bytes())
}

/// Read the tracking entry for `rel_path`, `None` when none exists yet (or it is
/// unreadable / unparseable — a corrupt entry simply forces a re-review).
pub fn read_entry(repo_path: &Path, rel_path: &str) -> Option<TrackingEntry> {
    let path = entry_path(repo_path, rel_path);
    let content = std::fs::read_to_string(path).ok()?;
    serde_yaml_ng::from_str(&content).ok()
}

/// Upsert the tracking entry for one reviewed file.
///
/// Lazily [`ensure_gitignore`]s before the first write so the hash dir is never
/// committed, creates the path-mirrored parent directory, and writes the entry
/// atomically (temp + rename) so a concurrent writer never observes a torn file.
/// Overwrites any existing entry in place.
///
/// # Errors
///
/// Returns [`AvpError::Io`] on a filesystem failure or [`AvpError::Json`] (YAML)
/// when the entry cannot be serialized.
pub fn upsert_entry(repo_path: &Path, entry: &TrackingEntry) -> Result<(), AvpError> {
    ensure_gitignore(repo_path)?;
    let path = entry_path(repo_path, &entry.path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml_ng::to_string(entry)
        .map_err(|e| AvpError::Context(format!("failed to serialize tracking entry: {e}")))?;
    write_atomic(&path, yaml.as_bytes())
}

/// Record tracking entries for every reviewed file after a successful pass.
///
/// For each `path` in `files`, reads the file's current working-tree content and
/// upserts its entry against `rules_hash`, stamped `reviewed_at`. A file that
/// cannot be read (e.g. deleted in the same change) is skipped — it has no
/// content to hash and will resolve fresh next pass. The entry is written
/// **regardless of findings**: an unchanged file with an open finding is
/// correctly not re-scanned, because re-scanning only re-derives the same
/// finding; when the implementer edits it, its content hash changes and it
/// re-enters scope.
///
/// # Errors
///
/// Returns the first [`AvpError`] from [`upsert_entry`] (a filesystem or
/// serialization failure).
pub fn record_reviewed(
    repo_path: &Path,
    files: &[String],
    rules_hash: &str,
    reviewed_at: &str,
) -> Result<(), AvpError> {
    for path in files {
        let Ok(content) = std::fs::read_to_string(repo_path.join(path)) else {
            continue;
        };
        let entry = TrackingEntry::new(path, &content, rules_hash, reviewed_at);
        upsert_entry(repo_path, &entry)?;
    }
    Ok(())
}

/// Subtract files whose tracking entry's `context_hash` matches their current
/// content, returning only the survivors that must be (re-)reviewed.
///
/// For each candidate `(path, current_content)`, the file is **dropped** when a
/// tracking entry exists whose `context_hash` equals the freshly-computed
/// `context_hash(path, current_content, rules_hash)`; otherwise it is kept. A
/// changed file, a brand-new file (no entry), and — because `rules_hash` feeds
/// the context hash — *every* file after a rule edit, are all kept. This decides
/// only inclusion; the caller keeps each survivor's real `before`/diff intact.
pub fn subtract_unchanged(
    repo_path: &Path,
    candidates: &[(String, String)],
    rules_hash: &str,
) -> Vec<String> {
    candidates
        .iter()
        .filter(|(path, content)| {
            let current = context_hash(path, content, rules_hash);
            match read_entry(repo_path, path) {
                Some(entry) => entry.context_hash != current,
                None => true,
            }
        })
        .map(|(path, _)| path.clone())
        .collect()
}

/// Write `bytes` to `path` atomically: write a sibling temp file, then rename it
/// over the target so a concurrent reader sees either the old or the new file,
/// never a partial write.
///
/// The temp file is created in the target's own directory so the final rename is
/// a same-filesystem atomic operation. A unique suffix keeps two concurrent
/// writers from colliding on the temp name.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AvpError> {
    let parent = path.parent().ok_or_else(|| {
        AvpError::Context(format!("tracking path has no parent: {}", path.display()))
    })?;
    std::fs::create_dir_all(parent)?;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("entry");
    let tmp = parent.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        unique_suffix()
    ));
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(AvpError::Io(e))
        }
    }
}

/// A process-monotonic suffix that keeps concurrent atomic writes within one
/// process from colliding on the temp filename.
fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::review::test_support::{loader_with, ruleset};
    use crate::validators::Severity;

    const RULES: &str = "sha256:rules-baseline";
    const NOW: &str = "2026-06-14T18:40:00Z";

    // ---- context_hash ----------------------------------------------------

    #[test]
    fn context_hash_is_stable_for_identical_inputs() {
        let a = context_hash("src/error.rs", "fn x() {}", RULES);
        let b = context_hash("src/error.rs", "fn x() {}", RULES);
        assert_eq!(a, b, "the same inputs must hash identically");
        assert!(a.starts_with(HASH_PREFIX), "the hash is algorithm-prefixed");
    }

    #[test]
    fn context_hash_changes_when_the_path_changes() {
        let a = context_hash("src/a.rs", "fn x() {}", RULES);
        let b = context_hash("src/b.rs", "fn x() {}", RULES);
        assert_ne!(a, b, "a different path must change the context hash");
    }

    #[test]
    fn context_hash_changes_when_the_content_changes() {
        let a = context_hash("src/a.rs", "fn x() {}", RULES);
        let b = context_hash("src/a.rs", "fn y() {}", RULES);
        assert_ne!(a, b, "a different content must change the context hash");
    }

    #[test]
    fn context_hash_changes_when_the_rules_change() {
        let a = context_hash("src/a.rs", "fn x() {}", "sha256:rules-1");
        let b = context_hash("src/a.rs", "fn x() {}", "sha256:rules-2");
        assert_ne!(a, b, "different rules must change the context hash");
    }

    #[test]
    fn context_hash_is_not_fooled_by_a_path_content_boundary_shift() {
        // Without length-prefixing, ("ab","c") and ("a","bc") would concatenate
        // identically. Domain separation must keep them distinct.
        let a = context_hash("ab", "c", RULES);
        let b = context_hash("a", "bc", RULES);
        assert_ne!(a, b, "the component boundary must be part of the hash");
    }

    // ---- rules_hash ------------------------------------------------------

    #[test]
    fn rules_hash_is_stable_and_changes_with_a_rule_edit() {
        let loader = loader_with("dead-code", "*.rs", &[], Severity::Warn);
        let h1 = rules_hash(&loader);
        let h2 = rules_hash(&loader);
        assert_eq!(h1, h2, "the same loaded rules hash identically");

        // A loader whose rule body differs must hash differently.
        let mut edited = ValidatorLoader::new();
        let mut rs = ruleset("dead-code", "*.rs", &[], Severity::Warn);
        rs.rules[0].body = "a different rule body".to_string();
        edited.add_builtin_ruleset(rs);
        assert_ne!(
            h1,
            rules_hash(&edited),
            "a changed rule body changes the hash"
        );
    }

    #[test]
    fn rules_hash_is_independent_of_loader_insertion_order() {
        let mut a = ValidatorLoader::new();
        a.add_builtin_ruleset(ruleset("alpha", "*.rs", &[], Severity::Warn));
        a.add_builtin_ruleset(ruleset("beta", "*.rs", &[], Severity::Warn));

        let mut b = ValidatorLoader::new();
        b.add_builtin_ruleset(ruleset("beta", "*.rs", &[], Severity::Warn));
        b.add_builtin_ruleset(ruleset("alpha", "*.rs", &[], Severity::Warn));

        assert_eq!(
            rules_hash(&a),
            rules_hash(&b),
            "the rules hash must not depend on load order"
        );
    }

    // ---- TrackingEntry round-trip ----------------------------------------

    #[test]
    fn tracking_entry_round_trips_through_yaml() {
        let entry = TrackingEntry::new("src/error.rs", "fn x() {}", RULES, NOW);
        let yaml = serde_yaml_ng::to_string(&entry).unwrap();
        // The plaintext path and component hashes are eyeball-able in the file.
        assert!(yaml.contains("path: src/error.rs"), "got:\n{yaml}");
        assert!(yaml.contains("context_hash: sha256:"), "got:\n{yaml}");
        assert!(yaml.contains("content_hash: sha256:"), "got:\n{yaml}");
        assert!(yaml.contains("rules_hash: sha256:"), "got:\n{yaml}");
        assert!(yaml.contains("reviewed_at:"), "got:\n{yaml}");

        let parsed: TrackingEntry = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed, entry, "the entry round-trips byte-for-byte");
    }

    // ---- ensure_gitignore ------------------------------------------------

    #[test]
    fn ensure_gitignore_creates_the_file_with_the_hashes_line() {
        let repo = TempDir::new().unwrap();
        ensure_gitignore(repo.path()).unwrap();

        let gitignore = repo.path().join(".validators").join(".gitignore");
        let content = std::fs::read_to_string(&gitignore).unwrap();
        assert!(
            content.lines().any(|l| l.trim() == ".hashes/"),
            "the gitignore must ignore .hashes/, got:\n{content}"
        );
    }

    #[test]
    fn ensure_gitignore_is_idempotent() {
        let repo = TempDir::new().unwrap();
        ensure_gitignore(repo.path()).unwrap();
        ensure_gitignore(repo.path()).unwrap();

        let gitignore = repo.path().join(".validators").join(".gitignore");
        let content = std::fs::read_to_string(&gitignore).unwrap();
        let count = content.lines().filter(|l| l.trim() == ".hashes/").count();
        assert_eq!(count, 1, "the ignore line is written once, got:\n{content}");
    }

    #[test]
    fn ensure_gitignore_preserves_existing_committed_lines() {
        let repo = TempDir::new().unwrap();
        let dir = repo.path().join(".validators");
        std::fs::create_dir_all(&dir).unwrap();
        // A pre-existing project-authored gitignore (e.g. ignoring scratch files).
        std::fs::write(dir.join(".gitignore"), "scratch/\n").unwrap();

        ensure_gitignore(repo.path()).unwrap();

        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(
            content.contains("scratch/"),
            "existing lines are kept: {content}"
        );
        assert!(
            content.contains(".hashes/"),
            "the hashes line is appended: {content}"
        );
    }

    // ---- upsert / read entry ---------------------------------------------

    #[test]
    fn upsert_writes_a_path_mirrored_entry_and_ensures_gitignore() {
        let repo = TempDir::new().unwrap();
        let entry = TrackingEntry::new("src/nested/error.rs", "fn x() {}", RULES, NOW);
        upsert_entry(repo.path(), &entry).unwrap();

        // The entry lands at the path-mirrored location.
        let on_disk = repo
            .path()
            .join(".validators/.hashes/src/nested/error.rs.yaml");
        assert!(on_disk.exists(), "entry written at {}", on_disk.display());

        // Writing an entry lazily ensured the gitignore.
        assert!(
            repo.path().join(".validators/.gitignore").exists(),
            "the first upsert lazily creates .validators/.gitignore"
        );

        // It reads back equal.
        let read = read_entry(repo.path(), "src/nested/error.rs").unwrap();
        assert_eq!(read, entry);
    }

    #[test]
    fn upsert_overwrites_in_place() {
        let repo = TempDir::new().unwrap();
        upsert_entry(
            repo.path(),
            &TrackingEntry::new("src/a.rs", "v1", RULES, NOW),
        )
        .unwrap();
        let updated = TrackingEntry::new("src/a.rs", "v2", RULES, "2026-06-14T19:00:00Z");
        upsert_entry(repo.path(), &updated).unwrap();

        let read = read_entry(repo.path(), "src/a.rs").unwrap();
        assert_eq!(
            read, updated,
            "the second upsert replaces the first in place"
        );
    }

    #[test]
    fn read_entry_is_none_for_an_unwritten_file() {
        let repo = TempDir::new().unwrap();
        assert!(read_entry(repo.path(), "src/never.rs").is_none());
    }

    // ---- subtract_unchanged ----------------------------------------------

    #[test]
    fn subtract_keeps_changed_and_new_files_and_drops_unchanged() {
        let repo = TempDir::new().unwrap();
        // Seed entries for two files at their reviewed content.
        record_reviewed(repo.path(), &[], RULES, NOW).unwrap();
        upsert_entry(
            repo.path(),
            &TrackingEntry::new("src/unchanged.rs", "stable", RULES, NOW),
        )
        .unwrap();
        upsert_entry(
            repo.path(),
            &TrackingEntry::new("src/changed.rs", "old", RULES, NOW),
        )
        .unwrap();

        let candidates = vec![
            ("src/unchanged.rs".to_string(), "stable".to_string()), // matches → drop
            ("src/changed.rs".to_string(), "new content".to_string()), // differs → keep
            ("src/brand-new.rs".to_string(), "fresh".to_string()),  // no entry → keep
        ];
        let survivors = subtract_unchanged(repo.path(), &candidates, RULES);

        assert!(
            !survivors.contains(&"src/unchanged.rs".to_string()),
            "an unchanged file is subtracted, got: {survivors:?}"
        );
        assert!(
            survivors.contains(&"src/changed.rs".to_string()),
            "a changed file survives, got: {survivors:?}"
        );
        assert!(
            survivors.contains(&"src/brand-new.rs".to_string()),
            "a brand-new file survives, got: {survivors:?}"
        );
    }

    #[test]
    fn subtract_keeps_everything_after_a_rules_change() {
        let repo = TempDir::new().unwrap();
        upsert_entry(
            repo.path(),
            &TrackingEntry::new("src/a.rs", "content", "sha256:rules-1", NOW),
        )
        .unwrap();

        // The same file content, but the rules hash differs: the context hash no
        // longer matches, so the file must survive (a rule edit re-sweeps all).
        let candidates = vec![("src/a.rs".to_string(), "content".to_string())];
        let survivors = subtract_unchanged(repo.path(), &candidates, "sha256:rules-2");
        assert_eq!(
            survivors,
            vec!["src/a.rs".to_string()],
            "a rules change invalidates the entry and keeps the file"
        );
    }

    #[test]
    fn record_reviewed_writes_an_entry_per_readable_file() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("a.rs"), "content-a").unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/b.rs"), "content-b").unwrap();

        record_reviewed(
            repo.path(),
            &[
                "a.rs".to_string(),
                "src/b.rs".to_string(),
                "missing.rs".to_string(), // unreadable → skipped
            ],
            RULES,
            NOW,
        )
        .unwrap();

        assert!(read_entry(repo.path(), "a.rs").is_some());
        assert!(read_entry(repo.path(), "src/b.rs").is_some());
        assert!(
            read_entry(repo.path(), "missing.rs").is_none(),
            "an unreadable file is skipped, not recorded"
        );

        // A second pass with no edits subtracts both recorded files.
        let candidates = vec![
            ("a.rs".to_string(), "content-a".to_string()),
            ("src/b.rs".to_string(), "content-b".to_string()),
        ];
        let survivors = subtract_unchanged(repo.path(), &candidates, RULES);
        assert!(
            survivors.is_empty(),
            "an unedited second pass subtracts every recorded file, got: {survivors:?}"
        );
    }
}
