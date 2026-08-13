//! Acceptance tests for the shipped `missing-docs-rust` tool rule.
//!
//! Each test drives the SHIPPED script over a probe cargo package and reads
//! what the real `cargo clippy` reported.
//!
//! The module stands beside `missing_docs`, which holds the whole family to
//! its fixture pair, because `cargo clippy` gives one exit status to a run it
//! could not make and to a run it made from end to end. The shapes that tell
//! those two apart are cargo's own, so they are measured for Rust alone.

use super::*;

/// Acceptance: the shipped Rust tool rule reports an undocumented public
/// item on a real cargo workspace, through the real clippy pipeline.
///
/// No LLM reads the pair: the rule plans healthy, so the `missing-docs`
/// prompt rule is suppressed for the file, and the finding comes from the
/// script's stdout — [`execute_tool_runs`] never reaches an agent.
#[test]
fn the_shipped_rust_tool_rule_reports_an_undocumented_public_item() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("Cargo.toml"),
        UNDOCUMENTED_PACKAGE_MANIFEST,
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join(UNDOCUMENTED_LIB_PATH), UNDOCUMENTED_LIB_RS).unwrap();
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_MISSING_DOCS_RULE);
    let work = code_hygiene_work(&[UNDOCUMENTED_LIB_PATH]);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, RUST_MISSING_DOCS_RULE);
    assert_eq!(run.files(), [UNDOCUMENTED_LIB_PATH.to_string()]);
    assert!(
        plan.suppression()
            .suppressed_rules(CODE_HYGIENE_SET, UNDOCUMENTED_LIB_PATH)
            .contains(MISSING_DOCS_PROMPT_RULE),
        "a healthy tool rule must suppress the prompt rule, so no LLM reads the pair"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNDOCUMENTED_LIB_PATH,
        CODE_HYGIENE_SET,
        RUST_MISSING_DOCS_RULE,
        "missing documentation",
    );
}

/// The manifest of the root package of the Rust workspace probe.
///
/// It names `shared` as a dependency AND as a build-dependency, so cargo
/// compiles `shared` two times and clippy writes its finding two times.
/// `lonely` is a member no package depends on, so cargo builds it only when
/// the command selects the whole workspace.
const RUST_WORKSPACE_ROOT_MANIFEST: &str = concat!(
    "[package]\nname = \"workspace-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[dependencies]\nshared = { path = \"shared\" }\n",
    "\n[build-dependencies]\nshared = { path = \"shared\" }\n",
    "\n[workspace]\nmembers = [\"shared\", \"lonely\"]\n",
);

/// The build script of the Rust workspace probe. It carries a crate comment,
/// because `missing_docs` asks each compiled target for one.
const RUST_WORKSPACE_BUILD_RS: &str =
    "//! The build script of the workspace probe.\n\nfn main() {}\n";

/// The manifest of the `shared` member of the Rust workspace probe.
const RUST_WORKSPACE_SHARED_MANIFEST: &str =
    "[package]\nname = \"shared\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// The manifest of the `lonely` member of the Rust workspace probe.
const RUST_WORKSPACE_LONELY_MANIFEST: &str =
    "[package]\nname = \"lonely\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";

/// Every file of the Rust workspace probe the work-list does not name.
const RUST_WORKSPACE_SUPPORT_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", RUST_WORKSPACE_ROOT_MANIFEST),
    ("build.rs", RUST_WORKSPACE_BUILD_RS),
    ("shared/Cargo.toml", RUST_WORKSPACE_SHARED_MANIFEST),
    ("lonely/Cargo.toml", RUST_WORKSPACE_LONELY_MANIFEST),
];

/// The library of the root package of the Rust workspace probe.
const RUST_WORKSPACE_ROOT_LIB_PATH: &str = "src/lib.rs";

/// The library of the `shared` member, the one cargo compiles two times.
const RUST_WORKSPACE_SHARED_LIB_PATH: &str = "shared/src/lib.rs";

/// The library of the `lonely` member, the one no package depends on.
const RUST_WORKSPACE_LONELY_LIB_PATH: &str = "lonely/src/lib.rs";

