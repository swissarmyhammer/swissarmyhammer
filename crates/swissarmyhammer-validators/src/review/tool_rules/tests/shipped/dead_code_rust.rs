//! Acceptance tests for the shipped `dead-code-rust` tool rule.
//!
//! Each test drives the SHIPPED script over a probe cargo package and reads
//! what that run answered.
//!
//! The module stands beside `dead_code`, which holds the whole family to its
//! fixture pair, because `cargo check` gives one exit status to a run it could
//! not make and to a run it made from end to end. The shapes that tell those
//! two apart are cargo's own, so they are measured for Rust alone.
//!
//! The tests fall in two halves, the way the script does. The first six drive
//! the `cargo check` half. The seven under them drive the ORPHAN-MODULE half,
//! which reads the tree itself and reports a file no declaration of its crate
//! names.
//!
//! Every test here stands under `#[serial_test::serial(env)]`. One of them
//! leads `PATH` with a command that answers nothing, and `PATH` is process
//! state: a probe that read that stubbed `PATH` would answer for another run's
//! staging. Measured over the six this module shipped with: run together
//! without the marker, the two probes that hold a MEASURED run reported
//! nothing, because their own `jq` was the stub the sixth test had put on
//! `PATH`.

use super::*;

/// Where the manifest of a Rust dead-code probe stands, as the work-list holds
/// it.
const RUST_DEAD_CODE_MANIFEST_PATH: &str = "Cargo.toml";

/// Where the library of a Rust dead-code probe stands, as the work-list holds
/// it.
const RUST_DEAD_CODE_LIB_PATH: &str = "src/lib.rs";

/// A cargo package that holds one dead item and nothing else the lint reports.
/// `[workspace]` keeps cargo inside the temporary directory.
const RUST_DEAD_CODE_MANIFEST: &str = concat!(
    "[package]\nname = \"dead-code-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// A Rust library holding one private function nothing reaches, which
/// `dead_code` reports at line 3 — the crate documentation line and the blank
/// line under it stand above it.
const RUST_DEAD_CODE_LIB: &str = concat!(
    "//! A probe crate for the shipped Rust dead-code tool rule.\n",
    "\n",
    "fn unused_helper() -> i32 {\n",
    "    1\n",
    "}\n",
);

/// The one finding the dead item of [`RUST_DEAD_CODE_LIB`] must report.
const RUST_DEAD_CODE_REPORTS: &[&str] = &["src/lib.rs:3"];

/// A Rust library the compiler cannot build: the struct declaration ends with
/// no semicolon.
const RUST_UNCOMPILABLE_LIB: &str = concat!(
    "//! A probe crate the compiler cannot build.\n",
    "\n",
    "pub struct Undocumented\n",
);

/// Every file of the uncompilable probe crate the work-list does not name.
const RUST_UNCOMPILABLE_SUPPORT_FILES: &[(&str, &str)] =
    &[(RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST)];

/// The line the script writes when a crate of the workspace did not compile,
/// so cargo never ran the lint over it.
const RUST_UNCOMPILABLE_LINE: &str = "dead-code-rust: cargo could not compile the workspace";

/// What the one error of a crate cargo cannot compile must name: the script's
/// own line, and cargo's own words beside it.
const RUST_UNCOMPILABLE_ERROR: &[&str] = &[RUST_UNCOMPILABLE_LINE, "could not compile"];

/// The `dead-code-rust` probe over a crate cargo cannot compile.
const RUST_UNCOMPILABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_UNCOMPILABLE_ERROR,
    },
    prompt_rule: DEAD_CODE_PROMPT_RULE,
    change_purpose: "a Rust crate the compiler cannot build",
    path: RUST_DEAD_CODE_LIB_PATH,
    source: Some(RUST_UNCOMPILABLE_LIB.as_bytes()),
    support: RUST_UNCOMPILABLE_SUPPORT_FILES,
};

