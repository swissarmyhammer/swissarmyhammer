//! Acceptance tests for the shipped missing-docs tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool, so each measures the shipped script rather than a copy.
//!
//! Each language stands in its own module under this one, so each file stays
//! small enough for the review engine to read whole.
//!
//! The Rust rule stands in `missing_docs_rust`, because the shapes `cargo
//! clippy` answers a broken run with are cargo's own.

use super::*;

/// The row the declaration that OPENS a probe source stands on: the first line
/// of the file.
///
/// Every probe source of this file that its tool reports at the head writes
/// that declaration on the first line of the staged bytes, so the number is a
/// fact of the fixture and not a value the probe chose.
///
/// The row stands in a macro beside the probes that hold it because each probe
/// keeps its rows in a `&'static [&'static str]` built with `concat!`, and
/// `concat!` takes literals alone.
macro_rules! opening_declaration_row {
    () => {
        "1"
    };
}

/// The row the declaration NESTED under the opening declaration stands on: the
/// second line of the file.
///
/// Each nested probe source writes its member on the line directly under the
/// type head that opens the file, so the number is a fact of the staged bytes.
///
/// It stands in a macro for the reason [`opening_declaration_row`] states.
macro_rules! nested_declaration_row {
    () => {
        "2"
    };
}

/// Acceptance: every shipped missing-docs tool rule passes its fixture pair
/// in doctor, and supersedes the `missing-docs` prompt rule.
///
/// A tool that reads the whole public surface answers the documentation
/// question the prompt rule asks, so it replaces it for the files it covers.
/// [`verify_shipped_tool_rules_pass_fixtures`] carries the rest of the
/// contract, including what a machine without the tool proves.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_missing_docs_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_MISSING_DOCS_RULES, MISSING_DOCS_PROMPT_RULE);
}

mod dart;
mod go;
mod python;
mod swift;
mod typescript;
