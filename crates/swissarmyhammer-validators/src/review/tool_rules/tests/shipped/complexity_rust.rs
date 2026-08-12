//! Acceptance tests for the shipped `complexity-rust` tool rule.
//!
//! Each test drives the SHIPPED script over a probe cargo package and reads
//! what the real `cargo clippy` reported.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::complexity::complexity_work;
use super::*;

/// A cargo package holding one function over the nesting gate and nothing
/// else the four lints report. `[workspace]` keeps cargo inside the
/// temporary directory.
const COMPLEX_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"complex-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// The library of [`COMPLEX_PACKAGE_MANIFEST`]. `fold_grid` is a free
/// function at control-flow depth 6, and a free function body is itself one
/// level, so its innermost block sits at nesting level 7 against the gate
/// of 6. The Rust tool rule must report it once. The body stays well under
/// the line gate and takes two arguments, so the same run reports nothing
/// else.
const COMPLEX_LIB_RS: &str = r#"//! A probe crate for the shipped Rust complexity tool rule.

/// Folds a grid of readings into one band, one nested block for each test.
pub fn fold_grid(grid: &[Vec<i32>], limit: i32) -> i32 {
    let mut band = 0;
    for row in grid {
        for cell in row {
            if *cell > 0 {
                if *cell < limit {
                    while band < *cell {
                        if band % 2 == 0 {
                            band += 2;
                        }
                        band += 1;
                    }
                }
            }
        }
    }
    band
}
"#;

/// The library path inside the complexity probe package, as the work-list
/// holds it.
const COMPLEX_LIB_PATH: &str = "src/lib.rs";

/// Acceptance: the shipped Rust complexity tool rule reports an over-complex
/// function on a real cargo workspace, through the real clippy pipeline,
/// and suppresses both prompt rules it supersedes.
///
/// The suppression half is what a rule that supersedes two names buys. A
/// healthy `complexity-rust` must silence `cognitive-complexity` AND
/// `function-length` for the file, so no LLM re-reads a gate the tool
/// already decided.
///
/// The reporting half also proves the threshold reached clippy.
/// `excessive-nesting-threshold` defaults to `0`, which turns the lint off
/// altogether, so the probe reports only when the script's temporary
/// `clippy.toml` is the file `CLIPPY_CONF_DIR` names.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_an_over_complex_function() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("Cargo.toml"), COMPLEX_PACKAGE_MANIFEST).unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join(COMPLEX_LIB_PATH), COMPLEX_LIB_RS).unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_COMPLEXITY_RULE);
    let work = complexity_work(RUST_COMPLEXITY_RULE, COMPLEX_LIB_PATH, COMPLEX_LIB_RS);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_COMPLEXITY_RULE);
    assert_eq!(run.files(), [COMPLEX_LIB_PATH.to_string()]);
    let suppressed = plan
        .suppression()
        .suppressed_rules(CODE_HYGIENE_SET, COMPLEX_LIB_PATH);
    for prompt_rule in SUPERSEDES_BOTH_COMPLEXITY_GATES {
        assert!(
            suppressed.contains(*prompt_rule),
            "a healthy tool rule that supersedes two prompt rules must suppress both; \
             `{prompt_rule}` is missing from {suppressed:?}"
        );
    }

    verify_run_reports_one_finding(
        run,
        repo.path(),
        COMPLEX_LIB_PATH,
        CODE_HYGIENE_SET,
        RUST_COMPLEXITY_RULE,
        "too nested",
    );
}

/// Where the probe cargo manifest stands inside a Rust probe repository.
const RUST_PROBE_MANIFEST_PATH: &str = "Cargo.toml";

/// The probe cargo manifest, staged beside a Rust probe file that the
/// work-list does NOT name.
///
/// A `workspace`-scope rule loads a project rather than a file list, so cargo
/// breaks on the staged file only when it finds a package to lint.
const RUST_PROBE_SUPPORT_FILES: &[(&str, &str)] =
    &[(RUST_PROBE_MANIFEST_PATH, COMPLEX_PACKAGE_MANIFEST)];

/// One line of the body of a function built to run over the length gate.
const LONG_FUNCTION_BODY_LINE: &str = "    let _ = 1;\n";

/// How many body lines carry a function over `too-many-lines-threshold = 250`.
///
/// Clippy counts the body lines and the two brace lines alike, so 300 body
/// lines answer 302 against the gate of 250.
const LONG_FUNCTION_BODY_LINES: usize = 300;