/// Acceptance: the shipped Rust dead-code tool rule BREAKS on a crate cargo
/// cannot compile, through the real `cargo check` pipeline.
///
/// `cargo check` exits 101 for such a crate and writes no `dead_code`
/// diagnostic for it. An earlier shape of this script was one pipe that ended
/// in `sort -u`, and a shell pipeline takes the status of its last command, so
/// that shape answered exit 0 with no finding and the engine read the crate as
/// clean. Measured with cargo 1.97.1 over this probe: the pipe wrote 0 findings
/// and exited 0; the script writes the report to a file, tests it, and exits 1
/// with a line that names the rule.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_breaks_on_a_crate_that_does_not_compile() {
    verify_shipped_run_breaks(&RUST_UNCOMPILABLE_PROBE);
}

/// The root manifest of a probe workspace that holds two members.
const RUST_TWO_MEMBER_ROOT_MANIFEST: &str =
    "[workspace]\nmembers = [\"good\", \"bad\"]\nresolver = \"2\"\n";

/// The manifest of the member that compiles.
const RUST_GOOD_MEMBER_MANIFEST: &str =
    "[package]\nname = \"good\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// Where the manifest of the member that compiles stands.
const RUST_GOOD_MEMBER_MANIFEST_PATH: &str = "good/Cargo.toml";

/// Where the library of the member that compiles stands.
const RUST_GOOD_MEMBER_LIB_PATH: &str = "good/src/lib.rs";

/// The manifest of the member the compiler refuses.
const RUST_BAD_MEMBER_MANIFEST: &str =
    "[package]\nname = \"bad\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// Where the manifest of the member the compiler refuses stands.
const RUST_BAD_MEMBER_MANIFEST_PATH: &str = "bad/Cargo.toml";

/// Where the library of the member the compiler refuses stands.
const RUST_BAD_MEMBER_LIB_PATH: &str = "bad/src/lib.rs";

/// A probe workspace of two members: one the compiler refuses, and one that
/// holds a dead item of its own.
const RUST_TWO_MEMBER_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_UNCOMPILABLE_ERROR,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_TWO_MEMBER_ROOT_MANIFEST),
        (RUST_GOOD_MEMBER_MANIFEST_PATH, RUST_GOOD_MEMBER_MANIFEST),
        (RUST_GOOD_MEMBER_LIB_PATH, RUST_DEAD_CODE_LIB),
        (RUST_BAD_MEMBER_MANIFEST_PATH, RUST_BAD_MEMBER_MANIFEST),
        (RUST_BAD_MEMBER_LIB_PATH, RUST_UNCOMPILABLE_LIB),
    ],
    reason: "a workspace member the compiler refuses must break the run, whatever the \
             member beside it reported",
};

/// Acceptance: the shipped Rust dead-code tool rule BREAKS on a workspace whose
/// MEMBER cannot compile, even when another member fills the findings file,
/// through the real `cargo check` pipeline.
///
/// This rule states `scope: workspace`, and a real repository holds many
/// members. A gate that reads the filtered findings file cannot see the member
/// that failed: the member that compiles writes its dead item into that file,
/// the file is not empty, and the status test never runs. Measured with cargo
/// 1.97.1 over this shape: cargo exits 101 and the report holds `good/src/lib.rs`
/// beside no error code at all for the member that did not parse.
///
/// The RAW report holds what the filtered file drops, so the script tests that
/// report and the member that compiles cannot hide the member that does not.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_breaks_on_a_workspace_member_it_cannot_compile() {
    verify_shipped_tree_breaks(&RUST_TWO_MEMBER_PROBE);
}

/// The manifest of a probe package cargo must run a build script for before it
/// can check the library.
const RUST_BUILD_SCRIPT_MANIFEST: &str = concat!(
    "[package]\nname = \"build-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "build = \"build.rs\"\n",
    "\n[workspace]\n",
);

/// Where the build script of a probe package stands.
const RUST_BUILD_SCRIPT_PATH: &str = "build.rs";

/// A build script that breaks.
///
/// cargo writes its own words to stderr for this failure and writes NO
/// `compiler-message` into the report, so a gate that reads the compiler
/// messages alone cannot see it.
const RUST_BROKEN_BUILD_SCRIPT: &str =
    "fn main() { panic!(\"the build script of this probe breaks on purpose\"); }\n";

/// A build script that runs and does nothing.
const RUST_WORKING_BUILD_SCRIPT: &str = "fn main() {}\n";

