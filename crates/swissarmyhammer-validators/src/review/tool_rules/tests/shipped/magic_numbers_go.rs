//! Acceptance tests for the shipped `magic-numbers-go` tool rule, over the run
//! shapes that carry no finding.
//!
//! The tests that hold what `mnd` REPORTS stand in `magic_numbers.rs`, beside
//! the other four languages. The tests here hold the other half: what the run
//! answers when golangci-lint judged no literal at all.
//!
//! Each shape below writes a report a pipe ending in `jq` reads as a clean
//! tree. The status, the report and the stderr channel are what separate them
//! from a module that really holds no unnamed literal, and the script reads
//! all three.

use super::*;

use super::go_probe::{
    GO_ANOTHER_LINTER_ERROR, GO_BROKEN_STATUS_ERROR, GO_MODULE_MANIFEST, GO_MODULE_MANIFEST_PATH,
    GO_TOOL_BINARY_NAME, GO_UNPARSABLE_PATH, GO_UNPARSABLE_SOURCE, GO_UNREADABLE_PATH,
    GO_UNREADABLE_SOURCE,
};

/// Where the file holding the one unnamed literal stands inside the probe
/// repository.
const GO_MAGIC_NUMBER_PATH: &str = "shapes/shapes.go";

/// A Go file holding one unnamed literal.
///
/// `404` stands outside the `ignored-numbers` list the rule states, and it
/// stands in a `return`, which is one of the six positions `mnd` checks.
/// Measured with golangci-lint 2.12.2 over the shipped command line: one
/// `mnd` row reading `Magic number: 404, in <return> detected`, at exit 1 with
/// 0 bytes on stderr.
const GO_MAGIC_NUMBER_SOURCE: &str = concat!(
    "package probe\n\n",
    "func Check(status int) bool {\n\treturn status == 404\n}\n",
);

/// The text the one unnamed literal of the probe stands in.
///
/// `mnd` anchors each finding on the line the literal stands on, so this is
/// the text [`expected_row`] reads that line from.
const GO_MAGIC_NUMBER_HEAD: &str = "return status == 404";

/// The `path:line` row a run over the probe module must report.
fn go_magic_numbers_expected_row() -> String {
    expected_row(
        GO_MAGIC_NUMBER_PATH,
        GO_MAGIC_NUMBER_SOURCE,
        GO_MAGIC_NUMBER_HEAD,
    )
}

/// Acceptance: the shipped Go magic-numbers tool rule reports the one unnamed
/// literal of its probe module, through the real `mnd` pipeline.
///
/// This is the control half of the three tests under it. A gate that broke
/// every run it could not read at a glance would pass each of those and throw
/// away the findings of a run the tool DID make, so the same probe module has
/// to report on its own.
#[test]
fn the_shipped_go_magic_numbers_tool_rule_reports_its_probe_module() {
    verify_staged_rows_report(
        GO_PROJECT_TYPES,
        GO_MAGIC_NUMBERS_RULE,
        &[
            (GO_MODULE_MANIFEST_PATH, GO_MODULE_MANIFEST),
            (GO_MAGIC_NUMBER_PATH, GO_MAGIC_NUMBER_SOURCE),
        ],
        &[&go_magic_numbers_expected_row()],
        "the literal stands outside the ignored-numbers list, so the run reports the one \
         row and exits 0",
    );
}

/// What the run must say for a file golangci-lint could not parse: the row the
/// tool wrote, and the rule's own sentence.
const GO_MAGIC_NUMBERS_UNPARSABLE_ERRORS: &[&str] = &[GO_UNPARSABLE_PATH, GO_ANOTHER_LINTER_ERROR];

/// Why a file golangci-lint cannot parse breaks the run.
const GO_MAGIC_NUMBERS_UNPARSABLE_REASON: &str =
    "one Go file that does not parse drops every mnd row of the same run, so the run \
     measured no literal and must say so rather than read as a clean tree";

