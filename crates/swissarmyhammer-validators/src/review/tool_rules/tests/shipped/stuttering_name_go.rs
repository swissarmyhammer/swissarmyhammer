//! Acceptance tests for the shipped `stuttering-name-go` tool rule.
//!
//! Each test drives the SHIPPED script over a probe Go file and reads what the
//! real revive reported.
//!
//! The rule runs the same revive `exported` rule `missing-docs-go` runs, and
//! the two split its output: `missing-docs-go` owns the `comments` category
//! and this rule owns the `naming` one. One test in this file drives both
//! shipped scripts over one file and holds that split, so a config change on
//! either side that made a finding fall between them breaks a test.

use super::*;

/// The materialized name of the `stuttering-name-go` fail fixture.
const GO_STUTTERING_NAME_FAIL_FIXTURE: &str = "stuttering-name-go.fail.go";

/// Where the `stuttering-name-go` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const GO_STUTTERING_NAME_FIXTURE_PATH: &str = "src/stuttering_name_go_fail.go";

/// The qualified name revive writes into the message of each finding the
/// `stuttering-name-go` fail fixture holds.
///
/// The qualified name is what a caller outside the package would write, and it
/// is the part of the message that does not move: the
/// `sayRepetitiveInsteadOfStutters` argument rewrites "and that stutters" to
/// "and that is repetitive" and leaves the name where it stands. A probe that
/// read the verb would break the day a config stated that argument.
///
/// The three entries are the three shapes the check reads: a repetitive TYPE,
/// a repetitive FUNCTION, and a name whose rune under the package name is an
/// underscore rather than an upper case letter.
const GO_STUTTERING_NAME_FAIL_NAMES: &[&str] = &[
    "fixtures.FixturesRecord",
    "fixtures.FixturesBuild",
    "fixtures.Fixtures_Thing",
];

/// The `stuttering-name-go` fail fixture, and every repetitive name the real
/// revive pipeline must report inside it.
const GO_STUTTERING_NAME_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_STUTTERING_NAME_RULE,
        expected: GO_STUTTERING_NAME_FAIL_NAMES,
    },
    fixture: GO_STUTTERING_NAME_FAIL_FIXTURE,
    path: GO_STUTTERING_NAME_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "repetitive exported name",
};

/// Acceptance: the shipped Go stuttering-name tool rule reports every
/// repetitive name its fail fixture holds, through the real revive pipeline.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// pass fixture holds a name equal to the package name, a name whose next rune
/// is lower case, a repetitive constant, a repetitive variable, a repetitive
/// method and a repetitive unexported function, so a run that reported one of
/// them would fail the pair; holding this run to exactly these three states
/// the same silence from the other side.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_reports_every_fail_fixture_name() {
    verify_shipped_fail_fixture_reports_each(
        &GO_STUTTERING_NAME_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an exported type and function that repeat their package name",
                CODE_HYGIENE_SET,
                [GO_STUTTERING_NAME_RULE.to_string()],
                [(GO_STUTTERING_NAME_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, name| reported.contains(name),
    );
}

/// Acceptance: every shipped stuttering-name tool rule passes its fixture pair
/// in doctor, and supersedes nothing.
///
/// The `supersedes` half is the load-bearing one. No shipped prompt rule reads
/// a Go NAME, so a rule that named one here would silence a rule that answers
/// another question, and a machine without revive would lose that answer too.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_stuttering_name_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_STUTTERING_NAME_RULES,
        STUTTERING_NAME_RULE_KIND,
    );
}

/// One staged position of the carve-out probe: `head` — the package clause,
/// and the generated header where the position carries one — then the two
/// declarations every position shares.
///
/// Two names, one for each package clause the probe writes. `StagedType`
/// repeats `package staged` and `MainType` repeats `package main`, so whatever
/// clause a position carries, the position holds a name that WOULD report. A
/// probe carrying one name would leave the command position silent for the
/// name rather than for the carve-out.
///
/// The shared declarations stand in a macro rather than in a constant because
/// `concat!` takes literals, and every position is a `&'static str` the probe
/// holds as a constant.
macro_rules! go_carve_out_source {
    ($head:literal) => {
        concat!(
            $head,
            "type StagedType struct{}\n",
            "\n",
            "type MainType struct{}\n"
        )
    };
}