/// The line the script writes when cargo compiled a build script and never ran
/// it, so cargo never checked the crate that build script serves.
const RUST_BUILD_SCRIPT_BROKEN_LINE: &str =
    "dead-code-rust: a build script did not run, so cargo did not check every crate";

/// What the one error of a build script that breaks must name.
const RUST_BUILD_SCRIPT_BROKEN_ERROR: &[&str] = &[RUST_BUILD_SCRIPT_BROKEN_LINE];

/// A probe package whose build script breaks, holding one dead item cargo
/// never reaches.
const RUST_BROKEN_BUILD_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_BUILD_SCRIPT_BROKEN_ERROR,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_BUILD_SCRIPT_MANIFEST),
        (RUST_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (RUST_DEAD_CODE_LIB_PATH, RUST_DEAD_CODE_LIB),
    ],
    reason: "a build script that breaks must break the run, because cargo never checked \
             the crate that build script serves",
};

/// Acceptance: the shipped Rust dead-code tool rule BREAKS on a package whose
/// BUILD SCRIPT breaks, through the real `cargo check` pipeline.
///
/// cargo runs a build script before it compiles the crate that script serves,
/// so a build script that breaks leaves that crate unchecked. Measured with
/// cargo 1.97.1 over this probe: cargo writes
/// `{"reason":"build-finished","success":false}`, one `compiler-artifact` for
/// the build script it compiled, and NO `compiler-message` at all. So the
/// report holds no error code, and a gate that reads the compiler messages
/// alone answers 0 findings at exit 0 — while the control below, the same
/// package under a build script that runs, reports its dead item.
///
/// cargo writes one `build-script-executed` entry for every build script it
/// RAN, and that entry is what a broken build script leaves out.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_breaks_on_a_build_script_that_breaks() {
    verify_shipped_tree_breaks(&RUST_BROKEN_BUILD_SCRIPT_PROBE);
}

/// The control of [`RUST_BROKEN_BUILD_SCRIPT_PROBE`]: the same package under a
/// build script that runs.
const RUST_WORKING_BUILD_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_DEAD_CODE_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_BUILD_SCRIPT_MANIFEST),
        (RUST_BUILD_SCRIPT_PATH, RUST_WORKING_BUILD_SCRIPT),
        (RUST_DEAD_CODE_LIB_PATH, RUST_DEAD_CODE_LIB),
    ],
    reason: "a build script that runs leaves the run measured, so the dead item must stand",
};

/// Acceptance: the shipped Rust dead-code tool rule MEASURES a package whose
/// build script RUNS, through the real `cargo check` pipeline.
///
/// This is the control of the test above. The two packages hold the same
/// manifest and the same library, and the build script is the one difference,
/// so a gate that broke this run would break every package that carries a
/// build script. This repository carries fifteen of them.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_measures_a_package_beside_a_build_script_that_runs() {
    verify_shipped_tree_reports(&RUST_WORKING_BUILD_SCRIPT_PROBE);
}

/// A cargo package that holds one rustc lint at deny level.
///
/// `[lints.rust]` is one of the three shapes that raise a lint to deny; the
/// other two are a crate-level `#![deny(...)]` and `RUSTFLAGS="-D warnings"`.
/// Each makes cargo exit nonzero for a workspace it checked from end to end.
const RUST_DENY_LEVEL_MANIFEST: &str = concat!(
    "[package]\nname = \"deny-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[lints.rust]\nunused_variables = \"deny\"\n",
    "\n[workspace]\n",
);

/// A Rust library that holds one variable nothing reads above the dead item,
/// so the denied lint stands beside the finding of this rule. The dead item
/// then stands at line 4.
const RUST_DENY_LEVEL_LIB: &str = concat!(
    "//! A probe crate for the shipped Rust dead-code tool rule.\n",
    "\n",
    "pub fn first() -> i32 { let unused = 1; 2 }\n",
    "fn unused_helper() -> i32 {\n",
    "    1\n",
    "}\n",
);

/// The one finding the dead item of [`RUST_DENY_LEVEL_LIB`] must report.
const RUST_DENY_LEVEL_REPORTS: &[&str] = &["src/lib.rs:4"];

