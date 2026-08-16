//! Acceptance tests for the shipped unused-dependency tool rules.
//!
//! One test holds the roster to its fixture pair. The tests under it drive
//! Rust through the real tool: one holds the finding it reports, one holds it
//! to STATING the manifest machete could not read while the findings of the
//! manifests it did read survive, and one holds it to breaking rather than
//! answering zero when machete cannot run at all.

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

/// The name the script calls machete by, which cargo resolves on `PATH`.
const MACHETE_BINARY_NAME: &str = "cargo-machete";

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
/// when it found unused dependencies, 0 when it found none, and 2 when it
/// could not walk the path it was given. The script accepts the first two and
/// breaks on every other status, so a machete a machine cannot run reaches the
/// review as an error rather than as a clean tree. Measured over this probe
/// package, which gives one finding, with machete replaced by a command that
/// exits 127: the pipe shape wrote 0 findings and exited 0; the shipped shape
/// writes no finding, that line, and exit 1.
///
/// This is the one test of the module that writes the process environment, so
/// it is the one that stands under `#[serial_test::serial(env)]`. The two tests
/// above it read the tool a stubbed `PATH` hands them, because that stub breaks
/// only for a run whose working directory holds the marker file this probe
/// stages. The roster test at the head of the module writes the working
/// directory instead, so it stands under the `cwd` key.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run() {
    verify_shipped_tree_breaks_without(&MACHETE_BROKEN_PROBE, MACHETE_BINARY_NAME);
}
