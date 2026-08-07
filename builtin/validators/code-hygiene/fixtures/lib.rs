//! The crate root that makes the `missing-docs-rust` fixtures a cargo package.
//!
//! `cargo clippy` lints a package, never a loose file, so both fixture files
//! are modules here. The doctor runs the rule's script in this directory and
//! reads the findings of each fixture file on its own.

#[path = "missing-docs-rust.fail.rs"]
pub mod fail;

#[path = "missing-docs-rust.pass.rs"]
pub mod pass;
