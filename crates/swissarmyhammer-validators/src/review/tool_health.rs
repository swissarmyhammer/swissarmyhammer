//! The stored fixture verdict for a tool rule.
//!
//! Proving a tool rule healthy runs the rule's own `run` script twice — once
//! against the fail fixture and once against the pass fixture (see
//! [`crate::doctor`]). For a `workspace`-scope Rust rule each of those runs is
//! a real `cargo clippy`, so one rule costs tens of seconds and a review paid
//! it again for every rule, every run, before the fan-out could start.
//!
//! A PASS cannot change while the tool and the rule stay the same, so this
//! module stores the pass under the workspace and reads it back. A stored
//! verdict stands only while BOTH of its keys still hold:
//!
//! - the tool version `doctor.check_version_command` reports, and
//! - a digest of everything a fixture run reads: the rule's whole `tool`
//!   block, and every file in the set's `fixtures/` directory.
//!
//! The rule NAME is not part of that digest. It is part of the storage key
//! (`<set>/<rule>`), which is a different mechanism: two rules of one set that
//! carry identical `tool` blocks share a digest and still hold their own
//! verdicts, because they do not share a key.
//!
//! A tool upgrade, an edited `run` script, an edited fixture, or a rule this
//! workspace never proved therefore runs the fixtures again. A rule that
//! declares no version command is never stored at all — an upgrade of its tool
//! would be undetectable, and a verdict nothing can invalidate is worse than
//! no verdict.
//!
//! Only a PASS is stored. A fixture run breaks for reasons that say nothing
//! about the tool or the rule: a `cargo clippy` that lost the build lock, a
//! full disk, a network failure. A stored failure would put the rule on its
//! prompt fallback on every later review until the tool version or the rule
//! content changed, so a rule that does not pass is proved again every run and
//! any verdict standing under its key is dropped.
//!
//! Presence and version are themselves never stored — each is one cheap
//! command, and reading them fresh is what makes the stored verdict safe.
//!
//! `sah doctor` never reads a stored verdict. It asks for
//! [`HealthProof::Fresh`], which runs the fixtures and writes what they did:
//! a pass replaces the stored verdict, and anything else drops it. Doctor
//! therefore stays the ground truth, and a review that follows it never
//! replays a pass the tool no longer earns.
//!
//! The drop reaches the DISK, because doctor and the review are two processes
//! and the file is all they share. A drop that empties the map deletes the
//! stored file, so a workspace whose only rule broke does not keep its last
//! pass ([`ToolHealthCache::save`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

use crate::doctor::ToolRuleStatus;
use crate::doctor::{check_fixtures, check_tool_rule, check_tool_rule_with, FixtureOutcome};
use crate::validators::types::{Rule, RuleSet, ToolSpec, FIXTURES_DIR_NAME};

/// The subdirectory of the workspace `.sah` directory that holds rebuildable
/// engine artifacts. The managed directory creates it, and git-ignores it, at
/// the moment a verdict is saved, so a stored verdict never reaches a commit.
const CACHE_SUBDIR: &str = "tmp";

/// The file that holds the stored verdicts.
const CACHE_FILE_NAME: &str = "review-tool-health.json";

/// How much proof a caller wants for a tool rule's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthProof {
    /// A stored verdict is enough while its keys still hold. The review
    /// engine's choice: the fixtures prove nothing a stored verdict does not
    /// already say.
    Stored,

    /// The fixtures must run, whatever is stored, and the stored verdict takes
    /// their result. `sah doctor`'s choice, so doctor stays the ground truth.
    Fresh,
}

/// One stored PASS, with the two keys that keep it usable.
///
/// The entry carries no outcome, because only a pass is ever stored: an entry
/// standing under a key IS the statement that the rule passed its fixtures
/// under those keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredVerdict {
    /// The tool version `doctor.check_version_command` reported when the
    /// fixtures passed.
    version: String,

    /// The digest of the rule content and the fixture files the run read.
    content: String,
}