/// A Rust function named `name` whose body runs [`LONG_FUNCTION_BODY_LINES`]
/// lines, with `head` written above its `pub fn` line.
///
/// Every shape the length gate measures runs past 250 lines, and
/// [`ShippedStagedRows`] carries only `&'static` bytes, so a probe of that gate
/// builds its source here rather than writing 300 lines out for each function.
fn long_rust_function(head: &str, name: &str) -> String {
    format!(
        "{head}pub fn {name}() {{\n{}}}\n",
        LONG_FUNCTION_BODY_LINE.repeat(LONG_FUNCTION_BODY_LINES)
    )
}

/// Drives the shipped `complexity-rust` script over a probe cargo package that
/// holds `files`, and answers each finding it reported as `path:line`, sorted.
///
/// The manifest is staged beside `files` because cargo lints a package and
/// never a loose file. The findings are the SCRIPT's own, before the engine
/// keeps only the ones in the changed files.
fn rust_complexity_findings(files: &[(&str, &str)]) -> Vec<String> {
    rust_complexity_findings_under(COMPLEX_PACKAGE_MANIFEST, files)
}

/// Drives the shipped `complexity-rust` script over a probe cargo package whose
/// manifest holds `manifest` and which holds `files`, and answers each finding
/// it reported as `path:line`, sorted.
///
/// The manifest is a parameter because one probe writes a clippy lint at deny
/// level into it, and that shape shares its exit status with a run cargo could
/// not make.
fn rust_complexity_findings_under(manifest: &str, files: &[(&str, &str)]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let mut staged: Vec<(&str, &str)> = vec![(RUST_PROBE_MANIFEST_PATH, manifest)];
    staged.extend_from_slice(files);
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let reported = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect("the shipped Rust complexity script must judge the probe package and exit 0");

    sorted_names(&reported)
}

/// The `path:line` entry [`shipped_script_findings`] answers a finding at row
/// `row` of `path` with.
///
/// Every expected entry of a Rust complexity probe is built here, from the
/// same constant the probe stages the file under, so a path that moves moves
/// its expected entries with it.
fn probe_row(path: &str, row: usize) -> String {
    format!("{path}:{row}")
}

/// How many lines one [`long_rust_function`] runs with no head above it: the
/// `pub fn` line, [`LONG_FUNCTION_BODY_LINES`] body lines, and the closing
/// brace.
const BARE_LONG_FUNCTION_LINES: usize = LONG_FUNCTION_BODY_LINES + 2;

/// The annotation the rule states for a function the length gate reports.
const LENGTH_GATE_ANNOTATION: &str =
    "#[expect(clippy::too_many_lines, reason = \"one line for each field\")]\n";

/// The row the one finding of the annotation probe stands on.
///
/// The bare function opens the probe library with no head above it, so its
/// `pub fn` line is row 1. The annotated function holds the same body under
/// it, and a run that reported it as well would name the row directly under
/// [`BARE_LONG_FUNCTION_LINES`] plus its own annotation line.
const RUST_BARE_LONG_FUNCTION_ROW: usize = 1;

