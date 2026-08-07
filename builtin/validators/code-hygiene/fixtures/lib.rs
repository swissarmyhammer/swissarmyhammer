//! The crate root that makes the `missing-docs-rust` fixtures a cargo package.
//!
//! `cargo clippy` lints a package, never a loose file, so both fixture files
//! are modules here. The doctor runs the rule's script in this directory and
//! reads the findings of each fixture file on its own.

/// The failing fixture: it holds the undocumented public item the tool must
/// report.
#[path = "missing-docs-rust.fail.rs"]
pub mod fail;

/// The passing fixture: every public item in it is documented, so the tool
/// must report nothing.
#[path = "missing-docs-rust.pass.rs"]
pub mod pass;