/// The stored tool-rule fixture verdicts for one workspace.
#[derive(Debug)]
pub struct ToolHealthCache {
    /// The workspace the verdicts belong to. [`ToolHealthCache::save`] derives
    /// the state directory from it, and nothing else does.
    workspace_root: PathBuf,

    /// The verdict for each `<set>/<rule>` key.
    verdicts: Mutex<BTreeMap<String, StoredVerdict>>,
}

impl ToolHealthCache {
    /// Open the verdicts stored for the workspace at `workspace_root`.
    ///
    /// Opening reads and creates nothing. A workspace with no stored file, or
    /// one this version cannot read, opens empty, which is not an error: a
    /// cache that cannot answer costs the fixture runs it was meant to save
    /// and nothing else.
    pub fn open(workspace_root: &Path) -> Self {
        let verdicts = read_verdicts(&cache_path(workspace_root)).unwrap_or_default();
        Self {
            workspace_root: workspace_root.to_path_buf(),
            verdicts: Mutex::new(verdicts),
        }
    }

    /// Write the verdicts back to the workspace.
    ///
    /// An empty map DELETES the stored file rather than leaving it alone. The
    /// map empties when [`ToolHealthCache::prove`] drops the last verdict, and
    /// `sah doctor` runs in a process of its own, so the drop reaches the next
    /// review only through the file. Leaving the file would replay a pass the
    /// tool no longer earns, which is the one thing doctor being the ground
    /// truth forbids.
    ///
    /// A workspace with no stored file writes nothing and creates nothing, so
    /// a review that stored no verdict leaves the tree it reviewed exactly as
    /// it found it. That matters beyond tidiness: `Scope::Working` reads
    /// untracked files, so a review that created a state directory would write
    /// its own next scope.
    ///
    /// A write that fails is reported and dropped — the next run proves the
    /// rules again, which is what it did before anything was stored.
    pub fn save(&self) {
        // The snapshot is a statement of its own, so the guard drops at its
        // semicolon. Neither the encoding below nor the blocking write then
        // holds the lock, whatever either one calls.
        let verdicts = self.verdicts().clone();
        if verdicts.is_empty() {
            self.remove_stored_verdicts();
            return;
        }

        let bytes = match serde_json::to_vec_pretty(&verdicts) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "the tool health verdicts could not be encoded; nothing was written"
                );
                return;
            }
        };

        let Some(path) = self.writable_cache_path() else {
            return;
        };
        if let Err(error) = std::fs::write(&path, bytes) {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "the tool health verdicts could not be written; the next review proves the rules again"
            );
        }
    }

    /// Delete the stored verdict file, because no verdict stands any more.
    ///
    /// This is how a drop becomes durable. Deleting is taken over writing an
    /// empty map because it creates nothing: a workspace that never stored a
    /// verdict has no file to delete, so the tree under review is left as it
    /// was found.
    ///
    /// A file that cannot be deleted is reported and dropped. The stored pass
    /// then stands one more review, and the next doctor run drops it again.
    fn remove_stored_verdicts(&self) {
        let path = cache_path(&self.workspace_root);
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!(
                path = %path.display(),
                "no tool health verdict stands any more; the stored file was deleted"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(
                path = %path.display(),
                error = %error,
                "the stored tool health verdicts could not be deleted; a dropped verdict may be replayed"
            ),
        }
    }

    /// The verdict file, with the state directory that holds it created.
    ///
    /// This is the ONE place the engine writes into the tree it is reviewing,
    /// and it runs only when there is a verdict to keep. The managed directory
    /// writes the `.gitignore` that covers [`CACHE_SUBDIR`], so the verdict
    /// file never reaches a commit.
    fn writable_cache_path(&self) -> Option<PathBuf> {
        let managed = ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(
            self.workspace_root.clone(),
        )
        .and_then(|dir| dir.ensure_subdir(CACHE_SUBDIR));
        match managed {
            Ok(dir) => Some(dir.join(CACHE_FILE_NAME)),
            Err(error) => {
                tracing::warn!(
                    workspace = %self.workspace_root.display(),
                    error = %error,
                    "the workspace has no writable state directory; tool health is proved every run"
                );
                None
            }
        }
    }

    /// The verdicts, with a poisoned lock taken over rather than panicked on:
    /// a verdict is a cache entry, and losing one costs fixture runs.
    fn verdicts(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, StoredVerdict>> {
        self.verdicts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether a stored verdict says the rule under `key` passed its fixtures,
    /// and said so under keys that still hold.
    fn passed(&self, key: &str, keys: &VerdictKeys) -> bool {
        self.verdicts()
            .get(key)
            .is_some_and(|stored| stored.version == keys.version && stored.content == keys.content)
    }

    /// Run the fixtures and store the verdict under `key` when they passed.
    ///
    /// A run that did not pass stores nothing AND drops whatever stood under
    /// the key. Both halves matter: a break that the environment caused must
    /// not become this rule's verdict, and a pass the tool no longer earns
    /// must not stand once a run has shown it broken.
    fn prove(
        &self,
        key: String,
        keys: &VerdictKeys,
        ruleset: &RuleSet,
        rule: &Rule,
        spec: &ToolSpec,
    ) -> FixtureOutcome {
        let fixtures = check_fixtures(ruleset, rule, spec);
        if fixtures == FixtureOutcome::Passed {
            self.verdicts().insert(
                key,
                StoredVerdict {
                    version: keys.version.clone(),
                    content: keys.content.clone(),
                },
            );
        } else {
            self.verdicts().remove(&key);
        }
        fixtures
    }
}

/// What a stored verdict turns on: change either and the fixtures run again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerdictKeys {
    /// The tool version `doctor.check_version_command` reports now.
    version: String,

    /// The digest of the rule content and the fixture files a run reads.
    content: String,
}