/// The ordinary position: a library file with no generated header and a name
/// that is not a test name. Its `StagedType` must report.
const GO_ORDINARY_PATH: &str = "staged.go";

/// The test position. revive skips a file whose name ends in `_test.go`, and
/// it skips the whole file, so these same bytes must stay silent.
const GO_TEST_PATH: &str = "staged_test.go";

/// The generator position. The name carries the protobuf compiler's suffix and
/// the file carries the generated header.
const GO_GENERATED_PATH: &str = "staged.pb.go";

/// The command position. revive reads no file of `package main`, because a
/// command exports nothing to a caller outside itself.
const GO_MAIN_PATH: &str = "cmd/probe/main.go";

/// The control for the command position: the same declarations under a package
/// clause that is not `main`.
///
/// `Main` is a legal Go package name and it is one rune of CASE away from
/// `main`, so this position and the one above differ in that rune and in
/// nothing else. Its `MainType` must report, which is what makes the silence
/// of the command position a carve-out rather than a name that never matched.
const GO_NOT_MAIN_PATH: &str = "cmd/probe/notmain.go";

/// The ordinary library file: the package clause and nothing over it.
const GO_ORDINARY_SOURCE: &str = go_carve_out_source!("package staged\n\n");

/// The generated file: the `go generate` header line, then the same bytes the
/// ordinary position holds. That one line is the whole of what revive reads to
/// know a file is generated; the name of the file says nothing.
const GO_GENERATED_SOURCE: &str =
    go_carve_out_source!("// Code generated by the sah probe. DO NOT EDIT.\n\npackage staged\n\n");

/// The command file: the `main` package clause, and nothing else moved.
const GO_MAIN_SOURCE: &str = go_carve_out_source!("package main\n\n");

/// The control file: the `Main` package clause, one rune of case away from the
/// command file.
const GO_NOT_MAIN_SOURCE: &str = go_carve_out_source!("package Main\n\n");

/// Every position the carve-out probe stages, in the order the work-list holds
/// them.
const GO_CARVE_OUT_FILES: &[(&str, &str)] = &[
    (GO_ORDINARY_PATH, GO_ORDINARY_SOURCE),
    (GO_TEST_PATH, GO_ORDINARY_SOURCE),
    (GO_GENERATED_PATH, GO_GENERATED_SOURCE),
    (GO_MAIN_PATH, GO_MAIN_SOURCE),
    (GO_NOT_MAIN_PATH, GO_NOT_MAIN_SOURCE),
];

/// The `path:line` row of each finding the carve-out probe must report: the
/// `StagedType` of the ordinary file, and the `MainType` of the control file.
///
/// The two rows are compared as a SET. revive reads its files at the same
/// time, so the order it answers in is its own: measured over 20 runs of this
/// probe, 13 answered the ordinary file first and 7 the control file first.
const GO_CARVE_OUT_REPORTED: &[&str] = &["staged.go:3", "cmd/probe/notmain.go:5"];

/// The carve-out probe: the same two declarations at five positions, and the
/// two of them the real revive pipeline must report.
const GO_CARVE_OUT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_STUTTERING_NAME_RULE,
        expected: GO_CARVE_OUT_REPORTED,
    },
    staged: GO_CARVE_OUT_FILES,
    reason: "the ordinary library file reports its repetitive type, the control file under \
             `package Main` reports its own, and the test file, the generated file and the \
             command file report nothing",
};

