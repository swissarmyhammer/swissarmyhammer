//! End-to-end hook tests using PlaybackAgent + real command hooks.
//!
//! These tests verify that the full hook pipeline works correctly:
//! config JSON → command hook → shell execution → stdin JSON → exit code/stdout →
//! decision interpretation → agent behavior.
//!
//! # Architecture
//!
//! - [`PlaybackAgent`] replays deterministic agent sessions from JSON fixtures
//! - Real shell scripts act as command hooks, receiving JSON on stdin
//! - Assertions verify both that hooks fired AND that decisions were applied
//!
//! # Adding new hook event types
//!
//! When a new variant is added to [`HookEventKind`], the exhaustive match in
//! [`helpers::all_event_kinds`] will cause a compile error. Add the new variant
//! there and create corresponding test(s).

// `avp_schema_tests` exercises `HookEvent::to_command_input_full` against
// AVP's strongly-typed `HookInput` schema (see `avp_common::HookInput`).
// `avp-common` is currently unbuildable under ACP 0.11 — it depends
// transitively on `claude-agent` and `llama-agent`, which still target the
// 0.10 `Agent` trait. Once those sibling tasks land, add `avp-common` back
// to `[dev-dependencies]` in this crate's `Cargo.toml` and re-enable this
// module. The test source is preserved so re-enabling is a one-line change.
//mod avp_schema_tests;
mod cross_cutting_tests;
mod exit2_tests;
mod helpers;
mod hook_edge_case_tests;
mod json_continue_tests;
mod json_output_tests;
mod json_specific_output_tests;
