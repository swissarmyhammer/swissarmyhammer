//! Acceptance tests for the shipped unused-dependency tool rules.
//!
//! One test holds the roster to its fixture pair. The tests under it drive
//! Rust through the real tool: one holds the finding it reports, and two hold
//! it to STATING a manifest machete could not judge — one it could not read,
//! one it could not walk — while the findings of the manifests it did read
//! survive. Two more hold it to breaking rather than answering zero: one when
//! machete cannot run at all, and one when machete answers its own failure
//! status over a shape that is not a walk failure.

use super::*;

/// Acceptance: every shipped unused-dependency tool rule passes its fixture
/// pair in doctor, and supersedes nothing.
///
/// The pass fixture is the load-bearing half. It declares the same unused
/// dependency its fail fixture declares, and the only difference between
/// the two is `[package.metadata.cargo-machete] ignored`, so a machete
/// release that stopped reading that key — or stopped reading it through
/// the trailing comment the entry carries — makes the pair fail and takes
/// the rule out of the review.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_unused_dependency_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_UNUSED_DEPENDENCY_RULES,
        UNUSED_DEPENDENCIES_RULE_KIND,
    );
}

/// A cargo package that uses one dependency and declares a second one no
/// source names. `[workspace]` keeps cargo inside the temporary directory.
const UNUSED_DEPENDENCY_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"unused-dependency-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies]\nlibc = \"0.2\"\nserde = \"1\"\n",
    "\n[workspace]\n",
);

/// The library of [`UNUSED_DEPENDENCY_PACKAGE_MANIFEST`]. It names `libc`
/// and never `serde`, so `serde` is the one finding the rule must report.
const UNUSED_DEPENDENCY_LIB_RS: &str = concat!(
    "//! A probe crate for the shipped Rust unused-dependency tool rule.\n\n",
    "/// The system page size, read through the one dependency this file names.\n",
    "pub fn page_size() -> i64 {\n",
    "    unsafe { libc::sysconf(libc::_SC_PAGESIZE) }\n",
    "}\n",
);

/// The manifest path inside the probe repository, as the work-list holds
/// it. This is the file the finding must land on — not the source file that
/// fails to name the dependency.
const UNUSED_DEPENDENCY_MANIFEST_PATH: &str = "Cargo.toml";

/// The library path inside the probe package.
const UNUSED_DEPENDENCY_LIB_PATH: &str = "src/lib.rs";

/// A one-validator work-list over `path` for the builtin `manifests` set,
/// naming its one tool rule.
fn manifests_work(path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a declared dependency no source names",
        vec![ValidatorWork::new(
            MANIFESTS_SET,
            RuleNames::new([RUST_UNUSED_DEPENDENCIES_RULE.to_string()]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Rust unused-dependency tool rule reports a
/// declared dependency no source names, on a real cargo package, through
/// the real `cargo machete` pipeline.
///
/// This is the production path the fixture pair cannot reach. A fixture is
/// a manifest under a fixture name, which machete refuses to read, so the
/// script normalizes it into a temporary package; a real manifest is named
/// `Cargo.toml` and is scanned where it lies. Only a run over a real
/// package exercises that half, and only it proves the finding lands on the
/// manifest rather than on the source file.
///
/// The test states the tool as a precondition — [`require_tool_installed`]
/// names the missing tool and the command that installs it — and then
/// REQUIRES the run. A rule that planned no run fails the test and names
/// the plan's fallbacks, which carry the doctor's reason. Returning early
/// instead would leave the test asserting nothing, and a test that cannot
/// fail is not a gate.
#[test]
fn the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join(UNUSED_DEPENDENCY_MANIFEST_PATH),
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join(UNUSED_DEPENDENCY_LIB_PATH),
        UNUSED_DEPENDENCY_LIB_RS,
    )
    .unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_UNUSED_DEPENDENCIES_RULE);
    let work = manifests_work(
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
    );

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_UNUSED_DEPENDENCIES_RULE);
    assert_eq!(
        run.files(),
        [UNUSED_DEPENDENCY_MANIFEST_PATH.to_string()],
        "the run must carry the changed manifest, so the engine keeps the finding"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        MANIFESTS_SET,
        RUST_UNUSED_DEPENDENCIES_RULE,
        "unused dependency `serde`",
    );
}

/// A manifest that declares a package and does not parse as TOML: the
/// `[dependencies` table header ends with no closing bracket.
///
/// The script finds this file the way it finds every manifest, by the
/// `[package]` table on its first line, and machete then cannot read it.
const UNPARSABLE_MANIFEST: &str = concat!(
    "[package]\nname = \"unparsable-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies\nserde = \"1\"\n",
);

