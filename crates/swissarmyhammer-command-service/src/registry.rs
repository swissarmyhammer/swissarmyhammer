//! Override-stack registry for registered commands.
//!
//! Each command id maps to a stack of registrations keyed by the
//! registering caller. The most recent registration is "active"; when a
//! caller pops or is purged, the next-most-recent registration for that id
//! re-emerges. Within a single caller, re-registering the same id replaces
//! that caller's entry in place rather than pushing a duplicate — the
//! stack's height is bounded by `unique_callers × unique_ids`.
//!
//! See `ideas/plugins/command-service.md` §"Override stack semantics" for
//! the canonical contract this module enforces.

use std::collections::HashMap;
use std::time::Instant;

use swissarmyhammer_plugin::CallerId;

use crate::RegisterCommand;

/// One registration on the override stack for a command id.
///
/// The triple `(caller, registration, registered_at)` is everything the
/// dispatch layer needs to: route execute / available callbacks back to the
/// originating isolate (`caller`), serve the active registration's payload
/// (`registration`), and break ties deterministically when two callers
/// happen to register at the same monotonic instant (`registered_at`).
#[derive(Debug, Clone)]
pub struct StackEntry {
    /// The caller that registered this entry (host, plugin, external).
    pub caller: CallerId,
    /// The full registration payload as published by the caller.
    pub registration: RegisterCommand,
    /// Monotonic instant the entry landed on the stack. Used only for
    /// observability — stack ordering itself comes from insertion order, so
    /// the registry never reads this field for routing decisions.
    pub registered_at: Instant,
}

/// In-memory override-stack registry for the `command` service.
///
/// The registry is the single source of truth for "which registration is
/// active for command id X" inside the service. It is not thread-safe on
/// its own; the surrounding service wraps it in a lock.
///
/// Stacks are ordered oldest-first. The active entry is always
/// `stack.last()` — the most recent registration wins.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    /// command id → stack of registrations (oldest-first; top of stack is
    /// the active entry).
    stacks: HashMap<String, Vec<StackEntry>>,
}

impl CommandRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push (or replace) a registration for `id` under `caller`.
    ///
    /// If `caller` already has an entry for `registration.id`, that entry is
    /// removed and the new one is appended at the top of the stack — so the
    /// re-registering caller's payload becomes active and no duplicate
    /// entry per caller appears on the stack.
    ///
    /// The push is O(stack height): both the dedupe scan and the active
    /// lookup walk the per-id stack, and command stacks are bounded by the
    /// number of distinct callers that have ever registered that id.
    pub fn push(&mut self, caller: CallerId, registration: RegisterCommand) {
        let id = registration.id.clone();
        let stack = self.stacks.entry(id).or_default();
        // Per-caller dedupe: drop this caller's existing entry, if any.
        // Calling this caller's `push` again moves the entry to the top of
        // the stack, which is the documented behavior.
        stack.retain(|entry| entry.caller != caller);
        stack.push(StackEntry {
            caller,
            registration,
            registered_at: Instant::now(),
        });
    }

    /// Remove this caller's entry for `id`, if any.
    ///
    /// Returns `true` when an entry was removed, `false` otherwise. After
    /// the pop, [`Self::active`] returns the next-most-recent entry on the
    /// stack (or `None` when the stack is empty).
    pub fn pop_caller(&mut self, caller: &CallerId, id: &str) -> bool {
        let Some(stack) = self.stacks.get_mut(id) else {
            return false;
        };
        let before = stack.len();
        stack.retain(|entry| &entry.caller != caller);
        let removed = stack.len() != before;
        if stack.is_empty() {
            self.stacks.remove(id);
        }
        removed
    }

    /// Drop every entry registered by `caller` from every command stack.
    ///
    /// Used on plugin unload: the platform issues a single `purge_caller`
    /// for the unloaded caller and every registration that caller made
    /// disappears in one pass, with the next-most-recent entry for each id
    /// re-emerging as active.
    pub fn purge_caller(&mut self, caller: &CallerId) {
        self.stacks.retain(|_id, stack| {
            stack.retain(|entry| &entry.caller != caller);
            !stack.is_empty()
        });
    }

    /// Return the active (top-of-stack) entry for `id`, if any.
    pub fn active(&self, id: &str) -> Option<&StackEntry> {
        self.stacks.get(id).and_then(|stack| stack.last())
    }

    /// Return the full stack for `id`, oldest-first. Empty when the id has
    /// no registrations.
    ///
    /// This is the introspection accessor — verb handlers use [`Self::active`]
    /// instead. Tests and diagnostics use this when they need to inspect
    /// shadowed entries.
    pub fn stack_for(&self, id: &str) -> &[StackEntry] {
        self.stacks.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Return every active (top-of-stack) entry across all command ids.
    ///
    /// This is the discovery surface — the `list command` verb projects this
    /// onto callback-free metadata. Overridden entries never appear here.
    /// Order is unspecified (the underlying map is unordered); callers that
    /// need a stable order must sort.
    pub fn list(&self) -> Vec<&StackEntry> {
        self.stacks
            .values()
            .filter_map(|stack| stack.last())
            .collect()
    }

    /// Number of distinct command ids with at least one registration.
    ///
    /// Useful for diagnostics and tests. Cheap — proxies to the underlying
    /// `HashMap::len`.
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    /// `true` when the registry holds no registrations.
    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }

    /// Total stack entries across every command id.
    ///
    /// Sums the heights of every per-id stack. Used by
    /// [`crate::CommandService::purge_caller`] to detect a no-op purge:
    /// the map's `len` only counts distinct ids, so it misses the case
    /// where purging a caller's entry leaves another caller's entry still
    /// active for the same id.
    pub fn total_entries(&self) -> usize {
        self.stacks.values().map(Vec::len).sum()
    }
}
