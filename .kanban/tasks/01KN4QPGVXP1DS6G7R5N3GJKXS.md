---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffe380
title: swissarmyhammer-store crate — TrackedStore trait, StoreHandle, undo stack, changelog
---
## What

Create the `swissarmyhammer-store` crate with the `TrackedStore` trait, `StoreHandle` blanket impl, undo stack, changelog, and `StoreContext`. Uses text-level diff/patch on serialized output for changelog entries and undo/redo — source-code-merge semantics.

**Key insight:** Both entities (YAML, MD+YAML) and perspectives (YAML) serialize to text. The changelog stores text diffs of the serialized representation. Undo applies the reverse patch. Redo applies the forward patch. Three-way merge handles concurrent edits. This replaces the existing field-level diff system with a single text diff algorithm.

**One store = one directory.** Entity types each get their own store (`tasks/`, `columns/`, `tags/`, etc.). Perspectives get one store (`perspectives/`).

**New crate:** `swissarmyhammer-store/`

**Files to create:**
- `swissarmyhammer-store/Cargo.toml` (depends on `ulid`, `serde`, `serde_json`, `async-trait`, `tokio`, `chrono`, `diffy`)
- `swissarmyhammer-store/src/lib.rs` — re-exports
- `swissarmyhammer-store/src/id.rs` — `UndoEntryId(ulid::Ulid)` newtype
- `swissarmyhammer-store/src/store.rs` — `TrackedStore` trait (4 methods + 2 associated types)
- `swissarmyhammer-store/src/handle.rs` — `StoreHandle<S>` blanket impl (write, delete, get, undo, redo, flush_changes, transactions)
- `swissarmyhammer-store/src/changelog.rs` — `ChangelogEntry`, JSONL append/read
- `swissarmyhammer-store/src/diff.rs` — text diff/patch wrapper around `diffy`, three-way merge
- `swissarmyhammer-store/src/stack.rs` — `UndoStack`, `UndoEntry`
- `swissarmyhammer-store/src/context.rs` — `StoreContext` (owns undo stack + registered stores)
- `swissarmyhammer-store/src/event.rs` — `ChangeEvent` struct
- `swissarmyhammer-store/src/erased.rs` — `ErasedStore` trait + blanket impl
- `swissarmyhammer-store/src/trash.rs` — trash/restore file operations

**Also modify:**
- `Cargo.toml` (workspace) — add `swissarmyhammer-store` to members

### TrackedStore trait
```rust
pub trait TrackedStore: Send + Sync + 'static {
    type Item: Send + Sync;
    /// Item ID — Clone not Copy, since some IDs are String-based (slugs).
    type ItemId: Send + Sync + Clone + Eq + Hash + Display + FromStr;

    /// The single directory this store manages.
    fn root(&self) -> &Path;
    /// Extract the item's unique ID.
    fn item_id(&self, item: &Self::Item) -> Self::ItemId;
    /// Serialize item to on-disk text (YAML, MD+YAML, etc.)
    /// Responsible for stripping computed fields, applying defaults —
    /// the text returned is exactly what goes to disk.
    fn serialize(&self, item: &Self::Item) -> String;
    /// Deserialize from on-disk text. Receives the item ID (parsed from filename)
    /// so the store can inject it into the deserialized item.
    fn deserialize(&self, id: &Self::ItemId, text: &str) -> Result<Self::Item>;
}
```

Note: `deserialize` takes `id` because the ID comes from the filename, not the file content. The store needs to inject it.

Note: `ItemId` is `Clone` not `Copy` — entity IDs are String-based slugs for columns/tags/actors, ULIDs for tasks. `UndoEntryId` remains `Copy` since it's always a ULID.

### UndoEntryId newtype
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UndoEntryId(ulid::Ulid);
// Display, FromStr, Serialize, Deserialize — all delegate to Ulid
```

### StoreHandle — blanket impl with text diff/patch

```rust
pub struct StoreHandle<S: TrackedStore> {
    store: Arc<S>,
    cache: RwLock<HashMap<String, String>>,  // item_id.to_string() → last-known text
    changelog_path: PathBuf,                 // {root}/changelog.jsonl
    trash_dir: PathBuf,                      // {root}/.trash/
}
```

**Write path:**
1. `store.serialize(item)` → new_text
2. Read old_text from cache or disk (None if creating)
3. If old_text == new_text → return Ok(None) (idempotent, no changelog entry)
4. Compute forward/reverse text patches via `diffy`
5. Append `ChangelogEntry` to JSONL
6. Atomic write: temp file → rename
7. Update cache
8. Return `UndoEntryId`

**Undo path (source-code merge):**
- Create → trash the file
- Update → three-way merge with reverse patch against current file content
- Delete → restore from trash

**Transaction support:**
- `begin_transaction()` / `commit_transaction()` group multiple writes under one undo entry
- Undo reverses all constituent writes in reverse order

**Trash:** delete moves file to `.trash/`, undo-delete restores

**flush_changes:** scan dir, diff vs cache, produce ChangeEvents

### ChangelogEntry
```rust
pub struct ChangelogEntry {
    pub id: UndoEntryId,
    pub timestamp: DateTime<Utc>,
    pub op: ChangeOp,
    pub item_id: String,           // ItemId.to_string()
    pub before: Option<String>,    // Full text before
    pub after: Option<String>,     // Full text after
    pub forward_patch: Option<String>,
    pub reverse_patch: Option<String>,
    pub transaction_id: Option<String>,
}
```

### StoreContext
Holds `Vec<Arc<dyn ErasedStore>>`, owns shared `UndoStack`. Dispatches undo/redo to correct store. `flush_all()` aggregates events. `store_for_path()` routes file watcher.

Multiple stores can be registered — one per entity type plus one for perspectives. A single board might have 6+ stores (tasks, columns, swimlanes, tags, actors, board, perspectives).

## Acceptance Criteria
- [ ] `UndoEntryId(Ulid)` newtype — Copy, Ord
- [ ] `TrackedStore` trait: `Item` + `ItemId` (Clone not Copy), 4 methods
- [ ] `StoreHandle<S>` provides get, write, delete, undo, redo, flush_changes
- [ ] `ChangelogEntry` with before/after text + patches, JSONL format
- [ ] Text diff/patch via `diffy`, three-way merge for concurrent edits
- [ ] Transaction support
- [ ] Trash for deletes
- [ ] Idempotent writes
- [ ] Atomic file writes
- [ ] `UndoStack` with `UndoEntryId`, pointer model, YAML persistence
- [ ] `ErasedStore` + `StoreContext`
- [ ] All tested with mock TrackedStore

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-store` — full suite with mock store