/// Where the manifest machete cannot read stands inside the probe repository.
///
/// The script walks the manifests it found in sorted order, and this name
/// sorts AFTER `Cargo.toml` under a byte order and under a case-folding order
/// alike. So the run reads the manifest it can measure first, writes that
/// finding to stdout, and only then meets the manifest that refuses — which is
/// the order that costs a finding when the refusal ends the whole run.
const UNPARSABLE_MANIFEST_PATH: &str = "unparsable/Cargo.toml";

/// The dependency entry [`UNUSED_DEPENDENCY_PACKAGE_MANIFEST`] declares and no
/// source names, which is the line the one finding must land on.
const UNUSED_DEPENDENCY_ENTRY: &str = "serde = \"1\"";

/// Acceptance: the shipped Rust unused-dependency tool rule DECLINES a
/// manifest machete cannot read, through the real `cargo machete` pipeline.
///
/// Machete answers this shape at exit 0 and writes `didn't find any unused
/// dependencies` to stdout, the same sentence a clean package gets, with
/// `error when handling <path>` on stderr. So the script cannot read the
/// status alone, and it tests stderr beside it.
///
/// What it does with that answer is this test. Measured with machete 0.9.2
/// over this probe: the package the run CAN read reports `serde` at exit 1,
/// and the manifest that refuses is a separate machete process that judged
/// nothing. One manifest of a run that measured the rest is ONE declined item,
/// so the script states it under the `sah-diagnostic:` marker and exits 0.
///
/// Both halves are the test. A run that keeps the finding and says nothing
/// about the refusing manifest reads a package no tool measured as a clean
/// package. A run that states the refusal and loses the finding threw away the
/// work it did do — measured with the earlier shape of this script, which
/// exited 1 there: the `serde` finding stood on stdout and the engine read
/// none of it.
#[test]
fn the_shipped_rust_unused_dependency_tool_rule_declines_a_manifest_it_cannot_read() {
    let expected = expected_row(
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
        UNUSED_DEPENDENCY_ENTRY,
    );

    verify_unjudged_file_is_declined(
        RUST_PROJECT_TYPES,
        RUST_UNUSED_DEPENDENCIES_RULE,
        &[
            (
                UNUSED_DEPENDENCY_MANIFEST_PATH,
                UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
            ),
            (UNUSED_DEPENDENCY_LIB_PATH, UNUSED_DEPENDENCY_LIB_RS),
            (UNPARSABLE_MANIFEST_PATH, UNPARSABLE_MANIFEST),
        ],
        UNPARSABLE_MANIFEST_PATH,
        &[&expected],
    );
}

/// Where a manifest machete cannot read stands when its own path carries the
/// `: ` that separates machete's path from machete's reason.
///
/// Measured with machete 0.9.2 over a package staged here: machete writes
/// `error when handling a: b/Cargo.toml: TOML parse error at line 6, column
/// 14`, so a script that strips to the FIRST `: ` takes `a: ` off the front and
/// leaves `b/Cargo.toml: ` standing inside the reason.
const COLON_MANIFEST_PATH: &str = "a: b/Cargo.toml";

/// The one diagnostic a run over [`COLON_MANIFEST_PATH`] must state, with the
/// `sah-diagnostic:` marker taken off as `marked_diagnostics` hands it on.
const COLON_MANIFEST_DIAGNOSTIC: &str =
    "cargo machete could not read a: b/Cargo.toml: TOML parse error at line 6, column 14";

/// Acceptance: the shipped Rust unused-dependency tool rule states machete's
/// reason with the path taken off, over a manifest whose path carries `: `.
///
/// The reason is machete's `error when handling ` line with the prefix and the
/// path taken off, and the path is the value the script HANDED machete. A strip
/// to the first `: ` reads the path off a separator instead, so it cuts a path
/// carrying `: ` in the wrong place and states a reason that repeats the tail
/// of the path.
///
/// Every path this workspace holds is free of `: `, so no run over a real tree
/// tells the two readings apart. This probe stages the one that does.
#[test]
fn the_shipped_rust_unused_dependency_tool_rule_states_the_reason_with_the_path_taken_off() {
    verify_declined_item_reads(
        RUST_PROJECT_TYPES,
        RUST_UNUSED_DEPENDENCIES_RULE,
        &[(COLON_MANIFEST_PATH, UNPARSABLE_MANIFEST)],
        COLON_MANIFEST_DIAGNOSTIC,
    );
}

