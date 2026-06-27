//! The resolved-action replay cache: deterministic replay without the model.
//!
//! `ideas/expect.md` §"Determinism comes from not calling the model, not from
//! temp=0" follows Stagehand: cache each resolved action keyed by a hash of
//! (normalized target + state snapshot + method), and on replay execute the
//! cached action **without** the model, re-resolving via the agent only on a
//! cache miss or fingerprint drift. This is what turns a fuzzy authoring step
//! into a fast, mostly-deterministic CI gate.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ExpectError;
use crate::observe::cache_path;

/// The algorithm prefix on a [`ReplayKey`] digest (mirrors the ledger's
/// `sha256:` form so the fingerprint is self-describing).
const REPLAY_KEY_PREFIX: &str = "sha256:";

/// The maximum tolerated state drift for a cached action to still replay.
pub const MAX_REPLAY_DRIFT: f64 = 0.15;

/// The decision identity of a resolved action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayKey {
    /// The normalized target the action acts on.
    pub target: String,
    /// The method/surface the action is resolved for.
    pub method: String,
}

impl ReplayKey {
    /// Build a key from a `target` and `method`, normalizing whitespace.
    pub fn new(target: impl AsRef<str>, method: impl AsRef<str>) -> Self {
        ReplayKey {
            target: normalize_ws(target.as_ref()),
            method: normalize_ws(method.as_ref()),
        }
    }

    /// The stable digest used as the cache map key.
    ///
    /// `target` and `method` are each length-prefixed before hashing, so a
    /// boundary shift between them (`"ab"`+`"c"` vs `"a"`+`"bc"`) cannot collide
    /// — the same domain-separation the ledger's [`spec_hash`](crate::spec_hash)
    /// uses.
    fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        for field in [&self.target, &self.method] {
            hasher.update((field.len() as u64).to_le_bytes());
            hasher.update(field.as_bytes());
        }
        format!("{REPLAY_KEY_PREFIX}{:x}", hasher.finalize())
    }
}

/// A resolved action recorded for deterministic replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedAction {
    /// The normalized target the action acts on.
    pub target: String,
    /// The method/surface the action was resolved for.
    pub method: String,
    /// The normalized state snapshot the action was resolved against.
    pub state: String,
    /// The resolved action to replay.
    pub action: serde_json::Value,
}

/// Why a [`ResolvedAction`] was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplaySource {
    /// Replayed from the cache without the model.
    Cached,
    /// Re-resolved via the agent on a cache miss.
    Resolved,
    /// Re-resolved via the agent because the cached state fingerprint drifted.
    Drifted,
}

impl ReplaySource {
    /// Whether this resolution was a fingerprint-drift re-resolve.
    pub fn is_drift(&self) -> bool {
        matches!(self, ReplaySource::Drifted)
    }
}

/// The outcome of consulting the cache for one action.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedAction {
    /// The action to execute.
    pub action: serde_json::Value,
    /// How the action was produced.
    pub source: ReplaySource,
}

/// An on-disk, per-expectation cache of resolved actions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayCache {
    /// Cached actions keyed by their [`ReplayKey`] digest.
    entries: BTreeMap<String, CachedAction>,
}

impl ReplayCache {
    /// An empty cache.
    pub fn new() -> Self {
        ReplayCache::default()
    }