/// Acceptance: the shipped Rust complexity tool rule drops a long function
/// that carries the length-gate annotation, and keeps the bare one beside it,
/// through the real clippy pipeline.
///
/// `function-length`, one of the two prompt rules this rule supersedes, exempts
/// "Functions that are mostly configuration/data (e.g., builder patterns with
/// many options)" and "Initialization functions that set many fields". Clippy
/// counts a data line like a code line, and its configuration holds no key that
/// tells the two apart, so the run cannot reproduce that carve-out. The
/// annotation is the whole answer, and this test holds it.
///
/// Both functions hold the same 300 body lines, so the annotation is the one
/// difference between the function that reports and the function that stays
/// silent.
#[test]
fn the_shipped_rust_complexity_tool_rule_answers_the_length_gate_annotation() {
    let source = format!(
        "{}{}",
        long_rust_function("", "bare_defaults"),
        long_rust_function(LENGTH_GATE_ANNOTATION, "annotated_defaults")
    );

    let reported = rust_complexity_findings(&[(COMPLEX_LIB_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[probe_row(COMPLEX_LIB_PATH, RUST_BARE_LONG_FUNCTION_ROW)]),
        "the annotation is the author's answer to the data carve-out, so the annotated \
         function must stay silent and the bare one must report"
    );
}

/// A probe crate root with nothing for the four lints to read, for a probe that
/// measures another file of the same package.
const EMPTY_PROBE_LIB_RS: &str = "//! A probe crate root with nothing to lint.\n";

/// Where the probe integration test stands inside the probe repository.
const RUST_PROBE_INTEGRATION_TEST_PATH: &str = "tests/it.rs";

/// The attribute that marks the probe's test function at its DEFINITION, which
/// is the mark `cognitive-complexity` states for its test carve-out.
const RUST_TEST_ATTRIBUTE: &str = "#[test]\n";

/// How many lines [`RUST_TEST_ATTRIBUTE`] runs above the `pub fn` line it
/// marks.
const RUST_TEST_ATTRIBUTE_LINES: usize = 1;

/// The row the test function of the test-carve-out probe stands on. The
/// attribute stands above it.
const RUST_TEST_FUNCTION_ROW: usize = RUST_TEST_ATTRIBUTE_LINES + 1;

/// The row the helper beside that test function stands on. The test function
/// runs [`RUST_TEST_ATTRIBUTE_LINES`] plus [`BARE_LONG_FUNCTION_LINES`] lines,
/// and the helper opens on the next one.
const RUST_TEST_HELPER_ROW: usize = RUST_TEST_ATTRIBUTE_LINES + BARE_LONG_FUNCTION_LINES + 1;

/// Acceptance: the shipped Rust complexity tool rule REPORTS a long test
/// function, and the helper beside it, through the real clippy pipeline.
///
/// Both prompt rules this rule supersedes exempt a test, and
/// `cognitive-complexity` names the DEFINITION as the mark: "A complex helper
/// named `build_request` in a file called `foo_test.rs` is still a complex
/// function and is still listed."
///
/// Clippy holds no flag and no configuration key that reads `#[test]`, so the
/// run reproduces none of that carve-out and the author answers it with the
/// annotation. `--all-targets` is what puts the test target in front of the
/// gates. Dropping the flag would read the TARGET, which is the mark the prompt
/// rule forbids: it drops the helper beside the test, and it drops every
/// `#[cfg(test)]` module as well. This test holds both rows, so a run that
/// silenced either half answers another list.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_a_test_function_and_its_helper() {
    let source = format!(
        "{}{}",
        long_rust_function(RUST_TEST_ATTRIBUTE, "test_table"),
        long_rust_function("", "build_request")
    );

    let reported = rust_complexity_findings(&[
        (COMPLEX_LIB_PATH, EMPTY_PROBE_LIB_RS),
        (RUST_PROBE_INTEGRATION_TEST_PATH, &source),
    ]);

    assert_eq!(
        reported,
        sorted_names(&[
            probe_row(RUST_PROBE_INTEGRATION_TEST_PATH, RUST_TEST_FUNCTION_ROW),
            probe_row(RUST_PROBE_INTEGRATION_TEST_PATH, RUST_TEST_HELPER_ROW),
        ]),
        "`--all-targets` puts the test target in front of the gates, and no clippy key \
         reads `#[test]`, so the test function and the helper beside it both report"
    );
}

/// The head a generated Rust file carries in the probe: one generator writes
/// the first line, another writes the second, and clippy reads neither.
const RUST_GENERATED_HEAD: &str = concat!(
    "// This file is @generated by prost-build.\n",
    "// Code generated by tool. DO NOT EDIT.\n",
);

/// The crate root of the generated-code probe.
///
/// It names two module files that hold the same bytes, so the annotation on the
/// second declaration is the one difference between them. The declaration
/// stands in this file, which the generator never writes again.
const RUST_GENERATED_ROOT_RS: &str = concat!(
    "//! A probe crate root that names two generated modules.\n",
    "pub mod bare;\n",
    "#[expect(clippy::too_many_lines, reason = \"the generator writes this file\")]\n",
    "pub mod annotated;\n",
);

/// Where the generated module file with no annotation stands inside the probe
/// repository.
const RUST_GENERATED_BARE_PATH: &str = "src/bare.rs";

/// Where the generated module file whose declaration carries the annotation
/// stands inside the probe repository.
const RUST_GENERATED_ANNOTATED_PATH: &str = "src/annotated.rs";

/// How many lines [`RUST_GENERATED_HEAD`] runs above the `pub fn` line under
/// it.
const RUST_GENERATED_HEAD_LINES: usize = 2;