/// Where the manifest machete could not WALK stands inside the probe
/// repository.
///
/// It sorts after `Cargo.toml` under a byte order and under a case-folding
/// order alike, so the run measures the manifest it CAN read first and writes
/// that finding to stdout before it meets this one — the order that costs a
/// finding when a walk failure ends the whole run.
const UNWALKABLE_MANIFEST_PATH: &str = "zunwalkable/Cargo.toml";

/// A manifest that declares a package and parses as TOML.
///
/// The script finds this file the way it finds every manifest, by the
/// `[package]` table on its first line, and machete then fails to WALK the path
/// it was handed rather than failing to read the bytes.
const UNWALKABLE_MANIFEST: &str = concat!(
    "[package]\nname = \"unwalkable-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies]\nserde = \"1\"\n",
);

/// The name the script calls machete by, which cargo resolves on `PATH`.
const MACHETE_BINARY_NAME: &str = "cargo-machete";

/// The whole of what machete 0.9.2 answers for a path it could not walk, as
/// the stub of one run replays it.
///
/// Measured against the real binary over three constructions — a path that
/// holds no file, a manifest whose parent directory carries mode 000, and a
/// `Cargo.toml` that is a broken symbolic link — and all three answer these
/// four stderr lines and this status. `run_machete` collects a walk failure for
/// each path it was given and bails after the loop, so the failure is PER
/// INVOKED PATH, and a script that runs one machete process for each manifest
/// reads it for one manifest alone.
///
/// The path is read off `$1` rather than written in, because the script decides
/// what path it hands machete and the answer must name that same path.
const MACHETE_WALK_FAILURE_ANSWER: &str = concat!(
    "  echo \"Analyzing dependencies of crates in $1...\" >&2\n",
    "  echo \"Done!\" >&2\n",
    "  echo \"Error: Errors when walking over directories:\" >&2\n",
    "  echo \"$1: IO error for operation on $1: Permission denied (os error 13)\" >&2\n",
    "  exit 2",
);

/// Acceptance: the shipped Rust unused-dependency tool rule DECLINES a manifest
/// machete could not WALK, and keeps the findings of the manifests it did read.
///
/// Machete answers this shape at its own failure status, 2, with nothing on
/// stdout, so the script cannot read the `error when handling ` line the
/// unparsable shape writes. It reads the status beside machete's own
/// `Errors when walking over directories` sentence instead.
///
/// What it does with that answer is this test. The walk failure belongs to the
/// ONE path machete was handed, and the next manifest gets a machete process of
/// its own that measures normally. One manifest of a run that measured the rest
/// is ONE declined item, so the script states it under the `sah-diagnostic:`
/// marker and exits 0.
///
/// Both halves are the test. Measured with the earlier shape of this script,
/// which broke the whole run on any status outside machete's own two: the
/// `serde` finding stood on stdout, the run exited 1, and
/// [`read_script_output`] answers `Err` before it reads stdout, so the finding
/// reached no reader.
///
/// The walk failure is staged as a stub because the script's own `[package]`
/// guard reads the manifest with `grep` before machete walks it, and every
/// construction that makes the real binary fail its walk also makes that `grep`
/// fail. The stub replays what the real binary answered, and
/// [`the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_fails_over_no_walk`]
/// is the control that keeps the reading narrow.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_unused_dependency_tool_rule_declines_a_manifest_it_cannot_walk() {
    let expected = expected_row(
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
        UNUSED_DEPENDENCY_ENTRY,
    );
    let staged = [
        (
            UNUSED_DEPENDENCY_MANIFEST_PATH,
            UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
        ),
        (UNUSED_DEPENDENCY_LIB_PATH, UNUSED_DEPENDENCY_LIB_RS),
        (UNWALKABLE_MANIFEST_PATH, UNWALKABLE_MANIFEST),
        (STUBBED_RUN_MARKER, ""),
    ];
    let named = [
        UNUSED_DEPENDENCY_MANIFEST_PATH,
        UNUSED_DEPENDENCY_LIB_PATH,
        UNWALKABLE_MANIFEST_PATH,
    ];
    let narrowed = format!(" && [ \"$1\" = \"{UNWALKABLE_MANIFEST_PATH}\" ]");
    let _path = lead_path_with_stub(
        MACHETE_BINARY_NAME,
        &stubbed_run_condition(&narrowed),
        MACHETE_WALK_FAILURE_ANSWER,
    );

    verify_declined_item_is_stated(
        RUST_PROJECT_TYPES,
        RUST_UNUSED_DEPENDENCIES_RULE,
        &ShippedStaging::of(&staged),
        &named,
        UNWALKABLE_MANIFEST_PATH,
        &[&expected],
    );
}

