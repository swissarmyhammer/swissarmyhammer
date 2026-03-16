---
position_column: done
position_ordinal: b8
title: Add Initializable trait to swissarmyhammer-common
---
## What

Add an `Initializable` trait in `swissarmyhammer-common/src/lifecycle.rs` (new file), parallel to the `Doctorable` trait in `health.rs`. This trait lets any component — tool or non-tool — declare its lifecycle operations. The caller decides when to invoke them.

**Four lifecycle operations, all explicit — nothing automatic:**
- `init` / `deinit` — one-time project setup/teardown (`sah init` / `sah deinit`)
- `start` / `stop` — runtime background work (indexing, LSP, watchers — started explicitly by the serve command when ready, not by constructors)

**Key design decisions:**
- `InitScope` enum in common (not tied to clap): `Project`, `Local`, `User`
- `init()`/`deinit()` take scope — project setup varies by target
- `start()`/`stop()` have no scope — runtime is runtime
- Returns `Vec<InitResult>` for structured feedback
- `is_applicable()` default method (same pattern as `Doctorable`)
- `priority()` for ordering — foundational components first
- Sync, not async — init operations are filesystem work. `start()` can spawn tokio tasks internally if needed.
- Nothing fires automatically. The caller (init command, serve command) explicitly iterates and calls.

**Files:**
- NEW: `swissarmyhammer-common/src/lifecycle.rs`
- EDIT: `swissarmyhammer-common/src/lib.rs` (add `pub mod lifecycle`)

**Trait sketch:**
```rust
pub enum InitScope { Project, Local, User }

pub enum InitStatus { Ok, Warning, Error, Skipped }

pub struct InitResult {
    pub name: String,
    pub status: InitStatus,
    pub message: String,
}

pub trait Initializable {
    fn name(&self) -> &str;
    fn category(&self) -> &str;
    fn priority(&self) -> i32 { 0 }
    fn init(&self, scope: &InitScope) -> Vec<InitResult>;
    fn deinit(&self, scope: &InitScope) -> Vec<InitResult>;
    fn start(&self) -> Vec<InitResult>;
    fn stop(&self) -> Vec<InitResult>;
    fn is_applicable(&self) -> bool { true }
}

pub struct InitRegistry { ... }
```

## Acceptance Criteria
- [ ] `Initializable` trait defined with init/deinit/start/stop/priority/is_applicable
- [ ] `InitScope` enum defined (Project, Local, User)
- [ ] `InitResult` struct with status/message (parallel to `HealthCheck`)
- [ ] `InitRegistry` with register/run_all_init/run_all_deinit/run_all_start/run_all_stop (sorts by priority)
- [ ] Unit tests for registry ordering, filtering by is_applicable, result collection
- [ ] `pub mod lifecycle` exported from lib.rs
- [ ] Default impls for all methods return empty Vec (opt-in, not forced)

## Tests
- [ ] `swissarmyhammer-common/src/lifecycle.rs` — inline `#[cfg(test)]` module
- [ ] Test registry priority ordering
- [ ] Test is_applicable filtering
- [ ] Test init/start/stop/deinit result collection
- [ ] `cargo test -p swissarmyhammer-common` passes