/// Acceptance: the shipped Go magic-numbers tool rule BREAKS on a Go file it
/// cannot parse, through the real `mnd` pipeline.
///
/// golangci-lint reports such a file as a `typecheck` row, and its
/// `invalid_issue` processor then answers with the typecheck rows ALONE —
/// `if len(tcIssues) > 0 { return tcIssues, nil }`. So the probe's one unnamed
/// literal gets no `mnd` row of its own, and the run measured no literal at
/// all.
///
/// Measured with golangci-lint 2.12.2 over the shipped command line, the
/// literal in one package and a file whose call never closes in another: the
/// report carried the `typecheck` row and no `mnd` row, at exit 1 with 0 bytes
/// on stderr. The same probe without the second package reported the `mnd`
/// row.
///
/// The earlier shape of this run dropped stderr and ended in `jq`, which reads
/// exactly like a clean tree: no finding, exit 0. `sah-diagnostic:` is the
/// answer for a declined ITEM of a SOUND run, and this run is not sound, so
/// the script names the row and exits 1 instead.
#[test]
fn the_shipped_go_magic_numbers_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_staged_tree_breaks(
        GO_PROJECT_TYPES,
        GO_MAGIC_NUMBERS_RULE,
        &[
            (GO_MODULE_MANIFEST_PATH, GO_MODULE_MANIFEST),
            (GO_MAGIC_NUMBER_PATH, GO_MAGIC_NUMBER_SOURCE),
            (GO_UNPARSABLE_PATH, GO_UNPARSABLE_SOURCE),
        ],
        GO_MAGIC_NUMBERS_UNPARSABLE_ERRORS,
        GO_MAGIC_NUMBERS_UNPARSABLE_REASON,
    );
}

/// What the run must say for a workspace holding no module: the status
/// golangci-lint answered with, and the rule's own words for it.
const GO_MAGIC_NUMBERS_NO_MODULE_ERRORS: &[&str] = &[GO_BROKEN_STATUS_ERROR, "measured no literal"];

/// Why a workspace holding no module breaks the run.
const GO_MAGIC_NUMBERS_NO_MODULE_REASON: &str =
    "golangci-lint loads a module, so a workspace holding none is a run that measured \
     nothing, and its report is an empty issue list a pipe reads as a clean tree";

/// Acceptance: the shipped Go magic-numbers tool rule BREAKS on a workspace
/// that holds no Go module, through the real `mnd` pipeline.
///
/// Measured with golangci-lint 2.12.2 over the shipped command line, a `.go`
/// file holding the literal with no `go.mod` beside it: `Issues: []` on
/// stdout, `typechecking error: pattern ./...: directory prefix . does not
/// contain main module or its selected dependencies` on stderr, and exit 7.
/// The report reads exactly like a clean tree, so the STATUS is the only thing
/// that separates the two, and the earlier pipe ending in `jq` threw it away.
#[test]
fn the_shipped_go_magic_numbers_tool_rule_breaks_on_a_workspace_holding_no_module() {
    verify_staged_tree_breaks(
        GO_PROJECT_TYPES,
        GO_MAGIC_NUMBERS_RULE,
        &[(GO_MAGIC_NUMBER_PATH, GO_MAGIC_NUMBER_SOURCE)],
        GO_MAGIC_NUMBERS_NO_MODULE_ERRORS,
        GO_MAGIC_NUMBERS_NO_MODULE_REASON,
    );
}

/// What the run must say for a workspace whose one Go package golangci-lint
/// could not read: the path the tool named, and the rule's own words for the
/// status.
const GO_MAGIC_NUMBERS_UNREADABLE_ERRORS: &[&str] = &[
    GO_UNREADABLE_PATH,
    GO_BROKEN_STATUS_ERROR,
    "measured no literal",
];

/// Why a workspace whose one package golangci-lint cannot read breaks the run.
const GO_MAGIC_NUMBERS_UNREADABLE_REASON: &str =
    "the one package of the workspace is the package golangci-lint cannot load, so the \
     run measured no literal at all and must say so";

/// Acceptance: the shipped Go magic-numbers tool rule BREAKS on a Go file it
/// may not read, through the real `mnd` pipeline.
///
/// golangci-lint refuses such a file with
/// `level=error msg="[linters_context] typechecking error: open <path>:
/// permission denied"` on stderr, and it writes no row for that package. The
/// probe workspace holds that package and NO OTHER, so the run measured no
/// literal at all. Measured with golangci-lint 2.12.2 over this workspace with
/// an empty cache directory: `Issues: []` on the report, at exit 7.
///
/// The workspace holds no second package on purpose. What a run reports for
/// the OTHER packages of such a workspace is a race inside golangci-lint
/// rather than a fact about the tool, which the `function-length-go` rule body
/// records over the same command line.
///
/// The earlier shape of this run dropped stderr and ended in `jq`, so that run
/// read as a clean tree. The probe takes every permission off the file, which
/// is a mode, so it runs on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_go_magic_numbers_tool_rule_breaks_on_a_file_it_may_not_read() {
    let staged = [(GO_MODULE_MANIFEST_PATH, GO_MODULE_MANIFEST)];
    let named: Vec<&str> = staged
        .iter()
        .map(|(path, _)| *path)
        .chain(std::iter::once(GO_UNREADABLE_PATH))
        .collect();
    let unreadable = ShippedUnreadableFile::Forbidden(GO_UNREADABLE_SOURCE);
    let prepare = |repo: &Path| stage_probe_unreadable(repo, GO_UNREADABLE_PATH, &unreadable);
    let staging = ShippedStaging {
        prepare: &prepare,
        ..ShippedStaging::of(&staged)
    };

    verify_staging_breaks(
        GO_PROJECT_TYPES,
        GO_MAGIC_NUMBERS_RULE,
        &staging,
        &named,
        GO_MAGIC_NUMBERS_UNREADABLE_ERRORS,
        GO_MAGIC_NUMBERS_UNREADABLE_REASON,
    );
}