/// Acceptance: the shipped Go stuttering-name tool rule reads neither a
/// generated file, a test file nor a command file, through the real revive
/// pipeline.
///
/// `Apply` returns before the walk for a file that is not importable — a
/// `_test.go` file or a `package main` file — and the linter skips a generated
/// file before that, so the same three carve-outs cover the repetitive-name
/// check and the documentation check alike.
///
/// Each position differs from the ordinary one in ONE thing. The test file
/// holds the same bytes, so its NAME is the whole difference. The generated
/// file adds the header LINE and nothing else. The command file changes the
/// package CLAUSE, and the control file beside it changes that clause by one
/// rune of case and reports.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_reads_neither_a_generated_a_test_nor_a_command_file() {
    verify_shipped_tree_reports(&GO_CARVE_OUT_PROBE);
}

/// A Go file that does not parse: the parameter list of `Broken` never closes.
const GO_UNPARSABLE_SOURCE: &str = concat!("package staged\n", "\n", "func Broken( {\n");

/// Where the unparsable file stands inside the probe repository.
const GO_UNPARSABLE_PATH: &str = "broken.go";

/// Where the Go file the run CAN judge stands, beside the file it cannot
/// judge.
const GO_JUDGED_PATH: &str = "judged.go";

/// A Go file the run judges: one exported type that opens with the name of its
/// own package, so this rule owns exactly one finding of it.
///
/// The type carries no doc comment as well, so the same run also makes a
/// `comments` finding that `missing-docs-go` owns. This rule must report the
/// `naming` one alone, which is what holds the expected row at a single entry.
const GO_JUDGED_SOURCE: &str = concat!("package staged\n", "\n", "type StagedType struct{}\n");

/// The declaration line the one finding of the judged Go file stands on.
const GO_JUDGED_DECLARATION: &str = "type StagedType struct{}";

/// The `path:line` row the run must report for the judged Go file.
fn go_stuttering_name_judged_row() -> String {
    expected_row(GO_JUDGED_PATH, GO_JUDGED_SOURCE, GO_JUDGED_DECLARATION)
}

/// Acceptance: the shipped Go stuttering-name tool rule DECLINES a Go file it
/// cannot parse, through the real revive pipeline.
///
/// revive exits 0 for such a file and writes the failure onto the SAME report
/// as a finding, with an EMPTY `RuleName` and the `validity` category rather
/// than the rule name `exported`. The category filter of this rule drops that
/// record twice over, so the file would read as clean.
///
/// An `exit 1` answers that no better. Measured with revive 1.15.0 under the
/// config this rule ships, over `func Broken( {` beside one exported type that
/// repeats its package name: three records on the report — the unnamed record
/// of the file it could not parse, the `naming` finding this rule owns and the
/// `comments` finding `missing-docs-go` owns, both of the file it read — at
/// exit 0, with 0 bytes on stderr. revive judged the other file, so its
/// finding is there to lose, and the shipped script before this fix threw it
/// away: nothing on stdout, one unmarked line on stderr, exit 1.
///
/// The unparsable file is therefore one declined item of a sound run, and the
/// script states it under the `sah-diagnostic:` marker at exit 0.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_declines_a_file_it_cannot_parse() {
    let expected = go_stuttering_name_judged_row();

    verify_unjudged_file_is_declined(
        GO_PROJECT_TYPES,
        GO_STUTTERING_NAME_RULE,
        &[
            (GO_JUDGED_PATH, GO_JUDGED_SOURCE),
            (GO_UNPARSABLE_PATH, GO_UNPARSABLE_SOURCE),
        ],
        GO_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// The `stuttering-name-go` probe over a path no reader may open, beside the
/// Go file the run judges.
///
/// The judged file is the one [`GO_JUDGED_SOURCE`] holds: one exported type
/// that repeats its package name, so the run has one row to lose. Losing it is
/// what a nonzero exit over a declined item costs, and staying silent about the
/// path is what reads that path as a clean file.
fn go_stuttering_name_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: GO_PROJECT_TYPES,
        rule: GO_STUTTERING_NAME_RULE,
        judged: vec![(GO_JUDGED_PATH, GO_JUDGED_SOURCE.to_string())],
        path: GO_FORBIDDEN_PATH,
        expected: vec![go_stuttering_name_judged_row()],
    }
}

