//! Integration-test harness for the `swissarmyhammer-entity` crate.
//!
//! Each submodule is an end-to-end scenario standing up a real
//! `EntityContext` + `EntityCache` + `StoreContext` and driving the
//! undo/redo + reconcile path.

mod integration {
    mod undo_redo_emits_transition_events_e2e;
}