/// The one undocumented declaration every library of the Rust workspace probe
/// holds.
const RUST_WORKSPACE_DECLARATIONS: &str = "pub struct Undocumented;\n";

/// The three libraries of the Rust workspace probe. Each carries a crate
/// comment of its own, so the only undocumented item is the shared
/// declaration.
const RUST_WORKSPACE_STAGED_FILES: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: RUST_WORKSPACE_ROOT_LIB_PATH,
        head: &["//! The root package of the workspace probe.\n\n"],
    },
    ShippedStagedFile {
        path: RUST_WORKSPACE_SHARED_LIB_PATH,
        head: &["//! The shared member of the workspace probe.\n\n"],
    },
    ShippedStagedFile {
        path: RUST_WORKSPACE_LONELY_LIB_PATH,
        head: &["//! The lonely member of the workspace probe.\n\n"],
    },
];

/// Each library of the Rust workspace probe, one time, in the order `sort -u`
/// leaves them.
///
/// `lonely/src/lib.rs` is what `--workspace` buys: cargo builds no member
/// nothing depends on, so a command without the flag never reads it.
/// `shared/src/lib.rs` standing one time is what `sort -u` buys: cargo
/// compiles that member two times, and clippy writes its finding two times.
const RUST_WORKSPACE_REPORTS: &[&str] = &[
    RUST_WORKSPACE_LONELY_LIB_PATH,
    RUST_WORKSPACE_SHARED_LIB_PATH,
    RUST_WORKSPACE_ROOT_LIB_PATH,
];

/// The three libraries of the Rust workspace probe, and what the real clippy
/// pipeline must report over them.
const RUST_MISSING_DOCS_WORKSPACE_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: &["rust"],
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_WORKSPACE_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public struct in each package of a workspace",
    declarations: RUST_WORKSPACE_DECLARATIONS,
    staged: RUST_WORKSPACE_STAGED_FILES,
    support: RUST_WORKSPACE_SUPPORT_FILES,
    reason: "the rule declares `scope: workspace`, so the run reads every member one time: the \
             member no package depends on reports, and the member cargo compiles two times \
             reports one finding and not two",
};

/// Acceptance: the shipped Rust missing-docs tool rule reports every member of
/// a workspace, one time each, through the real clippy pipeline.
///
/// Two parts of the command are load-bearing here, and the probe holds both.
/// `--workspace` selects every member; without it cargo builds the package the
/// working directory names and the packages that package depends on, so
/// `lonely/src/lib.rs` stays unread. `sort -u` collapses the repeat; without it
/// `shared/src/lib.rs` arrives two times, because the root package names that
/// member as a dependency and as a build-dependency and cargo therefore
/// compiles it two times.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_reports_every_workspace_member() {
    verify_shipped_staged_positions_report(&RUST_MISSING_DOCS_WORKSPACE_PROBE);
}

/// The manifest of the generated-code probe crate.
const RUST_GENERATED_MANIFEST: &str = concat!(
    "[package]\nname = \"generated-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// The build script of the generated-code probe crate. It writes one
/// undocumented public struct and one undocumented public function into
/// `OUT_DIR`, which is a directory under `target/` that no author edits.
const RUST_GENERATED_BUILD_RS: &str = r#"//! The build script of the generated-code probe.

use std::io::Write;

fn main() {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let generated = std::path::Path::new(&out).join("generated.rs");
    let mut file = std::fs::File::create(generated).expect("create the generated file");
    writeln!(file, "pub struct GeneratedUndocumented;").expect("write the generated struct");
    writeln!(file, "pub fn generated_undocumented() {{}}").expect("write the generated function");
}
"#;

/// The library of the generated-code probe crate. It reads the generated file
/// with an `include!`, which is how a crate takes code out of `OUT_DIR`, and it
/// holds one undocumented item of its own.
const RUST_GENERATED_LIB_RS: &str = concat!(
    "//! A probe crate for the generated-code step of the shipped Rust rule.\n",
    "\n",
    "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
    "\n",
    "pub struct HandWritten;\n",
);

/// Every file of the generated-code probe crate.
const RUST_GENERATED_PROBE_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", RUST_GENERATED_MANIFEST),
    ("build.rs", RUST_GENERATED_BUILD_RS),
    ("src/lib.rs", RUST_GENERATED_LIB_RS),
];

