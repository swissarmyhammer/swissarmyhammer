use crate::chat_template::ChatTemplateEngine;
use crate::generation::GenerationHelper;
use crate::model::ModelManager;

use crate::types::{
    FinishReason, GenerationRequest, GenerationResponse, QueueConfig, QueueError, Session,
    StreamChunk,
};
use async_trait::async_trait;
use llama_common::async_utils;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::model::LlamaModel;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};
use ulid::Ulid;

/// One cached session: the complete llama.cpp context state (including the KV
/// cache) plus the tokenization of the PROMPT those bytes were produced from.
///
/// `prompt_tokens` is `Some` for the streaming path, which uses a
/// longest-common-prefix check (see `prepare_streaming_kv_cache`) to verify the
/// cache is still a valid prefix of the next turn's prompt before reusing it —
/// rather than trusting a bare length comparison. The batch path stores `None`
/// (it relies on its own append-only `cached_message_count` bookkeeping).
///
/// `draft_state_bytes` is the per-seq snapshot of the MTP **draft** context's
/// KV cache (via `state_seq_get_data(seq=0)`), saved on streaming turns where
/// MTP was actually used. Without it, turn 2+ of an MTP-enabled session would
/// start with an empty draft context — its recurrent state has never seen the
/// prefix, so drafts collapse to noise, the target rejects them all, and we
/// pay double-prefill cost for zero speedup. Some draft contexts are large
/// (Q8 KV at full ctx) so this is bounded by the same byte budget as the
/// target state.
/// The state blobs are `Arc<[u8]>` so a forked session can alias its parent's
/// snapshot with NO byte copy (see [`SessionStateStore::fork`]) — the byte
/// budget counts a shared blob once, and handing a snapshot to a worker is an
/// `Arc` clone instead of a multi-hundred-MB `Vec` copy.
///
/// `pinned` entries are skipped by budget-driven eviction
/// ([`SessionStateStore::evict`]) so a prefix session a review fleet is
/// actively forking from cannot be dropped mid-flight. Lifecycle-driven
/// [`SessionStateStore::remove`] (the session is known to be gone) still
/// reclaims pinned entries.
struct CachedSession {
    state_bytes: Arc<[u8]>,
    prompt_tokens: Option<Vec<i32>>,
    draft_state_bytes: Option<Arc<[u8]>>,
    pinned: bool,
}

impl CachedSession {
    /// Combined byte footprint of this entry's blobs (shared or not). The
    /// store-wide budget uses [`SessionStateStore::cur_bytes`], which counts
    /// each shared blob once.
    fn byte_size(&self) -> usize {
        self.state_bytes.len() + self.draft_state_bytes.as_ref().map_or(0, |b| b.len())
    }
}

/// Cloned-out snapshot of a cached session — target state bytes, prompt
/// fingerprint tokens, and optional draft state bytes. Returned by
/// [`SessionStateStore::get`] so callers can drop the lock before the
/// comparatively slow `set_state_data`/`state_seq_set_data`. The byte blobs
/// are cheap `Arc` clones.
type CachedStateSnapshot = (Arc<[u8]>, Option<Vec<i32>>, Option<Arc<[u8]>>);

/// Observable status of one session's cached state, for the
/// `session/state_status` extension ("never fork blind"): a client confirms a
/// snapshot exists — and carries a prompt fingerprint — before forking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStateStatus {
    /// Number of prompt tokens the snapshot covers, when the snapshot carries
    /// a fingerprint (`None` for the batch path's unfingerprinted snapshots,
    /// which cannot seed a strict-prefix fork).
    pub prompt_tokens: Option<usize>,
    /// Byte size of the snapshot (target + draft blobs).
    pub state_bytes: usize,
    /// Whether the entry is pinned against budget eviction.
    pub pinned: bool,
}

/// Outcome of a successful session-state fork: what the child inherited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStateForkInfo {
    /// Number of prompt tokens the aliased snapshot covers — the strict
    /// prefix the fork's first decode resumes after.
    pub prefix_tokens: usize,
    /// Byte size of the aliased snapshot (shared with the parent, counted
    /// once by the budget).
    pub state_bytes: usize,
}

/// Why a session-state fork could not attach the parent's snapshot. The two
/// cases are deliberately distinguishable so a client can tell "no such
/// parent state" from "state exists but cannot seed a strict-prefix restore"
/// and react accordingly (re-prime vs plain new session).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionStateForkError {
    /// No cached snapshot exists for the parent session.
    #[error("no cached state for parent session {0}")]
    ParentStateNotFound(String),
    /// A snapshot exists but carries no prompt-token fingerprint (a batch-path
    /// save), so a strict-prefix restore cannot be verified against it.
    #[error(
        "cached state for parent session {0} has no prompt fingerprint and cannot seed a fork"
    )]
    ParentStateUnusable(String),
}

/// LRU, byte-bounded in-memory store of per-session llama.cpp context state for
/// efficient multi-turn conversations (restore without disk I/O).
///
/// Replaces a bare `HashMap<session_id, Vec<u8>>`, which had two problems the
/// streaming change made acute: (1) eviction iterated `HashMap` keys in
/// arbitrary order and dropped the first N — it could evict the ACTIVE session
/// and keep stale ones; (2) it bounded only entry COUNT, but each entry is a
/// FULL context-state copy (hundreds of MB on large models), so memory was
/// unbounded in bytes. This store evicts least-recently-used first and enforces
/// both an entry-count and a total-byte budget.
struct SessionStateStore {
    entries: HashMap<String, CachedSession>,
    /// Session ids ordered least-recently-used (front) to most-recently-used (back).
    lru: VecDeque<String>,
    max_entries: usize,
    max_bytes: usize,
    /// Count of budget-driven evictions ([`evict`](Self::evict)) since
    /// construction. Each increment is also emitted as a `warn!` so a
    /// pin-failure cluster is diagnosable from the log; the counter is the
    /// deterministic in-process observable the tests assert on (a `warn!`
    /// reaches whichever tracing subscriber is installed, which is racy to
    /// capture under a concurrent test harness, but the count is not).
    evictions: u64,
}

impl SessionStateStore {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
            max_entries: max_entries.max(1),
            max_bytes,
            evictions: 0,
        }
    }

    /// Total bytes held by the store, counting each shared blob ONCE.
    ///
    /// Forked entries alias their parent's `Arc<[u8]>` blobs, so summing
    /// per-entry sizes would charge the same hundreds-of-MB snapshot once per
    /// fork and evict aggressively for memory that is not actually used.
    /// Deduplication is by blob data pointer; the entry count is small (cores/2
    /// plus live forks), so the quadratic scan is negligible next to the
    /// multi-hundred-MB `memcpy`s this store brackets.
    fn cur_bytes(&self) -> usize {
        let mut seen: Vec<*const u8> = Vec::with_capacity(self.entries.len() * 2);
        let mut total = 0;
        for entry in self.entries.values() {
            for blob in std::iter::once(&entry.state_bytes).chain(entry.draft_state_bytes.iter()) {
                let ptr = blob.as_ptr();
                if !seen.contains(&ptr) {
                    seen.push(ptr);
                    total += blob.len();
                }
            }
        }
        total
    }

    fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Move `id` to the most-recently-used end of the LRU order.
    fn touch(&mut self, id: &str) {
        if let Some(pos) = self.lru.iter().position(|k| k == id) {
            if let Some(k) = self.lru.remove(pos) {
                self.lru.push_back(k);
            }
        }
    }

    /// Clone out a session's target state bytes + prompt fingerprint + draft
    /// state bytes, marking it most-recently-used. Bytes are cloned (not
    /// borrowed) so the caller can drop the lock before the comparatively slow
    /// `set_state_data`/`state_seq_set_data`.
    fn get(&mut self, id: &str) -> Option<CachedStateSnapshot> {
        let out = self.entries.get(id).map(|c| {
            (
                c.state_bytes.clone(),
                c.prompt_tokens.clone(),
                c.draft_state_bytes.clone(),
            )
        });
        if out.is_some() {
            self.touch(id);
        }
        out
    }

    /// Find the cached session whose stored prompt tokens have the longest
    /// common prefix with `new_tokens`, prioritising the caller's own session
    /// when its LCP ties.
    ///
    /// This is the local-side version of a service prompt-prefix cache (e.g.
    /// Anthropic's `cache_control` blocks): two ACP sessions that share the
    /// same system + tools header — which is the common case for the kanban
    /// app with multiple windows on the same agent — should not each pay the
    /// 28k-token cold prefill cost. The same `common_prefix_len` /
    /// `streaming_reuse_decision` machinery that handles within-session
    /// continuations handles cross-session sharing once the scan picks the
    /// right donor.
    ///
    /// Returns `None` when no cached entry shares any prefix with the new
    /// prompt (no usable cache) or when no entry carries a prompt fingerprint
    /// (e.g. the batch path's snapshots, which we cannot safely match
    /// against).
    fn find_best_prefix_match(
        &mut self,
        target_session_id: &str,
        new_tokens: &[i32],
    ) -> Option<PrefixMatch> {
        // Pass 1: scan immutably to identify the best (id, lcp). Two-pass so
        // we can `touch(id)` after, which mutably borrows.
        let mut best: Option<(String, usize)> = None;
        for (id, entry) in &self.entries {
            let Some(cached_tokens) = entry.prompt_tokens.as_ref() else {
                continue;
            };
            let lcp = common_prefix_len(cached_tokens, new_tokens);
            if lcp == 0 {
                continue;
            }
            let is_current = id == target_session_id;
            let take = match &best {
                None => true,
                Some((_, best_lcp)) if lcp > *best_lcp => true,
                // Tie-breaker: prefer the caller's own session id. Reusing
                // own state avoids a foreign-state set_state_data copy on
                // the typical warm-continuation case where the caller's
                // prior turn IS the longest-matching prefix.
                Some((best_id, best_lcp))
                    if lcp == *best_lcp && is_current && best_id != target_session_id =>
                {
                    true
                }
                _ => false,
            };
            if take {
                best = Some((id.clone(), lcp));
            }
        }

        let (source_id, lcp) = best?;
        self.touch(&source_id);
        let entry = self
            .entries
            .get(&source_id)
            .expect("just-touched id exists");
        Some(PrefixMatch {
            source_session_id: source_id.clone(),
            state_bytes: entry.state_bytes.clone(),
            draft_state_bytes: entry.draft_state_bytes.clone(),
            lcp,
        })
    }

    /// Insert/replace a session's cached state, then evict LRU entries until
    /// within BOTH the entry-count and total-byte budgets.
    ///
    /// Replacing an existing entry preserves its pinned flag — a pin applies
    /// to the session id until explicitly unpinned, across re-saves. A forked
    /// child's first own save lands here too, replacing its parent-aliased
    /// entry with fresh bytes (copy-on-write at save time) without touching
    /// the parent's entry.
    fn insert(
        &mut self,
        id: String,
        state_bytes: Vec<u8>,
        prompt_tokens: Option<Vec<i32>>,
        draft_state_bytes: Option<Vec<u8>>,
    ) {
        self.insert_inner(id, state_bytes, prompt_tokens, draft_state_bytes, false);
    }

    /// Insert/replace a session's cached state ATOMICALLY PINNED: the entry is
    /// born pinned, so it is never an eviction candidate from the moment its
    /// bytes land. This closes the prime→pin race — under the two-step
    /// "save then pin" protocol a freshly-saved (still unpinned) prefix could
    /// be evicted by another session's concurrent save before its pin landed,
    /// and the pin would then fail with `failed to pin primed prefix state`.
    /// Pinning at save time removes that window entirely.
    ///
    /// Used by the store-level race tests to validate the born-pinned
    /// invariant. Production currently relies on the RAM-scaled byte budget
    /// ([`default_max_cache_bytes`]) plus the [`MIN_SESSION_CACHE_ENTRIES`]
    /// floor holding every concurrently-primed prefix so the two-step protocol's
    /// pin always lands. Wiring this atomic save through the cross-crate prime
    /// path — so the race is closed structurally rather than only by budget
    /// headroom — is tracked as kanban task `01KV13WRXZSNYVDYDQRG3Z7786`
    /// (`local-review` project); these tests are its spec.
    #[cfg(test)]
    fn insert_pinned(&mut self, id: String, state_bytes: Vec<u8>, prompt_tokens: Option<Vec<i32>>) {
        self.insert_inner(id, state_bytes, prompt_tokens, None, true);
    }

    /// Shared insert body. `pin_on_save` forces the new entry pinned; otherwise
    /// the entry inherits any existing pin for this id (a pin applies to the
    /// session id across re-saves).
    fn insert_inner(
        &mut self,
        id: String,
        state_bytes: Vec<u8>,
        prompt_tokens: Option<Vec<i32>>,
        draft_state_bytes: Option<Vec<u8>>,
        pin_on_save: bool,
    ) {
        let pinned = pin_on_save || self.entries.get(&id).is_some_and(|e| e.pinned);
        self.entries.insert(
            id.clone(),
            CachedSession {
                state_bytes: Arc::from(state_bytes),
                prompt_tokens,
                draft_state_bytes: draft_state_bytes.map(Arc::from),
                pinned,
            },
        );
        if self.lru.iter().any(|k| k == &id) {
            self.touch(&id);
        } else {
            self.lru.push_back(id);
        }
        self.evict();
    }

    /// Alias the parent's cached snapshot under `child_id` with no byte copy:
    /// the child entry shares the parent's `Arc<[u8]>` blobs and is registered
    /// with the parent's prompt-token fingerprint, so the child's first prompt
    /// (which strictly extends the parent's) matches its own entry with
    /// `lcp == donor length` — a zero-rollback restore.
    ///
    /// The child starts unpinned; pin it separately if it must survive
    /// pressure. The parent entry is left untouched.
    ///
    /// # Errors
    ///
    /// [`SessionStateForkError::ParentStateNotFound`] when the parent has no
    /// cached snapshot, [`SessionStateForkError::ParentStateUnusable`] when
    /// the snapshot has no prompt fingerprint (a batch-path save) and so
    /// cannot seed a verified strict-prefix restore.
    fn fork(
        &mut self,
        parent_id: &str,
        child_id: String,
    ) -> Result<SessionStateForkInfo, SessionStateForkError> {
        let parent = self
            .entries
            .get(parent_id)
            .ok_or_else(|| SessionStateForkError::ParentStateNotFound(parent_id.to_string()))?;
        let Some(tokens) = parent.prompt_tokens.as_ref() else {
            return Err(SessionStateForkError::ParentStateUnusable(
                parent_id.to_string(),
            ));
        };
        let info = SessionStateForkInfo {
            prefix_tokens: tokens.len(),
            state_bytes: parent.byte_size(),
        };
        let child = CachedSession {
            state_bytes: parent.state_bytes.clone(),
            prompt_tokens: parent.prompt_tokens.clone(),
            draft_state_bytes: parent.draft_state_bytes.clone(),
            pinned: false,
        };
        self.entries.insert(child_id.clone(), child);
        if self.lru.iter().any(|k| k == &child_id) {
            self.touch(&child_id);
        } else {
            self.lru.push_back(child_id);
        }
        self.evict();
        Ok(info)
    }

    /// Report a session's cached-state status, or `None` when no snapshot is
    /// cached. A pure query: does not touch the LRU order.
    fn status(&self, id: &str) -> Option<SessionStateStatus> {
        self.entries.get(id).map(|entry| SessionStateStatus {
            prompt_tokens: entry.prompt_tokens.as_ref().map(|t| t.len()),
            state_bytes: entry.byte_size(),
            pinned: entry.pinned,
        })
    }

    /// Pin (or unpin) a session's cached entry against budget eviction.
    /// Returns whether an entry existed to update.
    fn set_pinned(&mut self, id: &str, pinned: bool) -> bool {
        match self.entries.get_mut(id) {
            Some(entry) => {
                entry.pinned = pinned;
                true
            }
            None => false,
        }
    }

    /// Drop least-recently-used UNPINNED entries until under both budgets.
    ///
    /// The most-recently-used entry (the active session's fresh save) is never
    /// a victim, and pinned entries are skipped — so when everything else is
    /// pinned the loop terminates over budget rather than deadlocking or
    /// evicting the active entry. Pinned bytes still count against the budget.
    fn evict(&mut self) {
        while self.entries.len() > self.max_entries || self.cur_bytes() > self.max_bytes {
            let victim = self
                .lru
                .iter()
                .take(self.lru.len().saturating_sub(1))
                .find(|id| !self.entries.get(id.as_str()).is_some_and(|e| e.pinned))
                .cloned();
            let Some(victim) = victim else {
                break;
            };
            // Eviction is the only way a not-yet-pinned prefix can vanish out
            // from under an imminent pin. It used to be silent — `grep evict`
            // on the log returned nothing — so a pin-failure cluster was
            // undiagnosable. Warn with the pressure that forced this drop so a
            // future cluster can be traced to budget rather than a logic bug.
            warn!(
                evicted_session = %victim,
                cur_bytes = self.cur_bytes(),
                max_bytes = self.max_bytes,
                entries = self.entries.len(),
                max_entries = self.max_entries,
                "session-state cache eviction under budget pressure (an unpinned entry was dropped)"
            );
            self.evictions += 1;
            self.remove(&victim);
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Number of budget-driven evictions since construction. Each eviction
    /// also emits a `warn!`; this counter is the deterministic observable the
    /// tests assert on.
    #[cfg(test)]
    fn eviction_count(&self) -> u64 {
        self.evictions
    }

    /// Reclaim a single session's cached state, returning whether an entry was
    /// removed. Unlike [`evict`](Self::evict) this is lifecycle-driven, not
    /// budget-driven: it deliberately bypasses the "keep at least one entry"
    /// guard because the caller knows this specific session is gone (cancelled,
    /// aborted, or idle-swept). Without it an ended session's full context-state
    /// blob — hundreds of MB of KV — stays pinned until the 2 GiB LRU budget
    /// forces it out, which with only one or two live sessions never happens, so
    /// the worker eventually runs out of KV slots (`NoKvCacheSlot`).
    fn remove(&mut self, id: &str) -> bool {
        if self.entries.remove(id).is_none() {
            return false;
        }
        if let Some(pos) = self.lru.iter().position(|k| k == id) {
            self.lru.remove(pos);
        }
        true
    }

    /// Drop all cached state (used on queue teardown).
    fn clear(&mut self) {
        self.entries.clear();
        self.lru.clear();
    }
}

type SessionStateCache = Arc<Mutex<SessionStateStore>>;

/// Result of [`SessionStateStore::find_best_prefix_match`].
///
/// Carries the donor session's id (purely informational, for logging which
/// cached state was reused) plus the bytes the caller restores into its
/// fresh context.
struct PrefixMatch {
    /// The session id whose cached state was selected — may differ from the
    /// caller's session id when we share a prefix across sessions.
    source_session_id: String,
    /// Full target-context state bytes for `set_state_data`.
    state_bytes: Arc<[u8]>,
    /// Per-seq draft KV bytes for MTP, when the donor turn used MTP.
    draft_state_bytes: Option<Arc<[u8]>>,
    /// Number of leading tokens that matched the new prompt.
    lcp: usize,
}

/// Floor for the session-state cache entry ceiling: enough slots to hold a
/// full review fleet's concurrently-primed prefixes (plus a margin for their
/// forks) so the entry-COUNT budget never count-evicts a not-yet-pinned
/// prefix before its pin lands. A review fans out ~15 validators; 64 covers
/// that fleet and its in-flight forks comfortably.
///
/// Memory is bounded by the RAM-scaled BYTE budget
/// ([`default_max_cache_bytes`]), so a generous entry floor does not risk
/// unbounded memory — it only stops the count budget from being the binding
/// constraint that reintroduces the prime→pin eviction race.
const MIN_SESSION_CACHE_ENTRIES: usize = 64;

/// How the entry ceiling scales with core count, expressed as
/// `numerator / denominator` (cores / 2 → one cache slot per two cores).
/// A scaling pair (mirroring [`SESSION_CACHE_RAM_FRACTION_NUM`]/`_DEN` for the
/// byte budget) rather than a bare `/ 2` literal so the cores→entries ratio is
/// self-documenting and consistent with the byte-budget fraction.
const SESSION_CACHE_ENTRIES_PER_CORE_NUM: usize = 1;
const SESSION_CACHE_ENTRIES_PER_CORE_DEN: usize = 2;

/// Entry ceiling for the session-state cache given a core count:
/// `max(MIN_SESSION_CACHE_ENTRIES, cores × NUM / DEN)`. Pure (takes the core
/// count) so it is unit-testable independent of the test machine's parallelism.
fn cache_entry_ceiling_for_cores(cores: usize) -> usize {
    (cores * SESSION_CACHE_ENTRIES_PER_CORE_NUM / SESSION_CACHE_ENTRIES_PER_CORE_DEN)
        .max(MIN_SESSION_CACHE_ENTRIES)
}

/// Default entry ceiling for the session-state cache: at least a full
/// validator fleet ([`MIN_SESSION_CACHE_ENTRIES`]), scaling with
/// [`SESSION_CACHE_ENTRIES_PER_CORE_NUM`]/`_DEN` (cores / 2) on larger machines.
fn default_max_cache_entries() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    cache_entry_ceiling_for_cores(cores)
}

/// Floor for the session-state cache byte budget: 2 GiB, the previous
/// hardcoded ceiling. The budget never drops below this even on a small or
/// memory-unreadable machine — a sub-2 GiB budget would evict prefixes
/// aggressively and reintroduce the prime→pin eviction race.
const MIN_SESSION_CACHE_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// Fraction of total system RAM the session-state cache may use, expressed as
/// `numerator / denominator` (0.25). Each cached entry is a full llama
/// context-state copy (hundreds of MB on large models), and a review fleet
/// primes ~15 validator prefixes plus their forks concurrently; a quarter of a
/// high-RAM box holds them all, so a pin requested for a just-saved prefix is
/// not defeated by another validator's concurrent save evicting it under a
/// too-small fixed budget.
const SESSION_CACHE_RAM_FRACTION_NUM: u64 = 1;
const SESSION_CACHE_RAM_FRACTION_DEN: u64 = 4;