impl VerdictKeys {
    /// The keys one tool rule's verdict turns on, or `None` when the rule
    /// reports no version to key on.
    fn of(ruleset: &RuleSet, spec: &ToolSpec, version: Option<&str>) -> Option<Self> {
        let fixtures = fixture_digest(&ruleset.base_path.join(FIXTURES_DIR_NAME));
        Some(Self {
            version: version?.to_string(),
            content: content_digest(spec, &fixtures)?,
        })
    }
}

/// The health of one tool rule under `proof`, recorded in `health` when a
/// cache is open.
///
/// With no cache open every call runs the fixtures, which is what the engine
/// did before any verdict was stored.
pub fn tool_rule_health(
    health: Option<&ToolHealthCache>,
    proof: HealthProof,
    ruleset: &RuleSet,
    rule: &Rule,
    spec: &ToolSpec,
) -> ToolRuleStatus {
    let Some(cache) = health else {
        return check_tool_rule(ruleset, rule, spec);
    };

    check_tool_rule_with(ruleset, rule, spec, |ruleset, rule, spec, version| {
        let Some(keys) = VerdictKeys::of(ruleset, spec, version) else {
            tracing::debug!(
                validator = %ruleset.name(),
                rule = %rule.name,
                "the rule reports no tool version, so its verdict cannot be invalidated; proving it"
            );
            return check_fixtures(ruleset, rule, spec);
        };

        let key = verdict_key(ruleset, rule);
        if proof == HealthProof::Stored && cache.passed(&key, &keys) {
            tracing::debug!(
                validator = %ruleset.name(),
                rule = %rule.name,
                version = %keys.version,
                "the stored fixture verdict still applies; the fixtures do not run"
            );
            return FixtureOutcome::Passed;
        }
        cache.prove(key, &keys, ruleset, rule, spec)
    })
}

/// The key one tool rule's verdict is stored under.
fn verdict_key(ruleset: &RuleSet, rule: &Rule) -> String {
    format!("{}/{}", ruleset.name(), rule.name)
}