/// What the shipped script must name over the generated-code probe: the
/// hand-written item, and no generated one.
const RUST_GENERATED_REPORTS: &[&str] = &["src/lib.rs:5"];

/// Acceptance: the shipped Rust missing-docs tool rule names no generated file,
/// through the real clippy pipeline.
///
/// Cargo writes generated code under `OUT_DIR`, and clippy reports an item
/// there with the absolute path of a file the author cannot edit. The
/// `select(.file | startswith("/") | not)` step drops it. Measured over this
/// probe without the step: 3 findings, two of them at an absolute path under
/// `target/`.
///
/// The script's OWN findings are what this test reads, because the engine keeps
/// only the findings in the changed files and would drop a generated one on its
/// own. The step is what makes the script's answer equal the rule's answer.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_names_no_generated_file() {
    let loader = builtin_loader();
    let project_types = ["rust"];
    require_tool_installed(&loader, &project_types, RUST_MISSING_DOCS_RULE);

    let reported = shipped_script_findings(
        &loader,
        RUST_MISSING_DOCS_RULE,
        RUST_GENERATED_PROBE_FILES,
        &[],
    )
    .expect("the shipped script must judge the probe crate and exit 0");

    assert_eq!(
        reported,
        expected_script_findings(RUST_GENERATED_REPORTS),
        "the script must name the hand-written item and no file under `target/`"
    );
}

/// The manifest of the probe crate cargo cannot compile.
const RUST_UNCOMPILABLE_MANIFEST: &str = concat!(
    "[package]\nname = \"uncompilable-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// Every file of the uncompilable probe crate the work-list does not name.
const RUST_UNCOMPILABLE_SUPPORT_FILES: &[(&str, &str)] =
    &[("Cargo.toml", RUST_UNCOMPILABLE_MANIFEST)];

/// A Rust library that does not compile: the struct declaration ends with no
/// semicolon.
const RUST_UNCOMPILABLE_SOURCE: &str = concat!(
    "//! A probe crate the compiler cannot build.\n",
    "\n",
    "pub struct Undocumented\n",
);

/// Where the library that does not compile stands inside the probe repository.
const RUST_UNCOMPILABLE_PATH: &str = "src/lib.rs";

/// What cargo puts at the front of the failure it writes for a crate it cannot
/// compile. The run's error detail must carry it, so the agent reading the
/// error learns what broke.
const RUST_CANNOT_COMPILE_MESSAGE: &str = "could not compile";

/// The line the script writes when cargo read the workspace and clippy never
/// linted part of it.
const RUST_UNLINTABLE_LINE: &str = "missing-docs-rust: cargo clippy could not lint the workspace";

/// What the one error of a crate cargo cannot compile must name: the script's
/// own line, and cargo's own words beside it.
///
/// The two probes that expect it name a different package, so the fragments
/// name neither.
const RUST_UNCOMPILABLE_ERROR: &[&str] = &[RUST_UNLINTABLE_LINE, RUST_CANNOT_COMPILE_MESSAGE];

/// The `missing-docs-rust` probe over a crate cargo cannot compile.
const RUST_UNCOMPILABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: &["rust"],
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_UNCOMPILABLE_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Rust crate the compiler cannot build",
    path: RUST_UNCOMPILABLE_PATH,
    source: Some(RUST_UNCOMPILABLE_SOURCE.as_bytes()),
    support: RUST_UNCOMPILABLE_SUPPORT_FILES,
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a crate cargo
/// cannot compile, through the real clippy pipeline.
///
/// `cargo clippy` exits 101 for such a crate and writes no `missing_docs`
/// diagnostic for it. A shell pipeline takes the exit status of its LAST
/// command, so the earliest pipe — which ended in `jq` — exited 0 and reported
/// nothing, a run answering zero for a reason other than a clean crate.
///
/// Measured with clippy 0.1.97 over this probe: cargo exits 101, writes
/// `{"reason":"build-finished","success":false}`, and writes one error-level
/// `compiler-message` with NO code at all, because the crate does not parse.
/// The script reads that message out of the raw report and breaks the run with
/// a line of its own.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile() {
    verify_shipped_run_breaks(&RUST_UNCOMPILABLE_PROBE);
}