/// The row the one finding of the generated-code probe stands on. The head
/// stands above the `pub fn` line of each module file, and the two module
/// files hold the same bytes, so a run that reported the annotated one as well
/// would name the same row of [`RUST_GENERATED_ANNOTATED_PATH`].
const RUST_GENERATED_FUNCTION_ROW: usize = RUST_GENERATED_HEAD_LINES + 1;

/// Acceptance: the shipped Rust complexity tool rule REPORTS a checked-in
/// generated file, and drops the one whose module declaration carries the
/// annotation, through the real clippy pipeline.
///
/// Both prompt rules this rule supersedes exempt generated code. Rust states no
/// generated-file header convention, and clippy reads no header: the two module
/// files here each carry the header two generators write, and the bare one
/// still reports. A header test in the script would name the first lines of one
/// generator and never a convention, which is why the sibling `complexity-go`
/// makes such a test and this rule does not.
///
/// The author answers this carve-out at the `mod` declaration, which stands in
/// the PARENT file and which the generator never writes again. The two module
/// files hold the same bytes, so the annotation is the one difference between
/// the file that reports and the file that stays silent.
#[test]
fn the_shipped_rust_complexity_tool_rule_reports_a_generated_file() {
    let generated = long_rust_function(RUST_GENERATED_HEAD, "fold_grid");

    let reported = rust_complexity_findings(&[
        (COMPLEX_LIB_PATH, RUST_GENERATED_ROOT_RS),
        (RUST_GENERATED_BARE_PATH, &generated),
        (RUST_GENERATED_ANNOTATED_PATH, &generated),
    ]);

    assert_eq!(
        reported,
        sorted_names(&[probe_row(
            RUST_GENERATED_BARE_PATH,
            RUST_GENERATED_FUNCTION_ROW,
        )]),
        "clippy reads no generated-file header, so the bare module reports; the annotation \
         on the other module's declaration is what silences it"
    );
}

/// A Rust library the compiler refuses: the body of `broken` answers a string
/// where its signature states an integer.
const RUST_COMPLEXITY_UNCOMPILABLE_SOURCE: &str = concat!(
    "//! A probe crate the compiler refuses.\n",
    "pub fn broken() -> i32 { \"not an integer\" }\n",
);

/// The line the script writes when clippy read the workspace but never linted
/// part of it.
const RUST_UNLINTABLE_LINE: &str = "complexity-rust: cargo clippy could not lint the workspace";

/// What the one error of a workspace cargo cannot lint must name: the script's
/// own line, and cargo's own words beside it.
const RUST_COMPLEXITY_UNCOMPILABLE_ERROR: &[&str] = &[RUST_UNLINTABLE_LINE, "could not compile"];

/// The `complexity-rust` probe over a workspace that does not compile.
const RUST_COMPLEXITY_UNCOMPILABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_COMPLEXITY_RULE,
        expected: RUST_COMPLEXITY_UNCOMPILABLE_ERROR,
    },
    prompt_rule: COGNITIVE_COMPLEXITY_PROMPT_RULE,
    change_purpose: "a Rust workspace the compiler refuses",
    path: COMPLEX_LIB_PATH,
    source: Some(RUST_COMPLEXITY_UNCOMPILABLE_SOURCE.as_bytes()),
    support: RUST_PROBE_SUPPORT_FILES,
};

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a workspace it
/// cannot compile, through the real clippy pipeline.
///
/// `cargo clippy` lints nothing when the workspace does not compile: it writes
/// its own errors to stderr, writes no lint message, and exits nonzero. An
/// earlier shape of this script was one pipe that ended in `sort -u`, and a
/// shell pipeline takes the status of its last command, so that shape answered
/// exit 0 with no finding and the engine read the whole tree as clean. The
/// script now writes the report to a file, tests the status, and exits 1 with a
/// line that names the rule.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile() {
    verify_shipped_run_breaks(&RUST_COMPLEXITY_UNCOMPILABLE_PROBE);
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

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a workspace
/// whose MEMBER cannot compile, even when another member fills the findings
/// file, through the real clippy pipeline.
///
/// This rule states `scope: workspace`, and a real repository holds many
/// members. A gate that reads the filtered findings file cannot see the member
/// that failed: the member that compiles writes a finding, the file is not
/// empty, and the status test never runs. Measured over this shape with clippy
/// 0.1.97: cargo exits 101, the earlier gate wrote `good/src/lib.rs:1` alone
/// and exited 0, and the long function of `bad/src/lib.rs` was read as clean.
///
/// The RAW report holds what the filtered file drops. A member that fails its
/// type check writes a rustc error code, `E0308` here, and clippy runs the four
/// lints only after that type check. So the script tests the raw report for a
/// rustc error code, and the member that compiles cannot hide the member that
/// does not.
///
/// Both members hold the same long function, so the member that compiles is the
/// one that fills the findings file.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_member_it_cannot_compile() {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let good = long_rust_function("", "fold_grid");
    let bad = format!(
        "{RUST_COMPLEXITY_UNCOMPILABLE_SOURCE}{}",
        long_rust_function("", "fold_grid")
    );
    let staged = [
        (RUST_PROBE_MANIFEST_PATH, RUST_TWO_MEMBER_ROOT_MANIFEST),
        (RUST_GOOD_MEMBER_MANIFEST_PATH, RUST_GOOD_MEMBER_MANIFEST),
        (RUST_GOOD_MEMBER_LIB_PATH, good.as_str()),
        (RUST_BAD_MEMBER_MANIFEST_PATH, RUST_BAD_MEMBER_MANIFEST),
        (RUST_BAD_MEMBER_LIB_PATH, bad.as_str()),
    ];
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let failure = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect_err("a workspace member the compiler refuses must break the run");

    let detail = failure.to_string();
    assert!(
        detail.contains(RUST_UNLINTABLE_LINE),
        "the run must break with '{RUST_UNLINTABLE_LINE}'; got '{detail}'"
    );
}