/// A probe package that stands a rustc lint at deny level beside one dead item.
const RUST_DENY_LEVEL_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_DENY_LEVEL_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DENY_LEVEL_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_DENY_LEVEL_LIB),
    ],
    reason: "a lint at deny level makes cargo exit nonzero for a workspace it DID check, so \
             the run must keep the finding the report holds",
};

/// Acceptance: the shipped Rust dead-code tool rule MEASURES a workspace that
/// stands a rustc lint at deny level, through the real `cargo check` pipeline.
///
/// `cargo check` exits nonzero for two different reasons. cargo could not check
/// the workspace, and cargo checked the workspace correctly while a lint stands
/// at deny level. A gate that reads the status alone cannot tell the two apart,
/// and it throws away every finding of the second one.
///
/// Measured with cargo 1.97.1 over this probe: cargo exits 101, writes
/// `error: could not compile` to stderr, and writes the `dead_code` diagnostic
/// for `unused_helper` into the report at level `warning`. So the script tests
/// the REPORT beside the status, which is what `builtin/validators/README.md`
/// states for a status two shapes share.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_measures_a_workspace_beside_a_deny_level_lint() {
    verify_shipped_tree_reports(&RUST_DENY_LEVEL_PROBE);
}

/// The line the script writes when the filter could not read the report.
const RUST_FILTER_BROKEN_LINE: &str = "dead-code-rust: jq could not read the cargo report";

/// What the one error of a filter that cannot read the report must name.
const RUST_FILTER_BROKEN_ERROR: &[&str] = &[RUST_FILTER_BROKEN_LINE];

/// The healthy probe package, read with a filter that cannot run.
const RUST_FILTER_BROKEN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_FILTER_BROKEN_ERROR,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_DEAD_CODE_LIB),
    ],
    reason: "a filter that cannot read the report must break the run",
};

/// Acceptance: the shipped Rust dead-code tool rule BREAKS when the filter
/// cannot read the cargo report, through the real `cargo check` pipeline.
///
/// The filter step once stood in a pipe that ended in `sort -u`, and a shell
/// pipeline takes the status of its last command. Measured over this probe
/// package, which gives one finding, with the filter replaced by a command that
/// exits 127: the pipe shape wrote the orphan half alone and exited 0, so the
/// whole `dead_code` half went missing without a word. The shipped shape writes
/// no finding, that line, and exit 1.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_breaks_when_the_filter_cannot_read_the_report() {
    verify_shipped_tree_breaks_without(&RUST_FILTER_BROKEN_PROBE, FILTER_BINARY_NAME);
}

/// What a probe of the orphan-module half reports when nothing names the file
/// it stages at [`RUST_ORPHAN_PATH`].
const RUST_ORPHAN_REPORTS: &[&str] = &["src/orphan.rs:1"];

/// What a probe reports when every file it stages is compiled.
const RUST_NO_REPORTS: &[&str] = &[];

/// Where a probe stages the file no `mod` declaration names.
const RUST_ORPHAN_PATH: &str = "src/orphan.rs";

/// A file that carries an inner documentation comment and nothing else, so a
/// probe measures the SCAN rather than any diagnostic the file could raise.
const RUST_ORPHAN_FILE: &str = "//! A file the probe crate names nowhere.\n";

/// A crate root that names no module at all.
const RUST_BARE_LIB: &str = "//! A probe crate whose root names no module.\n";

/// A probe package that stages one file no declaration names.
///
/// This is the control of every probe under it: each of those stages the same
/// orphan file beside a crate root that names it in one shape or another, so a
/// scan that stopped reporting altogether would pass them all and break this
/// one.
const RUST_ORPHAN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_ORPHAN_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_BARE_LIB),
        (RUST_ORPHAN_PATH, RUST_ORPHAN_FILE),
    ],
    reason: "a file no declaration of the crate names is an orphan",
};

/// Acceptance: the shipped Rust dead-code tool rule REPORTS a file that no
/// declaration of its crate names.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reports_a_file_no_declaration_names() {
    verify_shipped_tree_reports(&RUST_ORPHAN_PROBE);
}

/// Where a probe stages the file its crate root compiles through `include!`.
const RUST_INCLUDED_PATH: &str = "src/generated.rs";