/// Where the manifest of a one-package Rust missing-docs probe stands, as the
/// work-list holds it.
const RUST_MISSING_DOCS_MANIFEST_PATH: &str = "Cargo.toml";

/// Where the library of a one-package Rust missing-docs probe stands, as the
/// work-list holds it.
const RUST_MISSING_DOCS_LIB_PATH: &str = "src/lib.rs";

/// A cargo package that holds one undocumented public item and nothing else
/// the lint reports. `[workspace]` keeps cargo inside the temporary directory.
const RUST_MISSING_DOCS_MANIFEST: &str = concat!(
    "[package]\nname = \"missing-docs-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// A Rust library holding one undocumented `pub struct`, which `missing_docs`
/// reports at line 3 — the crate documentation line and the blank line under
/// it stand above it.
const RUST_MISSING_DOCS_LIB: &str = concat!(
    "//! A probe crate for the shipped Rust missing-docs tool rule.\n",
    "\n",
    "pub struct Undocumented;\n",
);

/// The one finding the undocumented item of [`RUST_MISSING_DOCS_LIB`] must
/// report.
const RUST_MISSING_DOCS_REPORTS: &[&str] = &["src/lib.rs:3"];

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
/// holds an undocumented public item of its own.
const RUST_TWO_MEMBER_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_UNCOMPILABLE_ERROR,
    },
    staged: &[
        (
            RUST_MISSING_DOCS_MANIFEST_PATH,
            RUST_TWO_MEMBER_ROOT_MANIFEST,
        ),
        (RUST_GOOD_MEMBER_MANIFEST_PATH, RUST_GOOD_MEMBER_MANIFEST),
        (RUST_GOOD_MEMBER_LIB_PATH, RUST_MISSING_DOCS_LIB),
        (RUST_BAD_MEMBER_MANIFEST_PATH, RUST_BAD_MEMBER_MANIFEST),
        (RUST_BAD_MEMBER_LIB_PATH, RUST_UNCOMPILABLE_SOURCE),
    ],
    reason: "a workspace member the compiler refuses must break the run, whatever the \
             member beside it reported",
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a workspace
/// whose MEMBER cannot compile, even when another member fills the findings
/// file, through the real clippy pipeline.
///
/// This rule states `scope: workspace`, and a real repository holds many
/// members. A gate that reads the filtered findings file cannot see the member
/// that failed: the member that compiles writes its undocumented item into
/// that file, the file is not empty, and the status test never runs. Measured
/// with clippy 0.1.97 over this shape: cargo exits 101 and the report holds
/// `good/src/lib.rs` beside an error-level message with no code at all for the
/// member that did not parse.
///
/// The RAW report holds what the filtered file drops, so the script tests that
/// report and the member that compiles cannot hide the member that does not.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_workspace_member_it_cannot_compile() {
    verify_shipped_tree_breaks(&RUST_TWO_MEMBER_PROBE);
}

/// The line the script writes when cargo could not make a run at all.
const RUST_NO_RUN_LINE: &str = "missing-docs-rust: cargo could not run clippy over the workspace";

/// What the one error of a tree cargo cannot read must name: the script's own
/// line, and cargo's own words beside it.
const RUST_NO_RUN_ERROR: &[&str] = &[RUST_NO_RUN_LINE, "could not find `Cargo.toml`"];

/// A probe tree that stages a Rust library and no manifest at all, so cargo
/// makes no run.
const RUST_NO_MANIFEST_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_NO_RUN_ERROR,
    },
    staged: &[(RUST_MISSING_DOCS_LIB_PATH, RUST_MISSING_DOCS_LIB)],
    reason: "a tree cargo cannot read must break the run, because clippy linted nothing",
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a tree that
/// holds no manifest, through the real clippy pipeline.
///
/// cargo writes one `build-finished` entry for every run it made, and no entry
/// at all for a run it could not make. Measured with cargo 1.97.1 over this
/// probe: cargo writes `error: could not find `Cargo.toml`` to stderr, writes
/// 0 bytes to the report, and exits 101. So the PRESENCE of the entry states
/// that cargo ran, and its absence beside a nonzero status is the one shape
/// this gate answers.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_tree_that_holds_no_manifest() {
    verify_shipped_tree_breaks(&RUST_NO_MANIFEST_PROBE);
}