/// The line the script writes when machete answered its own failure status
/// over a shape that is not a walk failure.
const MACHETE_FAILURE_LINE: &str = "unused-dependencies-rust: cargo machete exited 2";

/// What the one error of a machete that failed over no walk must name.
const MACHETE_FAILURE_ERROR: &[&str] = &[MACHETE_FAILURE_LINE];

/// The healthy probe package, read with a machete that fails over no walk.
const MACHETE_FAILING_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_UNUSED_DEPENDENCIES_RULE,
        expected: MACHETE_FAILURE_ERROR,
    },
    staged: &[
        (
            UNUSED_DEPENDENCY_MANIFEST_PATH,
            UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
        ),
        (UNUSED_DEPENDENCY_LIB_PATH, UNUSED_DEPENDENCY_LIB_RS),
    ],
    reason: "a machete that failed over no walk must break the run",
};

/// What a machete that failed for a reason of its own answers, as the stub of
/// one run replays it.
///
/// `main` maps every `Err` of `run_machete` to one `Error: ` line and status 2,
/// and the walk failure is one such `Err` among others. So the status alone
/// cannot say a manifest was declined, and this is the shape that proves the
/// script reads machete's own sentence beside the status.
const MACHETE_OTHER_FAILURE_ANSWER: &str = concat!(
    "  echo \"Analyzing dependencies of crates in $1...\" >&2\n",
    "  echo \"Error: serde not found in tables:\" >&2\n",
    "  exit 2",
);

/// Acceptance: the shipped Rust unused-dependency tool rule BREAKS when machete
/// answers its own failure status over a shape that is NOT a walk failure.
///
/// This is the control of
/// [`the_shipped_rust_unused_dependency_tool_rule_declines_a_manifest_it_cannot_walk`].
/// A fix that read status 2 alone would answer every failure of machete with a
/// marked line and exit 0, and a run that judged nothing while reporting
/// nothing reads exactly like a clean tree. The script therefore declines only
/// the status machete pairs with its own `Errors when walking over directories`
/// sentence, and breaks on every other failure it can answer.
///
/// This test writes the process environment, so it stands under
/// `#[serial_test::serial(env)]` beside the two other stubbed probes.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_fails_over_no_walk() {
    verify_shipped_tree_breaks_with_stub(
        &MACHETE_FAILING_PROBE,
        MACHETE_BINARY_NAME,
        "",
        MACHETE_OTHER_FAILURE_ANSWER,
    );
}

/// The line the script writes when machete answered a status neither a clean
/// run nor a run with findings answers with.
const MACHETE_STATUS_LINE: &str = "unused-dependencies-rust: cargo machete exited 127";

/// What the one error of a machete that could not run must name.
const MACHETE_STATUS_ERROR: &[&str] = &[MACHETE_STATUS_LINE];

/// The healthy probe package, read with a machete that cannot run.
const MACHETE_BROKEN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_UNUSED_DEPENDENCIES_RULE,
        expected: MACHETE_STATUS_ERROR,
    },
    staged: &[
        (
            UNUSED_DEPENDENCY_MANIFEST_PATH,
            UNUSED_DEPENDENCY_PACKAGE_MANIFEST,
        ),
        (UNUSED_DEPENDENCY_LIB_PATH, UNUSED_DEPENDENCY_LIB_RS),
    ],
    reason: "a machete that could not run must break the run",
};

/// Acceptance: the shipped Rust unused-dependency tool rule BREAKS when
/// machete cannot run at all.
///
/// Machete keeps one status for findings and another for a failure: it exits 1
/// when it found unused dependencies, 0 when it found none, and 2 for every
/// error it answers. A status outside those three is a machete that never ran,
/// so the script breaks on it and reaches the review as an error rather than as
/// a clean tree. Measured over this probe package, which gives one finding,
/// with machete replaced by a command that exits 127: the pipe shape wrote 0
/// findings and exited 0; the shipped shape writes no finding, that line, and
/// exit 1.
///
/// This test writes the process environment, so it stands under
/// `#[serial_test::serial(env)]` beside the two other stubbed probes. The tests
/// that drive the real tool read the tool a stubbed `PATH` hands them, because
/// each stub answers only for a run whose working directory holds the marker
/// file its own probe stages. The roster test at the head of the module writes
/// the working directory instead, so it stands under the `cwd` key.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run() {
    verify_shipped_tree_breaks_without(&MACHETE_BROKEN_PROBE, MACHETE_BINARY_NAME);
}