/// Acceptance: the shipped Go stuttering-name tool rule DECLINES a Go file it
/// may not read, through the real revive pipeline.
///
/// revive STATS each path before it lints. A path it can stat and cannot open
/// is dropped in SILENCE: measured with revive 1.15.0 under the config this
/// rule ships, over a file at mode 000 beside one exported type that repeats
/// its package name, revive reported the finding of the file it read, wrote NO
/// record of the refusing path under any category, wrote 0 bytes on stderr and
/// exited 0. The same file alone answered `null` on stdout at exit 0, which is
/// the report and the status of a clean file.
///
/// So there is no line to forward and no record to select. The script tests
/// each path BEFORE revive starts, which is the shape
/// `builtin/validators/README.md` names for a tool that can exit 0 for a file
/// it could not open.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_go_stuttering_name_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &go_stuttering_name_decline_probe(),
        &ShippedUnreadableFile::Forbidden(GO_FORBIDDEN_SOURCE),
    );
}

/// An exported Go type that opens with the name of its package. revive reports
/// the declaration, so each file holds one finding.
///
/// This is the same source [`GO_JUDGED_SOURCE`] stages, under the name the
/// probes below read it by. One literal serves both, so a change to the shape
/// cannot leave one probe measuring another one.
const GO_UNREAD_SOURCE: &str = GO_JUDGED_SOURCE;

/// Every Go file staged in the probe repository the script is given none of.
///
/// revive reads the package standing in the working directory, so the file at
/// the root stands inside its default target and the nested file stands
/// outside it.
const GO_UNREAD_FILES: &[(&str, &str)] = &[
    ("staged.go", GO_UNREAD_SOURCE),
    ("deep/nested/other.go", GO_UNREAD_SOURCE),
];

/// Each finding the script reports over the two files it is given, as
/// `path:line`.
const GO_READ_FINDINGS: &[&str] = &["staged.go:3", "deep/nested/other.go:3"];

/// The `stuttering-name-go` probe over a run that is given no file.
const GO_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_STUTTERING_NAME_RULE,
        expected: NO_FINDINGS,
    },
    staged: GO_UNREAD_FILES,
    with_files: GO_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Go stuttering-name tool rule reads only the files it
/// is given, through the real revive pipeline.
///
/// revive with no path argument reads the package standing in the working
/// directory. Measured over this probe with no argument: without the guard the
/// script reported 1 finding, on `staged.go`, and exited 0; with the guard it
/// reports none and exits 0. The same script over the two staged files reports
/// 2, so the guard is the whole difference and the nested file is what the
/// default target leaves out.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&GO_EMPTY_RUN_PROBE);
}

/// Where the file the two revive rules split stands inside the probe
/// repository.
const GO_SPLIT_PATH: &str = "staged.go";

/// One Go file holding every shape revive's `exported` rule answers for.
///
/// Four names are repetitive — a documented type, an undocumented type, an
/// underscore name and a function — and ten declarations lack the doc comment
/// `exported` asks for. Neither rule may drop a finding of the other, and
/// neither may claim one.
const GO_SPLIT_SOURCE: &str = concat!(
    "package staged\n",
    "\n",
    "// StagedType is documented and still opens with the package name.\n",
    "type StagedType struct{}\n",
    "\n",
    "type PlainType struct{}\n",
    "\n",
    "type StagedRecord struct{}\n",
    "\n",
    "type Staged struct{}\n",
    "\n",
    "type Stagedly struct{}\n",
    "\n",
    "type Staged_Thing struct{}\n",
    "\n",
    "type stagedPrivate struct{}\n",
    "\n",
    "const StagedLimit = 1\n",
    "\n",
    "var StagedVar = 1\n",
    "\n",
    "func StagedBuild() {}\n",
    "\n",
    "func (s StagedType) StagedMethod() {}\n",
    "\n",
    "func PlainBuild() {}\n",
);