/// A crate root that compiles the file beside it through `include!`, which
/// names that file by no `mod` declaration and by no `#[path]` attribute.
const RUST_INCLUDE_LIB: &str = concat!(
    "//! A probe crate that compiles the file beside it through `include!`.\n",
    "\n",
    "include!(\"generated.rs\");\n",
);

/// The file `include!` compiles into the crate root.
const RUST_INCLUDED_FILE: &str = concat!(
    "/// An item the crate root compiles through `include!`.\n",
    "pub fn generated() -> i32 {\n",
    "    1\n",
    "}\n",
);

/// A probe package that compiles a file through `include!`.
const RUST_INCLUDE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_NO_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_INCLUDE_LIB),
        (RUST_INCLUDED_PATH, RUST_INCLUDED_FILE),
    ],
    reason: "`include!` compiles the file it names, so that file is no orphan",
};

/// Acceptance: the shipped Rust dead-code tool rule KEEPS a file its crate
/// compiles through `include!`.
///
/// The claim the orphan line makes is that nothing compiles the file. An
/// `include!` compiles it, so the claim is false for such a file and the scan
/// must stay silent. Measured over this probe against the earlier index, which
/// read `mod` declarations and `#[path]` attributes alone: the run reported
/// `src/generated.rs:1` for a file the compiler reads.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_keeps_a_file_an_include_compiles() {
    verify_shipped_tree_reports(&RUST_INCLUDE_PROBE);
}

/// An orphan file carrying the whole-file suppression marker with a reason.
const RUST_MARKED_ORPHAN_FILE: &str = concat!(
    "// sah:ignore orphan-module the build script of the consumer crate reads this file\n",
    "//! A file the probe crate names nowhere.\n",
);

/// A probe package whose orphan file carries the suppression marker.
const RUST_MARKED_ORPHAN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_NO_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_BARE_LIB),
        (RUST_ORPHAN_PATH, RUST_MARKED_ORPHAN_FILE),
    ],
    reason: "the marker beside a reason states that something the scan cannot read \
             compiles the file",
};

/// Acceptance: the shipped Rust dead-code tool rule READS the whole-file
/// suppression marker.
///
/// A crate can compile a file through a `mod` name a macro builds, through an
/// `include!` whose path is an expression, or through a build script. The scan
/// reads none of those, so the exemption a person would argue for in prose
/// becomes a marker the scan reads, which is what
/// `builtin/validators/README.md` asks of every tool rule.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reads_the_orphan_marker() {
    verify_shipped_tree_reports(&RUST_MARKED_ORPHAN_PROBE);
}

/// An orphan file carrying the suppression marker with no reason after it.
const RUST_REASONLESS_ORPHAN_FILE: &str = concat!(
    "// sah:ignore orphan-module\n",
    "//! A file the probe crate names nowhere.\n",
);

/// A probe package whose orphan file carries a marker that states no reason.
const RUST_REASONLESS_ORPHAN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_ORPHAN_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_BARE_LIB),
        (RUST_ORPHAN_PATH, RUST_REASONLESS_ORPHAN_FILE),
    ],
    reason: "a marker that states no reason names nothing, so it suppresses nothing",
};

/// Acceptance: the shipped Rust dead-code tool rule REPORTS an orphan whose
/// marker states no reason.
///
/// The reason is the whole content of the marker: it names what compiles the
/// file. A marker with no reason is a claim with no subject, so it leaves the
/// finding standing.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reports_an_orphan_whose_marker_states_no_reason() {
    verify_shipped_tree_reports(&RUST_REASONLESS_ORPHAN_PROBE);
}

/// A crate root that names the orphan file in comments alone — one line
/// comment, and one block comment holding the declaration at column zero.
const RUST_COMMENTED_MOD_LIB: &str = concat!(
    "//! A probe crate that names the orphan in comments alone.\n",
    "\n",
    "// mod orphan;\n",
    "/*\n",
    "mod orphan;\n",
    "*/\n",
);

/// A probe package whose only `mod` declaration stands in a comment.
const RUST_COMMENTED_MOD_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_ORPHAN_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_COMMENTED_MOD_LIB),
        (RUST_ORPHAN_PATH, RUST_ORPHAN_FILE),
    ],
    reason: "a comment compiles nothing, so a `mod` declaration inside one names no file",
};