/// The line a stubbed golangci-lint answers a run whose cache already holds
/// the finding, and which met one file it could not read.
///
/// Every byte of it is the answer the real golangci-lint 2.12.2 gave, measured
/// over one workspace run three times against the shipped command line: the
/// file unreadable on a cold cache, the file readable, and the file unreadable
/// again. The third run reported the `mnd` row out of the cache the second run
/// filled, wrote the `[linters_context]` line to stderr, and exited 1.
///
/// The stub is what makes that answer DETERMINISTIC. A warm cache gives it
/// every time. A workspace whose cache is cold gives it only when the sound
/// package wins a race inside golangci-lint, which the `function-length-go`
/// rule body measures over the same command line. The stub therefore stands in
/// for the cache rather than for a shape no probe could stage.
#[cfg(unix)]
const GO_MAGIC_NUMBERS_DECLINED_ANSWER: &str = concat!(
    "  printf '{\"Issues\":[{\"FromLinter\":\"mnd\",\"Text\":\"Magic number: 404, in ",
    "<return> detected\",\"Pos\":{\"Filename\":\"%s/shapes/shapes.go\",\"Line\":4}}]}\\n' ",
    "\"$PWD\"\n",
    "  printf 'level=error msg=\"[linters_context] typechecking error: open ",
    "%s/noread/unreadable.go: permission denied\"\\n' \"$PWD\" >&2\n",
    "  exit 1"
);

/// How many items the declining run could not judge: the one package nobody
/// may read.
#[cfg(unix)]
const GO_MAGIC_NUMBERS_DECLINED_ITEMS: usize = 1;

/// Acceptance: the shipped Go magic-numbers tool rule DECLINES a Go file it
/// may not read, over a run that still reported its findings.
///
/// A run whose cache already holds the answer for every package it CAN load
/// reports those findings beside the `[linters_context]` line for the package
/// it cannot. That run judged the code and could not judge ONE item, which is
/// the shape `builtin/validators/README.md` states a `sah-diagnostic:` line at
/// exit 0 for: an `exit 1` there would throw away every finding the run did
/// make.
///
/// The stub answers with the bytes the real golangci-lint 2.12.2 wrote for
/// that run, because a probe that staged the workspace would read that answer
/// only when a race inside golangci-lint fell its way. The probe leads `PATH`
/// with the stub, which is process state, so it stands under
/// `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_go_magic_numbers_tool_rule_declines_a_file_it_may_not_read() -> ProbeResult<()> {
    let run = drive_shipped_script_with_stub(
        GO_PROJECT_TYPES,
        GO_MAGIC_NUMBERS_RULE,
        &[
            (GO_MODULE_MANIFEST_PATH, GO_MODULE_MANIFEST),
            (GO_MAGIC_NUMBER_PATH, GO_MAGIC_NUMBER_SOURCE),
        ],
        NO_SCRIPT_FILES,
        GO_TOOL_BINARY_NAME,
        GO_MAGIC_NUMBERS_DECLINED_ANSWER,
    );
    // A script handed an item it cannot judge must judge the rest and exit 0.
    // A failure here is therefore the answer of the run, and the test reads
    // the script's own stderr rather than a sentence of its own.
    let outcome = run.outcome?;

    assert_eq!(
        sorted_names(&finding_rows(&outcome, &run.repo_root)),
        vec![go_magic_numbers_expected_row()],
        "the run must report every finding it made, or one file it could not read throws \
         away the work the run did do"
    );
    let stated = script_diagnostics(&outcome, &run.repo_root);
    assert_eq!(
        stated.len(),
        GO_MAGIC_NUMBERS_DECLINED_ITEMS,
        "the run must state the one item it declined; it stated {stated:?}"
    );
    assert!(
        stated[0].contains(GO_UNREADABLE_PATH),
        "the diagnostic must name the file it declined; it said '{}'",
        stated[0]
    );
    Ok(())
}