/// The manifest of a probe package cargo must run a build script for before
/// clippy can lint the library.
const RUST_BUILD_SCRIPT_MANIFEST: &str = concat!(
    "[package]\nname = \"build-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "build = \"build.rs\"\n",
    "\n[workspace]\n",
);

/// Where the build script of a probe package stands.
const RUST_BUILD_SCRIPT_PATH: &str = "build.rs";

/// A build script that breaks. It carries a crate comment, because
/// `missing_docs` asks each compiled target for one.
///
/// cargo writes its own words to stderr for this failure and writes NO
/// `compiler-message` into the report, so a gate that reads the compiler
/// messages alone cannot see it.
const RUST_BROKEN_BUILD_SCRIPT: &str = concat!(
    "//! The build script of the probe package.\n",
    "\n",
    "fn main() { panic!(\"the build script of this probe breaks on purpose\"); }\n",
);

/// A build script that runs and does nothing. It carries a crate comment for
/// the same reason [`RUST_BROKEN_BUILD_SCRIPT`] does.
const RUST_WORKING_BUILD_SCRIPT: &str = concat!(
    "//! The build script of the probe package.\n",
    "\n",
    "fn main() {}\n",
);

/// The line the script writes when cargo compiled a build script and never ran
/// it, so clippy never linted the crate that build script serves.
const RUST_BUILD_SCRIPT_BROKEN_LINE: &str =
    "missing-docs-rust: a build script did not run, so clippy did not lint every crate";

/// What the one error of a build script that breaks must name.
const RUST_BUILD_SCRIPT_BROKEN_ERROR: &[&str] = &[RUST_BUILD_SCRIPT_BROKEN_LINE];

/// A probe package whose build script breaks, holding one undocumented public
/// item clippy never reaches.
const RUST_BROKEN_BUILD_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_BUILD_SCRIPT_BROKEN_ERROR,
    },
    staged: &[
        (RUST_MISSING_DOCS_MANIFEST_PATH, RUST_BUILD_SCRIPT_MANIFEST),
        (RUST_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (RUST_MISSING_DOCS_LIB_PATH, RUST_MISSING_DOCS_LIB),
    ],
    reason: "a build script that breaks must break the run, because clippy never linted \
             the crate that build script serves",
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a package
/// whose BUILD SCRIPT breaks, through the real clippy pipeline.
///
/// cargo runs a build script before it compiles the crate that script serves,
/// so a build script that breaks leaves that crate unlinted. Measured with
/// clippy 0.1.97 over this probe: cargo writes
/// `{"reason":"build-finished","success":false}`, one `compiler-artifact` for
/// the build script it compiled, and NO `compiler-message` at all, in a report
/// of 1124 bytes. So the report holds no error code, and a gate that reads the
/// compiler messages alone answers 0 findings at exit 0 — while the control
/// below, the same package under a build script that runs, reports its
/// undocumented item.
///
/// cargo writes one `build-script-executed` entry for every build script it
/// RAN, and that entry is what a broken build script leaves out.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_build_script_that_breaks() {
    verify_shipped_tree_breaks(&RUST_BROKEN_BUILD_SCRIPT_PROBE);
}

/// The control of [`RUST_BROKEN_BUILD_SCRIPT_PROBE`]: the same package under a
/// build script that runs.
const RUST_WORKING_BUILD_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_MISSING_DOCS_REPORTS,
    },
    staged: &[
        (RUST_MISSING_DOCS_MANIFEST_PATH, RUST_BUILD_SCRIPT_MANIFEST),
        (RUST_BUILD_SCRIPT_PATH, RUST_WORKING_BUILD_SCRIPT),
        (RUST_MISSING_DOCS_LIB_PATH, RUST_MISSING_DOCS_LIB),
    ],
    reason: "a build script that runs leaves the run measured, so the undocumented item \
             must stand",
};