    /// Load the cache for spec `identity` under `repo_root`, or an empty cache
    /// when none has been written yet (the first run of a new expectation, not
    /// an error).
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Expectation`] when `identity` is unsafe (see
    /// [`cache_path`]), [`ExpectError::Io`] when the file exists but cannot be
    /// read, or [`ExpectError::Json`] when it cannot be parsed.
    pub fn load(repo_root: &Path, identity: &str) -> Result<Self, ExpectError> {
        let path = cache_path(repo_root, identity)?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ReplayCache::default()),
            Err(err) => Err(ExpectError::Io(err)),
        }
    }

    /// Persist the cache to its committed slot under `repo_root`
    /// (`.expect/cache/<identity>.cache.json`), creating parent directories, and
    /// return the path written.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Expectation`] when `identity` is unsafe (see
    /// [`cache_path`]), [`ExpectError::Json`] when the cache cannot be
    /// serialized, or [`ExpectError::Io`] when the file cannot be written.
    pub fn save(&self, repo_root: &Path, identity: &str) -> Result<PathBuf, ExpectError> {
        let path = cache_path(repo_root, identity)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    /// Resolve the action for `key` against the current `state`, replaying the
    /// cached action **without** calling `resolve` (the model) on a
    /// fingerprint-matching hit, and invoking `resolve` only on a cache miss or
    /// a fingerprint drift.
    ///
    /// The three outcomes carried by the returned [`ResolvedAction::source`]:
    ///
    /// - [`ReplaySource::Cached`] — a cached action exists for `key` and the
    ///   current state is within [`MAX_REPLAY_DRIFT`] of the one it was resolved
    ///   against; the cached action is replayed and `resolve` is **not** called.
    /// - [`ReplaySource::Resolved`] — no cached action for `key` (a miss);
    ///   `resolve` is invoked once and the result recorded.
    /// - [`ReplaySource::Drifted`] — a cached action exists but the state
    ///   drifted past the threshold; `resolve` is invoked, the fresh action
    ///   replaces the stale one, and the re-resolve is **surfaced as drift**,
    ///   never silently applied — "a wrong cached click is worse than a slow
    ///   click."
    ///
    /// The lookup keys on `key` (target + method) so a miss and a drift stay
    /// distinguishable; the state snapshot is the drift fingerprint, not part of
    /// the lookup key (folding it into the key would make a changed state an
    /// indistinguishable miss, defeating the drift guard).
    ///
    /// # Errors
    ///
    /// Propagates whatever error `resolve` returns on a miss or drift.
    pub async fn resolve_or_replay<F, Fut>(
        &mut self,
        key: &ReplayKey,
        state: &str,
        resolve: F,
    ) -> Result<ResolvedAction, ExpectError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<serde_json::Value, ExpectError>>,
    {
        let digest = key.digest();
        let normalized_state = normalize_ws(state);

        let drift_source = match self.entries.get(&digest) {
            Some(cached) if state_drift(&cached.state, &normalized_state) <= MAX_REPLAY_DRIFT => {
                return Ok(ResolvedAction {
                    action: cached.action.clone(),
                    source: ReplaySource::Cached,
                });
            }
            // A cached action existed but its state drifted past the threshold:
            // a fallback re-resolve surfaced as drift, never silently replayed.
            Some(_) => ReplaySource::Drifted,
            // No cached action for this decision: a plain first-time resolve.
            None => ReplaySource::Resolved,
        };

        let action = resolve().await?;
        self.entries.insert(
            digest,
            CachedAction {
                target: key.target.clone(),
                method: key.method.clone(),
                state: normalized_state,
                action: action.clone(),
            },
        );
        Ok(ResolvedAction {
            action,
            source: drift_source,
        })
    }
}

/// Collapse runs of whitespace and trim.
fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The drift between two state snapshots: Jaccard distance over tokens.
fn state_drift(a: &str, b: &str) -> f64 {
    let sa: BTreeSet<&str> = a.split_whitespace().collect();
    let sb: BTreeSet<&str> = b.split_whitespace().collect();
    if sa.is_empty() && sb.is_empty() {
        return 0.0;
    }
    let intersection = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    1.0 - intersection / union
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    const IDENTITY: &str = "src/checkout/coupon";

    /// Resolve `key`/`state` against `cache`, counting agent invocations in
    /// `calls` and returning a fixed `action` on each (miss/drift) resolve.
    async fn resolve(
        cache: &mut ReplayCache,
        key: &ReplayKey,
        state: &str,
        calls: &AtomicUsize,
        action: serde_json::Value,
    ) -> ResolvedAction {
        cache
            .resolve_or_replay(key, state, || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok(action) }
            })
            .await
            .expect("resolve_or_replay")
    }

    #[tokio::test]
    async fn miss_invokes_the_agent_once() {
        let mut cache = ReplayCache::new();
        let key = ReplayKey::new("click submit", "cli");
        let calls = AtomicUsize::new(0);

        let resolved = resolve(
            &mut cache,
            &key,
            "state-a",
            &calls,
            serde_json::json!({"do": 1}),
        )
        .await;

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a cache miss invokes the agent once"
        );
        assert_eq!(resolved.source, ReplaySource::Resolved);
    }

    #[tokio::test]
    async fn hit_replays_cached_action_without_invoking_the_agent() {
        let mut cache = ReplayCache::new();
        let key = ReplayKey::new("click submit", "cli");
        let calls = AtomicUsize::new(0);
        let action = serde_json::json!({"do": 1});

        resolve(&mut cache, &key, "state-a", &calls, action.clone()).await;
        // Second run, unchanged target+state: replay with NO model call.
        let resolved = resolve(
            &mut cache,
            &key,
            "state-a",
            &calls,
            serde_json::json!({"do": 99}),
        )
        .await;

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the second run replays without the model"
        );
        assert_eq!(resolved.source, ReplaySource::Cached);
        assert_eq!(
            resolved.action, action,
            "the cached action is replayed, not the new one"
        );
    }

    #[tokio::test]
    async fn fingerprint_drift_re_resolves_and_surfaces_drift() {
        let mut cache = ReplayCache::new();
        let key = ReplayKey::new("click submit", "cli");
        let calls = AtomicUsize::new(0);

        resolve(
            &mut cache,
            &key,
            "alpha beta gamma",
            &calls,
            serde_json::json!({"v": 1}),
        )
        .await;
        // The state snapshot changed beyond the threshold: re-resolve, surfaced.
        let resolved = resolve(
            &mut cache,
            &key,
            "totally different other tokens",
            &calls,
            serde_json::json!({"v": 2}),
        )
        .await;

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "drift re-resolves via the agent"
        );
        assert_eq!(resolved.source, ReplaySource::Drifted);
        assert!(
            resolved.source.is_drift(),
            "the re-resolve is surfaced as drift"
        );
        assert_eq!(
            resolved.action,
            serde_json::json!({"v": 2}),
            "the fresh action replaces the stale one"
        );
    }

    #[tokio::test]
    async fn state_change_within_threshold_still_replays() {
        let mut cache = ReplayCache::new();
        let key = ReplayKey::new("click submit", "cli");
        let calls = AtomicUsize::new(0);
        let cached_state = "t1 t2 t3 t4 t5 t6 t7 t8 t9 t10 t11 t12 t13 t14 t15 t16 t17 t18 t19 t20";
        let nudged_state = format!("{cached_state} t21");

        resolve(
            &mut cache,
            &key,
            cached_state,
            &calls,
            serde_json::json!({"v": 1}),
        )
        .await;
        let resolved = resolve(
            &mut cache,
            &key,
            &nudged_state,
            &calls,
            serde_json::json!({"v": 2}),
        )
        .await;

        // The nudge is within the documented threshold, so the action replays.
        assert!(
            state_drift(cached_state, &nudged_state) <= MAX_REPLAY_DRIFT,
            "the test's nudge must stay within MAX_REPLAY_DRIFT"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a within-threshold nudge still replays"
        );
        assert_eq!(resolved.source, ReplaySource::Cached);
    }

    #[tokio::test]
    async fn key_is_target_plus_method_with_state_as_the_drift_fingerprint() {
        let mut cache = ReplayCache::new();
        let calls = AtomicUsize::new(0);

        // Target + method are the lookup key. Two keys whose naive concatenation
        // would collide ("ab"+"c" vs "a"+"bc") must remain distinct decisions,
        // each with its own action (domain separation).
        let key_one = ReplayKey::new("ab", "c");
        let key_two = ReplayKey::new("a", "bc");
        resolve(
            &mut cache,
            &key_one,
            "state",
            &calls,
            serde_json::json!({"k": "one"}),
        )
        .await;
        resolve(
            &mut cache,
            &key_two,
            "state",
            &calls,
            serde_json::json!({"k": "two"}),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 2, "distinct keys each miss");

        let one = resolve(
            &mut cache,
            &key_one,
            "state",
            &calls,
            serde_json::json!({"k": "x"}),
        )
        .await;
        let two = resolve(
            &mut cache,
            &key_two,
            "state",
            &calls,
            serde_json::json!({"k": "x"}),
        )
        .await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "both replay without the model"
        );
        assert_eq!(one.action, serde_json::json!({"k": "one"}));
        assert_eq!(
            two.action,
            serde_json::json!({"k": "two"}),
            "no key collision"
        );

        // The method participates in the key: same target, different method.
        let other_method = ReplayKey::new("ab", "http");
        let other = resolve(
            &mut cache,
            &other_method,
            "state",
            &calls,
            serde_json::json!({"k": "m"}),
        )
        .await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "a different method is a different decision"
        );
        assert_eq!(other.source, ReplaySource::Resolved);

        // The state snapshot is NOT part of the lookup key — it is the drift
        // fingerprint — so the same target+method with a far-drifted state is a
        // re-resolve surfaced as DRIFT, not a fresh miss and not a silent hit.
        let drifted = resolve(
            &mut cache,
            &key_one,
            "an entirely unrelated state",
            &calls,
            serde_json::json!({"k": "drifted"}),
        )
        .await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            4,
            "a drifted state re-resolves the same decision"
        );
        assert_eq!(
            drifted.source,
            ReplaySource::Drifted,
            "state participates as the drift fingerprint, not the lookup key"
        );
    }

    #[tokio::test]
    async fn cache_round_trips_through_disk_and_replays() {
        let repo = TempDir::new().expect("temp repo");
        let key = ReplayKey::new("click submit", "cli");
        let calls = AtomicUsize::new(0);
        let action = serde_json::json!({"do": "it"});

        let mut cache = ReplayCache::load(repo.path(), IDENTITY).expect("load empty");
        resolve(&mut cache, &key, "state-a", &calls, action.clone()).await;
        let saved = cache.save(repo.path(), IDENTITY).expect("save");
        assert_eq!(
            saved,
            cache_path(repo.path(), IDENTITY).expect("cache path"),
            "the cache persists to its committed .expect/cache slot"
        );

        // A fresh load from disk replays the cached action with NO model call.
        let mut reloaded = ReplayCache::load(repo.path(), IDENTITY).expect("reload");
        let resolved = resolve(
            &mut reloaded,
            &key,
            "state-a",
            &calls,
            serde_json::json!({"do": "other"}),
        )
        .await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the reloaded cache replays without the model"
        );
        assert_eq!(resolved.source, ReplaySource::Cached);
        assert_eq!(resolved.action, action);
    }

    #[test]
    fn state_drift_is_zero_for_identical_and_one_for_disjoint() {
        assert_eq!(state_drift("a b c", "a b c"), 0.0);
        assert_eq!(state_drift("", ""), 0.0);
        assert_eq!(state_drift("a b", "c d"), 1.0);
    }
}