/// Compute the cache byte budget from a machine's total memory:
/// `max(MIN_SESSION_CACHE_BYTES, total × fraction)`.
///
/// Pure (takes total bytes as an argument) so it is unit-testable without a
/// live `sysinfo` probe. A `total_memory` of 0 (sysinfo unavailable) yields
/// the floor.
fn cache_byte_budget_for_total_memory(total_memory: u64) -> usize {
    let scaled = total_memory / SESSION_CACHE_RAM_FRACTION_DEN * SESSION_CACHE_RAM_FRACTION_NUM;
    (scaled as usize).max(MIN_SESSION_CACHE_BYTES)
}

/// Total-byte ceiling for the session-state cache, scaled to detected system
/// RAM ([`cache_byte_budget_for_total_memory`]). Each entry is a full llama
/// context-state copy, so this bounds worst-case memory; scaling with RAM
/// (rather than a fixed 2 GiB) lets a high-RAM machine hold every validator
/// prefix + fork so no prime's pin loses the eviction race.
fn default_max_cache_bytes() -> usize {
    let total = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
    )
    .total_memory();
    cache_byte_budget_for_total_memory(total)
}

/// Detect whether a loaded model has an MTP / NextN head by sniffing the
/// GGUF metadata for an `*.nextn_predict_layers` entry.
///
/// The new llama-cpp-rs fork no longer exposes `LlamaModel::has_mtp()` or a
/// `nextn_predict_layers()` accessor; the hparam still lives in the model
/// file though, under a key like `qwen3.nextn_predict_layers`. Returns the
/// parsed count when present and positive (signals MTP is available), `None`
/// otherwise.
fn detect_nextn_predict_layers(model: &LlamaModel) -> Option<u32> {
    let meta_count = model.meta_count();
    for i in 0..meta_count {
        let key = match model.meta_key_by_index(i) {
            Ok(k) => k,
            Err(_) => continue,
        };
        if !key.ends_with(".nextn_predict_layers") {
            continue;
        }
        let value = match model.meta_val_str_by_index(i) {
            Ok(v) => v,
            Err(_) => return None,
        };
        return value.parse::<u32>().ok().filter(|n| *n > 0);
    }
    None
}

/// Length of the longest common prefix of two token sequences.
fn common_prefix_len(a: &[i32], b: &[i32]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// Check whether we have a cached KV-state for this session, and log the
/// resume/fresh-start decision. Returns true if the cache is usable.
fn check_and_log_session_cache(
    worker_id: usize,
    session: &Session,
    session_state_cache: &SessionStateCache,
) -> bool {
    let has_cached_state = {
        let cache = session_state_cache.lock().unwrap();
        cache.contains(&session.id.to_string())
    };
    let can_use_cache = has_cached_state && session.cached_message_count > 0;

    if can_use_cache {
        info!(
            "Worker {} continuing session {} from memory: {} cached messages, {} new messages to process",
            worker_id,
            session.id,
            session.cached_message_count,
            session.messages.len() - session.cached_message_count
        );
    } else {
        info!(
            "Worker {} starting new session {}: processing all {} messages",
            worker_id,
            session.id,
            session.messages.len()
        );
    }
    can_use_cache
}

/// Always render the FULL conversation — the restored KV cache will already
/// have the already-processed tokens, so llama.cpp only processes new ones.
fn render_session_prompt(
    worker_id: usize,
    chat_template: &ChatTemplateEngine,
    session: &Session,
    model: &LlamaModel,
    model_manager: &ModelManager,
) -> Result<String, QueueError> {
    info!(
        "Worker {} rendering full conversation: {} messages",
        worker_id,
        session.messages.len()
    );
    let prompt = chat_template
        .render_session_with_config(session, model, Some(model_manager.get_config()))
        .map_err(|e| {
            error!("Failed to render session prompt: {}", e);
            QueueError::WorkerError(format!("Template rendering failed: {}", e))
        })?;
    debug!(
        "Worker {} rendered prompt length: {} bytes",
        worker_id,
        prompt.len()
    );
    Ok(prompt)
}

/// Create the llama.cpp context for this inference request, with error wrapping.
fn create_session_context<'m>(
    model_manager: &ModelManager,
    model: &'m LlamaModel,
    session: &Session,
) -> Result<LlamaContext<'m>, QueueError> {
    model_manager
        .create_session_context(model, &session.id)
        .map_err(|e| {
            error!("Failed to create session context: {}", e);
            QueueError::WorkerError(format!("Session context creation failed: {}", e))
        })
}

/// Translate the KV-cache position into the `template_token_count` that
/// GenerationHelper expects (i.e. the *next* token position).
fn compute_template_token_count(worker_id: usize, kv_cache_position: i32) -> Option<usize> {
    if kv_cache_position < 0 {
        return None;
    }
    let next_position = (kv_cache_position + 1) as usize;
    info!(
        "Worker {} using token offset: {} tokens already in KV cache (position 0 to {})",
        worker_id, next_position, kv_cache_position
    );
    Some(next_position)
}

/// Render the session prompt for a streaming request. Matches the non-streaming
/// path's behaviour; errors are wrapped in the caller's preferred channel.
fn render_streaming_prompt(
    chat_template: &ChatTemplateEngine,
    session: &Session,
    model: &LlamaModel,
    model_manager: &ModelManager,
) -> Result<String, crate::types::TemplateError> {
    chat_template.render_session_with_config(session, model, Some(model_manager.get_config()))
}

/// Create the per-request llama.cpp context for streaming. Wraps the model
/// manager call so the caller can keep its error handling linear.
fn create_streaming_context<'m>(
    model_manager: &ModelManager,
    model: &'m LlamaModel,
    session: &Session,
) -> Result<LlamaContext<'m>, crate::types::ModelError> {
    model_manager.create_session_context(model, &session.id)
}

/// Push a worker-side error onto the streaming channel without ever blocking.
fn report_stream_error<E: std::fmt::Display>(
    stream_sender: &mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
    context: &str,
    error: &E,
) {
    error!("Streaming error: {}: {}", context, error);
    let _ = stream_sender.send(Err(QueueError::WorkerError(format!(
        "{}: {}",
        context, error
    ))));
}

/// Block on the shared receiver until a request arrives or the channel closes.
/// Logs the close event and returns None so the caller can break its loop.
async fn recv_next_request(
    receiver: &Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
    worker_id: usize,
) -> Option<QueuedRequest> {
    let mut receiver = receiver.lock().await;
    match receiver.recv().await {
        Some(request) => Some(request),
        None => {
            info!("Worker {} shutting down - channel closed", worker_id);
            None
        }
    }
}

/// Handle a request whose cancellation token was fired before we got around to
/// processing it: reply with a cancellation error and bump the metric.
fn reject_cancelled_request(
    worker_id: usize,
    queued_request: QueuedRequest,
    queue_time: Duration,
    metrics: &QueueMetrics,
) {
    warn!(
        "Worker {} dropping cancelled request {} (queued for {:?})",
        worker_id, queued_request.id, queue_time
    );
    let _ = queued_request
        .response_sender
        .send(Err(QueueError::WorkerError(
            "Request cancelled".to_string(),
        )));
    metrics.record_request_cancelled();
}

/// Lock-free counters describing the live state of a [`RequestQueue`].
///
/// Every counter is an atomic so workers and submitters can update them without
/// contending on a mutex. Call [`QueueMetrics::get_stats`] to take a
/// point-in-time snapshot.
#[derive(Debug, Default)]
pub struct QueueMetrics {
    /// Total number of requests ever submitted to the queue (including failures and cancels).
    pub total_requests: AtomicU64,
    /// Requests that completed successfully.
    pub completed_requests: AtomicU64,
    /// Requests that ended in a worker error.
    pub failed_requests: AtomicU64,
    /// Requests that were cancelled before or during processing.
    pub cancelled_requests: AtomicU64,
    /// Number of requests currently queued or in flight.
    pub current_queue_size: AtomicUsize,
    /// Sum of processing wall-time (milliseconds) across all completed requests.
    pub total_processing_time_ms: AtomicU64,
    /// Total tokens generated across all completed requests.
    pub total_tokens_generated: AtomicU64,
    /// Largest `current_queue_size` value observed since startup.
    pub peak_queue_size: AtomicUsize,
    /// Throughput (tokens/second) measured on the most recent completed request.
    pub last_throughput_tokens_per_second: AtomicU64,
}

