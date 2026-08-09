//! The stored fixture verdict for a tool rule.
//!
//! Proving a tool rule healthy runs the rule's own `run` script twice — once
//! against the fail fixture and once against the pass fixture (see
//! [`crate::doctor`]). For a `workspace`-scope Rust rule each of those runs is
//! a real `cargo clippy`, so one rule costs tens of seconds and a review paid
//! it again for every rule, every run, before the fan-out could start.
//!
//! The verdict cannot change while the tool and the rule stay the same, so
//! this module stores it under the workspace and reads it back. A stored
//! verdict stands only while BOTH of its keys still hold:
//!
//! - the tool version `doctor.check_version_command` reports, and
//! - a digest of everything a fixture run reads: the rule name the fixture
//!   files are named for, the rule's whole `tool` block, and every file in the
//!   set's `fixtures/` directory.
//!
//! A tool upgrade, an edited `run` script, an edited fixture, or a rule this
//! workspace never proved therefore runs the fixtures again. A rule that
//! declares no version command is never stored at all — an upgrade of its tool
//! would be undetectable, and a verdict nothing can invalidate is worse than
//! no verdict.
//!
//! Presence and version are themselves never stored — each is one cheap
//! command, and reading them fresh is what makes the stored verdict safe.
//!
//! `sah doctor` never reads a stored verdict. It asks for
//! [`HealthProof::Fresh`], which runs the fixtures and replaces the stored
//! verdict with what they did, so doctor stays the ground truth and a review
//! that follows it reads doctor's own answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

use crate::doctor::ToolRuleStatus;
use crate::doctor::{
    check_fixtures, check_tool_rule, check_tool_rule_with, FixtureOutcome, FIXTURES_DIR_NAME,
};
use crate::validators::types::{Rule, RuleSet, ToolSpec};

/// The subdirectory of the workspace `.sah` directory that holds rebuildable
/// engine artifacts. It is created and git-ignored by the managed directory
/// itself, so a stored verdict never reaches a commit.
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

/// One stored fixture verdict, with the two keys that keep it usable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredVerdict {
    /// The tool version `doctor.check_version_command` reported when the
    /// fixtures ran.
    version: String,

    /// The digest of the rule content and the fixture files the run read.
    content: String,

    /// What the fixtures did.
    fixtures: FixtureOutcome,
}

/// The stored tool-rule fixture verdicts for one workspace.
#[derive(Debug)]
pub struct ToolHealthCache {
    /// Where the verdicts are stored, or `None` when the workspace has no
    /// writable state directory. A cache with no file still answers within one
    /// run and simply starts empty on the next one.
    path: Option<PathBuf>,

    /// The verdict for each `<set>/<rule>` key.
    verdicts: Mutex<BTreeMap<String, StoredVerdict>>,
}

impl ToolHealthCache {
    /// Open the verdicts stored for the workspace at `workspace_root`.
    ///
    /// A workspace with no readable stored file opens empty, and a workspace
    /// with no writable state directory opens with nowhere to save. Neither is
    /// an error: a cache that cannot answer costs the fixture runs it was
    /// meant to save and nothing else.
    pub fn open(workspace_root: &Path) -> Self {
        let path = cache_path(workspace_root);
        let verdicts = path.as_deref().and_then(read_verdicts).unwrap_or_default();
        Self {
            path,
            verdicts: Mutex::new(verdicts),
        }
    }

    /// Write the verdicts back to the workspace.
    ///
    /// A write that fails is reported and dropped — the next run proves the
    /// rules again, which is what it did before anything was stored.
    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let verdicts = self.verdicts();
        match serde_json::to_vec_pretty(&*verdicts) {
            Ok(bytes) => {
                if let Err(error) = std::fs::write(path, bytes) {
                    tracing::warn!(
                        path = %path.display(),
                        error = %error,
                        "the tool health verdicts could not be written; the next review proves the rules again"
                    );
                }
            }
            Err(error) => tracing::warn!(
                error = %error,
                "the tool health verdicts could not be encoded; nothing was written"
            ),
        }
    }

    /// The verdicts, with a poisoned lock taken over rather than panicked on:
    /// a verdict is a cache entry, and losing one costs fixture runs.
    fn verdicts(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, StoredVerdict>> {
        self.verdicts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The verdict stored under `key`, when one is stored AND both of its keys
    /// still hold.
    fn stored(&self, key: &str, keys: &VerdictKeys) -> Option<FixtureOutcome> {
        let verdicts = self.verdicts();
        let stored = verdicts.get(key)?;
        let matches = stored.version == keys.version && stored.content == keys.content;
        matches.then(|| stored.fixtures.clone())
    }

    /// Run the fixtures and store what they did under `key`.
    fn prove(
        &self,
        key: String,
        keys: &VerdictKeys,
        ruleset: &RuleSet,
        rule: &Rule,
        spec: &ToolSpec,
    ) -> FixtureOutcome {
        let fixtures = check_fixtures(ruleset, rule, spec);
        self.verdicts().insert(
            key,
            StoredVerdict {
                version: keys.version.clone(),
                content: keys.content.clone(),
                fixtures: fixtures.clone(),
            },
        );
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
        if proof == HealthProof::Stored {
            if let Some(stored) = cache.stored(&key, &keys) {
                tracing::debug!(
                    validator = %ruleset.name(),
                    rule = %rule.name,
                    version = %keys.version,
                    "the stored fixture verdict still applies; the fixtures do not run"
                );
                return stored;
            }
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

/// The digest of every file in `fixtures_dir`, by name and by content.
///
/// A fixture is half of what a health check proves, so an edited fixture must
/// invalidate the verdict the old one proved. The WHOLE directory counts,
/// because the doctor's fixture check copies all of it into the scratch
/// directory the run script works in — a `workspace`-scope tool reads the
/// fixture's neighbours as well as the fixture.
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
        hasher.update(path.to_string_lossy().as_bytes());
        if let Ok(bytes) = std::fs::read(&path) {
            hasher.update(bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

/// The file the workspace at `workspace_root` stores its verdicts in, or
/// `None` when the workspace has no writable state directory.
fn cache_path(workspace_root: &Path) -> Option<PathBuf> {
    let managed =
        ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(workspace_root.to_path_buf())
            .and_then(|dir| dir.ensure_subdir(CACHE_SUBDIR));
    match managed {
        Ok(dir) => Some(dir.join(CACHE_FILE_NAME)),
        Err(error) => {
            tracing::warn!(
                workspace = %workspace_root.display(),
                error = %error,
                "the workspace has no writable state directory; tool health is proved every run"
            );
            None
        }
    }
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