/// Acceptance: the shipped Rust dead-code tool rule READS no `mod` declaration
/// out of a comment.
///
/// The earlier index came from `grep -rhoE '\bmod[[:space:]]+[A-Za-z_]...'`,
/// which matched the word wherever it stood. Measured over this probe against
/// that index: the run reported 0 findings, so a commented-out declaration hid
/// a real orphan. A block comment hides it the same way, and the declaration
/// stands at column zero inside one here, where no anchor can reach it.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reads_no_module_declaration_from_a_comment() {
    verify_shipped_tree_reports(&RUST_COMMENTED_MOD_PROBE);
}

/// A crate root that holds the text of a module declaration in a string
/// literal, which declares nothing.
const RUST_STRING_MOD_LIB: &str = concat!(
    "//! A probe crate that holds a module declaration in a string.\n",
    "\n",
    "/// The text of a module declaration, which declares no module.\n",
    "pub const DECLARATION: &str = \"mod orphan;\";\n",
);

/// A probe package whose only `mod` text stands inside a string literal.
const RUST_STRING_MOD_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_ORPHAN_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_STRING_MOD_LIB),
        (RUST_ORPHAN_PATH, RUST_ORPHAN_FILE),
    ],
    reason: "a string literal is data, so a `mod` declaration inside one names no file",
};

/// Acceptance: the shipped Rust dead-code tool rule READS no `mod` declaration
/// out of a string literal.
///
/// This crate holds many such strings: every probe of this very file stages a
/// crate root as a Rust string. Measured over this probe against the earlier
/// index: the run reported 0 findings.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reads_no_module_declaration_from_a_string_literal() {
    verify_shipped_tree_reports(&RUST_STRING_MOD_PROBE);
}

/// Where a probe stages the file its inline test module really names.
const RUST_TEST_MODULE_PATH: &str = "src/tests/orphan.rs";

/// A crate root whose inline `#[cfg(test)]` module names a module of its own.
///
/// The nested declaration names `src/tests/orphan.rs`, never `src/orphan.rs`,
/// because an inline module adds its own name to the module directory.
const RUST_TEST_MODULE_LIB: &str = concat!(
    "//! A probe crate whose inline test module names a module of its own.\n",
    "\n",
    "#[cfg(test)]\n",
    "mod tests {\n",
    "    mod orphan;\n",
    "}\n",
);

/// The module the inline test module of the probe names.
const RUST_TEST_MODULE_FILE: &str = concat!(
    "//! The module the inline test module of the probe names.\n",
    "\n",
    "#[test]\n",
    "fn probe() {}\n",
);

/// A probe package whose inline test module names one file, beside an orphan
/// of the same stem.
const RUST_TEST_MODULE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_DEAD_CODE_RULE,
        expected: RUST_ORPHAN_REPORTS,
    },
    staged: &[
        (RUST_DEAD_CODE_MANIFEST_PATH, RUST_DEAD_CODE_MANIFEST),
        (RUST_DEAD_CODE_LIB_PATH, RUST_TEST_MODULE_LIB),
        (RUST_TEST_MODULE_PATH, RUST_TEST_MODULE_FILE),
        (RUST_ORPHAN_PATH, RUST_ORPHAN_FILE),
    ],
    reason: "a nested declaration names the file under its own module directory, so it \
             excuses that file and no other",
};

/// Acceptance: the shipped Rust dead-code tool rule reads a NESTED `mod`
/// declaration as the file it really names.
///
/// The earlier index held bare stems, so a `mod orphan;` inside
/// `#[cfg(test)] mod tests` excused every `orphan.rs` of the crate. Measured
/// over this probe against that index: the run reported 0 findings, and the
/// real orphan at `src/orphan.rs` went missing.
///
/// The probe measures the other direction in the same run: `src/tests/orphan.rs`
/// is the file the nested declaration names, and it must stay out of the
/// report.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_dead_code_tool_rule_reads_a_nested_module_declaration_as_its_own_file() {
    verify_shipped_tree_reports(&RUST_TEST_MODULE_PROBE);
}