impl QueueMetrics {
    /// Construct a fresh metrics block with every counter at zero.
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            completed_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            cancelled_requests: AtomicU64::new(0),
            current_queue_size: AtomicUsize::new(0),
            total_processing_time_ms: AtomicU64::new(0),
            total_tokens_generated: AtomicU64::new(0),
            peak_queue_size: AtomicUsize::new(0),
            last_throughput_tokens_per_second: AtomicU64::new(0),
        }
    }

    /// Increment submission counters and update `peak_queue_size` if we just
    /// surpassed the previous high-water mark.
    pub fn record_request_submitted(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let current_size = self.current_queue_size.fetch_add(1, Ordering::Relaxed) + 1;

        // Update peak queue size if necessary
        let mut peak = self.peak_queue_size.load(Ordering::Relaxed);
        while current_size > peak {
            match self.peak_queue_size.compare_exchange_weak(
                peak,
                current_size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    /// Record a successful completion: updates totals, processing time, and
    /// the rolling throughput measurement.
    pub fn record_request_completed(&self, processing_time: Duration, tokens_generated: u32) {
        self.completed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);

        let processing_ms = processing_time.as_millis() as u64;
        self.total_processing_time_ms
            .fetch_add(processing_ms, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(tokens_generated as u64, Ordering::Relaxed);

        // Calculate and store current throughput (tokens per second)
        if let Some(throughput) = (tokens_generated as u64 * 1000).checked_div(processing_ms) {
            self.last_throughput_tokens_per_second
                .store(throughput, Ordering::Relaxed);
        }
    }

    /// Record a failed request and decrement the live queue-size counter.
    pub fn record_request_failed(&self) {
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record a cancelled request and decrement the live queue-size counter.
    pub fn record_request_cancelled(&self) {
        self.cancelled_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

    /// Take a consistent snapshot of all counters as a plain `QueueStats`.
    pub fn get_stats(&self) -> QueueStats {
        QueueStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            completed_requests: self.completed_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            cancelled_requests: self.cancelled_requests.load(Ordering::Relaxed),
            current_queue_size: self.current_queue_size.load(Ordering::Relaxed),
            average_processing_time_ms: {
                let total_time = self.total_processing_time_ms.load(Ordering::Relaxed);
                let completed = self.completed_requests.load(Ordering::Relaxed);
                total_time.checked_div(completed).unwrap_or(0)
            },
            total_tokens_generated: self.total_tokens_generated.load(Ordering::Relaxed),
            peak_queue_size: self.peak_queue_size.load(Ordering::Relaxed),
            current_throughput_tps: self
                .last_throughput_tokens_per_second
                .load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time snapshot of the counters in a [`QueueMetrics`].
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// See [`QueueMetrics::total_requests`].
    pub total_requests: u64,
    /// See [`QueueMetrics::completed_requests`].
    pub completed_requests: u64,
    /// See [`QueueMetrics::failed_requests`].
    pub failed_requests: u64,
    /// See [`QueueMetrics::cancelled_requests`].
    pub cancelled_requests: u64,
    /// See [`QueueMetrics::current_queue_size`].
    pub current_queue_size: usize,
    /// Mean per-request processing time in milliseconds (0 if no completions yet).
    pub average_processing_time_ms: u64,
    /// See [`QueueMetrics::total_tokens_generated`].
    pub total_tokens_generated: u64,
    /// See [`QueueMetrics::peak_queue_size`].
    pub peak_queue_size: usize,
    /// Most recently observed throughput in tokens/second.
    pub current_throughput_tps: u64,
}

/// The inference half of the queue, abstracted behind a trait so the worker
/// loop's lifecycle (dequeue → run a turn → release the worker → record metrics)
/// can be exercised deterministically without a live llama.cpp model.
///
/// The single production implementation is [`ModelManagerExecutor`], which runs
/// the real `with_model(...)` + `GenerationHelper` inference path byte-for-byte.
/// Tests substitute a scripted executor so they can drive every turn outcome
/// (normal / EOS / max-tokens / context-full / error / cancel) and assert that
/// the worker is always released afterwards — the regression guard for the
/// "Queue is full on retry" bug.
///
/// Each method performs only the inference itself: it returns the outcome (or,
/// for streaming, pushes chunks onto the supplied sender) and leaves
/// metric-recording and response relay to the worker, exactly as the original
/// inline dispatch did.
#[async_trait]
pub(crate) trait QueueExecutor: Send + Sync {
    /// Run a batch (non-streaming) turn and return the full response, or a
    /// queue-level error if inference failed.
    async fn execute_batch(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
    ) -> Result<GenerationResponse, QueueError>;

    /// Run a streaming turn, pushing `StreamChunk`s onto `stream_sender` as they
    /// are produced. Returns `Ok(())` when the turn finished (the worker is then
    /// released regardless of outcome) or an error to relay onto the stream.
    async fn execute_streaming(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
    ) -> Result<(), QueueError>;
}

/// Production [`QueueExecutor`]: drives the real llama.cpp model through the
/// `ModelManager::with_model` borrow and `GenerationHelper`. This carries the
/// exact inference logic that previously lived inline in
/// `RequestQueue::dispatch_{batch,streaming}_request`.
pub(crate) struct ModelManagerExecutor {
    model_manager: Arc<ModelManager>,
    chat_template: Arc<ChatTemplateEngine>,
    session_config: crate::types::SessionConfig,
    session_state_cache: SessionStateCache,
    /// Machine-wide cross-process GPU lock. The in-process worker is already
    /// one-turn-at-a-time; this extends "one at a time" across every `sah serve`
    /// process sharing the single local GPU. Keyed on the model-source identity
    /// so it is data-driven, not a second hardcoded path.
    gpu_lock: crate::gpu_lock::GpuLock,
}

impl ModelManagerExecutor {
    /// Build the production executor from the shared model and queue state.
    fn new(
        model_manager: Arc<ModelManager>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
    ) -> Self {
        let gpu_lock =
            crate::gpu_lock::GpuLock::for_model(&model_manager.get_config().compute_model_hash());
        Self {
            model_manager,
            chat_template,
            session_config,
            session_state_cache,
            gpu_lock,
        }
    }

    /// Acquire the machine-wide GPU lock without stalling the async executor.
    ///
    /// [`crate::gpu_lock::GpuLock::acquire_blocking`] is a blocking
    /// `flock(LOCK_EX)` that parks the thread while another process holds the
    /// GPU, so it must run on a blocking thread. The returned guard is held
    /// across the synchronous generation turn and dropped immediately after,
    /// releasing the lock for the next process (or the next in-process turn).
    async fn acquire_gpu(&self) -> Result<crate::gpu_lock::GpuLockGuard, QueueError> {
        let lock = self.gpu_lock.clone();
        tokio::task::spawn_blocking(move || lock.acquire_blocking())
            .await
            .map_err(|e| QueueError::WorkerError(format!("GPU lock task panicked: {}", e)))?
            .map_err(|e| QueueError::WorkerError(format!("GPU lock acquisition failed: {}", e)))
    }
}

#[async_trait]
impl QueueExecutor for ModelManagerExecutor {
    async fn execute_batch(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
    ) -> Result<GenerationResponse, QueueError> {
        if !self.model_manager.is_loaded().await {
            return Err(QueueError::WorkerError("Model not loaded".to_string()));
        }
        let request_id = queued_request.id.clone();
        let start_time = Instant::now();
        // Hold the machine-wide GPU lock for the whole decode turn — one GPU,
        // one generation at a time across all serve processes. Dropped at the
        // end of this method, releasing it for the next turn/process.
        let _gpu_guard = self.acquire_gpu().await?;
        let result = self
            .model_manager
            .with_model(|model| {
                RequestQueue::process_batch_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    &self.model_manager,
                    &queued_request.cancellation_token,
                    &self.chat_template,
                    &self.session_config,
                    &self.session_state_cache,
                )
            })
            .await;
        let _ = start_time;
        match result {
            Ok(inner) => inner,
            Err(model_error) => Err(QueueError::WorkerError(format!(
                "Model error: {}",
                model_error
            ))),
        }
    }

    async fn execute_streaming(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
    ) -> Result<(), QueueError> {
        if !self.model_manager.is_loaded().await {
            return Err(QueueError::WorkerError("Model not loaded".to_string()));
        }
        let request_id = queued_request.id.clone();
        // Hold the machine-wide GPU lock for the whole streaming decode turn,
        // same as the batch path. Released when this method returns.
        let _gpu_guard = self.acquire_gpu().await?;
        let result = self
            .model_manager
            .with_model(|model| {
                RequestQueue::process_streaming_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    &self.model_manager,
                    stream_sender.clone(),
                    &queued_request.cancellation_token,
                    &self.chat_template,
                    &self.session_state_cache,
                )
            })
            .await;
        match result {
            Ok(inner) => inner,
            Err(model_error) => Err(QueueError::WorkerError(format!(
                "Model error: {}",
                model_error
            ))),
        }
    }
}

/// Envelope carrying a single request from `submit_request` to a worker task.
#[derive(Debug)]
pub struct QueuedRequest {
    /// Per-request ULID used for logging/tracing.
    pub id: String,
    /// The user-visible generation request.
    pub request: GenerationRequest,
    /// Session that owns this request (messages, KV cache identity, etc.).
    pub session: Session,
    /// oneshot channel for the batch response.
    pub response_sender: oneshot::Sender<Result<GenerationResponse, QueueError>>,
    /// Optional streaming channel. When set, the request is dispatched via the
    /// streaming code path instead of the batch path.
    pub stream_sender: Option<mpsc::UnboundedSender<Result<StreamChunk, QueueError>>>,
    /// When the request was enqueued (used for queue-time metrics).
    pub submitted_at: Instant,
    /// Token used by callers to cancel this specific request.
    pub cancellation_token: CancellationToken,
    /// Guard that clears this request's `active_requests` entry when the
    /// request is dropped. The streaming submit sets it so the entry lives
    /// exactly as long as the turn: the worker drops the request — and with
    /// it the guard — when the stream finishes (and the enqueue-failure
    /// paths drop it on unwind). `None` for the batch path, whose submit
    /// future holds its own guard, and for test-only enqueues that never
    /// track a token.
    ///
    /// Never read — it does its work in `Drop`, which the dead-code lint
    /// does not count as a use.
    #[allow(dead_code)]
    pub(crate) active_request_guard: Option<ActiveRequestGuard>,
}

/// A session's tracked in-flight request: the [`CancellationToken`] that
/// [`RequestQueue::cancel_session`] fires, tagged with the monotonic
/// generation id of the submit that inserted it so deferred cleanup (the
/// guard's spawned removal) can tell "my entry" from a newer turn's entry on
/// the same session.
#[derive(Debug)]
struct TrackedToken {
    generation: u64,
    token: CancellationToken,
}

/// Map of in-flight sessions to the [`TrackedToken`] that aborts each.
///
/// Shared between [`RequestQueue`] and [`ActiveRequestGuard`] so the guard can
/// clear an entry on any exit path.
type ActiveRequests = Arc<TokioMutex<HashMap<crate::types::SessionId, TrackedToken>>>;

/// RAII guard that removes a session's entry from `active_requests` when it is
/// dropped. The batch submit holds it across the whole
/// [`RequestQueue::submit_request`] call so every exit path (success, error,
/// cancellation) cleans up; the streaming submit instead hands it to the
/// [`QueuedRequest`] itself, so the entry lives exactly as long as the turn —
/// cleared when the worker finishes the stream (or the enqueue fails), never
/// leaked when a stream completes normally.
///
/// The map is a long-lived `tokio::Mutex`, and `Drop` cannot `.await`, so the
/// guard spawns a tiny detached task to take the lock and remove the key. This
/// avoids the leak the old success-only cleanup left behind: any early `?`
/// return (`enqueue_request` failing/cancelled, the response channel closing)
/// previously skipped the removal, so failed/cancelled requests accumulated
/// stale entries on the cached shared server.
///
/// Because the removal task is detached, it can run AFTER a back-to-back next
/// turn on the same session has tracked a fresh token. The removal is
/// therefore conditional on `generation`: it only deletes the entry if the
/// map still holds *this* guard's generation, so a stale cleanup never
/// clobbers a newer turn's token.
#[derive(Debug)]
pub(crate) struct ActiveRequestGuard {
    active_requests: ActiveRequests,
    session_id: crate::types::SessionId,
    /// The generation id [`RequestQueue::track_cancellation_token`] assigned
    /// to the entry this guard is responsible for.
    generation: u64,
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        let active_requests = self.active_requests.clone();
        let session_id = self.session_id;
        let generation = self.generation;
        // `Drop` is synchronous and the map is behind a `tokio::Mutex`; spawn
        // the removal so it never blocks the dropping task. The guard is
        // always dropped inside the Tokio runtime (submit futures and worker
        // tasks), so a handle is available.
        tokio::spawn(async move {
            let mut active = active_requests.lock().await;
            // Only remove the entry this guard tracked: a newer submit on the
            // same session may have already replaced it with a fresh token.
            if active
                .get(&session_id)
                .is_some_and(|tracked| tracked.generation == generation)
            {
                active.remove(&session_id);
            }
        });
    }
}

/// Bounded, multi-worker queue that routes `QueuedRequest`s through the
/// llama.cpp model and streams responses back to the caller.
pub struct RequestQueue {
    sender: Option<mpsc::Sender<QueuedRequest>>,
    worker_handles: Vec<JoinHandle<()>>,
    metrics: Arc<QueueMetrics>,
    _chat_template: Arc<ChatTemplateEngine>,
    _session_config: crate::types::SessionConfig,
    /// Track active requests by session ID for cancellation support
    active_requests: ActiveRequests,
    /// Monotonic source of the generation ids that tag `active_requests`
    /// entries (see [`TrackedToken`]).
    request_generation: AtomicU64,
    /// Kept alive for duration of queue - workers hold references to this cache
    #[allow(dead_code)]
    session_state_cache: SessionStateCache,
}

impl RequestQueue {
    /// Build a new `RequestQueue`, spawning `config.worker_threads` worker
    /// tasks that each share the provided model manager. Workers stay alive
    /// until the queue is dropped or `shutdown` is called.
    pub fn new(
        model_manager: Arc<ModelManager>,
        config: QueueConfig,
        session_config: crate::types::SessionConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let metrics = Arc::new(QueueMetrics::new());
        // The chat template engine needs the right strategy so it renders
        // tools and parses tool calls in the format the loaded model was
        // trained on. We derive the identifier from the model config in the
        // same way `AgentServer::initialize` does — see
        // `crate::agent::model_identifier_for_strategy`. Without this the
        // queue's engine stays strategy-less and silently falls back to the
        // legacy HashMap parsers.
        let model_identifier =
            crate::agent::model_identifier_for_strategy(model_manager.get_config());
        let chat_template = Arc::new(ChatTemplateEngine::with_model_strategy(&model_identifier));
        let session_state_cache: SessionStateCache = Arc::new(Mutex::new(SessionStateStore::new(
            default_max_cache_entries(),
            default_max_cache_bytes(),
        )));

        let executor: Arc<dyn QueueExecutor> = Arc::new(ModelManagerExecutor::new(
            model_manager.clone(),
            chat_template.clone(),
            session_config.clone(),
            session_state_cache.clone(),
        ));

        Self::assemble(
            sender,
            receiver,
            config,
            metrics,
            chat_template,
            session_config,
            session_state_cache,
            executor,
        )
    }

    /// Shared constructor body: spawn the workers against `executor` and build
    /// the `RequestQueue`. Both the production [`RequestQueue::new`] and the
    /// test-only `with_executor` constructor funnel through here so worker setup
    /// stays in one place.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        sender: mpsc::Sender<QueuedRequest>,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        config: QueueConfig,
        metrics: Arc<QueueMetrics>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
        executor: Arc<dyn QueueExecutor>,
    ) -> Self {
        let worker_handles = Self::spawn_workers(&config, &receiver, &metrics, &executor);

        // KV-cache reuse (SessionStateStore restore→generate→save) is NOT guarded
        // by a per-session lock: it is correct only when turns of a given session
        // are serialized. The default single worker guarantees that. With more
        // than one worker, two concurrent turns of the SAME session could
        // interleave restore and save and corrupt the cached state, so warn.
        if config.worker_threads > 1 {
            warn!(
                "RequestQueue configured with {} workers: per-session KV-cache reuse assumes \
                 single-worker serialization and has no per-session lock. Concurrent turns of \
                 the same session may corrupt cached context state. See follow-up card \
                 01KSSSQ6EP42C2TCHJWNY2JFNH.",
                config.worker_threads
            );
        }

        info!(
            "RequestQueue initialized with {} workers, max queue size: {}",
            config.worker_threads, config.max_queue_size
        );

        Self {
            sender: Some(sender),
            worker_handles,
            metrics,
            _chat_template: chat_template,
            _session_config: session_config,
            active_requests: Arc::new(TokioMutex::new(HashMap::new())),
            request_generation: AtomicU64::new(0),
            session_state_cache,
        }
    }

    /// Enqueue a batch request without awaiting its response or applying
    /// backpressure, returning only the non-blocking enqueue outcome. Used by
    /// capacity tests to fill the bounded channel and observe
    /// `QueueError::Full` at — and only at — capacity. This `try_send` is the
    /// only remaining non-blocking enqueue: both production submit paths
    /// route through [`RequestQueue::enqueue_request`] and wait for capacity.
    #[cfg(test)]
    fn try_enqueue_for_test(&self, session: &Session) -> Result<(), QueueError> {
        let (response_sender, _response_receiver) = oneshot::channel();
        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request: GenerationRequest {
                session_id: session.id,
                max_tokens: Some(8),
                temperature: Some(0.0),
                top_p: None,
                stop_tokens: Vec::new(),
                stopping_config: None,
            },
            session: session.clone(),
            response_sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token: CancellationToken::new(),
            active_request_guard: None,
        };
        self.metrics.record_request_submitted();
        let sender = self.sender.as_ref().expect("test queue sender present");
        if sender.try_send(queued_request).is_err() {
            self.metrics.record_request_failed();
            return Err(QueueError::Full);
        }
        Ok(())
    }

    /// Build a `RequestQueue` whose workers run turns through a caller-supplied
    /// [`QueueExecutor`] instead of the production model-backed executor.
    ///
    /// This is the seam the queue-lifecycle tests use to drive deterministic
    /// turn outcomes (normal / EOS / max-tokens / context-full / error / cancel)
    /// without a live llama.cpp model, exercising the real worker loop, release
    /// invariants, FIFO ordering, backpressure, and queue-full handling.
    #[cfg(test)]
    fn with_executor(config: QueueConfig, executor: Arc<dyn QueueExecutor>) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let metrics = Arc::new(QueueMetrics::new());
        let chat_template = Arc::new(ChatTemplateEngine::new());
        let session_state_cache: SessionStateCache = Arc::new(Mutex::new(SessionStateStore::new(
            default_max_cache_entries(),
            default_max_cache_bytes(),
        )));
        let session_config = crate::types::SessionConfig::default();

        Self::assemble(
            sender,
            receiver,
            config,
            metrics,
            chat_template,
            session_config,
            session_state_cache,
            executor,
        )
    }

    /// Spawn the configured number of worker tasks, cloning the shared receiver,
    /// metrics, and executor each iteration. Kept out of `new` so the
    /// constructor stays concise.
    fn spawn_workers(
        config: &QueueConfig,
        receiver: &Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        metrics: &Arc<QueueMetrics>,
        executor: &Arc<dyn QueueExecutor>,
    ) -> Vec<JoinHandle<()>> {
        (0..config.worker_threads)
            .map(|worker_id| {
                let receiver = receiver.clone();
                let metrics = metrics.clone();
                let executor = executor.clone();
                tokio::spawn(async move {
                    Self::worker_loop(worker_id, receiver, metrics, executor).await;
                })
            })
            .collect()
    }

    /// Submit a batch (non-streaming) generation request and await the full
    /// `GenerationResponse`.
    ///
    /// A saturated queue does **not** return [`QueueError::Full`]: this path
    /// applies backpressure, awaiting a free slot (see [`enqueue_request`]) so
    /// work is never silently dropped. Returns [`QueueError::WorkerError`] if
    /// the worker fails, [`QueueError::ShuttingDown`] if the queue is shutting
    /// down, and [`QueueError::Cancelled`] if the request's
    /// [`CancellationToken`] fires while waiting for capacity.
    ///
    /// [`enqueue_request`]: RequestQueue::enqueue_request
    pub async fn submit_request(
        &self,
        request: GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, QueueError> {
        let (response_sender, response_receiver) = oneshot::channel();
        let cancellation_token = CancellationToken::new();
        let session_id = request.session_id;

        let generation = self
            .track_cancellation_token(session_id, cancellation_token.clone())
            .await;
        // Clear the tracked entry on every exit path (success, error, cancel),
        // not just success: any early `?` return below would otherwise leak it.
        let _guard = ActiveRequestGuard {
            active_requests: self.active_requests.clone(),
            session_id,
            generation,
        };

        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request,
            session: session.clone(),
            response_sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token,
            active_request_guard: None,
        };
        debug!("Submitting request to queue: {}", queued_request.id);
        self.metrics.record_request_submitted();

        self.enqueue_request(queued_request).await?;

        let result = response_receiver
            .await
            .map_err(|_| QueueError::WorkerError("Response channel closed".to_string()))?;
        result
        // `_guard` drops here, clearing the `active_requests` entry.
    }

    /// Register a cancellation token for this session so concurrent cancels can
    /// find it. Returns the generation id assigned to the entry, which the
    /// submit's [`ActiveRequestGuard`] records so its deferred cleanup only
    /// removes the entry it created (never a later turn's fresh token).
    async fn track_cancellation_token(
        &self,
        session_id: crate::types::SessionId,
        token: CancellationToken,
    ) -> u64 {
        let generation = self.request_generation.fetch_add(1, Ordering::Relaxed);
        let mut active = self.active_requests.lock().await;
        active.insert(session_id, TrackedToken { generation, token });
        generation
    }

    /// Push the fully-built request onto the worker queue, applying
    /// backpressure when the bounded channel is full instead of dropping the
    /// request.
    ///
    /// A saturated local model (the review fan-out's single shared server) used
    /// to overflow the bounded queue and `try_send` would reject the task with
    /// [`QueueError::Full`], silently dropping work — the review then reported
    /// zero findings. Awaiting `send` makes the submitter WAIT for a worker to
    /// free a slot, which is the correct response to a busy model.
    ///
    /// The wait is cancellation-aware: the request's own
    /// [`CancellationToken`] aborts the `send` via `tokio::select!`, so a
    /// cancelled request never wedges the submitter against a permanently-full
    /// queue. Both production submit paths — batch
    /// ([`RequestQueue::submit_request`]) and streaming
    /// ([`RequestQueue::submit_streaming_request`]) — route through here, so
    /// neither ever observes [`QueueError::Full`]; the variant is preserved
    /// for the test-only capacity probe (`try_enqueue_for_test`).
    ///
    /// Failure modes are distinct variants: [`QueueError::Cancelled`] when the
    /// token fires while waiting, [`QueueError::ShuttingDown`] when the queue
    /// is closed.
    async fn enqueue_request(&self, queued_request: QueuedRequest) -> Result<(), QueueError> {
        let sender = self.sender.as_ref().ok_or_else(|| {
            warn!("Queue is shutting down, rejecting request");
            self.metrics.record_request_failed();
            QueueError::ShuttingDown
        })?;
        let cancellation_token = queued_request.cancellation_token.clone();
        tokio::select! {
            send_result = sender.send(queued_request) => {
                if send_result.is_err() {
                    warn!("Queue sender closed, rejecting request");
                    self.metrics.record_request_failed();
                    return Err(QueueError::ShuttingDown);
                }
                Ok(())
            }
            _ = cancellation_token.cancelled() => {
                warn!("Request cancelled while waiting for queue capacity");
                self.metrics.record_request_cancelled();
                Err(QueueError::Cancelled)
            }
        }
    }

    /// Submit a streaming generation request and receive an `mpsc::Receiver`
    /// that will emit `StreamChunk`s (or errors) as the model produces them.
    /// The receiver closes once generation completes.
    ///
    /// Like [`submit_request`], a saturated queue does **not** return
    /// [`QueueError::Full`]: the submission routes through
    /// [`enqueue_request`], waiting for a free slot so work (e.g. review
    /// fleet prompt turns) is never silently dropped. Returns
    /// [`QueueError::ShuttingDown`] if the queue is shutting down and
    /// [`QueueError::Cancelled`] if the request is cancelled while waiting
    /// for capacity — either failure clears the session's `active_requests`
    /// entry.
    ///
    /// [`submit_request`]: RequestQueue::submit_request
    /// [`enqueue_request`]: RequestQueue::enqueue_request
    pub async fn submit_streaming_request(
        &self,
        request: GenerationRequest,
        session: &Session,
    ) -> Result<mpsc::UnboundedReceiver<Result<StreamChunk, QueueError>>, QueueError> {
        let (response_sender, _) = oneshot::channel();
        // Unbounded by design: see the long-form rationale on
        // `send_with_backpressure` in generation/mod.rs. A bounded(100)
        // stream channel had two distinct producer-wedge failure modes
        // (consumer briefly behind → producer spins on Full;
        // consumer task suspended → producer spins forever). StreamChunks
        // are too small for memory pressure to matter at the decode rate.
        let (stream_sender, stream_receiver) = mpsc::unbounded_channel();

        let cancellation_token = CancellationToken::new();
        let session_id = request.session_id;

        let generation = self
            .track_cancellation_token(session_id, cancellation_token.clone())
            .await;
        // The streaming turn outlives this call, so the request itself owns
        // the cleanup guard: the entry stays tracked — and the in-flight
        // stream cancellable — until the worker finishes the turn and drops
        // the request, and the enqueue-failure paths below (cancelled or
        // shutting down) drop the request on unwind, clearing the entry
        // instead of leaking it.
        let guard = ActiveRequestGuard {
            active_requests: self.active_requests.clone(),
            session_id,
            generation,
        };

        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request,
            session: session.clone(),
            response_sender,
            stream_sender: Some(stream_sender),
            submitted_at: Instant::now(),
            cancellation_token,
            active_request_guard: Some(guard),
        };

        debug!(
            "Submitting streaming request to queue: {}",
            queued_request.id
        );
        self.metrics.record_request_submitted();

        self.enqueue_request(queued_request).await?;

        Ok(stream_receiver)
    }

    /// Return the number of requests currently queued or in flight, read from
    /// the live metrics counter.
    pub fn get_queue_size(&self) -> usize {
        // Use metrics for more accurate queue size
        self.metrics.current_queue_size.load(Ordering::Relaxed)
    }

    /// Cancel an active request for a session
    ///
    /// This triggers the cancellation token for any active request associated with
    /// the given session ID. If the session has an active request, the generation
    /// will be cancelled gracefully.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID whose request should be cancelled
    ///
    /// # Returns
    ///
    /// * `true` if an active request was found and cancelled
    /// * `false` if no active request was found for this session
    pub async fn cancel_session(&self, session_id: &crate::types::SessionId) -> bool {
        let mut active = self.active_requests.lock().await;
        if let Some(tracked) = active.remove(session_id) {
            debug!("Cancelling request for session: {}", session_id);
            tracked.token.cancel();
            true
        } else {
            debug!("No active request found for session: {}", session_id);
            false
        }
    }

    /// Reclaim a finished session's cached context state (its KV blob) from the
    /// process-wide worker cache, returning whether an entry was freed.
    ///
    /// The cache otherwise only sheds entries under LRU/byte-budget pressure
    /// ([`SessionStateStore::evict`]), so a session that has ended — cancelled,
    /// its turn aborted, or idle-swept — keeps its full context state (hundreds
    /// of MB) pinned indefinitely when there are too few live sessions to reach
    /// the budget. That leak eventually exhausts the worker's KV slots
    /// (`NoKvCacheSlot`). Callers on a session-teardown path invoke this so the
    /// KV is released promptly. Cheap and idempotent: a no-op `false` when the
    /// session has no cached state (e.g. it never reached a prompt boundary).
    pub fn evict_session_state(&self, session_id: &crate::types::SessionId) -> bool {
        self.session_state_cache
            .lock()
            .unwrap()
            .remove(&session_id.to_string())
    }

    /// Alias the parent session's cached context state under the child id with
    /// no byte copy (the blobs are shared `Arc`s and counted once by the byte
    /// budget). The child entry carries the parent's prompt-token fingerprint,
    /// so the child's first prompt — a strict extension of the parent's —
    /// restores the full donor state and decodes strictly forward with zero KV
    /// rollback.
    ///
    /// # Errors
    ///
    /// Distinguishable per [`SessionStateForkError`]: the parent has no cached
    /// snapshot, or its snapshot has no prompt fingerprint to verify a
    /// strict-prefix restore against.
    pub fn fork_session_state(
        &self,
        parent_id: &crate::types::SessionId,
        child_id: &crate::types::SessionId,
    ) -> Result<SessionStateForkInfo, SessionStateForkError> {
        self.session_state_cache
            .lock()
            .unwrap()
            .fork(&parent_id.to_string(), child_id.to_string())
    }

    /// Report whether a session's context state is cached — and forkable —
    /// plus its size and pin state. `None` when no snapshot is cached.
    pub fn session_state_status(
        &self,
        session_id: &crate::types::SessionId,
    ) -> Option<SessionStateStatus> {
        self.session_state_cache
            .lock()
            .unwrap()
            .status(&session_id.to_string())
    }

    /// Pin (or unpin) a session's cached context state against budget
    /// eviction, returning whether a cached entry existed to update. Pinned
    /// bytes still count against the budget; lifecycle eviction
    /// ([`evict_session_state`](Self::evict_session_state)) still reclaims
    /// pinned entries because the caller knows the session is gone.
    pub fn pin_session_state(&self, session_id: &crate::types::SessionId, pinned: bool) -> bool {
        self.session_state_cache
            .lock()
            .unwrap()
            .set_pinned(&session_id.to_string(), pinned)
    }

    /// Seed a session's cached state directly — test-only hook so handler
    /// tests can fabricate a "primed" parent without running a model.
    #[cfg(test)]
    pub(crate) fn seed_session_state_for_test(
        &self,
        session_id: &crate::types::SessionId,
        state_bytes: Vec<u8>,
        prompt_tokens: Option<Vec<i32>>,
    ) {
        self.session_state_cache.lock().unwrap().insert(
            session_id.to_string(),
            state_bytes,
            prompt_tokens,
            None,
        );
    }

    /// Convenience shortcut for `self.metrics.get_stats()` — returns a
    /// consistent snapshot of the queue's counters.
    pub fn get_stats(&self) -> QueueStats {
        self.metrics.get_stats()
    }

    /// Whether the queue can no longer accept work: every future submit would
    /// fail with [`QueueError::ShuttingDown`].
    ///
    /// True after an explicit [`shutdown`](Self::shutdown) and — crucially —
    /// when every worker task has died (panicked or been aborted, e.g. because
    /// the runtime that spawned them was dropped): the workers own the only
    /// receiver clones, so their death closes the channel. Callers holding a
    /// long-lived shared queue use this to detect a dead queue and rebuild
    /// instead of handing out a corpse forever.
    pub fn is_closed(&self) -> bool {
        self.sender.as_ref().is_none_or(|sender| sender.is_closed())
    }

    async fn worker_loop(
        worker_id: usize,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        metrics: Arc<QueueMetrics>,
        executor: Arc<dyn QueueExecutor>,
    ) {
        info!("Worker {} started", worker_id);
        while let Some(queued_request) = recv_next_request(&receiver, worker_id).await {
            let queue_time = queued_request.submitted_at.elapsed();
            debug!(
                "Worker {} processing request {} (queue time: {:?})",
                worker_id, queued_request.id, queue_time
            );
            if queued_request.cancellation_token.is_cancelled() {
                reject_cancelled_request(worker_id, queued_request, queue_time, &metrics);
                continue;
            }
            Self::process_request(worker_id, queued_request, &metrics, executor.as_ref()).await;
        }
    }

    /// Run a single dequeued request through the executor, then release the
    /// worker by recording the outcome and relaying the response. This is the
    /// heart of the worker-release invariant: every path through here ends with
    /// a metric update and a response send, so the live queue size always
    /// returns to its pre-request value once the turn finishes — regardless of
    /// whether the turn completed, hit EOS, ran out of budget, filled the
    /// context, errored, or was cancelled.
    async fn process_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
    ) {
        let start_time = Instant::now();
        let request_id = queued_request.id.clone();

        if queued_request.stream_sender.is_some() {
            Self::dispatch_streaming_request(
                worker_id,
                queued_request,
                metrics,
                executor,
                start_time,
            )
            .await;
        } else {
            Self::dispatch_batch_request(worker_id, queued_request, metrics, executor, start_time)
                .await;
        }

        let processing_time = start_time.elapsed();
        debug!(
            "Worker {} completed request {} in {:?}",
            worker_id, request_id, processing_time
        );
    }

    /// Drive a streaming request through the executor and relay completion/error
    /// back onto the stream sender and metrics.
    async fn dispatch_streaming_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
        start_time: Instant,
    ) {
        let stream_sender = queued_request
            .stream_sender
            .as_ref()
            .expect("streaming dispatch requires stream_sender")
            .clone();
        let result = executor
            .execute_streaming(worker_id, &queued_request, stream_sender.clone())
            .await;
        match result {
            Ok(_) => {
                // Tokens are tracked inside the executor's streaming path.
                metrics.record_request_completed(start_time.elapsed(), 0);
            }
            Err(queue_error) => {
                // UnboundedSender::send is synchronous (no .await).
                let _ = stream_sender.send(Err(queue_error));
                metrics.record_request_failed();
            }
        }
    }

    /// Drive a batch request through the executor and send the
    /// GenerationResponse back on the request's oneshot response channel.
    async fn dispatch_batch_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
        start_time: Instant,
    ) {
        // Run the turn while only borrowing the request, then move the response
        // sender out afterwards to deliver the result on its oneshot channel.
        let final_result = executor.execute_batch(worker_id, &queued_request).await;
        match &final_result {
            Ok(response) => {
                metrics.record_request_completed(start_time.elapsed(), response.tokens_generated)
            }
            Err(_) => metrics.record_request_failed(),
        }
        let _ = queued_request.response_sender.send(final_result);
    }

    #[allow(clippy::too_many_arguments)]
    fn process_batch_request_sync(
        worker_id: usize,
        request_id: String,
        request: &GenerationRequest,
        session: &Session,
        model: &LlamaModel,
        model_manager: &ModelManager,
        cancellation_token: &CancellationToken,
        chat_template: &ChatTemplateEngine,
        _session_config: &crate::types::SessionConfig,
        session_state_cache: &SessionStateCache,
    ) -> Result<GenerationResponse, QueueError> {
        let start_time = Instant::now();
        debug!(
            "Worker {} starting batch inference for request {}",
            worker_id, request_id
        );
        let can_use_cache = check_and_log_session_cache(worker_id, session, session_state_cache);
        let prompt =
            render_session_prompt(worker_id, chat_template, session, model, model_manager)?;
        let mut ctx = create_session_context(model_manager, model, session)?;
        let kv_cache_position = if can_use_cache {
            Self::restore_session_kv_cache(worker_id, session, &mut ctx, session_state_cache)?
        } else {
            -1
        };
        let template_token_count = compute_template_token_count(worker_id, kv_cache_position);
        let generation_result = Self::run_generation(
            worker_id,
            &request_id,
            model,
            &mut ctx,
            &prompt,
            request,
            cancellation_token,
            model_manager.get_batch_size(),
            template_token_count,
        )?;
        Ok(Self::finalize_batch_response(
            worker_id,
            request_id,
            session,
            &mut ctx,
            chat_template,
            session_state_cache,
            generation_result,
            start_time,
        ))
    }

    /// After generation completes, promote the finish reason for detected tool
    /// calls, persist the session state, and build the final response.
    #[allow(clippy::too_many_arguments)]
    fn finalize_batch_response(
        worker_id: usize,
        request_id: String,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        chat_template: &ChatTemplateEngine,
        session_state_cache: &SessionStateCache,
        generation_result: GenerationResponse,
        start_time: Instant,
    ) -> GenerationResponse {
        let final_finish_reason = Self::refine_finish_reason_for_tool_calls(
            worker_id,
            &request_id,
            chat_template,
            &generation_result.generated_text,
            generation_result.finish_reason.clone(),
        );
        let generation_time = start_time.elapsed();
        debug!(
            "Worker {} completed batch inference for request {} in {:?} ({} tokens, finish_reason: {:?})",
            worker_id,
            request_id,
            generation_time,
            generation_result.tokens_generated,
            final_finish_reason
        );
        // Batch path stores no prompt fingerprint — it gates reuse via
        // `cached_message_count` and its append-only message bookkeeping, not the
        // streaming path's longest-common-prefix check. Batch also doesn't run
        // MTP, so no draft-context state to snapshot.
        Self::save_session_state(
            worker_id,
            &request_id,
            session,
            ctx,
            session_state_cache,
            None,
            None,
        );
        GenerationResponse {
            generated_text: generation_result.generated_text,
            tokens_generated: generation_result.tokens_generated,
            generation_time,
            finish_reason: final_finish_reason,
            complete_token_sequence: generation_result.complete_token_sequence,
        }
    }

    /// Run a single generation pass against `ctx` with appropriate logging and
    /// error wrapping. Extracted from `process_batch_request_sync` so the main
    /// function stays at a manageable length.
    #[allow(clippy::too_many_arguments)]
    fn run_generation(
        worker_id: usize,
        request_id: &str,
        model: &LlamaModel,
        ctx: &mut LlamaContext<'_>,
        prompt: &str,
        request: &GenerationRequest,
        cancellation_token: &CancellationToken,
        batch_size: usize,
        template_token_count: Option<usize>,
    ) -> Result<GenerationResponse, QueueError> {
        debug!(
            "Queue worker {} calling GenerationHelper for request {}",
            worker_id, request_id
        );
        match GenerationHelper::generate_text_with_borrowed_model_and_template_offset(
            model,
            ctx,
            prompt,
            request,
            cancellation_token,
            batch_size,
            template_token_count,
        ) {
            Ok(result) => {
                debug!(
                    "Queue worker {} GenerationHelper returned success for request {}",
                    worker_id, request_id
                );
                Ok(result)
            }
            Err(e) => {
                error!(
                    "GenerationHelper failed for worker {} request {}: {}",
                    worker_id, request_id, e
                );
                debug!(
                    "Queue worker {} GenerationHelper error details: {:?}",
                    worker_id, e
                );
                Err(QueueError::WorkerError(format!("Generation failed: {}", e)))
            }
        }
    }

    /// Restore a session's llama.cpp context state from the in-memory cache and
    /// return the KV cache position after restoration. Errors if we expected a
    /// cache entry but the cache has been evicted.
    fn restore_session_kv_cache(
        worker_id: usize,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        session_state_cache: &SessionStateCache,
    ) -> Result<i32, QueueError> {
        info!(
            "Worker {} restoring session state from memory for session {}",
            worker_id, session.id
        );

        let state_bytes = {
            let mut cache = session_state_cache.lock().unwrap();
            cache
                .get(&session.id.to_string())
                .map(|(bytes, _tokens, _draft)| bytes)
        };

        let Some(bytes) = state_bytes else {
            warn!(
                "Worker {} expected cached state but not found in memory - will process all messages",
                worker_id
            );
            return Err(QueueError::WorkerError(
                "Expected state cache missing from memory".to_string(),
            ));
        };

        let bytes_len = bytes.len();
        let bytes_read = unsafe { ctx.set_state_data(&bytes) };
        let kv_cache_position = ctx.kv_cache_seq_pos_max(0);

        info!(
            "Worker {} restored state: {} bytes available, {} bytes read, {} cached messages, KV cache position: {}",
            worker_id, bytes_len, bytes_read, session.cached_message_count, kv_cache_position
        );

        Ok(kv_cache_position)
    }

    /// Inspect the generated text for tool calls when the model stopped for a
    /// "natural" reason, and upgrade the finish reason accordingly.
    fn refine_finish_reason_for_tool_calls(
        worker_id: usize,
        request_id: &str,
        chat_template: &ChatTemplateEngine,
        generated_text: &str,
        finish_reason: FinishReason,
    ) -> FinishReason {
        let should_check = matches!(
            &finish_reason,
            FinishReason::Stopped(reason)
                if reason == "End of sequence token detected"
                    || reason == "Stop token detected"
                    || reason == "Maximum tokens reached"
        );
        if !should_check {
            return finish_reason;
        }

        match chat_template.extract_tool_calls(generated_text) {
            Ok(tool_calls) if !tool_calls.is_empty() => {
                debug!(
                    "Worker {} detected {} tool calls in generated text for request {}",
                    worker_id,
                    tool_calls.len(),
                    request_id
                );
                FinishReason::Stopped("Tool call detected".to_string())
            }
            Ok(_) => {
                debug!(
                    "Worker {} no tool calls detected in generated text for request {}",
                    worker_id, request_id
                );
                finish_reason
            }
            Err(e) => {
                warn!(
                    "Worker {} failed to extract tool calls for request {}: {}",
                    worker_id, request_id, e
                );
                finish_reason
            }
        }
    }

    /// Snapshot the target context's state at the prompt boundary into the
    /// session cache.
    ///
    /// Called from the streaming generators' `on_prefill_complete` hook,
    /// immediately after the full prompt has been prefilled and BEFORE any
    /// generation token is sampled. The saved state therefore always ends
    /// at exactly `prompt_tokens.len()` positions, so the next turn's LCP
    /// trim has a rollback distance of 0 on the common path (next prompt
    /// extends this one) and at worst a small distance when the prompt
    /// diverges before reaching the saved end. Either way, the `n_rs_seq`
    /// recurrent-state snapshot window never has to roll back over the
    /// upcoming generated tokens — which is the failure mode the old
    /// post-generation save kept hitting whenever a turn generated more
    /// tokens than the window covered.
    ///
    /// Empty `prompt_tokens` (a tokenization failure upstream) skips the
    /// save: without a fingerprint the cache entry is unusable.
    #[allow(clippy::too_many_arguments)]
    fn save_prompt_boundary_state(
        worker_id: usize,
        request_id: &str,
        session: &Session,
        ctx: &LlamaContext<'_>,
        draft_ctx: Option<&LlamaContext<'_>>,
        session_state_cache: &SessionStateCache,
        prompt_tokens: &[i32],
    ) {
        if prompt_tokens.is_empty() {
            warn!(
                "Worker {} skipping prompt-boundary save for request {}: empty prompt fingerprint",
                worker_id, request_id
            );
            return;
        }

        let state_size = ctx.get_state_size();
        let mut state_bytes = vec![0u8; state_size];
        let bytes_written = unsafe { ctx.copy_state_data(state_bytes.as_mut_ptr()) };

        if bytes_written == 0 {
            warn!(
                "Worker {} failed to snapshot prompt-boundary state (wrote 0 bytes) for request {}",
                worker_id, request_id
            );
            return;
        }
        state_bytes.truncate(bytes_written);

        // Snapshot the MTP draft's per-seq KV when this turn ran with MTP.
        // The draft mirrors the target up to the prompt boundary at this
        // point — saving its per-seq state lets the next turn restore both
        // halves and keep speculative decoding warm.
        let draft_state_bytes: Option<Vec<u8>> = draft_ctx.map(|d| d.state_seq_get_data(0));
        let draft_bytes_len = draft_state_bytes.as_ref().map_or(0, |b| b.len());

        let mut cache = session_state_cache.lock().unwrap();
        cache.insert(
            session.id.to_string(),
            state_bytes,
            Some(prompt_tokens.to_vec()),
            draft_state_bytes,
        );
        info!(
            "Worker {} cached {} bytes of target + {} bytes of draft state at prompt boundary for session {} ({} messages, {} prompt tokens)",
            worker_id,
            bytes_written,
            draft_bytes_len,
            session.id,
            session.messages.len(),
            prompt_tokens.len()
        );
    }

    /// Copy the llama.cpp context state into the session cache so the next
    /// turn can resume without reprocessing prior messages. The store evicts
    /// LRU entries to stay within its entry-count and byte budgets.
    ///
    /// `prompt_tokens` is the tokenization of the prompt these state bytes were
    /// produced from. The streaming path passes `Some(..)` so the next turn can
    /// verify the cache is still a valid prefix (longest-common-prefix) before
    /// reusing it; the batch path passes `None` (it gates reuse differently).
    ///
    /// `draft_state_bytes` is the MTP draft context's per-seq KV snapshot (via
    /// `state_seq_get_data(0)`). Passed `Some(..)` by the streaming MTP path
    /// only — the next turn restores both target and draft together so the
    /// speculative head keeps its prefix context across turns. `None` everywhere
    /// else (batch turns and streaming turns that didn't use MTP).
    fn save_session_state(
        worker_id: usize,
        request_id: &str,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        session_state_cache: &SessionStateCache,
        prompt_tokens: Option<Vec<i32>>,
        draft_state_bytes: Option<Vec<u8>>,
    ) {
        let state_size = ctx.get_state_size();
        info!(
            "Worker {} saving session state to memory: {} bytes for {} messages",
            worker_id,
            state_size,
            session.messages.len()
        );

        let mut state_bytes = vec![0u8; state_size];
        let bytes_written = unsafe { ctx.copy_state_data(state_bytes.as_mut_ptr()) };

        if bytes_written == 0 {
            warn!(
                "Worker {} failed to copy state data (wrote 0 bytes) for request {}",
                worker_id, request_id
            );
            return;
        }

        state_bytes.truncate(bytes_written);

        let draft_bytes_len = draft_state_bytes.as_ref().map_or(0, |b| b.len());
        let mut cache = session_state_cache.lock().unwrap();
        cache.insert(
            session.id.to_string(),
            state_bytes,
            prompt_tokens,
            draft_state_bytes,
        );
        info!(
            "Worker {} cached {} bytes of target + {} bytes of draft state for session {} ({} messages)",
            worker_id,
            bytes_written,
            draft_bytes_len,
            session.id,
            session.messages.len()
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn process_streaming_request_sync(
        worker_id: usize,
        request_id: String,
        request: &GenerationRequest,
        session: &Session,
        model: &LlamaModel,
        model_manager: &ModelManager,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
        cancellation_token: &CancellationToken,
        chat_template: &ChatTemplateEngine,
        session_state_cache: &SessionStateCache,
    ) -> Result<(), QueueError> {
        debug!(
            "Worker {} starting streaming inference for request {}",
            worker_id, request_id
        );

        let prompt = match render_streaming_prompt(chat_template, session, model, model_manager) {
            Ok(p) => p,
            Err(e) => {
                report_stream_error(&stream_sender, "Template rendering failed", &e);
                return Ok(());
            }
        };
        let mut ctx = match create_streaming_context(model_manager, model, session) {
            Ok(c) => c,
            Err(e) => {
                report_stream_error(&stream_sender, "Session context creation failed", &e);
                return Ok(());
            }
        };
        trace!("Formatted prompt for streaming: {}", prompt);

        // Reuse the prior turn's KV cache so generation only decodes the NEW
        // tokens this turn. The ACP agentic loop runs entirely through this
        // streaming entry point, so without this every turn re-prefills the full
        // prompt (system prompt + all tool schemas + history) from scratch — the
        // dominant cost that made local generation feel far slower than a hosted
        // model that caches the prompt prefix. This mirrors the batch path's
        // restore/save lifecycle.
        let prep = Self::prepare_streaming_kv_cache(
            worker_id,
            session,
            &mut ctx,
            &prompt,
            model,
            session_state_cache,
        );
        let template_token_count = prep.template_offset;
        let cached_draft_bytes = prep.draft_state_bytes;

        // Tokenize the full prompt ONCE up front so the post-prefill hook can
        // store the fingerprint without re-tokenizing inside the closure
        // (and without duplicating the tokenization between the standard and
        // MTP paths). Empty on tokenization error falls back to "no save"
        // inside `save_prompt_boundary_state` via an empty prompt_tokens guard.
        let prompt_tokens_for_save: Vec<i32> = model
            .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
            .ok()
            .map(|toks| toks.into_iter().map(|t| t.0).collect())
            .unwrap_or_default();

        // Auto-detect MTP: when the loaded model carries the NextN/MTP head,
        // run the draft-mtp speculative loop with a second MTP-context on the
        // same model (target=this ctx + draft=ctx_type::Mtp). Same KV-reuse on
        // the target; the draft is ALSO reused across turns via `apply_draft_kv_state`
        // so its recurrent state has seen the prefix. Else: standard path.
        //
        // The new llama-cpp-rs fork no longer exposes `LlamaModel::has_mtp()`
        // or `nextn_predict_layers()` directly, so we sniff the GGUF metadata
        // for the `*.nextn_predict_layers` key — every MTP-capable architecture
        // writes that hparam (e.g. `qwen3.nextn_predict_layers = 1`).
        let nextn_layers = detect_nextn_predict_layers(model);
        let use_mtp = nextn_layers.unwrap_or(0) > 0;
        // `used_mtp` is what actually ran (vs. what we wanted). Determines
        // whether to snapshot the draft below — we only save when MTP truly
        // executed, otherwise a future restore would expect an aligned draft
        // that doesn't exist.
        let result = if use_mtp {
            match model_manager.create_draft_session_context(model, &session.id) {
                Ok(mut draft_ctx) => {
                    let draft_ready = Self::apply_draft_kv_state(
                        worker_id,
                        session,
                        &mut draft_ctx,
                        cached_draft_bytes,
                        template_token_count,
                    );
                    if draft_ready {
                        info!(
                            "Worker {} streaming with MTP speculative decoding (nextn_predict_layers={})",
                            worker_id,
                            nextn_layers.unwrap_or(0)
                        );
                        crate::generation::mtp::generate_stream_mtp(
                            model,
                            &mut ctx,
                            &mut draft_ctx,
                            &prompt,
                            request,
                            &stream_sender,
                            cancellation_token,
                            model_manager.get_batch_size(),
                            template_token_count,
                            crate::generation::mtp::MtpParams::default(),
                            // Snapshot target + draft at the prompt boundary,
                            // BEFORE generation begins. See the rationale on
                            // `save_prompt_boundary_state` — saving here is
                            // what makes the next turn's LCP trim rollback
                            // distance ~0 on the common path, so n_rs_seq=64
                            // never has to absorb a multi-hundred-token
                            // generation tail.
                            |target_at_boundary, draft_at_boundary| {
                                Self::save_prompt_boundary_state(
                                    worker_id,
                                    &request_id,
                                    session,
                                    target_at_boundary,
                                    Some(draft_at_boundary),
                                    session_state_cache,
                                    &prompt_tokens_for_save,
                                );
                            },
                        )
                        .map_err(|e| {
                            crate::types::QueueError::WorkerError(format!(
                                "MTP generation failed: {e}"
                            ))
                        })
                    } else {
                        // Draft restore/trim failed — fall back to standard
                        // streaming for this turn. Drops the stale draft_ctx
                        // (and its now-cleared KV) so next turn starts fresh.
                        GenerationHelper::generate_stream_with_borrowed_model_and_template_offset(
                            model,
                            &mut ctx,
                            &prompt,
                            request,
                            &stream_sender,
                            cancellation_token,
                            model_manager.get_batch_size(),
                            template_token_count,
                            |target_at_boundary| {
                                Self::save_prompt_boundary_state(
                                    worker_id,
                                    &request_id,
                                    session,
                                    target_at_boundary,
                                    None,
                                    session_state_cache,
                                    &prompt_tokens_for_save,
                                );
                            },
                        )
                        .map_err(|e| {
                            crate::types::QueueError::WorkerError(format!("Generation failed: {e}"))
                        })
                    }
                }
                Err(e) => {
                    warn!(
                        "Worker {} failed to create MTP draft context ({}); falling back to standard streaming",
                        worker_id, e
                    );
                    GenerationHelper::generate_stream_with_borrowed_model_and_template_offset(
                        model,
                        &mut ctx,
                        &prompt,
                        request,
                        &stream_sender,
                        cancellation_token,
                        model_manager.get_batch_size(),
                        template_token_count,
                        |target_at_boundary| {
                            Self::save_prompt_boundary_state(
                                worker_id,
                                &request_id,
                                session,
                                target_at_boundary,
                                None,
                                session_state_cache,
                                &prompt_tokens_for_save,
                            );
                        },
                    )
                    .map_err(|e| {
                        crate::types::QueueError::WorkerError(format!("Generation failed: {e}"))
                    })
                }
            }
        } else {
            GenerationHelper::generate_stream_with_borrowed_model_and_template_offset(
                model,
                &mut ctx,
                &prompt,
                request,
                &stream_sender,
                cancellation_token,
                model_manager.get_batch_size(),
                template_token_count,
                |target_at_boundary| {
                    Self::save_prompt_boundary_state(
                        worker_id,
                        &request_id,
                        session,
                        target_at_boundary,
                        None,
                        session_state_cache,
                        &prompt_tokens_for_save,
                    );
                },
            )
            .map_err(|e| crate::types::QueueError::WorkerError(format!("Generation failed: {e}")))
        };

        // No post-generation save: the prompt-boundary hook above already
        // snapshotted state right after prefill, before any token was
        // generated. The cached state therefore always ends at the exact
        // prompt boundary, so the next turn's LCP rollback is 0 on the
        // common path — n_rs_seq never has to absorb a multi-hundred-token
        // generation tail (the bug that surfaced as "n_rs_seq window
        // exceeded → cold full reprocess" every few turns). Disconnect /
        // cancellation are also no longer save-skip cases: the saved state
        // is the prompt state, which is valid whether or not generation
        // completed.

        log_streaming_result(worker_id, &request_id, &stream_sender, result);
        Ok(())
    }

    /// Restore the prior turn's KV cache into `ctx` and return the token offset
    /// to resume generation from, so streaming only decodes the new tokens.
    ///
    /// Robust prefix validation (no length-only guessing): we store the
    /// tokenization of the prompt the cache was built from, and here compute the
    /// longest common prefix (LCP) between that and the NEW prompt's
    /// tokenization. We then TRIM the KV cache to exactly the LCP
    /// (`clear_kv_cache_seq`) so the new tokens append cleanly onto a verified
    /// prefix. This is correct under ANY prompt change:
    ///   - append-only growth → LCP is the shared history, the new turn's
    ///     messages append (the prior turn's generation-prompt suffix and
    ///     generated tokens past the LCP are trimmed and re-decoded — cheap, it
    ///     is only the last assistant turn);
    ///   - compaction / history rewrite → divergence shortens the LCP, and only
    ///     the still-valid prefix is reused;
    ///   - a stale/partial save → the divergent suffix is trimmed, never decoded
    ///     against.
    ///
    /// Returns `None` ("process the full prompt", after clearing the KV) when
    /// there is no cached state, no stored fingerprint, tokenization fails, or
    /// the LCP leaves nothing useful to reuse (`0` or the whole new prompt).
    /// Gated on cache PRESENCE (not `cached_message_count`) so the streaming/ACP
    /// path self-heals across turns without extra bookkeeping.
    ///
    /// Returns the resume token offset (`StreamingKvPrep::template_offset`) AND
    /// any cached draft-context bytes the MTP path should restore after creating
    /// its draft context. We extract the draft bytes here (rather than in a
    /// second cache lookup) so the LRU touch is single-shot and the bytes are
    /// available even on paths that don't re-enter the cache.
    fn prepare_streaming_kv_cache(
        worker_id: usize,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        prompt: &str,
        model: &LlamaModel,
        session_state_cache: &SessionStateCache,
    ) -> StreamingKvPrep {
        use llama_cpp_2::model::AddBos;

        // Tokenize the new prompt FIRST so the cache scan can compute an LCP
        // against every candidate donor's stored prompt tokens. Tokenization
        // is microseconds for 28k tokens — fast relative to the prefill it
        // potentially saves.
        let new_tokens: Vec<i32> = match model.str_to_token(prompt, AddBos::Always) {
            Ok(toks) => toks.into_iter().map(|t| t.0).collect(),
            Err(e) => {
                warn!(
                    "Worker {} streaming: failed to tokenize prompt for prefix check ({}); processing full prompt",
                    worker_id, e
                );
                return StreamingKvPrep::default();
            }
        };

        // Cross-session prefix matching: scan ALL cached states for the best
        // LCP match, not just this session's. This is the local equivalent
        // of a hosted prompt-prefix cache — two windows on the same agent
        // share the 27k-token system+tools header, and a fresh session
        // should NOT pay a cold 28k prefill when another session already
        // has those tokens warm in KV.
        let match_result = {
            let mut cache = session_state_cache.lock().unwrap();
            cache.find_best_prefix_match(&session.id.to_string(), &new_tokens)
        };
        let Some(prefix_match) = match_result else {
            info!(
                "Worker {} streaming: no usable cached prefix for session {} across {} cached donors, processing full prompt",
                worker_id,
                session.id,
                // Re-lock briefly only for the count; cheap.
                session_state_cache.lock().unwrap().len()
            );
            return StreamingKvPrep::default();
        };

        let PrefixMatch {
            source_session_id,
            state_bytes: bytes,
            draft_state_bytes,
            lcp,
        } = prefix_match;

        let is_cross_session = source_session_id != session.id.to_string();
        if is_cross_session {
            info!(
                "Worker {} streaming: reusing cached state from session {} as prefix donor for session {} (lcp={} of {} new tokens)",
                worker_id,
                source_session_id,
                session.id,
                lcp,
                new_tokens.len()
            );
        }

        // Diagnostic: when the LCP is surprisingly short (the kanban perf
        // pain is two windows on the same agent diverging at byte 47 out of
        // 27k+ that should be byte-identical), dump the slice around the
        // divergence point. Decoding 32 tokens on either side of the split
        // shows exactly what session-specific bytes are leaking into the
        // rendered prompt — e.g. a session ULID, the cwd, a timestamp, or
        // non-deterministic tool ordering from HashMap-backed MCP discovery.
        //
        // The bar of `lcp < new_tokens.len() / 2` matches "you expected to
        // reuse most of the prompt but barely reused anything" — exactly
        // the case worth investigating; full continuations and exact-match
        // reuses don't trip it.
        if is_cross_session && lcp < new_tokens.len() / 2 {
            // Decode the slice just before and after the divergence point
            // on each side, falling back to raw token ids if detokenization
            // fails for any reason (e.g. partial UTF-8). Bounded to keep
            // the log line readable.
            const DIAG_WIN: usize = 32;
            let _maybe_cached = session_state_cache.lock().unwrap();
            // We don't have the donor's `prompt_tokens` here — only their
            // bytes and lcp. Decode what we DO have: the new prompt's
            // boundary slice. If the donor is misaligned, the recipient's
            // boundary alone is enough to identify the variable region.
            let start = lcp.saturating_sub(DIAG_WIN);
            let end = (lcp + DIAG_WIN).min(new_tokens.len());
            let new_slice: Vec<llama_cpp_2::token::LlamaToken> = new_tokens[start..end]
                .iter()
                .map(|&t| llama_cpp_2::token::LlamaToken(t))
                .collect();
            let decoded = model
                .tokens_to_str(&new_slice, llama_cpp_2::model::Special::Tokenize)
                .unwrap_or_else(|_| format!("{:?}", &new_tokens[start..end]));
            warn!(
                "Worker {} streaming: cross-session LCP is only {} of {} tokens — investigate session-specific content in the rendered prompt. New prompt around divergence (tokens {}..{}): {:?}",
                worker_id,
                lcp,
                new_tokens.len(),
                start,
                end,
                decoded
            );
        }

        // Load the donor state, then verify+trim against the new prompt.
        let _bytes_read = unsafe { ctx.set_state_data(&bytes) };

        match streaming_reuse_decision(lcp, new_tokens.len()) {
            Some(offset) => {
                // Trim the KV to exactly the verified common prefix so the new
                // tokens append cleanly. This drops the prior turn's divergent
                // tail (its generation-prompt suffix + generated tokens, and any
                // rewritten span after compaction) — positions `[offset, ∞)`.
                //
                // On hybrid attention + recurrent contexts `seq_rm` can return
                // `Ok(false)` *silently* when the rollback distance exceeds the
                // context's `n_rs_seq` snapshot ring. The KV's `max_pos`
                // doesn't drop in that case, and the next decode trips
                // M-RoPE's position-monotonicity check. We have to surface that
                // and fall back to a cold full reprocess.
                let trim_result = ctx.clear_kv_cache_seq(Some(0), Some(offset as u32), None);
                match trim_result {
                    Ok(true) => { /* trimmed cleanly */ }
                    Ok(false) => {
                        warn!(
                            "Worker {} streaming: KV trim to common prefix returned false \
                             (rollback distance likely exceeded the recurrent-state snapshot \
                             window) — invalidating cache and processing full prompt",
                            worker_id
                        );
                        ctx.clear_kv_cache();
                        return StreamingKvPrep::default();
                    }
                    Err(e) => {
                        warn!(
                            "Worker {} streaming: failed to trim KV to common prefix ({:?}); processing full prompt",
                            worker_id, e
                        );
                        ctx.clear_kv_cache();
                        return StreamingKvPrep::default();
                    }
                }
                info!(
                    "Worker {} streaming reusing {} cached tokens, processing {} new tokens for session {}",
                    worker_id,
                    offset,
                    new_tokens.len().saturating_sub(offset),
                    session.id
                );
                // Pass the cached draft bytes through to the caller. If MTP is
                // active this turn, `apply_draft_kv_state` restores them into
                // the freshly-created draft context and trims it to the same
                // offset, so the speculative head keeps its prefix.
                StreamingKvPrep {
                    template_offset: Some(offset),
                    draft_state_bytes,
                }
            }
            None => {
                info!(
                    "Worker {} streaming: cached prefix for session {} no longer matches (common prefix {} of {} tokens); processing full prompt",
                    worker_id,
                    session.id,
                    lcp,
                    new_tokens.len()
                );
                ctx.clear_kv_cache();
                // No target reuse → no draft reuse either. The draft context is
                // brand new and would need a separate prefill to be useful, which
                // turn-1 cold-start already handles in the MTP streaming loop.
                StreamingKvPrep::default()
            }
        }
    }

    /// Restore a cached draft-context KV snapshot into a freshly-created draft
    /// context and trim it to `template_offset` so the speculative head's
    /// prefix lines up exactly with the target's. Returns `true` when the draft
    /// is ready to be used in MTP; `false` when the caller should skip MTP for
    /// this turn (fall back to standard streaming).
    ///
    /// Without this, turn 2+ of an MTP session runs with an empty draft
    /// context: the recurrent state never saw [0..offset), so drafts at high
    /// positions are uninformed, acceptance collapses to zero, and we pay
    /// double-prefill cost (per-chunk `sync_capture` on the draft) for no
    /// speedup. With it, only the [offset..total] suffix is mirrored on the
    /// draft — which is the entire point of the target-side KV reuse.
    fn apply_draft_kv_state(
        worker_id: usize,
        session: &Session,
        draft_ctx: &mut LlamaContext<'_>,
        cached_bytes: Option<Arc<[u8]>>,
        template_offset: Option<usize>,
    ) -> bool {
        // No target reuse this turn → caller will cold-prefill the draft on
        // every new token via `sync_capture`. That's correct (turn 1 path).
        let Some(offset) = template_offset else {
            return true;
        };
        let Some(bytes) = cached_bytes else {
            // We reused the target's state but never saved the draft (e.g.
            // turn 1 was processed without MTP, or save was skipped). The
            // draft starts empty — its recurrent state for [0..offset) is
            // missing, so drafts will be poor. Skip MTP for this turn so we
            // don't pay double-prefill cost for no speedup; next clean turn
            // will save draft state and resume MTP cleanly.
            info!(
                "Worker {} MTP: target KV reused at offset {} but no cached draft state for session {} — skipping MTP this turn",
                worker_id, offset, session.id
            );
            return false;
        };

        match draft_ctx.state_seq_set_data(&bytes, 0) {
            Ok(read) => {
                info!(
                    "Worker {} MTP: restored {} bytes of draft KV state for session {}",
                    worker_id, read, session.id
                );
            }
            Err(e) => {
                warn!(
                    "Worker {} MTP: failed to restore draft KV state ({:?}); skipping MTP this turn",
                    worker_id, e
                );
                draft_ctx.clear_kv_cache();
                return false;
            }
        }

        // Align draft KV to the same offset the target was trimmed to. The
        // saved draft state ran past the new LCP (it ended at the prior
        // turn's end-of-generation); the rollback distance is the prior
        // turn's generation length. Same `Ok(false)` silent-failure path as
        // the target trim — fall back to skipping MTP rather than running
        // with stale KV positions tripping M-RoPE's invariant.
        let trim_result = draft_ctx.clear_kv_cache_seq(Some(0), Some(offset as u32), None);
        match trim_result {
            Ok(true) => true,
            Ok(false) => {
                warn!(
                    "Worker {} MTP: draft KV trim to offset {} returned false (rollback exceeded recurrent-state window) — skipping MTP this turn",
                    worker_id, offset
                );
                draft_ctx.clear_kv_cache();
                false
            }
            Err(e) => {
                warn!(
                    "Worker {} MTP: failed to trim draft KV to offset {} ({:?}); skipping MTP this turn",
                    worker_id, offset, e
                );
                draft_ctx.clear_kv_cache();
                false
            }
        }
    }
}

/// What `prepare_streaming_kv_cache` decided about reusing the prior turn's KV
/// state. Carries both the target-side resume offset and any cached draft-side
/// bytes so the streaming MTP path can keep its draft context aligned with the
/// target across turns.
#[derive(Default)]
struct StreamingKvPrep {
    /// Token offset to resume target generation from (`None` = full reprocess).
    template_offset: Option<usize>,
    /// Per-seq draft-context bytes from the prior turn, when MTP was used.
    /// `None` when the prior turn didn't use MTP or no state was cached.
    draft_state_bytes: Option<Arc<[u8]>>,
}

/// Decide the streaming resume offset from the longest-common-prefix length
/// between the cached prompt tokens and the new prompt tokens.
///
/// Returns `Some(lcp)` — resume decoding at position `lcp`, the verified shared
/// prefix — when `0 < lcp < new_len`. Returns `None` ("reprocess the full
/// prompt") when there is nothing reusable (`lcp == 0`) or the cache already
/// covers the entire new prompt (`lcp >= new_len`, i.e. no new tokens to
/// decode). Because the caller trims the KV to exactly `lcp` before decoding,
/// any cached tokens beyond `lcp` are discarded — so this needs no separate
/// staleness/length guard.
fn streaming_reuse_decision(lcp: usize, new_len: usize) -> Option<usize> {
    if lcp == 0 || lcp >= new_len {
        return None;
    }
    Some(lcp)
}

/// Log the outcome of a streaming generation and, on error, relay the failure
/// onto the client's stream channel.
fn log_streaming_result(
    worker_id: usize,
    request_id: &str,
    stream_sender: &mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
    result: Result<(), impl std::fmt::Display + std::fmt::Debug>,
) {
    match result {
        Ok(()) => debug!(
            "Worker {} completed streaming inference for request {} using GenerationHelper",
            worker_id, request_id
        ),
        Err(e) => {
            error!(
                "GenerationHelper streaming failed for worker {} request {}: {}",
                worker_id, request_id, e
            );
            report_stream_error(stream_sender, "Generation failed", &e);
        }
    }
}

impl RequestQueue {
    /// Gracefully shutdown the queue, waiting for all workers to complete
    ///
    /// This method implements cooperative shutdown where workers complete their current
    /// requests naturally without artificial time limits. Workers detect shutdown when
    /// the channel closes and stop accepting new requests, but continue processing any
    /// request they have already started. This ensures request integrity and proper
    /// resource cleanup without forced termination.
    pub async fn shutdown(mut self) {
        info!("RequestQueue shutting down gracefully");
        let shutdown_start = Instant::now();
        let stats = self.get_stats();

        info!(
            "Shutdown initiated with {} requests in queue, {} total processed",
            stats.current_queue_size, stats.total_requests
        );

        // Close the sender to signal workers to shut down. This MUST happen
        // before we await the worker handles: a worker only exits its loop once
        // `recv()` returns `None`, which only happens after every sender is
        // dropped. Dropping the sender here (rather than letting `self` drop at
        // the end of the method) is what lets the awaits below complete instead
        // of deadlocking.
        self.sender = None;

        // Wait for all worker handles to complete gracefully
        let mut successful_shutdowns = 0;
        let total_workers = self.worker_handles.len();

        info!(
            "Waiting for {} workers to complete current requests...",
            total_workers
        );

        for (i, handle) in self.worker_handles.drain(..).enumerate() {
            info!("Waiting for worker {} to complete current request...", i);

            match handle.await {
                Ok(()) => {
                    info!("Worker {} shutdown successfully", i);
                    successful_shutdowns += 1;
                }
                Err(join_error) => {
                    warn!("Worker {} panicked during shutdown: {}", i, join_error);
                }
            }
        }

        let shutdown_duration = shutdown_start.elapsed();

        info!(
            "RequestQueue shutdown complete in {:?}: {}/{} workers successful",
            shutdown_duration, successful_shutdowns, total_workers
        );
    }

    /// Shutdown with timeout and return statistics
    pub async fn shutdown_with_timeout(self, timeout: Duration) -> QueueStats {
        let stats_before = self.get_stats();
        info!(
            "Starting RequestQueue shutdown with {} timeout",
            Pretty(&timeout)
        );

        let shutdown_future = async {
            self.shutdown().await;
            Ok::<_, ()>(())
        };

        let _result = async_utils::with_timeout_action(
            shutdown_future,
            timeout,
            async_utils::TimeoutAction::LogWarning,
            &format!(
                "RequestQueue shutdown (had {} requests in queue)",
                stats_before.current_queue_size
            ),
        )
        .await;

        if _result.is_ok() && _result.as_ref().unwrap().is_some() {
            info!("RequestQueue shutdown completed within timeout");
        }

        stats_before
    }
}

impl Drop for RequestQueue {
    fn drop(&mut self) {
        info!(
            "RequestQueue dropping - {} worker handles remaining",
            self.worker_handles.len()
        );

        // Drop sender to signal workers to shut down
        // This closes the channel, causing receiver.recv() to return None
        if self.sender.take().is_some() {
            info!("Closed sender channel to signal workers to shutdown");
        }

        // Clear session state cache to release any remaining references
        {
            let mut cache = self.session_state_cache.lock().unwrap();
            let cache_size = cache.len();
            if cache_size > 0 {
                info!(
                    "Clearing {} session state cache entries before cleanup",
                    cache_size
                );
                cache.clear();
            }
        }

        // Note: Worker handles will be aborted when dropped
        // For graceful shutdown with proper cleanup, use shutdown() or shutdown_with_timeout()
        // instead of relying on Drop, which must be non-blocking to avoid hanging tests
        info!("RequestQueue cleanup complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Message, MessageRole, ModelConfig, ModelSource, QueueConfig, RetryConfig, Session,
        SessionId,
    };
    use std::path::PathBuf;
    use std::time::SystemTime;
    use tempfile::TempDir;

    /// Build a synthetic state blob of `len` bytes, all set to `byte` —
    /// shared by the session-state store unit-test submodules.
    fn state(byte: u8, len: usize) -> Vec<u8> {
        vec![byte; len]
    }

    fn create_test_model_config() -> ModelConfig {
        ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/tmp"),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        }
    }

    fn create_test_queue_config() -> QueueConfig {
        QueueConfig {
            max_queue_size: 10,
            worker_threads: 2,
        }
    }

    fn create_test_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
            title: None,
        }
    }

    async fn setup_loaded_model_manager() -> Arc<ModelManager> {
        let temp_dir = TempDir::new().unwrap();
        let model_file = temp_dir.path().join("test.gguf");

        // Create dummy model file
        tokio::fs::write(&model_file, b"dummy model").await.unwrap();

        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        };

        let manager = Arc::new(ModelManager::new(config).expect("Failed to create ModelManager"));

        // Note: We don't actually load the model since dummy GGUF files fail
        // The queue tests should focus on queue functionality, not model loading
        // In a real application, the model would be properly loaded

        // Note: temp_dir will be automatically cleaned up when it goes out of scope
        // For test purposes, this is fine as the model manager only needs the path
        // during initialization, not for the entire lifetime
        drop(temp_dir);

        manager
    }

    #[tokio::test]
    async fn test_request_queue_creation() {
        // llama-agent tests run serially via the llama-embedding-serial test
        // group (see .config/nextest.toml), so ModelManager::new is always the
        // first call in this process — any error here is a real failure.
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();

        let queue = RequestQueue::new(model_manager, config, session_config);
        assert_eq!(queue.get_queue_size(), 0);
    }

    #[tokio::test]
    async fn test_submit_request_model_not_loaded() {
        // Serialized via nextest test group (see test_request_queue_creation).
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        assert!(matches!(result, Err(QueueError::WorkerError(_))));
    }

    #[tokio::test]
    async fn test_submit_request_model_not_loaded_fails() {
        let model_manager = setup_loaded_model_manager().await;
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        // Should fail because model is not actually loaded in test setup
        assert!(result.is_err());
        match result.unwrap_err() {
            QueueError::WorkerError(msg) => {
                assert!(msg.contains("Model not loaded") || msg.contains("Model error"));
            }
            _ => panic!("Expected WorkerError for unloaded model"),
        }
    }

    #[tokio::test]
    async fn test_submit_streaming_request_with_unloaded_model() {
        let model_manager = setup_loaded_model_manager().await;
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let mut receiver = queue
            .submit_streaming_request(request, &session)
            .await
            .unwrap();

        // Should receive an error since the model is not loaded (dummy model fails to load)
        let chunk_result = receiver.recv().await;
        assert!(chunk_result.is_some());
        match chunk_result.unwrap() {
            Err(QueueError::WorkerError(msg)) => {
                assert!(msg.contains("Model not loaded"));
            }
            Ok(_) => panic!("Expected error for unloaded model"),
            Err(other) => panic!("Unexpected error type: {:?}", other),
        }
    }

    /// Queue-lifecycle regression for bug 01KSNJ7CBK9333J0T9G4TCA7DH.
    ///
    /// With a single worker (the production config), a streaming turn must
    /// release the worker and decrement the live queue size once it finishes —
    /// success, empty, or error — so that a subsequent prompt still enqueues
    /// instead of being rejected with "Queue is full". This test drives the real
    /// `RequestQueue` (no mocks) with `worker_threads: 1`, drains a streaming
    /// turn to completion, then asserts the queue drained and a second streaming
    /// request enqueues without `QueueError::Full`.
    #[tokio::test]
    async fn test_streaming_worker_released_after_turn() {
        let model_manager = setup_loaded_model_manager().await;
        let config = QueueConfig {
            max_queue_size: 10,
            worker_threads: 1,
        };
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let make_request = || GenerationRequest {
            session_id: session.id,
            max_tokens: Some(16),
            temperature: Some(0.0),
            top_p: None,
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        // First streaming turn: drain it fully so the worker finishes and the
        // single worker is released back to the pool.
        let mut receiver = queue
            .submit_streaming_request(make_request(), &session)
            .await
            .expect("first streaming request should enqueue");
        while receiver.recv().await.is_some() {
            // Drain every chunk (the unloaded dummy model yields a single error
            // chunk) until the stream closes.
        }

        // Give the worker a moment to record completion metrics after the stream
        // sender is dropped.
        for _ in 0..50 {
            if queue.get_queue_size() == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            queue.get_queue_size(),
            0,
            "worker was not released after the first streaming turn — live queue \
             size should return to 0"
        );

        // Second streaming turn must NOT be rejected with Queue is full.
        let second = queue
            .submit_streaming_request(make_request(), &session)
            .await;
        assert!(
            !matches!(second, Err(QueueError::Full)),
            "second streaming request was rejected with Queue is full after the \
             first turn released: {:?}",
            second.err()
        );
        // Drain the second stream too so the test leaves no dangling work.
        if let Ok(mut receiver) = second {
            while receiver.recv().await.is_some() {}
        }
    }

    #[tokio::test]
    async fn test_streaming_request_functionality() {
        // Validates streaming queue submission and chunk handling when the
        // model is not loaded. Serialized via nextest test group (see
        // test_request_queue_creation).
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let queue = RequestQueue::new(
            model_manager,
            create_test_queue_config(),
            crate::types::SessionConfig::default(),
        );

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(10),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let mut receiver = queue
            .submit_streaming_request(request, &session)
            .await
            .expect("Streaming request submission should succeed");

        assert_model_not_loaded_stream(&mut receiver).await;
    }

    /// Assert that the first chunk is a `Model not loaded` worker error and
    /// that no further chunks are produced.
    async fn assert_model_not_loaded_stream(
        receiver: &mut mpsc::UnboundedReceiver<Result<StreamChunk, QueueError>>,
    ) {
        let chunk_result = receiver
            .recv()
            .await
            .expect("Should receive a chunk result");
        match chunk_result {
            Err(QueueError::WorkerError(msg)) => assert!(
                msg.contains("Model not loaded"),
                "Should receive 'Model not loaded' error, got: {}",
                msg
            ),
            Ok(chunk) => panic!(
                "Expected error for unloaded model, but got streaming chunk: {:?}",
                chunk
            ),
            Err(other) => panic!("Expected WorkerError for unloaded model, got: {:?}", other),
        }
        assert!(
            receiver.recv().await.is_none(),
            "Should not receive additional chunks after error"
        );
    }

    #[tokio::test]
    async fn test_queue_timeout() {
        // Create a loaded model manager but with very slow processing
        let model_manager = setup_loaded_model_manager().await;
        let config = QueueConfig {
            max_queue_size: 10,

            worker_threads: 1,
        };
        let queue = RequestQueue::new(
            model_manager,
            config,
            crate::types::SessionConfig::default(),
        );

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        // Should fail because model is not loaded
        assert!(result.is_err());
        // The error should be WorkerError about model not loaded
        match result.unwrap_err() {
            QueueError::WorkerError(msg) => {
                assert!(msg.contains("Model not loaded") || msg.contains("Model error"));
            }
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_queued_request_debug() {
        let (sender, _) = oneshot::channel();
        let session = create_test_session();
        let request = QueuedRequest {
            id: "test-123".to_string(),
            request: GenerationRequest {
                session_id: session.id,
                max_tokens: Some(100),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            },
            session,
            response_sender: sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token: CancellationToken::new(),
            active_request_guard: None,
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("test-123"));
    }

    /// Test batch processing with various prompt sizes
    mod batch_processing_tests {
        use super::*;

        fn create_test_config_with_batch_size(batch_size: u32) -> ModelConfig {
            ModelConfig {
                source: ModelSource::Local {
                    folder: PathBuf::from("/tmp"),
                    filename: Some("test.gguf".to_string()),
                },
                batch_size,
                n_seq_max: 1,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: RetryConfig::default(),
                debug: false,
            }
        }

        #[tokio::test]
        async fn test_small_prompt_within_batch_size() {
            // Test case: prompt smaller than batch size should work normally.
            // Serialized via nextest test group (see test_request_queue_creation).
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(512))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with a small prompt (well within batch size)
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "Small prompt".to_string(), // ~2-3 tokens
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // The test focuses on batch processing logic, not model loading
            // We expect this to fail due to model not being loaded, but not due to batch size
            if let Err(QueueError::WorkerError(msg)) = result {
                // Should not contain batch size error messages
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
            }
        }

        #[tokio::test]
        async fn test_prompt_exactly_at_batch_size() {
            // Test edge case: prompt exactly at batch size limit.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 8u32; // Small batch size for testing
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            assert_eq!(model_manager.get_batch_size(), batch_size as usize);

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with content that should tokenize to exactly batch_size tokens
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "word ".repeat(4), // Approximately 8 tokens including spaces
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // Should not fail due to batch size issues
            if let Err(QueueError::WorkerError(msg)) = result {
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
            }
        }

        #[tokio::test]
        async fn test_prompt_exceeding_batch_size() {
            // Test case: prompt larger than batch size should be processed in chunks.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 4u32; // Very small batch size for testing
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            assert_eq!(model_manager.get_batch_size(), batch_size as usize);

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with a large prompt (exceeding batch size)
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "This is a longer prompt that should exceed the small batch size limit and require chunked processing to handle properly without errors".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // Most importantly: should NOT fail with batch size error
            if let Err(QueueError::WorkerError(msg)) = result {
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
                // Other errors (like model not loaded) are acceptable for this test
            }
        }

        #[tokio::test]
        async fn test_streaming_with_large_prompt() {
            // Test streaming with prompt larger than batch size.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 4u32;
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "This is another long prompt for streaming that should exceed the batch size and test chunked processing in streaming mode".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let stream_result = queue.submit_streaming_request(request, &session).await;

            // Check that we don't get batch size errors
            match stream_result {
                Ok(mut stream) => {
                    if let Some(Err(QueueError::WorkerError(msg))) = stream.recv().await {
                        assert!(!msg.contains("exceeds batch size limit"));
                        assert!(!msg.contains("Prompt too long"));
                    }
                }
                Err(QueueError::WorkerError(msg)) => {
                    assert!(!msg.contains("exceeds batch size limit"));
                    assert!(!msg.contains("Prompt too long"));
                }
                Err(_) => {
                    // Other errors are acceptable for this test
                }
            }
        }

        #[tokio::test]
        async fn test_multiple_batch_sizes() {
            // Test with various batch sizes to ensure consistent behavior
            let batch_sizes = vec![1u32, 2, 4, 8, 16, 32];

            for batch_size in batch_sizes {
                let model_manager =
                    match ModelManager::new(create_test_config_with_batch_size(batch_size)) {
                        Ok(manager) => Arc::new(manager),
                        Err(_) => continue, // Skip if can't create manager
                    };

                assert_eq!(model_manager.get_batch_size(), batch_size as usize);

                let config = QueueConfig {
                    max_queue_size: 10,
                    worker_threads: 1,
                };
                let queue = RequestQueue::new(
                    model_manager,
                    config,
                    crate::types::SessionConfig::default(),
                );

                let mut session = create_test_session();
                session.messages = vec![Message {
                    role: MessageRole::User,
                    content:
                        "Test prompt with multiple words to ensure it exceeds smaller batch sizes"
                            .to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                }];

                let request = GenerationRequest {
                    session_id: session.id,
                    max_tokens: Some(5),
                    temperature: Some(0.7),
                    top_p: Some(0.9),
                    stop_tokens: Vec::new(),
                    stopping_config: None,
                };

                let result = queue.submit_request(request, &session).await;

                // Key assertion: no batch size limit errors regardless of batch_size
                if let Err(QueueError::WorkerError(msg)) = result {
                    assert!(
                        !msg.contains("exceeds batch size limit"),
                        "Batch size {} failed with batch size error: {}",
                        batch_size,
                        msg
                    );
                    assert!(
                        !msg.contains("Prompt too long"),
                        "Batch size {} failed with prompt length error: {}",
                        batch_size,
                        msg
                    );
                }
            }
        }

        #[test]
        fn test_batch_size_configuration() {
            // Test that different batch sizes are correctly configured
            let test_sizes = vec![1u32, 64, 256, 512, 1024, 2048];

            for expected_size in test_sizes {
                let config = create_test_config_with_batch_size(expected_size);
                assert_eq!(config.batch_size, expected_size);

                if let Ok(model_manager) = ModelManager::new(config) {
                    assert_eq!(model_manager.get_batch_size(), expected_size as usize);
                }
            }
        }

        #[test]
        fn test_chunk_processing_logic() {
            // Test the chunking logic without actual model processing
            let batch_size = 4;
            let tokens: Vec<i32> = (0..10).collect(); // 10 tokens: [0,1,2,3,4,5,6,7,8,9]

            let chunks: Vec<_> = tokens.chunks(batch_size).collect();

            // Should create 3 chunks: [0,1,2,3], [4,5,6,7], [8,9]
            assert_eq!(chunks.len(), 3);
            assert_eq!(chunks[0], &[0, 1, 2, 3]);
            assert_eq!(chunks[1], &[4, 5, 6, 7]);
            assert_eq!(chunks[2], &[8, 9]);

            // Verify no tokens are lost
            let reconstructed: Vec<i32> = chunks.into_iter().flatten().copied().collect();
            assert_eq!(reconstructed, tokens);
        }
    }

    /// Worker-lifecycle / state-machine coverage driven by a deterministic,
    /// weight-free executor.
    ///
    /// These tests run the *real* `RequestQueue` worker loop — `worker_loop`,
    /// `process_request`, `dispatch_{batch,streaming}_request`, enqueue, FIFO,
    /// cancellation, and backpressure — but substitute a [`ScriptedExecutor`]
    /// for the model-backed `ModelManagerExecutor` so every turn outcome is
    /// reproducible without a GPU or weights. The central invariant under test
    /// is the one the "Queue is full on retry" bug violated: **after any turn
    /// outcome the single worker must be released and the live queue size must
    /// return to zero, so a subsequent enqueue succeeds** (never a spurious
    /// `QueueError::Full`).
    mod worker_lifecycle_tests {
        use super::*;
        use crate::generation::scripted::{ScriptToken, ScriptedModel};
        use crate::generation::TextGenerator;
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        /// How long a test sleeps so a spawned task can make progress (a
        /// submit parking on backpressure, a guard's spawned removal running)
        /// before the test asserts its state. One named knob so retuning for a
        /// slow CI box is a single edit.
        const SETTLE_DELAY: Duration = Duration::from_millis(50);

        /// Suite-wide "must not wedge" budget wrapped around every
        /// `tokio::time::timeout` in these tests.
        const TEST_TIMEOUT: Duration = Duration::from_secs(5);

        /// Shared poll budget for the `await_*` wait helpers
        /// (`await_queue_drained`, `await_worker_parked`,
        /// `await_session_cleared`): retry up to `POLL_ATTEMPTS` times,
        /// sleeping `POLL_INTERVAL` between attempts.
        const POLL_ATTEMPTS: usize = 200;
        /// See [`POLL_ATTEMPTS`].
        const POLL_INTERVAL: Duration = Duration::from_millis(5);

        /// The scripted executors' canonical "model finished talking" finish
        /// reason, named once so assertions and fakes cannot drift apart.
        fn eos() -> FinishReason {
            FinishReason::Stopped("EndOfSequence".to_string())
        }

        /// What a scripted turn should do when the worker runs it. This lets a
        /// single executor cover the whole turn-outcome matrix the queue cares
        /// about: a turn that produces tokens and stops for some reason, a turn
        /// that produces nothing (the 0-token / immediate-EOS bug shape), and a
        /// turn that fails outright.
        #[derive(Clone)]
        enum TurnOutcome {
            /// Replay the scripted model to completion (reason determined by the
            /// script + the request's `max_tokens` / `stop_tokens` / context).
            Scripted(ScriptedModel),
            /// Fail the turn with a worker error, as a runaway/aborted turn
            /// would. The worker must still be released afterward.
            Error(String),
        }

        /// A [`QueueExecutor`] backed by [`TurnOutcome`] rather than a live
        /// model. Counts how many turns it has run so FIFO/serialization can be
        /// asserted.
        struct ScriptedExecutor {
            outcome: TurnOutcome,
            turns_run: Arc<AtomicUsize>,
        }

        impl ScriptedExecutor {
            fn new(outcome: TurnOutcome) -> Self {
                Self {
                    outcome,
                    turns_run: Arc::new(AtomicUsize::new(0)),
                }
            }

            /// Derive a deterministic prompt from the session's messages so the
            /// scripted model has something to record. Queue-lifecycle tests do
            /// not depend on a chat template, only on the worker mechanics.
            fn prompt_for(session: &Session) -> String {
                session
                    .messages
                    .iter()
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }

        #[async_trait]
        impl QueueExecutor for ScriptedExecutor {
            async fn execute_batch(
                &self,
                _worker_id: usize,
                queued_request: &QueuedRequest,
            ) -> Result<GenerationResponse, QueueError> {
                self.turns_run.fetch_add(1, AtomicOrdering::SeqCst);
                match &self.outcome {
                    TurnOutcome::Error(msg) => Err(QueueError::WorkerError(msg.clone())),
                    TurnOutcome::Scripted(model) => {
                        let prompt = Self::prompt_for(&queued_request.session);
                        let mut model = model.clone();
                        model
                            .generate_text(
                                &prompt,
                                queued_request.request.clone(),
                                queued_request.cancellation_token.clone(),
                            )
                            .map_err(|e| {
                                QueueError::WorkerError(format!("Generation failed: {}", e))
                            })
                    }
                }
            }

            async fn execute_streaming(
                &self,
                _worker_id: usize,
                queued_request: &QueuedRequest,
                stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
            ) -> Result<(), QueueError> {
                self.turns_run.fetch_add(1, AtomicOrdering::SeqCst);
                if let TurnOutcome::Error(msg) = &self.outcome {
                    return Err(QueueError::WorkerError(msg.clone()));
                }
                let TurnOutcome::Scripted(model) = &self.outcome else {
                    unreachable!("Error handled above");
                };

                let prompt = Self::prompt_for(&queued_request.session);
                let mut model = model.clone();

                // The scripted model streams into an unbounded channel; bridge
                // those chunks onto the bounded client channel, mirroring the
                // production streaming relay.
                let (tx, mut rx) = mpsc::unbounded_channel();
                let gen_result = model.generate_stream(
                    &prompt,
                    queued_request.request.clone(),
                    tx,
                    queued_request.cancellation_token.clone(),
                );
                while let Ok(chunk) = rx.try_recv() {
                    // UnboundedSender::send is synchronous; only Closed
                    // (receiver dropped) returns an error.
                    if stream_sender.send(chunk).is_err() {
                        break;
                    }
                }
                gen_result.map_err(|e| QueueError::WorkerError(format!("Generation failed: {}", e)))
            }
        }

        /// A single-worker queue running every turn through `outcome`.
        fn scripted_queue(outcome: TurnOutcome) -> (RequestQueue, Arc<AtomicUsize>) {
            let executor = ScriptedExecutor::new(outcome);
            let turns_run = executor.turns_run.clone();
            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(config, Arc::new(executor));
            (queue, turns_run)
        }

        /// A single-worker queue whose every turn parks on a release gate until
        /// the test fires it, so the bounded channel can be saturated
        /// deterministically. Mirrors [`scripted_queue`] for the gated path:
        /// returns the queue alongside the `gate` (fire with `notify_waiters`)
        /// and the `entered` counter (turns the worker has begun).
        fn gated_queue(
            max_queue_size: usize,
        ) -> (RequestQueue, Arc<tokio::sync::Notify>, Arc<AtomicUsize>) {
            let gate = Arc::new(tokio::sync::Notify::new());
            let entered = Arc::new(AtomicUsize::new(0));
            let config = QueueConfig {
                max_queue_size,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(
                config,
                Arc::new(GatedExecutor {
                    gate: gate.clone(),
                    entered: entered.clone(),
                }),
            );
            (queue, gate, entered)
        }

        /// Handles to a saturated single-slot gated queue, ready for a
        /// backpressure assertion. See [`saturated_gated_queue`].
        struct SaturatedGatedQueue {
            queue: Arc<RequestQueue>,
            gate: Arc<tokio::sync::Notify>,
            entered: Arc<AtomicUsize>,
            session: Session,
            /// The submit that occupies the worker, parked on the gate.
            first: JoinHandle<Result<GenerationResponse, QueueError>>,
            /// The submit that fills the single channel slot behind the worker.
            filler: JoinHandle<Result<GenerationResponse, QueueError>>,
        }

        /// Build the shared backpressure fixture used by the saturation tests: a
        /// single-worker, single-slot gated queue with the worker parked on the
        /// gate and the lone channel slot already filled. After this returns the
        /// queue is fully saturated, so the next `submit_request` must wait on
        /// backpressure rather than be dropped — which is exactly what the
        /// callers assert (one releases the gate, the other cancels).
        async fn saturated_gated_queue() -> SaturatedGatedQueue {
            let (queue, gate, entered) = gated_queue(1);
            let queue = Arc::new(queue);
            let session = create_test_session();

            // Occupy the worker: the first turn dequeues and parks on the gate.
            let first = {
                let queue = queue.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    queue
                        .submit_request(test_request(&session, 8), &session)
                        .await
                })
            };
            await_worker_parked(&entered).await;

            // Fill the single channel slot behind the parked worker so the next
            // submit has no capacity and must block on backpressure.
            let filler = {
                let queue = queue.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    queue
                        .submit_request(test_request(&session, 8), &session)
                        .await
                })
            };
            tokio::time::sleep(SETTLE_DELAY).await;

            SaturatedGatedQueue {
                queue,
                gate,
                entered,
                session,
                first,
                filler,
            }
        }

        fn test_request(session: &Session, max_tokens: u32) -> GenerationRequest {
            GenerationRequest {
                session_id: session.id,
                max_tokens: Some(max_tokens),
                temperature: Some(0.0),
                top_p: None,
                stop_tokens: Vec::new(),
                stopping_config: None,
            }
        }

        /// Poll the live queue size until it drains to zero or the budget runs
        /// out, so completion metrics (recorded after the stream sender drops)
        /// have a chance to land.
        async fn await_queue_drained(queue: &RequestQueue) {
            for _ in 0..POLL_ATTEMPTS {
                if queue.get_queue_size() == 0 {
                    return;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }

        /// Spin until the gated worker has dequeued a turn and parked on its
        /// gate (`entered == 1`) or the shared [`POLL_ATTEMPTS`] budget runs
        /// out.
        async fn await_worker_parked(entered: &AtomicUsize) {
            for _ in 0..POLL_ATTEMPTS {
                if entered.load(AtomicOrdering::SeqCst) == 1 {
                    return;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }

        /// Drive one streaming turn to completion and return the chunks observed.
        async fn run_streaming_turn(
            queue: &RequestQueue,
            session: &Session,
            request: GenerationRequest,
        ) -> Vec<Result<StreamChunk, QueueError>> {
            let mut receiver = queue
                .submit_streaming_request(request, session)
                .await
                .expect("streaming request should enqueue");
            let mut chunks = Vec::new();
            while let Some(item) = receiver.recv().await {
                chunks.push(item);
            }
            chunks
        }

        /// The heart of the regression suite: run a streaming turn with the
        /// given outcome, assert the worker was released (queue drains to zero),
        /// and assert a second turn still enqueues without `Full`. Returns the
        /// first turn's chunks so callers can additionally assert the outcome
        /// shape.
        async fn assert_worker_released_after(
            outcome: TurnOutcome,
            max_tokens: u32,
        ) -> Vec<Result<StreamChunk, QueueError>> {
            let (queue, turns_run) = scripted_queue(outcome);
            let session = create_test_session();

            let chunks =
                run_streaming_turn(&queue, &session, test_request(&session, max_tokens)).await;

            await_queue_drained(&queue).await;
            assert_eq!(
                queue.get_queue_size(),
                0,
                "worker was not released after the turn — live queue size should return to 0"
            );

            // The single worker must accept a second turn (no spurious Full).
            let second = queue
                .submit_streaming_request(test_request(&session, max_tokens), &session)
                .await;
            assert!(
                !matches!(second, Err(QueueError::Full)),
                "second turn rejected with Queue is full after release: {:?}",
                second.err()
            );
            if let Ok(mut receiver) = second {
                while receiver.recv().await.is_some() {}
            }
            await_queue_drained(&queue).await;
            assert!(
                turns_run.load(AtomicOrdering::SeqCst) >= 2,
                "both turns should have reached the worker"
            );

            chunks
        }

        /// Extract the completion chunk's finish reason from a stream.
        fn completion_reason(chunks: &[Result<StreamChunk, QueueError>]) -> Option<FinishReason> {
            chunks.iter().rev().find_map(|c| match c {
                Ok(chunk) if chunk.is_complete => chunk.finish_reason.clone(),
                _ => None,
            })
        }

        // --- Worker-release-on-every-outcome matrix -------------------------

        #[tokio::test]
        async fn worker_released_after_normal_completion() {
            // A short script that ends on its own EndOfSequence — the ordinary
            // "model finished talking" turn.
            let model = ScriptedModel::from_texts(["Hello", " world"]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            let text: String = chunks
                .iter()
                .filter_map(|c| c.as_ref().ok())
                .filter(|c| !c.is_complete)
                .map(|c| c.text.clone())
                .collect();
            assert_eq!(text, "Hello world");
            assert_eq!(completion_reason(&chunks), Some(eos()));
        }

        #[tokio::test]
        async fn worker_released_after_immediate_eos_zero_tokens() {
            // The 0-token bug shape: the model emits EOS before any token. The
            // worker must still be released and re-enqueue must succeed.
            let model = ScriptedModel::new([ScriptToken::EndOfSequence]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            let token_chunks = chunks
                .iter()
                .filter_map(|c| c.as_ref().ok())
                .filter(|c| !c.is_complete)
                .count();
            assert_eq!(token_chunks, 0, "immediate EOS yields zero token chunks");
            assert_eq!(completion_reason(&chunks), Some(eos()));
        }

        #[tokio::test]
        async fn worker_released_after_max_tokens() {
            // A script longer than the budget stops at MaxTokens — the
            // runaway-but-bounded turn.
            let model = ScriptedModel::from_texts(["a", "b", "c", "d", "e", "f"]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 3).await;
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("MaxTokens".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_context_full() {
            // A tiny context window trips the context-window guard mid-turn.
            // create_test_session()'s single message "Hello" is one word, so
            // simulated_prompt_tokens == 1; with context_size 3 the guard fires
            // when 1 + generated >= 2, i.e. after one generated token.
            let model = ScriptedModel::from_texts(["x", "y", "z", "w"]).with_context_size(3);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("ContextWindowFull".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_error() {
            // A turn that fails outright must still release the worker — this is
            // the literal second symptom of the shipped bug.
            let chunks =
                assert_worker_released_after(TurnOutcome::Error("runaway turn aborted".into()), 64)
                    .await;
            // The error is relayed onto the stream.
            let has_error = chunks.iter().any(|c| {
                matches!(c, Err(QueueError::WorkerError(msg)) if msg.contains("runaway turn aborted"))
            });
            assert!(
                has_error,
                "the worker error should be relayed onto the stream"
            );
        }

        #[tokio::test]
        async fn worker_released_after_cancelled_turn() {
            // A turn whose cancellation token is already fired releases the
            // worker without corrupting the queue, and a fresh turn still runs.
            let model = ScriptedModel::from_texts(["never", "emitted"]);
            let (queue, turns_run) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            // Submit, then immediately cancel this session's request. The worker
            // either rejects it pre-process (cancelled before dequeue) or the
            // scripted loop observes the cancel and stops cleanly — either way
            // the worker is released.
            let request = test_request(&session, 64);
            let mut receiver = queue
                .submit_streaming_request(request, &session)
                .await
                .expect("streaming request should enqueue");
            queue.cancel_session(&session.id).await;
            while receiver.recv().await.is_some() {}

            await_queue_drained(&queue).await;
            assert_eq!(
                queue.get_queue_size(),
                0,
                "cancelled turn must release the worker"
            );

            // A subsequent turn on a fresh session enqueues and runs.
            let session2 = create_test_session();
            let chunks = run_streaming_turn(&queue, &session2, test_request(&session2, 64)).await;
            assert!(
                !chunks.is_empty(),
                "a turn after cancellation should still produce a completion"
            );
            await_queue_drained(&queue).await;
            assert!(turns_run.load(AtomicOrdering::SeqCst) >= 1);
        }

        #[tokio::test]
        async fn worker_released_after_batch_completion() {
            // The batch (non-streaming) path must release the worker too, and
            // return the collected response.
            let model = ScriptedModel::from_texts(["one", "two", "three"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            let response = queue
                .submit_request(test_request(&session, 64), &session)
                .await
                .expect("batch turn should succeed");
            assert_eq!(response.generated_text, "onetwothree");
            assert_eq!(response.tokens_generated, 3);

            await_queue_drained(&queue).await;
            assert_eq!(queue.get_queue_size(), 0, "batch turn must release worker");

            // Re-enqueue succeeds.
            let second = queue
                .submit_request(test_request(&session, 64), &session)
                .await;
            assert!(second.is_ok(), "second batch turn should not be rejected");
        }

        // --- Queue-full only at capacity -----------------------------------

        /// An executor that parks every turn on a release gate until the test
        /// fires it, so the queue can be filled to capacity deterministically.
        struct GatedExecutor {
            gate: Arc<tokio::sync::Notify>,
            entered: Arc<AtomicUsize>,
        }
        #[async_trait]
        impl QueueExecutor for GatedExecutor {
            async fn execute_batch(
                &self,
                _worker_id: usize,
                _queued_request: &QueuedRequest,
            ) -> Result<GenerationResponse, QueueError> {
                self.entered.fetch_add(1, AtomicOrdering::SeqCst);
                self.gate.notified().await;
                Ok(GenerationResponse {
                    generated_text: String::new(),
                    tokens_generated: 0,
                    generation_time: Duration::from_millis(0),
                    finish_reason: eos(),
                    complete_token_sequence: None,
                })
            }
            async fn execute_streaming(
                &self,
                _worker_id: usize,
                _queued_request: &QueuedRequest,
                _stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
            ) -> Result<(), QueueError> {
                self.entered.fetch_add(1, AtomicOrdering::SeqCst);
                self.gate.notified().await;
                Ok(())
            }
        }

        #[tokio::test]
        async fn enqueue_returns_full_only_at_capacity() {
            // Park the single worker on a gated turn, then fill the bounded
            // channel to exactly capacity and prove the next enqueue — and only
            // it — returns Full, while every enqueue up to capacity succeeds.
            let max_queue_size = 3;
            let (queue, gate, entered) = gated_queue(max_queue_size);
            let session = create_test_session();

            // First request reaches the worker and parks on the gate, removing
            // itself from the channel buffer.
            let busy = queue.submit_request(test_request(&session, 8), &session);
            tokio::pin!(busy);
            // Poll it once to dispatch into the channel, then leave it pending.
            tokio::select! {
                _ = &mut busy => panic!("gated turn returned early"),
                _ = tokio::time::sleep(SETTLE_DELAY) => {}
            }
            assert_eq!(
                entered.load(AtomicOrdering::SeqCst),
                1,
                "the worker should be parked on the first turn"
            );

            // Now the worker is busy; the bounded channel holds `max_queue_size`
            // pending requests. Each enqueue up to capacity must succeed.
            let mut buffered = Vec::new();
            for i in 0..max_queue_size {
                let result = queue.try_enqueue_for_test(&session);
                assert!(
                    result.is_ok(),
                    "enqueue {} within capacity must succeed, got {:?}",
                    i,
                    result.err()
                );
                buffered.push(result);
            }

            // Capacity reached: the next enqueue must return Full.
            let overflow = queue.try_enqueue_for_test(&session);
            assert!(
                matches!(overflow, Err(QueueError::Full)),
                "enqueue past capacity must return QueueError::Full, got {:?}",
                overflow
            );

            // Release the worker so the test shuts down cleanly.
            gate.notify_waiters();
        }

        // --- Backpressure: batch submit waits when saturated, never drops ---

        #[tokio::test]
        async fn batch_submit_applies_backpressure_when_saturated() {
            // Regression for the review "Queue is full → silent drop" bug. A
            // saturated single-worker queue must make a non-streaming
            // `submit_request` WAIT for capacity, not reject it with
            // `QueueError::Full`. We park the worker on a gate and fill the
            // bounded channel to capacity, then submit one more request and
            // assert it stays pending (backpressure) until we release the gate,
            // after which it completes successfully.
            let SaturatedGatedQueue {
                queue,
                gate,
                entered,
                session,
                first,
                filler,
            } = saturated_gated_queue().await;
            assert_eq!(
                entered.load(AtomicOrdering::SeqCst),
                1,
                "the worker should be parked on the first turn"
            );

            // The backpressured submit: capacity is exhausted, so this must
            // wait rather than drop. Prove it stays pending while saturated.
            let backpressured = {
                let queue = queue.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    queue
                        .submit_request(test_request(&session, 8), &session)
                        .await
                })
            };
            tokio::time::sleep(SETTLE_DELAY).await;
            assert!(
                !backpressured.is_finished(),
                "a saturated batch submit must WAIT for capacity, not return immediately"
            );

            // Release the worker for every turn. The single worker parks on the
            // gate at the start of each turn, so we notify repeatedly until all
            // three submits resolve. None may be dropped with Full.
            let drain = async {
                let mut pending = vec![
                    ("first", first),
                    ("filler", filler),
                    ("backpressured", backpressured),
                ];
                let mut results = Vec::new();
                while !pending.is_empty() {
                    gate.notify_waiters();
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let mut still = Vec::new();
                    for (label, handle) in pending {
                        if handle.is_finished() {
                            results.push((label, handle.await.expect("join")));
                        } else {
                            still.push((label, handle));
                        }
                    }
                    pending = still;
                }
                results
            };
            let results = tokio::time::timeout(TEST_TIMEOUT, drain)
                .await
                .expect("all backpressured submits must eventually complete");
            for (label, result) in results {
                assert!(
                    !matches!(result, Err(QueueError::Full)),
                    "{label} submit must not be dropped with Full under backpressure: {:?}",
                    result.err()
                );
            }
        }

        #[tokio::test]
        async fn batch_submit_backpressure_honors_cancellation() {
            // A backpressured (waiting) batch submit must remain abortable via
            // its cancellation token, so a cancelled request does not wedge the
            // submitter forever when the queue is saturated.
            //
            // `_first`/`_filler` keep the worker parked and the lone channel
            // slot occupied for the lifetime of the test; `session` belongs to
            // those occupants — the backpressured submit below uses a separate
            // session so it can be cancelled in isolation.
            let SaturatedGatedQueue {
                queue,
                gate,
                entered: _entered,
                session: _session,
                first: _first,
                filler: _filler,
            } = saturated_gated_queue().await;

            // Submit one more on a DIFFERENT session so it blocks on
            // backpressure, then cancel that session. The waiting submit must
            // unblock with an error rather than hang.
            let cancel_session = create_test_session();
            let cancel_id = cancel_session.id;
            let backpressured = {
                let queue = queue.clone();
                tokio::spawn(async move {
                    queue
                        .submit_request(test_request(&cancel_session, 8), &cancel_session)
                        .await
                })
            };
            tokio::time::sleep(SETTLE_DELAY).await;
            assert!(
                !backpressured.is_finished(),
                "the third submit must be waiting on backpressure before we cancel"
            );

            assert!(
                queue.cancel_session(&cancel_id).await,
                "cancelling the waiting session's token must find an active request"
            );

            let result = tokio::time::timeout(TEST_TIMEOUT, backpressured)
                .await
                .expect("cancelled backpressured submit must not wedge forever")
                .expect("join");
            assert!(
                matches!(result, Err(QueueError::Cancelled)),
                "a cancelled backpressured submit must return QueueError::Cancelled, got {result:?}"
            );
            gate.notify_waiters();
        }

        // --- active_requests cleanup on every exit path ---------------------

        /// Wait until `active_requests` holds no entry for `session_id`, or the
        /// budget runs out. The cleanup guard removes the entry from a spawned
        /// task on drop, so callers must give that task a chance to run rather
        /// than reading the map synchronously the instant a submit resolves.
        async fn await_session_cleared(
            queue: &RequestQueue,
            session_id: &crate::types::SessionId,
        ) -> bool {
            for _ in 0..POLL_ATTEMPTS {
                if queue.active_requests.lock().await.get(session_id).is_none() {
                    return true;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            false
        }

        #[tokio::test]
        async fn submit_request_clears_active_requests_on_success() {
            // The happy path must still remove the session's entry once the
            // response resolves, so a healthy server never accumulates entries.
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(ScriptedModel::new([
                ScriptToken::EndOfSequence,
            ])));
            let session = create_test_session();

            let response = queue
                .submit_request(test_request(&session, 8), &session)
                .await;
            assert!(response.is_ok(), "scripted success turn must resolve Ok");

            assert!(
                await_session_cleared(&queue, &session.id).await,
                "active_requests must be empty after a successful submit"
            );
        }

        /// Shared body for the `*_clears_active_requests_on_cancellation`
        /// tests: with the queue saturated, run `submit` on a fresh session so
        /// it parks on backpressure, fire the session's tracked token directly
        /// — NOT via `cancel_session`, which removes the entry itself and
        /// would mask a leak — then assert the submit unwinds with
        /// [`QueueError::Cancelled`] and its `active_requests` entry is gone.
        async fn assert_cancelled_backpressured_submit_clears_entry<F, Fut>(submit: F)
        where
            F: FnOnce(Arc<RequestQueue>, Session) -> Fut,
            Fut: std::future::Future<Output = Result<(), QueueError>> + Send + 'static,
        {
            let SaturatedGatedQueue { queue, gate, .. } = saturated_gated_queue().await;

            let cancel_session = create_test_session();
            let cancel_id = cancel_session.id;
            let backpressured = tokio::spawn(submit(queue.clone(), cancel_session));
            tokio::time::sleep(SETTLE_DELAY).await;
            assert!(
                !backpressured.is_finished(),
                "the waiting submit must be parked on backpressure before we cancel"
            );

            // Fire the request's own token without removing its map entry, so
            // the cancellation arm of `enqueue_request` returns an error and
            // the submit unwinds through its early-return path.
            let token = queue
                .active_requests
                .lock()
                .await
                .get(&cancel_id)
                .map(|tracked| tracked.token.clone())
                .expect("the waiting submit must have tracked its cancellation token");
            token.cancel();

            let result = tokio::time::timeout(TEST_TIMEOUT, backpressured)
                .await
                .expect("cancelled backpressured submit must not wedge forever")
                .expect("join");
            assert!(
                matches!(result, Err(QueueError::Cancelled)),
                "a cancelled backpressured submit must return QueueError::Cancelled, got {result:?}"
            );
            assert!(
                await_session_cleared(&queue, &cancel_id).await,
                "active_requests must not retain the cancelled session's entry"
            );
            gate.notify_waiters();
        }

        #[tokio::test]
        async fn submit_request_clears_active_requests_on_cancellation() {
            // A submit that is cancelled while waiting for queue capacity (the
            // m8zac70 backpressure arm) early-returns from `enqueue_request`
            // before the old success-only cleanup ran.
            assert_cancelled_backpressured_submit_clears_entry(|queue, session| async move {
                queue
                    .submit_request(test_request(&session, 8), &session)
                    .await
                    .map(|_| ())
            })
            .await;
        }

        #[tokio::test]
        async fn streaming_submit_clears_active_requests_on_cancellation() {
            // Streaming twin of
            // `submit_request_clears_active_requests_on_cancellation`: the
            // streaming submit inserts into `active_requests` BEFORE waiting
            // for queue capacity, so a cancellation fired while it is parked
            // must both unblock it with an error and remove the entry.
            assert_cancelled_backpressured_submit_clears_entry(|queue, session| async move {
                queue
                    .submit_streaming_request(test_request(&session, 8), &session)
                    .await
                    .map(|_| ())
            })
            .await;
        }

        #[tokio::test]
        async fn streaming_turn_clears_active_requests_on_completion() {
            // A streaming turn that completes NORMALLY must also clear its
            // `active_requests` entry: removal on cancellation alone would let
            // the long-lived shared server accumulate one stale entry per
            // finished stream, and `cancel_session` would keep "finding" (and
            // firing) dead tokens for sessions whose turns ended long ago.
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(ScriptedModel::new([
                ScriptToken::EndOfSequence,
            ])));
            let session = create_test_session();

            let chunks = run_streaming_turn(&queue, &session, test_request(&session, 8)).await;
            assert!(!chunks.is_empty(), "the turn must produce a completion");

            assert!(
                await_session_cleared(&queue, &session.id).await,
                "active_requests must be empty once the streaming turn completes"
            );
        }

        #[tokio::test]
        async fn stale_guard_drop_does_not_clobber_newer_turns_token() {
            // The guard clears its entry from a detached spawned task, which
            // can run AFTER the next turn on the same session has tracked a
            // fresh token — the agentic loop's normal back-to-back shape. The
            // stale removal must not delete the fresh token, or the in-flight
            // turn becomes uncancellable.
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(ScriptedModel::new([
                ScriptToken::EndOfSequence,
            ])));
            let session = create_test_session();

            // Turn N tracks its token and holds its cleanup guard.
            let stale_generation = queue
                .track_cancellation_token(session.id, CancellationToken::new())
                .await;
            let stale_guard = ActiveRequestGuard {
                active_requests: queue.active_requests.clone(),
                session_id: session.id,
                generation: stale_generation,
            };

            // Turn N+1 tracks a fresh token before turn N's removal runs.
            let fresh = CancellationToken::new();
            queue
                .track_cancellation_token(session.id, fresh.clone())
                .await;

            // Drop the stale guard and give its spawned removal time to run.
            drop(stale_guard);
            tokio::time::sleep(SETTLE_DELAY).await;

            assert!(
                queue.cancel_session(&session.id).await,
                "the fresh turn's token must survive the stale guard's cleanup"
            );
            assert!(
                fresh.is_cancelled(),
                "cancel_session must fire the FRESH token, not a stale one"
            );
        }

        // --- FIFO ordering through the single worker ------------------------

        #[tokio::test]
        async fn batch_turns_processed_in_fifo_order() {
            // A single worker processes submitted requests in submission order.
            // We record the per-turn prompt the executor sees and assert it
            // matches submission order.
            let seen = Arc::new(Mutex::new(Vec::<String>::new()));

            struct RecordingExecutor {
                seen: Arc<Mutex<Vec<String>>>,
            }
            #[async_trait]
            impl QueueExecutor for RecordingExecutor {
                async fn execute_batch(
                    &self,
                    _worker_id: usize,
                    queued_request: &QueuedRequest,
                ) -> Result<GenerationResponse, QueueError> {
                    let content = queued_request.session.messages[0].content.clone();
                    self.seen.lock().unwrap().push(content.clone());
                    Ok(GenerationResponse {
                        generated_text: content,
                        tokens_generated: 1,
                        generation_time: Duration::from_millis(0),
                        finish_reason: eos(),
                        complete_token_sequence: None,
                    })
                }
                async fn execute_streaming(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                    _stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
                ) -> Result<(), QueueError> {
                    Ok(())
                }
            }

            let config = QueueConfig {
                max_queue_size: 16,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(
                config,
                Arc::new(RecordingExecutor { seen: seen.clone() }),
            );

            // Submit several batch requests in a fixed order, awaiting each so
            // the single worker handles them one at a time in submission order.
            let order = ["first", "second", "third", "fourth"];
            for label in order {
                let mut session = create_test_session();
                session.messages[0].content = label.to_string();
                let response = queue
                    .submit_request(test_request(&session, 8), &session)
                    .await
                    .expect("each batch turn should succeed");
                assert_eq!(response.generated_text, label);
            }

            let recorded = seen.lock().unwrap().clone();
            assert_eq!(
                recorded,
                order.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "the single worker must process requests in FIFO order"
            );
        }

        // --- Backpressure: worker_threads = 1 serializes -------------------

        #[tokio::test]
        async fn single_worker_serializes_concurrent_turns() {
            // With worker_threads = 1, concurrently-submitted turns must not run
            // in parallel. The executor tracks concurrent entries and asserts
            // the peak is exactly 1.
            let in_flight = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));

            struct SerializingExecutor {
                in_flight: Arc<AtomicUsize>,
                peak: Arc<AtomicUsize>,
            }
            #[async_trait]
            impl QueueExecutor for SerializingExecutor {
                async fn execute_batch(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                ) -> Result<GenerationResponse, QueueError> {
                    let now = self.in_flight.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                    self.peak.fetch_max(now, AtomicOrdering::SeqCst);
                    // Hold the worker briefly so any parallel entry would be
                    // observed as concurrency > 1.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    self.in_flight.fetch_sub(1, AtomicOrdering::SeqCst);
                    Ok(GenerationResponse {
                        generated_text: String::new(),
                        tokens_generated: 0,
                        generation_time: Duration::from_millis(0),
                        finish_reason: eos(),
                        complete_token_sequence: None,
                    })
                }
                async fn execute_streaming(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                    _stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
                ) -> Result<(), QueueError> {
                    Ok(())
                }
            }

            let config = QueueConfig {
                max_queue_size: 16,
                worker_threads: 1,
            };
            let queue = Arc::new(RequestQueue::with_executor(
                config,
                Arc::new(SerializingExecutor {
                    in_flight: in_flight.clone(),
                    peak: peak.clone(),
                }),
            ));

            // Fire several requests concurrently; the single worker must
            // serialize them.
            let mut handles = Vec::new();
            for _ in 0..5 {
                let queue = queue.clone();
                let session = create_test_session();
                handles.push(tokio::spawn(async move {
                    let _ = queue
                        .submit_request(test_request(&session, 8), &session)
                        .await;
                }));
            }
            for h in handles {
                h.await.unwrap();
            }

            assert_eq!(
                peak.load(AtomicOrdering::SeqCst),
                1,
                "a single worker must never run two turns concurrently"
            );
        }

        // --- Stats / metrics snapshot --------------------------------------

        #[tokio::test]
        async fn stats_reflect_completed_turns() {
            // After a batch turn completes, the stats snapshot reports it as
            // completed with the generated token count, and the live size is 0.
            let model = ScriptedModel::from_texts(["a", "b"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            let _ = queue
                .submit_request(test_request(&session, 8), &session)
                .await
                .expect("batch turn should succeed");
            await_queue_drained(&queue).await;

            let stats = queue.get_stats();
            assert_eq!(stats.total_requests, 1);
            assert_eq!(stats.completed_requests, 1);
            assert_eq!(stats.failed_requests, 0);
            assert_eq!(stats.current_queue_size, 0);
            assert_eq!(stats.total_tokens_generated, 2);
            assert!(stats.peak_queue_size >= 1);
        }

        #[tokio::test]
        async fn stats_reflect_failed_turns() {
            // A failed turn is counted as failed (not completed) and still
            // releases the worker.
            let (queue, _turns) = scripted_queue(TurnOutcome::Error("boom".into()));
            let session = create_test_session();

            let result = queue
                .submit_request(test_request(&session, 8), &session)
                .await;
            assert!(matches!(result, Err(QueueError::WorkerError(_))));
            await_queue_drained(&queue).await;

            let stats = queue.get_stats();
            assert_eq!(stats.completed_requests, 0);
            assert_eq!(stats.failed_requests, 1);
            assert_eq!(stats.current_queue_size, 0);
        }

        // --- cancel_session bookkeeping ------------------------------------

        #[tokio::test]
        async fn cancel_session_returns_false_when_no_active_request() {
            // Cancelling a session with no in-flight request returns false and
            // does not disturb the queue.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let unknown = SessionId::new();
            assert!(
                !queue.cancel_session(&unknown).await,
                "cancelling an unknown session returns false"
            );
        }

        // --- Backpressure on the streaming submit path ----------------------

        #[tokio::test]
        async fn streaming_submit_applies_backpressure_when_saturated() {
            // Regression for the review-fleet "Queue is full → silent drop"
            // bug on the STREAMING path (the batch path got backpressure
            // first): a saturated single-worker queue must make
            // `submit_streaming_request` WAIT for a slot, never reject it with
            // `QueueError::Full`. Saturate the queue, prove the streaming
            // submit stays pending, then open the gate and prove it enqueues.
            let SaturatedGatedQueue {
                queue,
                gate,
                entered,
                session,
                first: _first,
                filler: _filler,
            } = saturated_gated_queue().await;
            assert_eq!(
                entered.load(AtomicOrdering::SeqCst),
                1,
                "the worker should be parked on the first turn"
            );

            let backpressured = {
                let queue = queue.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    queue
                        .submit_streaming_request(test_request(&session, 8), &session)
                        .await
                })
            };
            tokio::time::sleep(SETTLE_DELAY).await;
            assert!(
                !backpressured.is_finished(),
                "a saturated streaming submit must WAIT for capacity, not return immediately"
            );

            // Release the worker turn by turn until the parked streaming
            // submit gets a channel slot and resolves.
            let wait = async {
                while !backpressured.is_finished() {
                    gate.notify_waiters();
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                backpressured.await.expect("join")
            };
            let result = tokio::time::timeout(TEST_TIMEOUT, wait)
                .await
                .expect("backpressured streaming submit must eventually enqueue");
            assert!(
                result.is_ok(),
                "streaming submit must enqueue once capacity frees, got {:?}",
                result.err()
            );
            gate.notify_waiters();
        }

        // --- Shutdown closes the sender, rejecting later enqueues ----------

        #[tokio::test]
        async fn graceful_shutdown_drains_workers() {
            // `shutdown()` closes the sender channel and joins every worker
            // handle, exercising the graceful shutdown loop.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();
            let _ = queue
                .submit_request(test_request(&session, 8), &session)
                .await;
            await_queue_drained(&queue).await;
            queue.shutdown().await;
        }

        #[tokio::test]
        async fn shutdown_with_timeout_returns_stats() {
            // shutdown_with_timeout drains workers within the budget and returns
            // a pre-shutdown stats snapshot.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();
            let _ = queue
                .submit_request(test_request(&session, 8), &session)
                .await;
            let stats = queue.shutdown_with_timeout(Duration::from_secs(5)).await;
            assert_eq!(stats.total_requests, 1);
        }

        // --- Queue health: `is_closed` detects a dead queue -----------------

        #[tokio::test]
        async fn is_closed_false_while_workers_alive() {
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            assert!(
                !queue.is_closed(),
                "a fresh queue with a live worker must report open"
            );
        }

        /// The production cascade this guards: the queue's lone worker task is
        /// aborted (in the incident, by dropping the per-review runtime that
        /// spawned it), the receiver closes, and every later submit fails
        /// `ShuttingDown`. A long-lived holder must be able to DETECT that via
        /// `is_closed` so it can rebuild instead of serving a corpse.
        #[tokio::test]
        async fn is_closed_true_after_worker_tasks_die() {
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));

            for handle in &queue.worker_handles {
                handle.abort();
            }
            // Abort completion (and thus the receiver drop) is asynchronous;
            // poll with the suite's standard budget.
            for _ in 0..POLL_ATTEMPTS {
                if queue.is_closed() {
                    break;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            assert!(
                queue.is_closed(),
                "a queue whose workers are all dead must report closed"
            );
        }
    }

    /// Unit tests for the module-private free functions that do not need a
    /// model, exercising branches the worker only reaches under specific
    /// conditions (e.g. cache growth past the per-process limit).
    mod free_fn_unit_tests {
        use super::*;

        #[test]
        fn store_evicts_lru_first_keeping_the_active_session() {
            // entry budget of 2; insert A, B, touch A (now MRU), insert C.
            // The least-recently-used (B) must be evicted, NOT the active A.
            let mut store = SessionStateStore::new(2, usize::MAX);
            store.insert("A".into(), state(1, 10), None, None);
            store.insert("B".into(), state(2, 10), None, None);
            assert!(store.get("A").is_some(), "touch A → most-recently-used");
            store.insert("C".into(), state(3, 10), None, None);

            assert_eq!(store.len(), 2, "entry budget of 2 enforced");
            assert!(store.contains("A"), "active (recently-used) session kept");
            assert!(store.contains("C"), "newest session kept");
            assert!(!store.contains("B"), "least-recently-used session evicted");
        }

        #[test]
        fn store_evicts_to_stay_within_byte_budget() {
            // Byte budget of 25 with 10-byte entries: only 2 fit; a 3rd evicts the
            // LRU even though the entry count (3) would otherwise be allowed.
            let mut store = SessionStateStore::new(100, 25);
            store.insert("A".into(), state(1, 10), None, None);
            store.insert("B".into(), state(2, 10), None, None);
            store.insert("C".into(), state(3, 10), None, None);

            assert_eq!(store.len(), 2, "byte budget caps total entries at 2");
            assert!(store.cur_bytes() <= 25, "total bytes stay within budget");
            assert!(!store.contains("A"), "LRU evicted under byte pressure");
            assert!(store.contains("B") && store.contains("C"));
        }

        #[test]
        fn store_always_keeps_at_least_one_entry() {
            // Even a single entry larger than the byte budget is retained — we
            // never evict the only (most-recently-used) entry.
            let mut store = SessionStateStore::new(4, 8);
            store.insert("solo".into(), state(1, 100), None, None);
            assert_eq!(store.len(), 1, "the only entry is never evicted");
            assert!(store.contains("solo"));
        }

        #[test]
        fn store_remove_frees_one_session_and_its_bytes() {
            // Lifecycle-driven reclamation: removing a single ended session drops
            // exactly its entry, its bytes, and its LRU slot, leaving the rest.
            let mut store = SessionStateStore::new(4, usize::MAX);
            store.insert("A".into(), state(1, 10), None, None);
            store.insert("B".into(), state(2, 20), None, None);
            assert_eq!(store.cur_bytes(), 30);

            assert!(store.remove("A"), "removing a present session reports true");
            assert!(!store.contains("A"), "removed session is gone");
            assert!(store.contains("B"), "other sessions are untouched");
            assert_eq!(
                store.cur_bytes(),
                20,
                "only the removed session's bytes are freed"
            );
            assert_eq!(store.len(), 1);
        }

        #[test]
        fn store_remove_can_empty_the_store_bypassing_the_keep_one_guard() {
            // Unlike budget eviction (which always keeps the MRU entry), explicit
            // reclamation of a known-dead session must be able to empty the store
            // — otherwise the last ended session's KV would stay pinned forever.
            let mut store = SessionStateStore::new(4, usize::MAX);
            store.insert("only".into(), state(1, 10), None, None);
            assert!(store.remove("only"));
            assert_eq!(store.len(), 0, "remove is not subject to keep-at-least-one");
            assert_eq!(store.cur_bytes(), 0, "byte accounting returns to zero");
        }

        #[test]
        fn store_remove_absent_session_is_a_noop() {
            let mut store = SessionStateStore::new(4, usize::MAX);
            store.insert("A".into(), state(1, 10), None, None);
            assert!(
                !store.remove("missing"),
                "removing an absent session is false"
            );
            assert_eq!(store.cur_bytes(), 10, "byte accounting unchanged");
            assert!(store.contains("A"));
        }

        #[test]
        fn store_insert_replaces_and_tracks_bytes() {
            let mut store = SessionStateStore::new(4, usize::MAX);
            store.insert("A".into(), state(1, 10), None, None);
            store.insert("A".into(), state(1, 30), None, None); // replace, larger
            assert_eq!(store.len(), 1, "replacing an id does not add an entry");
            assert_eq!(store.cur_bytes(), 30, "byte accounting follows replacement");
        }

        #[test]
        fn store_get_returns_bytes_and_fingerprint() {
            let mut store = SessionStateStore::new(4, usize::MAX);
            store.insert(
                "A".into(),
                state(7, 3),
                Some(vec![10, 20, 30]),
                Some(state(8, 4)),
            );
            let (bytes, toks, draft) = store.get("A").expect("present");
            assert_eq!(&bytes[..], &[7, 7, 7]);
            assert_eq!(toks, Some(vec![10, 20, 30]));
            assert_eq!(draft.as_deref(), Some(&[8, 8, 8, 8][..]));
            assert_eq!(store.get("missing"), None);
        }

        /// The cross-session prefix cache: when session B has nothing of its
        /// own cached but session A has a long shared prefix, the scan picks
        /// A as the donor. This is the local-side equivalent of Claude's
        /// hosted prompt-prefix cache — without it, two kanban windows on
        /// the same agent each pay a full cold 28k-token prefill.
        #[test]
        fn find_best_prefix_match_returns_cross_session_donor() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            // Session A has a long cached prefix.
            store.insert(
                "A".into(),
                state(1, 4),
                Some(vec![10, 20, 30, 40, 50]),
                None,
            );
            // Session B has nothing cached. Asking for B with a prompt that
            // shares the first three tokens with A's prompt must yield A as
            // the donor, with lcp=3.
            let m = store
                .find_best_prefix_match("B", &[10, 20, 30, 99, 100])
                .expect("must return a donor when a cross-session LCP exists");
            assert_eq!(m.source_session_id, "A");
            assert_eq!(m.lcp, 3);
        }

        /// Tie-break: when both the caller's own session AND another session
        /// share the same lcp, pick the caller's own — avoids an unnecessary
        /// foreign-state copy on the typical warm-continuation case where
        /// the session's own prior turn IS the longest match.
        #[test]
        fn find_best_prefix_match_prefers_current_session_on_tie() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("A".into(), state(1, 4), Some(vec![10, 20, 30]), None);
            store.insert("B".into(), state(2, 4), Some(vec![10, 20, 30]), None);
            let m = store
                .find_best_prefix_match("B", &[10, 20, 30, 40])
                .expect("must return a donor");
            assert_eq!(m.source_session_id, "B", "current session wins on lcp tie");
        }

        /// A longer cross-session match still wins over the caller's own
        /// shorter match — we always pick the deepest prefix to maximize
        /// the prefill we can skip.
        #[test]
        fn find_best_prefix_match_picks_deepest_lcp_across_sessions() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            // Session A: full 5-token prefix.
            store.insert(
                "A".into(),
                state(1, 4),
                Some(vec![10, 20, 30, 40, 50]),
                None,
            );
            // Session B (the caller): only 2 leading tokens match the new prompt.
            store.insert("B".into(), state(2, 4), Some(vec![10, 20, 99]), None);
            // New prompt for B: matches A by 5, matches B by 2.
            let m = store
                .find_best_prefix_match("B", &[10, 20, 30, 40, 50, 60])
                .expect("must return a donor");
            assert_eq!(
                m.source_session_id, "A",
                "deeper foreign LCP must win over shallower own-session LCP"
            );
            assert_eq!(m.lcp, 5);
        }

        /// No cached entry shares ANY prefix → no donor.
        #[test]
        fn find_best_prefix_match_returns_none_without_overlap() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("A".into(), state(1, 4), Some(vec![10, 20]), None);
            assert!(store.find_best_prefix_match("B", &[99, 100, 101]).is_none());
        }

        /// Entries written without a prompt fingerprint (the batch path's
        /// snapshots — `prompt_tokens = None`) are NEVER candidates, since
        /// we cannot verify their prefix.
        #[test]
        fn find_best_prefix_match_skips_unfingerprinted_entries() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("batch-only".into(), state(1, 4), None, None);
            assert!(store.find_best_prefix_match("B", &[10, 20, 30]).is_none());
        }

        #[test]
        fn store_byte_budget_includes_draft_bytes() {
            // Both target and draft bytes count against the byte budget so a
            // large pair of (target, draft) snapshots is correctly accounted
            // for under LRU pressure.
            let mut store = SessionStateStore::new(100, 30);
            store.insert("A".into(), state(1, 10), None, Some(state(2, 10))); // 20
            store.insert("B".into(), state(3, 10), None, Some(state(4, 10))); // 40 total
                                                                              // budget 30 < 40 → LRU 'A' evicted, only 'B' remains
            assert_eq!(store.len(), 1, "draft bytes count toward budget");
            assert!(store.contains("B"));
            assert!(!store.contains("A"));
        }

        #[test]
        fn common_prefix_len_basic() {
            assert_eq!(common_prefix_len(&[1, 2, 3], &[1, 2, 3, 4]), 3);
            assert_eq!(common_prefix_len(&[1, 2, 3], &[1, 2, 9, 4]), 2);
            assert_eq!(common_prefix_len(&[9, 1], &[1, 9]), 0);
            assert_eq!(common_prefix_len(&[], &[1, 2]), 0);
            assert_eq!(common_prefix_len(&[1, 2], &[]), 0);
        }

        #[test]
        fn template_token_count_maps_position_to_next() {
            // A non-negative KV-cache position maps to the next position; a
            // negative position (fresh context) maps to None.
            assert_eq!(compute_template_token_count(0, -1), None);
            assert_eq!(compute_template_token_count(0, 0), Some(1));
            assert_eq!(compute_template_token_count(0, 41), Some(42));
        }

        #[test]
        fn streaming_reuse_decision_reuses_strict_prefix() {
            // Common incremental case: cache shares the first `lcp` tokens of a
            // longer new prompt, so resume decoding at `lcp`.
            assert_eq!(streaming_reuse_decision(42, 100), Some(42));
            assert_eq!(streaming_reuse_decision(1, 2), Some(1));
        }

        #[test]
        fn streaming_reuse_decision_none_when_nothing_shared() {
            // No common prefix → nothing to reuse, reprocess in full.
            assert_eq!(streaming_reuse_decision(0, 100), None);
        }

        #[test]
        fn streaming_reuse_decision_none_when_no_new_tokens() {
            // Cache already covers the entire new prompt: no new tokens to decode.
            // (Also guards the divergence/shrink case — the new prompt's length
            // bounds `lcp`, and the caller trims the KV to `lcp` before decoding.)
            assert_eq!(streaming_reuse_decision(100, 100), None);
            assert_eq!(streaming_reuse_decision(120, 100), None);
            assert_eq!(streaming_reuse_decision(0, 0), None);
        }

        // The `should_persist_stream_state` predicate and its tests were
        // removed: state is now saved at the prompt boundary BEFORE
        // generation begins, so cancellation or disconnect mid-generation
        // never affects the cache (the saved state is the prompt state,
        // which is valid regardless of whether generation completed).
    }

    /// Unit tests for session-state forking, pinning, and status — the store
    /// machinery behind the `session/fork` / `session/state_status` /
    /// `session/pin` ACP extension methods.
    mod session_state_fork_tests {
        use super::*;

        /// Forking aliases the parent's state under the child id, registered
        /// with the parent's prompt-token fingerprint — so the child's first
        /// prompt (a strict extension of the parent's) matches its OWN entry
        /// with `lcp == donor length`: the trim offset equals the saved KV end
        /// and the restore needs zero rollback.
        #[test]
        fn fork_aliases_parent_state_with_parent_fingerprint() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("parent".into(), state(1, 100), Some(vec![10, 20, 30]), None);

            let info = store
                .fork("parent", "child".to_string())
                .expect("fork of a saved parent must succeed");
            assert_eq!(info.prefix_tokens, 3, "fork reports the donor token count");
            assert_eq!(info.state_bytes, 100, "fork reports the donor byte size");

            // The child's first rendered prompt strictly extends the parent's
            // saved prompt tokens.
            let m = store
                .find_best_prefix_match("child", &[10, 20, 30, 40, 50])
                .expect("child's aliased entry must be a donor");
            assert_eq!(m.source_session_id, "child", "child reuses its OWN entry");
            assert_eq!(m.lcp, 3, "lcp covers the full parent fingerprint");
            assert_eq!(
                streaming_reuse_decision(m.lcp, 5),
                Some(3),
                "strict-prefix fork resumes at exactly the donor length"
            );
        }

        /// Shared blobs are counted once against the byte budget, no matter
        /// how many forks alias them; a child's own later save adds its own
        /// (new) bytes.
        #[test]
        fn fork_shares_blob_bytes_counted_once() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert(
                "parent".into(),
                state(1, 100),
                Some(vec![1]),
                Some(state(2, 50)),
            );
            assert_eq!(store.cur_bytes(), 150);

            store.fork("parent", "c1".to_string()).expect("fork c1");
            store.fork("parent", "c2".to_string()).expect("fork c2");
            assert_eq!(
                store.cur_bytes(),
                150,
                "aliased forks add zero bytes — shared blobs count once"
            );

            // Copy-on-write at save time: c1's own end-of-turn save replaces
            // its alias with fresh bytes, which DO count.
            store.insert("c1".into(), state(3, 70), Some(vec![1, 2]), None);
            assert_eq!(store.cur_bytes(), 220);
        }

        /// A forked child's own save must not mutate the parent's entry.
        #[test]
        fn forked_child_save_leaves_parent_untouched() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("parent".into(), state(1, 100), Some(vec![10, 20, 30]), None);
            store.fork("parent", "child".to_string()).expect("fork");

            store.insert(
                "child".into(),
                state(9, 40),
                Some(vec![10, 20, 30, 40]),
                None,
            );

            let parent = store.status("parent").expect("parent entry still present");
            assert_eq!(parent.prompt_tokens, Some(3));
            assert_eq!(parent.state_bytes, 100);
            let (bytes, _, _) = store.get("parent").expect("parent bytes intact");
            assert_eq!(&bytes[..], &state(1, 100)[..]);
        }

        /// Fork failures are distinguishable: unknown parent vs a parent whose
        /// snapshot has no prompt fingerprint (not strict-prefix restorable).
        #[test]
        fn fork_failures_are_distinguishable() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            assert!(matches!(
                store.fork("missing", "child".to_string()),
                Err(SessionStateForkError::ParentStateNotFound(_))
            ));

            store.insert("batch-only".into(), state(1, 10), None, None);
            assert!(matches!(
                store.fork("batch-only", "child".to_string()),
                Err(SessionStateForkError::ParentStateUnusable(_))
            ));
        }

        /// `status` reports saved/pinned/token-count truthfully, and `None`
        /// for sessions with no snapshot.
        #[test]
        fn status_reports_saved_pinned_and_token_count() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            assert!(store.status("missing").is_none());

            store.insert("a".into(), state(1, 25), Some(vec![10, 20]), None);
            let status = store.status("a").expect("saved entry has a status");
            assert_eq!(status.prompt_tokens, Some(2));
            assert_eq!(status.state_bytes, 25);
            assert!(!status.pinned);

            assert!(store.set_pinned("a", true));
            assert!(store.status("a").unwrap().pinned);
            assert!(store.set_pinned("a", false));
            assert!(!store.status("a").unwrap().pinned);

            assert!(
                !store.set_pinned("missing", true),
                "pin of absent id is false"
            );
        }

        /// Pinned entries survive cache pressure that evicts unpinned ones.
        #[test]
        fn pinned_entry_survives_eviction_pressure() {
            let mut store = SessionStateStore::new(2, usize::MAX);
            store.insert("pinned".into(), state(1, 10), Some(vec![1]), None);
            assert!(store.set_pinned("pinned", true));

            // Two more inserts exceed the entry budget of 2 — eviction must
            // take the unpinned LRU victim, never the pinned entry.
            store.insert("b".into(), state(2, 10), None, None);
            store.insert("c".into(), state(3, 10), None, None);

            assert!(store.contains("pinned"), "pinned entry survives pressure");
            assert!(store.contains("c"), "newest entry survives");
            assert!(!store.contains("b"), "unpinned LRU entry is the victim");
        }

        /// Unpinning re-exposes the entry to eviction on the next pressure.
        #[test]
        fn unpinned_entry_is_evicted_after_unpin() {
            let mut store = SessionStateStore::new(2, usize::MAX);
            store.insert("a".into(), state(1, 10), Some(vec![1]), None);
            assert!(store.set_pinned("a", true));
            store.insert("b".into(), state(2, 10), None, None);
            store.insert("c".into(), state(3, 10), None, None);
            assert!(store.contains("a"));

            assert!(store.set_pinned("a", false));
            store.insert("d".into(), state(4, 10), None, None);
            assert!(!store.contains("a"), "unpinned LRU entry is evicted");
        }

        /// Pinned bytes still count against the budget, and eviction
        /// terminates (keeping the most-recently-used entry) even when every
        /// candidate is pinned — the cache can go over budget but never
        /// deadlocks or evicts the active insert.
        #[test]
        fn eviction_terminates_when_everything_is_pinned() {
            let mut store = SessionStateStore::new(8, 25);
            store.insert("a".into(), state(1, 20), Some(vec![1]), None);
            assert!(store.set_pinned("a", true));

            // Over the byte budget: "a" is pinned and "b" is the MRU insert,
            // so nothing is evictable and the loop must terminate over budget.
            store.insert("b".into(), state(2, 20), Some(vec![2]), None);
            assert!(store.contains("a"), "pinned entry kept despite pressure");
            assert!(store.contains("b"), "active (MRU) insert is never evicted");

            assert!(store.set_pinned("b", true));
            store.insert("c".into(), state(3, 20), Some(vec![3]), None);
            assert!(store.contains("a"), "pinned entry kept despite pressure");
            assert!(store.contains("b"), "pinned entry kept despite pressure");
            assert!(store.contains("c"), "active (MRU) insert is never evicted");
        }

        /// Lifecycle `remove` (session ended) reclaims even pinned entries —
        /// the caller knows the session is gone, which trumps the pin.
        #[test]
        fn lifecycle_remove_reclaims_pinned_entries() {
            let mut store = SessionStateStore::new(8, usize::MAX);
            store.insert("a".into(), state(1, 10), Some(vec![1]), None);
            assert!(store.set_pinned("a", true));
            assert!(store.remove("a"));
            assert!(!store.contains("a"));
            assert_eq!(store.cur_bytes(), 0);
        }

        /// (c) The RAM-scaled budget helper returns `max(2GiB, total × fraction)`:
        /// a generous fraction of a large machine's RAM, but never below the
        /// 2 GiB floor on a small one. This is what the cache must be built with
        /// so a high-RAM box holds every validator prefix + fork.
        #[test]
        fn cache_byte_budget_is_ram_scaled_floored_at_2gib() {
            const FLOOR: usize = 2 * 1024 * 1024 * 1024;
            const FRACTION_NUM: u64 = 1;
            const FRACTION_DEN: u64 = 4; // 0.25

            // A 128 GiB machine: 0.25 × 128 GiB = 32 GiB, well above the floor.
            let big = 128u64 * 1024 * 1024 * 1024;
            assert_eq!(
                cache_byte_budget_for_total_memory(big),
                (big / FRACTION_DEN * FRACTION_NUM) as usize,
                "high-RAM budget is a fraction of total memory"
            );

            // A 4 GiB machine: 0.25 × 4 GiB = 1 GiB < floor → floored at 2 GiB.
            let small = 4u64 * 1024 * 1024 * 1024;
            assert_eq!(
                cache_byte_budget_for_total_memory(small),
                FLOOR,
                "low-RAM budget is floored at 2 GiB"
            );

            // A machine that reports 0 total memory (sysinfo unavailable) still
            // gets the floor, never a 0-byte budget that would evict everything.
            assert_eq!(cache_byte_budget_for_total_memory(0), FLOOR);
        }

        /// (a, count axis) The entry-COUNT budget — not just bytes — drove the
        /// race: a review fleet primes ~15 validator prefixes concurrently, but
        /// the old `cores / 2` ceiling (4-8 on a typical box) count-evicted the
        /// earliest, not-yet-pinned prefixes before their pins landed,
        /// regardless of how much RAM the byte budget allowed. The entry
        /// ceiling must comfortably hold a full fleet on a low-core machine so
        /// count eviction is never the binding constraint for it.
        #[test]
        fn entry_ceiling_holds_a_full_validator_fleet_on_a_low_core_box() {
            const FLEET: usize = 15;
            // A 4-core machine: the old `cores / 2 = 2` ceiling would
            // count-evict 13 of a 15-validator fleet's prefixes before their
            // pins landed. The floor must hold the whole fleet.
            assert!(
                cache_entry_ceiling_for_cores(4) >= FLEET,
                "the entry ceiling on a 4-core box ({}) must hold a {FLEET}-validator \
                 fleet so count eviction does not drop a not-yet-pinned prefix",
                cache_entry_ceiling_for_cores(4)
            );
            // A high-core machine still scales above the floor (cores / 2).
            assert!(
                cache_entry_ceiling_for_cores(128) >= 64,
                "a high-core box scales the entry ceiling above the fleet floor"
            );
        }

        /// (a) The eviction-race: ~15 validators prime concurrently, the single
        /// GPU serializes them, so each prime's `save → evict()` runs in turn.
        /// With a post-save pin and a budget the saves exceed, an earlier
        /// validator's not-yet-pinned entry is the LRU victim of a later
        /// validator's save — its pin then fails. Born-pinned saves
        /// (`insert_pinned`) make every entry an eviction non-candidate from the
        /// moment its bytes land, so no concurrent save can defeat the pin.
        #[test]
        fn pin_on_save_survives_concurrent_eviction_race() {
            // Budget holds only ~3 of the 15 entries: post-save pinning loses
            // the race badly here, which is exactly the production failure.
            let n = 15usize;
            let entry_bytes = 100usize;
            let budget = 3 * entry_bytes;
            let mut store = SessionStateStore::new(n + 5, budget);

            for i in 0..n {
                let id = format!("validator-{i}");
                // Born pinned: the save and the pin are one atomic step, so the
                // entry is never an unpinned eviction candidate.
                store.insert_pinned(
                    id.clone(),
                    state(i as u8, entry_bytes),
                    Some(vec![i as i32]),
                );
            }

            // Every primed prefix is still resident and pinned — no pin was
            // defeated by another validator's save.
            for i in 0..n {
                let id = format!("validator-{i}");
                let status = store
                    .status(&id)
                    .unwrap_or_else(|| panic!("{id} must still be cached"));
                assert!(status.pinned, "{id} must be pinned");
            }
        }

        /// (a, RED-guard) Post-save pinning under the same pressure DOES lose
        /// the race: this documents the bug the atomic pin-on-save closes. The
        /// LRU early entries are evicted by later saves before their pin lands,
        /// so `set_pinned` returns false for them.
        #[test]
        fn post_save_pin_loses_the_race_without_pin_on_save() {
            let n = 15usize;
            let entry_bytes = 100usize;
            let budget = 3 * entry_bytes;
            let mut store = SessionStateStore::new(n + 5, budget);

            // All saves first (the GPU-serialized prime turns), then all pins —
            // the production prime→status→pin protocol's window.
            for i in 0..n {
                store.insert(
                    format!("validator-{i}"),
                    state(i as u8, entry_bytes),
                    Some(vec![i as i32]),
                    None,
                );
            }
            let pinned_ok = (0..n)
                .filter(|i| store.set_pinned(&format!("validator-{i}"), true))
                .count();
            assert!(
                pinned_ok < n,
                "post-save pinning must lose the race for at least one early entry \
                 (got {pinned_ok}/{n} pinned) — this is the bug pin-on-save fixes"
            );
        }

        /// (b) A pinned entry is never evicted even when a single later save
        /// would itself exceed the whole byte budget.
        #[test]
        fn pinned_entry_survives_a_single_oversized_save() {
            let mut store = SessionStateStore::new(8, 50);
            store.insert_pinned("prefix".into(), state(1, 40), Some(vec![1]));
            assert!(store.status("prefix").unwrap().pinned);

            // One save bigger than the entire budget. It cannot evict the pinned
            // prefix; the cache simply goes over budget (and warns).
            store.insert("huge".into(), state(2, 200), Some(vec![2]), None);

            assert!(
                store.contains("prefix"),
                "pinned prefix survives an oversized save"
            );
            assert!(store.contains("huge"), "the active (MRU) save is kept");
        }

        /// (d) Eviction while the cache is at/over budget emits an observable
        /// warn carrying the bytes/budget/entry-count pressure, so a future
        /// pin-failure cluster is diagnosable from the log (today eviction is
        /// silent).
        #[test]
        fn eviction_near_budget_is_observable() {
            // The eviction `warn!` is emitted alongside `evictions`; the
            // counter is the deterministic in-process observable (a `warn!`
            // only reaches whichever tracing subscriber is installed, which is
            // racy to capture under the concurrent test harness).
            let mut store = SessionStateStore::new(100, 25);
            store.insert("a".into(), state(1, 10), None, None);
            store.insert("b".into(), state(2, 10), None, None);
            assert_eq!(
                store.eviction_count(),
                0,
                "two 10-byte entries fit a 25-byte budget — no eviction yet"
            );

            // This third 10-byte save pushes total to 30 > 25 and evicts the
            // LRU under budget pressure, which is observed (and warned).
            store.insert("c".into(), state(3, 10), None, None);
            assert_eq!(
                store.eviction_count(),
                1,
                "an eviction under budget pressure must be observable"
            );
            assert!(
                !store.contains("a"),
                "the LRU entry was the eviction victim"
            );
        }
    }
}