/// The manifest of a probe package cargo must run a build script for before
/// clippy can lint its library.
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
/// it, so clippy never linted the crate that build script serves.
const RUST_BUILD_SCRIPT_BROKEN_LINE: &str =
    "complexity-rust: a build script did not run, so clippy did not lint every crate";

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a package whose
/// BUILD SCRIPT breaks, through the real clippy pipeline.
///
/// A build script that breaks is a fourth reason cargo exits nonzero, beside a
/// run cargo could not make, a crate that fails its type check, and a lint at
/// deny level. clippy never lints the crate that build script serves.
///
/// Measured with clippy 0.1.97 over this probe: cargo writes
/// `{"reason":"build-finished","success":false}`, one `compiler-artifact` for
/// the build script it compiled, and NO `compiler-message` at all. So the
/// report holds no rustc error code, and the earlier gate answered 0 findings
/// at exit 0. The control below, the same package under
/// [`RUST_WORKING_BUILD_SCRIPT`], reports `src/lib.rs:1`, so the earlier gate
/// lost one finding and read the package as clean.
///
/// cargo writes one `build-script-executed` entry for every build script it
/// RAN, and that entry is what a broken build script leaves out.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_build_script_that_breaks() {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let source = long_rust_function("", "fold_grid");
    let staged = [
        (RUST_PROBE_MANIFEST_PATH, RUST_BUILD_SCRIPT_MANIFEST),
        (RUST_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (COMPLEX_LIB_PATH, source.as_str()),
    ];
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let failure = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect_err("a build script that breaks must break the run");

    let detail = failure.to_string();
    assert!(
        detail.contains(RUST_BUILD_SCRIPT_BROKEN_LINE),
        "the run must break with '{RUST_BUILD_SCRIPT_BROKEN_LINE}'; got '{detail}'"
    );
}

/// Acceptance: the shipped Rust complexity tool rule MEASURES a package whose
/// build script RUNS, through the real clippy pipeline.
///
/// This is the control of the test above. The two packages hold the same
/// manifest and the same library, and the build script is the one difference,
/// so a gate that broke this run would break every package that carries a
/// build script. This repository carries eight of them.
#[test]
fn the_shipped_rust_complexity_tool_rule_measures_a_package_beside_a_build_script_that_runs() {
    let source = long_rust_function("", "fold_grid");

    let reported = rust_complexity_findings_under(
        RUST_BUILD_SCRIPT_MANIFEST,
        &[
            (RUST_BUILD_SCRIPT_PATH, RUST_WORKING_BUILD_SCRIPT),
            (COMPLEX_LIB_PATH, &source),
        ],
    );

    assert_eq!(
        reported,
        sorted_names(&[probe_row(COMPLEX_LIB_PATH, RUST_BARE_LONG_FUNCTION_ROW)]),
        "a build script that runs leaves the run measured, so the finding must stand"
    );
}

