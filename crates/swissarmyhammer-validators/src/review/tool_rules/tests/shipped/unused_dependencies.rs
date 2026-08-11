//! Acceptance tests for the shipped unused-dependency tool rules.
//!
//! One test holds the roster to its fixture pair. The test under it drives
//! Rust through the real tool.

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