/// Acceptance: the shipped Rust missing-docs tool rule MEASURES a package whose
/// build script RUNS, through the real clippy pipeline.
///
/// This is the control of the test above. The two packages hold the same
/// manifest and the same library, and the build script is the one difference,
/// so a gate that broke this run would break every package that carries a
/// build script. This repository carries fifteen of them.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_measures_a_package_beside_a_build_script_that_runs() {
    verify_shipped_tree_reports(&RUST_WORKING_BUILD_SCRIPT_PROBE);
}

/// The root manifest of a probe workspace that holds two members and one rustc
/// lint at deny level for both of them.
const RUST_TWO_MEMBER_DENY_ROOT_MANIFEST: &str = concat!(
    "[workspace]\nmembers = [\"good\", \"bad\"]\nresolver = \"2\"\n",
    "\n[workspace.lints.rust]\nunused_variables = \"deny\"\n",
);

/// The manifest of the member that compiles, which takes the deny-level lint
/// of the workspace.
const RUST_GOOD_MEMBER_DENY_MANIFEST: &str = concat!(
    "[package]\nname = \"good\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[lints]\nworkspace = true\n",
);

/// The manifest of the member whose build script cargo must run.
const RUST_BAD_MEMBER_BUILD_MANIFEST: &str = concat!(
    "[package]\nname = \"bad\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "build = \"build.rs\"\n",
);

/// Where the build script of that member stands.
const RUST_BAD_MEMBER_BUILD_SCRIPT_PATH: &str = "bad/build.rs";

/// A Rust library that holds one variable nothing reads above the undocumented
/// item, so the denied lint stands beside the finding of this rule. The
/// undocumented item then stands at line 5.
const RUST_DENY_LEVEL_LIB: &str = concat!(
    "//! A probe crate for the shipped Rust missing-docs tool rule.\n",
    "\n",
    "/// Reads one variable nothing reads.\n",
    "pub fn first() -> i32 { let unused = 1; 2 }\n",
    "pub struct Undocumented;\n",
);

/// A probe workspace whose member holds a build script that breaks, beside a
/// member that stands a rustc lint at deny level.
const RUST_DENY_LEVEL_BUILD_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_BUILD_SCRIPT_BROKEN_ERROR,
    },
    staged: &[
        (
            RUST_MISSING_DOCS_MANIFEST_PATH,
            RUST_TWO_MEMBER_DENY_ROOT_MANIFEST,
        ),
        (
            RUST_GOOD_MEMBER_MANIFEST_PATH,
            RUST_GOOD_MEMBER_DENY_MANIFEST,
        ),
        (RUST_GOOD_MEMBER_LIB_PATH, RUST_DENY_LEVEL_LIB),
        (
            RUST_BAD_MEMBER_MANIFEST_PATH,
            RUST_BAD_MEMBER_BUILD_MANIFEST,
        ),
        (RUST_BAD_MEMBER_BUILD_SCRIPT_PATH, RUST_BROKEN_BUILD_SCRIPT),
        (RUST_BAD_MEMBER_LIB_PATH, RUST_MISSING_DOCS_LIB),
    ],
    reason: "a build script that breaks must break the run, whatever else the report holds",
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS on a workspace
/// whose member holds a build script that breaks, BESIDE a lint at deny level
/// in another member, through the real clippy pipeline.
///
/// This shape carries two causes at one time, and it is what a repository under
/// `RUSTFLAGS="-D warnings"` gives for any warning at all. Measured with clippy
/// 0.1.97: cargo writes `success: false`, the report holds an error-level
/// `compiler-message` whose code is the LINT name `unused_variables`, and the
/// build script of `bad` never ran. So a gate that asks for NO error-level
/// message reads this run as measured, and the undocumented item of
/// `bad/src/lib.rs` stays hidden.
///
/// The build script entries answer the shape on their own: cargo compiled the
/// build script of `bad` and never ran it, whatever the other member reported.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_breaks_on_a_broken_build_script_beside_a_denied_lint() {
    verify_shipped_tree_breaks(&RUST_DENY_LEVEL_BUILD_SCRIPT_PROBE);
}