/// The manifest of the member whose build script cargo must run.
const RUST_BAD_MEMBER_BUILD_MANIFEST: &str = concat!(
    "[package]\nname = \"bad\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "build = \"build.rs\"\n",
);

/// Where the build script of that member stands.
const RUST_BAD_MEMBER_BUILD_SCRIPT_PATH: &str = "bad/build.rs";

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a workspace
/// whose MEMBER holds a build script that breaks, even when another member
/// fills the findings file, through the real clippy pipeline.
///
/// This rule states `scope: workspace`, and this repository holds more than 20
/// members and eight build scripts. Measured with clippy 0.1.97 over this
/// shape: the earlier gate wrote `good/src/lib.rs:1` alone and exited 0, and
/// the long function of `bad/src/lib.rs` was read as clean.
///
/// Both members hold the same long function, so the member that compiles is
/// the one that fills the findings file.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_member_build_script_that_breaks() {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let source = long_rust_function("", "fold_grid");
    let staged = [
        (RUST_PROBE_MANIFEST_PATH, RUST_TWO_MEMBER_ROOT_MANIFEST),
        (RUST_GOOD_MEMBER_MANIFEST_PATH, RUST_GOOD_MEMBER_MANIFEST),
        (RUST_GOOD_MEMBER_LIB_PATH, source.as_str()),
        (
            RUST_BAD_MEMBER_MANIFEST_PATH,
            RUST_BAD_MEMBER_BUILD_MANIFEST,
        ),
        (RUST_BAD_MEMBER_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (RUST_BAD_MEMBER_LIB_PATH, source.as_str()),
    ];
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let failure = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect_err("a member build script that breaks must break the run");

    let detail = failure.to_string();
    assert!(
        detail.contains(RUST_BUILD_SCRIPT_BROKEN_LINE),
        "the run must break with '{RUST_BUILD_SCRIPT_BROKEN_LINE}'; got '{detail}'"
    );
}

/// The root manifest of a probe workspace that holds two members and one
/// clippy lint at deny level for both of them.
const RUST_TWO_MEMBER_DENY_ROOT_MANIFEST: &str = concat!(
    "[workspace]\nmembers = [\"good\", \"bad\"]\nresolver = \"2\"\n",
    "\n[workspace.lints.clippy]\nunwrap_used = \"deny\"\n",
);

/// The manifest of the member that compiles, which takes the deny-level lint
/// of the workspace.
const RUST_GOOD_MEMBER_DENY_MANIFEST: &str = concat!(
    "[package]\nname = \"good\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[lints]\nworkspace = true\n",
);

/// Acceptance: the shipped Rust complexity tool rule BREAKS on a workspace
/// whose member holds a build script that breaks, BESIDE a lint at deny level
/// in another member, through the real clippy pipeline.
///
/// This shape carries both causes at one time, and it is what a repository
/// under `RUSTFLAGS="-D warnings"` gives for any warning at all. Measured with
/// clippy 0.1.97: cargo writes `success: false`, the report holds an
/// error-level `compiler-message` whose code is the LINT name
/// `clippy::unwrap_used`, and the build script of `bad` never ran. So a gate
/// that asks for NO error-level message reads this run as measured, and the
/// long function of `bad/src/lib.rs` stays hidden.
///
/// The build script entries answer the shape on their own: cargo compiled the
/// build script of `bad` and never ran it, whatever the other member reported.
#[test]
fn the_shipped_rust_complexity_tool_rule_breaks_on_a_broken_build_script_beside_a_denied_lint() {
    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let good = long_rust_function(DENY_LEVEL_UNWRAP_LINE, "fold_grid");
    let bad = long_rust_function("", "fold_grid");
    let staged = [
        (RUST_PROBE_MANIFEST_PATH, RUST_TWO_MEMBER_DENY_ROOT_MANIFEST),
        (
            RUST_GOOD_MEMBER_MANIFEST_PATH,
            RUST_GOOD_MEMBER_DENY_MANIFEST,
        ),
        (RUST_GOOD_MEMBER_LIB_PATH, good.as_str()),
        (
            RUST_BAD_MEMBER_MANIFEST_PATH,
            RUST_BAD_MEMBER_BUILD_MANIFEST,
        ),
        (RUST_BAD_MEMBER_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (RUST_BAD_MEMBER_LIB_PATH, bad.as_str()),
    ];
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let failure = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect_err("a build script that breaks must break the run, whatever else reported");

    let detail = failure.to_string();
    assert!(
        detail.contains(RUST_BUILD_SCRIPT_BROKEN_LINE),
        "the run must break with '{RUST_BUILD_SCRIPT_BROKEN_LINE}'; got '{detail}'"
    );
}

/// A cargo manifest that holds one clippy lint at deny level.
///
/// `[lints.clippy]` is one of the three shapes that raise a lint to deny; the
/// other two are a crate-level `#![deny(...)]` and `RUSTFLAGS="-D warnings"`.
/// Each makes cargo exit nonzero for a workspace clippy DID lint.
const DENY_LEVEL_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"deny-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[lints.clippy]\nunwrap_used = \"deny\"\n",
    "\n[workspace]\n",
);