/// The digest of everything a fixture run reads other than the fixtures: the
/// rule's whole `tool` block, which carries the `run` script the fixtures are
/// judged by and the doctor commands that decide the version.
///
/// `None` when the block does not encode, which leaves the rule unstored
/// rather than stored under a key that cannot tell two rules apart.
fn content_digest(spec: &ToolSpec, fixtures: &str) -> Option<String> {
    let encoded = serde_json::to_vec(spec)
        .inspect_err(|error| {
            tracing::warn!(
                error = %error,
                "the tool block does not encode, so its verdict is proved every run"
            );
        })
        .ok()?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    hasher.update(fixtures.as_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

/// The tag that marks a fixture the digest read.
const FIXTURE_READ: u8 = 0;

/// The tag that marks a fixture the digest could not read. It keeps an
/// unreadable fixture from digesting as an empty one.
const FIXTURE_UNREADABLE: u8 = 1;

/// The digest of every file in `fixtures_dir`, by name and by content.
///
/// A fixture is half of what a health check proves, so an edited fixture must
/// invalidate the verdict the old one proved. The WHOLE directory counts,
/// because the doctor's fixture check copies all of it into the scratch
/// directory the run script works in — a `workspace`-scope tool reads the
/// fixture's neighbours as well as the fixture.
///
/// Every name and every content blob goes in behind its own length, so two
/// different fixture sets cannot lay down one byte stream. Without the
/// framing, `<rule>.pass.rs` holding `XY` and `<rule>.pass.rsX` holding `Y`
/// digest the same, and both are live fixtures because the doctor finds a
/// fixture by the `<rule>.<kind>.` prefix.
///
/// A file the digest cannot read is marked as unreadable rather than skipped,
/// so it does not digest as an empty file.
///
/// The files are read on every check rather than remembered. A validator set
/// ships a handful of small fixtures, so re-reading them costs far less than
/// one fixture run, and nothing a check reads is then a remembered copy.
///
/// A directory that cannot be read digests as empty, which is stable: the
/// fixture check then reports the fixtures missing, and the directory
/// appearing later changes the digest.
fn fixture_digest(fixtures_dir: &Path) -> String {
    let mut names: Vec<PathBuf> = std::fs::read_dir(fixtures_dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    names.sort();

    let mut hasher = Sha256::new();
    for path in names {
        update_framed(&mut hasher, path.to_string_lossy().as_bytes());
        match std::fs::read(&path) {
            Ok(bytes) => {
                hasher.update([FIXTURE_READ]);
                update_framed(&mut hasher, &bytes);
            }
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "a fixture file could not be read; it digests as unreadable rather than as empty"
                );
                hasher.update([FIXTURE_UNREADABLE]);
            }
        }
    }
    format!("{:x}", hasher.finalize())
}

/// Feed `bytes` to `hasher` behind its own length, so one entry cannot run
/// into the next and let two different fixture sets share a byte stream.
fn update_framed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// The file the workspace at `workspace_root` stores its verdicts in.
///
/// The path is derived and nothing is created, so reading a workspace never
/// writes to it. [`ToolHealthCache::save`] is the one writer: it creates the
/// directory when it has a verdict to keep, and deletes this file when none
/// stands.
fn cache_path(workspace_root: &Path) -> PathBuf {
    workspace_root
        .join(ManagedDirectory::<SwissarmyhammerConfig>::dir_name())
        .join(CACHE_SUBDIR)
        .join(CACHE_FILE_NAME)
}

/// The verdicts stored in `path`, or `None` when the file is absent or does
/// not read as the verdicts this version writes.
fn read_verdicts(path: &Path) -> Option<BTreeMap<String, StoredVerdict>> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(verdicts) => Some(verdicts),
        Err(error) => {
            tracing::info!(
                path = %path.display(),
                error = %error,
                "the stored tool health verdicts do not read; proving every rule again"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests;