/// A cargo package that holds one rustc lint at deny level.
///
/// `[lints.rust]` is one of the three shapes that raise a lint to deny; the
/// other two are a crate-level `#![deny(...)]` and `RUSTFLAGS="-D warnings"`.
/// Each makes cargo exit nonzero for a workspace clippy linted from end to end.
const RUST_DENY_LEVEL_MANIFEST: &str = concat!(
    "[package]\nname = \"deny-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[lints.rust]\nunused_variables = \"deny\"\n",
    "\n[workspace]\n",
);

/// The one finding the undocumented item of [`RUST_DENY_LEVEL_LIB`] must
/// report.
const RUST_DENY_LEVEL_REPORTS: &[&str] = &["src/lib.rs:5"];

/// A probe package that stands a rustc lint at deny level beside one
/// undocumented public item.
const RUST_DENY_LEVEL_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_DENY_LEVEL_REPORTS,
    },
    staged: &[
        (RUST_MISSING_DOCS_MANIFEST_PATH, RUST_DENY_LEVEL_MANIFEST),
        (RUST_MISSING_DOCS_LIB_PATH, RUST_DENY_LEVEL_LIB),
    ],
    reason: "a lint at deny level makes cargo exit nonzero for a workspace clippy DID lint, \
             so the run must keep the finding the report holds",
};

/// Acceptance: the shipped Rust missing-docs tool rule MEASURES a workspace
/// that stands a rustc lint at deny level, through the real clippy pipeline.
///
/// `cargo clippy` exits nonzero for two different reasons. clippy could not
/// lint the workspace, and clippy linted the workspace from end to end while a
/// lint stands at deny level. A gate that reads the status alone cannot tell
/// the two apart, and it throws away every finding of the second one.
///
/// Measured with clippy 0.1.97 over this probe: cargo exits 101, writes
/// `error: could not compile `deny-probe`` to stderr, and writes the
/// `missing_docs` diagnostic for `Undocumented` into the report at level
/// `warning`. The earlier script wrote `set -e` and took cargo's status, so it
/// reported 0 findings and exited 101 for that run. The script tests the REPORT
/// beside the status, which is what `builtin/validators/README.md` states for a
/// status two shapes share.
#[test]
fn the_shipped_rust_missing_docs_tool_rule_measures_a_workspace_beside_a_deny_level_lint() {
    verify_shipped_tree_reports(&RUST_DENY_LEVEL_PROBE);
}

/// The line the script writes when the filter could not read the report.
const RUST_FILTER_BROKEN_LINE: &str = "missing-docs-rust: jq could not read the clippy report";

/// What the one error of a filter that cannot read the report must name.
const RUST_FILTER_BROKEN_ERROR: &[&str] = &[RUST_FILTER_BROKEN_LINE];

/// The healthy probe package, read with a filter that cannot run.
const RUST_FILTER_BROKEN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: RUST_PROJECT_TYPES,
        rule: RUST_MISSING_DOCS_RULE,
        expected: RUST_FILTER_BROKEN_ERROR,
    },
    staged: &[
        (RUST_MISSING_DOCS_MANIFEST_PATH, RUST_MISSING_DOCS_MANIFEST),
        (RUST_MISSING_DOCS_LIB_PATH, RUST_MISSING_DOCS_LIB),
    ],
    reason: "a filter that cannot read the report must break the run",
};

/// Acceptance: the shipped Rust missing-docs tool rule BREAKS when the filter
/// cannot read the clippy report, through the real clippy pipeline.
///
/// The filter step once stood in a pipe that ended in `sort -u`, and the script
/// writes `set -e` with no `pipefail`. A shell pipeline takes the status of its
/// last command, so that shape answered exit 0 for every failure of the filter.
/// Measured over this probe package, which gives one finding, with the filter
/// replaced by a command that exits [`COMMAND_NOT_FOUND_STATUS`]: the pipe
/// shape wrote 0 findings and exited 0, and the engine read a dirty tree as
/// clean. The shipped shape writes no finding, that line, and exit 1.
///
/// The probe leads `PATH` with a directory holding such a command, and `PATH`
/// is process state, so the test stands under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_rust_missing_docs_tool_rule_breaks_when_the_filter_cannot_read_the_report() {
    verify_shipped_tree_breaks_without(&RUST_FILTER_BROKEN_PROBE, FILTER_BINARY_NAME);
}