/// The line the deny-level probe writes above its long function: one `unwrap`
/// the manifest of that probe denies.
const DENY_LEVEL_UNWRAP_LINE: &str = "pub fn first() -> i32 { Some(1).unwrap() }\n";

/// How many lines [`DENY_LEVEL_UNWRAP_LINE`] runs above the `pub fn` line
/// under it.
const DENY_LEVEL_UNWRAP_LINES: usize = 1;

/// The row the one finding of the deny-level probe stands on. The denied line
/// stands above the long function.
const RUST_DENY_LEVEL_ROW: usize = DENY_LEVEL_UNWRAP_LINES + 1;

/// Acceptance: the shipped Rust complexity tool rule MEASURES a workspace that
/// stands a clippy lint at deny level, through the real clippy pipeline.
///
/// `cargo clippy` exits nonzero for two different reasons. The tool could not
/// lint the workspace, and the tool linted the workspace correctly while a lint
/// stands at deny level. A gate that reads the status alone cannot tell the two
/// apart, and it throws away every finding of the second one.
///
/// Measured with clippy 0.1.97 over this probe: cargo exits 101, writes
/// `error: could not compile` to stderr, and writes
/// `clippy::too_many_lines src/lib.rs:2 this function has too many lines
/// (300/250)` into the report. So the script must test the REPORT beside the
/// status, which is what `builtin/validators/README.md` states for a status
/// two shapes share.
#[test]
fn the_shipped_rust_complexity_tool_rule_measures_a_workspace_beside_a_deny_level_lint() {
    let source = long_rust_function(DENY_LEVEL_UNWRAP_LINE, "long_defaults");

    let reported =
        rust_complexity_findings_under(DENY_LEVEL_PACKAGE_MANIFEST, &[(COMPLEX_LIB_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[probe_row(COMPLEX_LIB_PATH, RUST_DENY_LEVEL_ROW)]),
        "a lint at deny level makes cargo exit nonzero for a workspace it DID lint, so the \
         run must keep the finding the report holds"
    );
}

/// Acceptance: the shipped Rust complexity tool rule MEASURES a workspace that
/// stands a clippy lint at deny level and holds NO finding of the four gates,
/// through the real clippy pipeline.
///
/// This is the clean half of the deny-level shape, and a gate that reads the
/// findings file answers it wrong. cargo exits 101 for the denied lint, the
/// findings file is empty because the workspace is clean at the four gates, and
/// a gate that reads that file calls the run broken. Measured over this probe:
/// the earlier gate wrote `complexity-rust: cargo clippy could not lint the
/// workspace` and exited 1, for a workspace clippy linted from end to end.
///
/// The correct answer is no finding at exit 0, and this test holds the script
/// to it.
#[test]
fn the_shipped_rust_complexity_tool_rule_measures_a_clean_workspace_beside_a_deny_level_lint() {
    let reported = rust_complexity_findings_under(
        DENY_LEVEL_PACKAGE_MANIFEST,
        &[(COMPLEX_LIB_PATH, DENY_LEVEL_UNWRAP_LINE)],
    );

    assert!(
        reported.is_empty(),
        "a workspace clippy linted from end to end is clean at the four gates, whatever \
         status a denied lint gives it; got {reported:?}"
    );
}

/// The variable a project raises every warning to an error with.
const RUST_FLAGS_ENV: &str = "RUSTFLAGS";

/// The flag that raises every rustc warning to an error.
const DENY_EVERY_WARNING_FLAG: &str = "-D warnings";

/// The line the deny-flags probe writes above its long function: one variable
/// nothing reads, which `unused_variables` reports.
const UNUSED_VARIABLE_LINE: &str = "pub fn first() -> i32 { let unused = 1; 2 }\n";

/// How many lines [`UNUSED_VARIABLE_LINE`] runs above the `pub fn` line under
/// it.
const UNUSED_VARIABLE_LINES: usize = 1;

/// The row the one finding of the deny-flags probe stands on. The line that
/// holds the unused variable stands above the long function.
const RUST_DENY_FLAGS_ROW: usize = UNUSED_VARIABLE_LINES + 1;

/// Acceptance: the shipped Rust complexity tool rule MEASURES a workspace that
/// raises every warning to an error through `RUSTFLAGS`, through the real
/// clippy pipeline.
///
/// `RUSTFLAGS="-D warnings"` is the third shape that makes cargo exit nonzero
/// for a workspace clippy DID lint, beside a crate-level `#![deny(...)]` and a
/// `[lints.clippy]` table. Measured with clippy 0.1.97 over this probe: cargo
/// exits 101, the raw report holds the error code `unused_variables`, and the
/// four gates arrive at level `error` rather than `warning`.
///
/// The filter selects on the lint CODE, so the finding stands at either level,
/// and the run must keep it.
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_complexity_tool_rule_measures_a_workspace_beside_deny_level_flags() {
    use swissarmyhammer_common::test_utils::EnvVarGuard;

    let source = format!(
        "{UNUSED_VARIABLE_LINE}{}",
        long_rust_function("", "long_defaults")
    );
    let _flags = EnvVarGuard::set(RUST_FLAGS_ENV, DENY_EVERY_WARNING_FLAG);

    let reported = rust_complexity_findings(&[(COMPLEX_LIB_PATH, &source)]);

    assert_eq!(
        reported,
        sorted_names(&[probe_row(COMPLEX_LIB_PATH, RUST_DENY_FLAGS_ROW)]),
        "`RUSTFLAGS=\"-D warnings\"` raises the four gates to level `error` and makes cargo \
         exit nonzero, and the run must keep the finding the report holds"
    );
}

/// The status a shell answers for a command it could not run.
const COMMAND_NOT_FOUND_STATUS: i32 = 127;

/// The mode that makes a file executable for its owner and readable for every
/// other user.
#[cfg(unix)]
const EXECUTABLE_MODE: u32 = 0o755;

/// The name the script calls the report filter by.
const FILTER_BINARY_NAME: &str = "jq";

/// The line the script writes when the filter could not read the report.
const FILTER_BROKEN_LINE: &str = "complexity-rust: jq could not read the clippy report";

/// Acceptance: the shipped Rust complexity tool rule BREAKS when the filter
/// cannot read the clippy report, through the real clippy pipeline.
///
/// The filter step once ended in a pipe to `sort -u`, and the script writes
/// `set -e` with no `pipefail`. A shell pipeline takes the status of its last
/// command, so that shape answered exit 0 for every failure of the filter.
/// Measured over this probe package, which gives one finding: with the filter
/// replaced by a command that exits [`COMMAND_NOT_FOUND_STATUS`], the pipe
/// shape wrote 0 findings and exited 0, and the engine read a dirty tree as
/// clean.
///
/// The probe leads `PATH` with a directory holding such a command, so the
/// SHIPPED script runs its own filter step and finds it broken.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_complexity_tool_rule_breaks_when_the_filter_cannot_read_the_report() {
    use std::os::unix::fs::PermissionsExt;
    use swissarmyhammer_common::test_utils::PathGuard;

    let loader = builtin_loader();
    require_tool_installed(&loader, RUST_PROJECT_TYPES, RUST_COMPLEXITY_RULE);
    let stubs = tempfile::tempdir().unwrap();
    let stub = stubs.path().join(FILTER_BINARY_NAME);
    std::fs::write(
        &stub,
        format!("#!/bin/sh\nexit {COMMAND_NOT_FOUND_STATUS}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(EXECUTABLE_MODE)).unwrap();
    let staged = [
        (RUST_PROBE_MANIFEST_PATH, COMPLEX_PACKAGE_MANIFEST),
        (COMPLEX_LIB_PATH, COMPLEX_LIB_RS),
    ];
    let paths: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();
    let _path = PathGuard::prepend(stubs.path());

    let failure = shipped_script_findings(&loader, RUST_COMPLEXITY_RULE, &staged, &paths)
        .expect_err("a filter that cannot read the report must break the run");

    let detail = failure.to_string();
    assert!(
        detail.contains(FILTER_BROKEN_LINE),
        "the run must break with '{FILTER_BROKEN_LINE}'; got '{detail}'"
    );
}