/// The qualified name of each finding `stuttering-name-go` must report over
/// [`GO_SPLIT_SOURCE`].
const GO_SPLIT_NAMING_CLAIMS: &[&str] = &[
    "staged.StagedType",
    "staged.StagedRecord",
    "staged.Staged_Thing",
    "staged.StagedBuild",
];

/// How many findings `missing-docs-go` must report over [`GO_SPLIT_SOURCE`].
///
/// Measured on revive 1.15.0: the plain undocumented `exported` run over this
/// file answers 14 findings, 10 of them `comments` and 4 of them `naming`. The
/// two rules must therefore answer 10 and 4, and the sum is the whole rule.
const GO_SPLIT_COMMENTS_FINDINGS: usize = 10;

/// The claims the shipped script of `rule` reported over `repo_root`, sorted.
///
/// The script is read where the set ships it, so the split this module
/// measures is the split the SHIPPED rules make.
fn claims_of_shipped_script(
    loader: &ValidatorLoader,
    rule: &str,
    repo_root: &Path,
    files: &[&str],
) -> Vec<String> {
    let shipped = required_shipped_tool_rule(loader, rule);
    let args = script_args(shipped.scope, files);
    let reported = run_script(&shipped.script, repo_root, &args)
        .expect("each shipped script must judge the probe file and exit 0");
    let mut claims: Vec<String> = reported
        .findings
        .into_iter()
        .map(|finding| finding.claim)
        .collect();
    claims.sort();
    claims
}

/// Acceptance: the two shipped Go rules that run revive's `exported` rule
/// split its findings between them, through both real revive pipelines.
///
/// `exported` answers two kinds of finding under one rule name, and revive
/// tells them apart by CATEGORY alone. `missing-docs-go` states
/// `disableStutteringCheck` and owns the `comments` half; this rule states no
/// argument and selects the `naming` half in its filter. Selection there is
/// attribution and not exemption, and this test is what states that: every
/// finding one rule drops, the other one reports.
///
/// A config change on either side that made a finding fall between the two
/// rules breaks this test, and nothing else in the suite would see it.
#[test]
fn the_shipped_go_rules_that_run_revives_exported_rule_split_its_findings() {
    let loader = builtin_loader();
    require_tool_installed(&loader, GO_PROJECT_TYPES, GO_STUTTERING_NAME_RULE);
    require_tool_installed(&loader, GO_PROJECT_TYPES, GO_MISSING_DOCS_RULE);
    let repo = tempfile::tempdir().expect("temp dir");
    stage_probe_files(repo.path(), [(GO_SPLIT_PATH, GO_SPLIT_SOURCE)]);
    let repo_root = probe_repository_root(repo.path());
    let files = [GO_SPLIT_PATH];

    let names = claims_of_shipped_script(&loader, GO_STUTTERING_NAME_RULE, &repo_root, &files);
    let comments = claims_of_shipped_script(&loader, GO_MISSING_DOCS_RULE, &repo_root, &files);

    for name in GO_SPLIT_NAMING_CLAIMS {
        assert!(
            names.iter().any(|claim| claim.contains(name)),
            "the naming rule must report `{name}`; it reported {names:?}"
        );
    }
    assert_eq!(
        names.len(),
        GO_SPLIT_NAMING_CLAIMS.len(),
        "the naming rule reports one finding for each repetitive name and no other; got {names:?}"
    );
    assert_eq!(
        comments.len(),
        GO_SPLIT_COMMENTS_FINDINGS,
        "the missing-docs rule reports every documentation finding of the same revive rule; \
         got {comments:?}"
    );
    assert!(
        names.iter().all(|claim| !comments.contains(claim)),
        "no finding may belong to both rules; the naming rule reported {names:?} and the \
         missing-docs rule reported {comments:?}"
    );
}

/// The one file the workspace probe stages: a library file holding one
/// repetitive exported type.
const GO_WORKSPACE_FILES: &[(&str, &str)] = &[(GO_ORDINARY_PATH, GO_UNREAD_SOURCE)];

/// The `path:line` row the workspace probe must report.
const GO_WORKSPACE_REPORTED: &[&str] = &["staged.go:3"];

/// A probe repository of one file, and the one row the shipped script must
/// report over it.
///
/// Two tests drive this shape. One drives it two times, which measures two
/// workspaces holding the same bytes. The other starts several runs of it
/// together in ONE workspace.
const GO_WORKSPACE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_STUTTERING_NAME_RULE,
        expected: GO_WORKSPACE_REPORTED,
    },
    staged: GO_WORKSPACE_FILES,
    reason: "each workspace reports its own file; a run answering from another workspace's \
             cache would report that workspace's paths",
};

/// How many workspaces the cache probe drives, one after the other, over the
/// same bytes.
///
/// Two is the whole shape: the first workspace is gone before the second one
/// runs, which is what two checkouts of one repository, and a review in a
/// worktree, do every day.
const CACHE_PROBE_WORKSPACES: usize = 2;

/// Acceptance: the shipped Go stuttering-name tool rule reads the workspace it
/// ran in, over bytes another workspace already read.
///
/// `magic-numbers-go` and `function-length-go` each name a
/// `GOLANGCI_LINT_CACHE` directory of their own, because golangci-lint answers
/// by package CONTENT and stores each finding with the ABSOLUTE path of the run
/// that first cached it. revive names no cache because it holds no answer
/// between runs, and this test is the assertion behind that sentence.
///
/// Measured over a module of 400 packages and a copy of it at a second path:
/// 0.12 s, 0.11 s, 0.12 s and 0.11 s over the first directory cold, the first
/// again, the second, and the second again, and each directory reported its
/// own 400 paths.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_reads_the_workspace_it_ran_in() {
    for _ in 0..CACHE_PROBE_WORKSPACES {
        verify_shipped_tree_reports(&GO_WORKSPACE_PROBE);
    }
}

/// How many runs of the shipped script this module starts together.
///
/// Eight is the count `function-length-go` measured golangci-lint's file lock
/// with: four runs never clashed there and eight clashed in every round. The
/// same count is used here so the two probes answer the same question.
const RUNS_STARTED_TOGETHER: usize = 8;

/// Acceptance: the shipped Go stuttering-name tool rule reports while other
/// runs of it stand in the same workspace, through the real revive pipeline.
///
/// golangci-lint takes one file lock for each run, so `function-length-go` asks
/// it to serialize: without `allow-serial-runners` a second instance stops with
/// `Error: parallel golangci-lint is running` on stderr, writes nothing to
/// stdout, and the run reads as a clean file. revive takes no lock — `revive
/// -h` states six flags and none of them is a lock or a cache — so this rule
/// needs no such key, and this test is the assertion behind that sentence.
#[test]
fn the_shipped_go_stuttering_name_tool_rule_reports_while_other_runs_stand_together() {
    let expected = sorted_names(&expected_script_findings(GO_WORKSPACE_PROBE.run.expected));
    let files: Vec<&str> = GO_WORKSPACE_PROBE
        .staged
        .iter()
        .map(|(path, _)| *path)
        .collect();

    let reported = rows_of_runs_started_together(
        &GO_WORKSPACE_PROBE.run,
        GO_WORKSPACE_PROBE.staged,
        &files,
        RUNS_STARTED_TOGETHER,
    );

    assert!(
        reported.iter().all(|run| *run == expected),
        "every run started together must report {expected:?}, because revive takes no lock \
         and holds no shared state; a run that stopped instead would read here as a clean \
         file: {reported:?}"
    );
}
