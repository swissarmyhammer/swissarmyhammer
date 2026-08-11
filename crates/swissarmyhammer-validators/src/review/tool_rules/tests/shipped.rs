//! What every shipped-rule acceptance test shares.
//!
//! A shipped-rule test drives the SHIPPED script over a probe repository and
//! reads what the real tool reports. This module carries the shapes those
//! tests are written in — a planned run, a fail fixture, a staged position
//! set, a run that must break, and a run that must read nothing — and the
//! helpers that drive each shape.
//!
//! The tests themselves stand one module per rule family, so each module
//! stays small enough for a reviewer, and for the review engine, to read
//! whole.

mod complexity;
mod dead_code;
mod magic_numbers;
mod missing_docs;
mod unused_dependencies;

use super::preconditions::require_tool_installed;
use super::*;

use std::path::PathBuf;

use swissarmyhammer_common::test_utils::CurrentDirGuard;

use crate::doctor::FIXTURE_TEMPLATE_SUFFIX;
use crate::review::scope::{FileWork, ProbeNames, RuleNames};
use crate::review::test_support::builtin_loader;
use crate::validators::types::FIXTURES_DIR_NAME;

/// The run `rule` planned, or a panic naming what the plan fell back to.
///
/// Every shipped-rule acceptance test REQUIRES its run. A rule that planned
/// none leaves a [`ToolFallback`] carrying the doctor's reason — the missing
/// tool, or the fixture pair that failed — so the panic names that reason
/// rather than reporting a bare absence. Returning early instead would leave
/// the test asserting nothing, and a test that cannot fail is not a gate.
fn required_run<'a>(plan: &'a ToolPlan, rule: &str) -> &'a ToolRun {
    plan.runs()
        .iter()
        .find(|run| run.rule() == rule)
        .unwrap_or_else(|| {
            panic!(
                "the shipped tool rule `{rule}` must plan a run; fallbacks: {:?}",
                plan.fallbacks()
            )
        })
}

/// A kind of file a builtin validator set ships beside its manifest.
///
/// Each kind names the one directory that holds the file and the one suffix
/// the set adds to the asked-for name, so one lookup serves every kind.
struct ShippedAssetKind {
    /// The directory under the set's base path that holds the file.
    dir: &'static str,
    /// What the set adds to the asked-for name on disk.
    suffix: &'static str,
    /// What to call the file in the failure message.
    label: &'static str,
}

/// The fixture template a set carries for a materialized file name.
///
/// A set stores `<name>.tmpl`, so a test that wants the shipped bytes asks
/// for `<name>` and gets the template beside it.
const FIXTURE_TEMPLATE_ASSET: ShippedAssetKind = ShippedAssetKind {
    dir: FIXTURES_DIR_NAME,
    suffix: FIXTURE_TEMPLATE_SUFFIX,
    label: "fixture template",
};

/// The rule source a set carries for a rule name, beside its `fixtures/`.
const RULE_SOURCE_ASSET: ShippedAssetKind = ShippedAssetKind {
    dir: "rules",
    suffix: ".md",
    label: "rule source",
};

/// The path of the `kind` file named `name`, inside whichever builtin
/// validator set carries it.
fn shipped_asset(loader: &ValidatorLoader, kind: &ShippedAssetKind, name: &str) -> PathBuf {
    loader
        .list_rulesets()
        .iter()
        .map(|ruleset| {
            ruleset
                .base_path
                .join(kind.dir)
                .join(format!("{name}{}", kind.suffix))
        })
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("a builtin validator set must ship a {name} {}", kind.label))
}

/// The run one probe asks the planner for, and exactly what that run must
/// report.
///
/// Every probe in this file states these same three things, whatever it
/// stages: which project types put the rule in the plan, which rule the run
/// belongs to, and one entry for each finding the run must report. Where the
/// probe repository takes its bytes, and how a finding becomes an entry, stay
/// with the probe that states them.
struct ShippedRun {
    /// The project types the rule is planned for.
    project_types: &'static [&'static str],

    /// The tool rule that must plan the run.
    rule: &'static str,

    /// One entry for each thing the run must report, and no other. The probe
    /// that holds the run states what one entry is.
    expected: &'static [&'static str],
}

/// A shipped fail fixture, and what the real pipeline of one tool rule must
/// report over it.
///
/// The doctor fixture contract asks a fail fixture for ONE finding, so a rule
/// that reported one position and stayed silent on every other would still
/// pass it. A probe names each position, so each one is load-bearing on its
/// own, and the count then states what the tool does NOT read as a measured
/// fact rather than leaving it to be discovered.
struct ShippedFailFixture {
    /// The run the fail fixture must produce.
    run: ShippedRun,

    /// The materialized name of the fail fixture the set ships.
    fixture: &'static str,

    /// Where the fixture stands inside the probe repository, as the work-list
    /// holds it.
    path: &'static str,

    /// Every other shipped fixture the probe repository needs beside the fail
    /// fixture, each with the name the set ships it under and the path it
    /// takes in the repository.
    ///
    /// A `files`-scope rule reads the files it is given and needs none. A
    /// `workspace`-scope rule loads a project rather than a file list, so the
    /// project manifest the set ships stands here.
    support: &'static [(&'static str, &'static str)],

    /// What one entry of the run's `expected` is, for the failure messages.
    noun: &'static str,
}

/// What a `files`-scope probe names for `support`: nothing. The tool reads the
/// files it is given, so the fail fixture alone is the whole repository.
const NO_SUPPORT_FIXTURES: &[(&str, &str)] = &[];

/// What a `files`-scope probe names for a support FILE list: nothing. The tool
/// reads the files it is given, so the staged files are the whole repository.
const NO_SUPPORT_FILES: &[(&str, &str)] = &[];

/// Writes each `(path, bytes)` pair of `files` into `repo`, making the
/// directory of each one first.
fn stage_probe_files<'a>(repo: &Path, files: impl IntoIterator<Item = (&'a str, &'a str)>) {
    for (path, content) in files {
        let file = repo.join(path);
        std::fs::create_dir_all(file.parent().expect("a staged path has a parent")).unwrap();
        std::fs::write(&file, content).unwrap();
    }
}

/// The RESOLVED root of the probe repository `repo`.
///
/// A tool prints the resolved path of each file it reads, and on macOS a
/// temporary directory stands behind a symbolic link. The engine strips the
/// repository root off each reported path, so the root it is given has to be
/// the resolved form or no path matches and every finding keeps an absolute
/// path.
fn probe_repository_root(repo: &tempfile::TempDir) -> PathBuf {
    repo.path()
        .canonicalize()
        .expect("resolve the probe repository path")
}

/// Copies the shipped fixture template named `fixture` into `repo` at `path`,
/// and answers the bytes it wrote.
///
/// The fixture is read where the set ships it, so a run over the copy measures
/// the SHIPPED bytes. A test that wrote its own copy would answer for the copy.
fn copy_shipped_fixture(
    loader: &ValidatorLoader,
    repo: &Path,
    fixture: &str,
    path: &str,
) -> String {
    let shipped = shipped_asset(loader, &FIXTURE_TEMPLATE_ASSET, fixture);
    let content = std::fs::read_to_string(&shipped).expect("read the shipped fixture template");
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().expect("the fixture path has a parent")).unwrap();
    std::fs::write(&file, &content).unwrap();
    content
}

/// Drives the shipped fail fixture of `probe` through the real tool pipeline,
/// and holds the run to exactly the entries the probe names.
///
/// The fixture, and every support fixture the probe names beside it, is copied
/// into a temporary repository by [`copy_shipped_fixture`].
///
/// Three callbacks carry what one language does not share with another.
/// `build_work` states the work-list, because the rule list a language names is
/// its own. `extract` reads the text one finding is held to, and takes the
/// fixture's source lines as well, because one language holds a finding to its
/// claim and another holds it to the source line it stands on. `matches` states
/// how an entry meets that text.
fn verify_shipped_fail_fixture_reports_each<W, E, M>(
    probe: &ShippedFailFixture,
    build_work: W,
    extract: E,
    matches: M,
) where
    W: FnOnce(&str) -> WorkList,
    E: Fn(&VerifiedFinding, &[&str]) -> String,
    M: Fn(&str, &str) -> bool,
{
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let repo = tempfile::tempdir().unwrap();
    let content = copy_shipped_fixture(&loader, repo.path(), probe.fixture, probe.path);
    for (fixture, path) in probe.support {
        copy_shipped_fixture(&loader, repo.path(), fixture, path);
    }
    let repo_root = probe_repository_root(&repo);
    let work = build_work(&content);

    let plan = plan_tool_rules(&work, &loader, probe.run.project_types, None);

    let run = required_run(&plan, probe.run.rule);
    let outcome = execute_tool_runs(std::slice::from_ref(run), &repo_root, None);
    assert!(
        outcome.errors().is_empty(),
        "the shipped pipeline must not break; errors: {:?}",
        outcome.errors()
    );

    let source: Vec<&str> = content.lines().collect();
    let reported: Vec<String> = outcome
        .findings()
        .iter()
        .filter(|verified| verified.finding.file == probe.path)
        .map(|verified| extract(verified, &source))
        .collect();
    for entry in probe.run.expected {
        assert!(
            reported.iter().any(|text| matches(text, entry)),
            "the fail fixture must report the {} `{entry}`; the run reported {reported:?}",
            probe.noun
        );
    }
    assert_eq!(
        reported.len(),
        probe.run.expected.len(),
        "the fail fixture holds one finding for each {} and no other; got {reported:?}",
        probe.noun
    );
}

/// The trimmed source line the finding stands on, as the `extract` callback
/// of [`verify_shipped_fail_fixture_reports_each`].
///
/// Three of the tools this file drives write one message for every finding and
/// never spell what they read, so the position is the only text that tells one
/// finding from another.
fn fail_fixture_source_line(verified: &VerifiedFinding, source: &[&str]) -> String {
    let line = verified.finding.line;
    source
        .get(line as usize - 1)
        .unwrap_or_else(|| panic!("line {line} stands past the end of the fixture"))
        .trim()
        .to_string()
}

/// The same declarations staged at several positions, and which of those
/// positions one missing-docs tool rule's real pipeline must report.
///
/// The doctor materializes one fixture as a loose file with no directory, so
/// no fixture can carry a position and no fixture pair can prove a carve-out
/// the tool decides by path or by header. A probe of several positions can:
/// every file holds the same declarations, so the position is the only thing
/// that tells one file of the run from another.
struct ShippedStagedPositions {
    /// The run the staged positions must produce. Its `expected` names the
    /// file of each finding, in the order the run reports them.
    run: ShippedRun,

    /// The prompt rule the work-list names beside the tool rule, which is the
    /// rule the tool rule supersedes.
    prompt_rule: &'static str,

    /// What the work-list states the change is for.
    change_purpose: &'static str,

    /// The declarations every staged file holds, each one undocumented.
    declarations: &'static str,

    /// Each position the declarations are staged at.
    staged: &'static [ShippedStagedFile],

    /// Each file staged beside the positions that the work-list does NOT name.
    ///
    /// A `files`-scope rule reads the files it is given and needs none. A
    /// `workspace`-scope rule loads a project rather than a file list, so the
    /// project manifests, and every other file the project needs to build,
    /// stand here.
    support: &'static [(&'static str, &'static str)],

    /// Why those files report and the others stay silent.
    reason: &'static str,
}

/// One staged position: where the file stands, and what stands above the
/// declarations every position shares.
///
/// A rule that decides on the PATH alone leaves the head empty at every
/// position. A rule that reads the head of the file states that head here, one
/// fragment for each thing the head carries, so two positions that share a
/// fragment share it by reference and cannot drift apart.
struct ShippedStagedFile {
    /// The path the file takes in the probe repository, as the work-list holds
    /// it.
    path: &'static str,

    /// The fragments above the shared declarations, in the order they are
    /// written.
    head: &'static [&'static str],
}

/// Drives every staged position of `probe` through the real tool pipeline, and
/// holds the run to reporting exactly the files the probe names.
fn verify_shipped_staged_positions_report(probe: &ShippedStagedPositions) {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let repo = tempfile::tempdir().unwrap();
    let staged: Vec<(&str, String)> = probe
        .staged
        .iter()
        .map(|file| {
            (
                file.path,
                format!("{}{}", file.head.concat(), probe.declarations),
            )
        })
        .collect();
    stage_probe_files(
        repo.path(),
        staged
            .iter()
            .map(|(path, content)| (*path, content.as_str())),
    );
    stage_probe_files(repo.path(), probe.support.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let work = tool_rule_work(
        probe.change_purpose,
        CODE_HYGIENE_SET,
        [probe.prompt_rule.to_string(), probe.run.rule.to_string()],
        staged
            .iter()
            .map(|(path, content)| (*path, content.as_str())),
    );

    let plan = plan_tool_rules(&work, &loader, probe.run.project_types, None);

    let run = required_run(&plan, probe.run.rule);
    assert_eq!(
        run.files(),
        staged
            .iter()
            .map(|(path, _)| (*path).to_string())
            .collect::<Vec<String>>(),
        "the run must carry every staged position, so what the tool reads is what decides"
    );

    let outcome = execute_tool_runs(std::slice::from_ref(run), &repo_root, None);

    assert!(
        outcome.errors().is_empty(),
        "the shipped pipeline must not break; errors: {:?}",
        outcome.errors()
    );
    let reported: Vec<&str> = outcome
        .findings()
        .iter()
        .map(|verified| verified.finding.file.as_str())
        .collect();
    assert_eq!(reported, probe.run.expected, "{}", probe.reason);
}

/// A staged file one tool rule cannot judge, and what the broken run must say.
///
/// A run that reports no finding and exits 0 for a file the tool never judged
/// reads exactly like a clean file. This shape measures the other behaviour: the
/// run reports no finding, and it reports one error that names what broke.
struct ShippedBrokenRun {
    /// The run the staged file must produce. Its `expected` names each fragment
    /// the one error detail must carry.
    run: ShippedRun,

    /// The prompt rule the work-list names beside the tool rule, which is the
    /// rule the tool rule supersedes.
    prompt_rule: &'static str,

    /// What the work-list states the change is for.
    change_purpose: &'static str,

    /// Where the staged file stands inside the probe repository, as the
    /// work-list holds it.
    path: &'static str,

    /// The bytes written at `path`, or `None` to write no file at all.
    ///
    /// `None` stages the file the tool cannot open. The work-list names the path
    /// either way, so the run reads the same file list, and the tool is the only
    /// thing that sees the difference.
    source: Option<&'static str>,

    /// Each file staged beside `path` that the work-list does NOT name.
    ///
    /// A `files`-scope rule reads the files it is given and needs none. A
    /// `workspace`-scope rule loads a project rather than a file list, so the
    /// project manifest stands here, and the tool then breaks on the staged
    /// file rather than on a project it could not find.
    support: &'static [(&'static str, &'static str)],
}

/// Drives the staged file of `probe` through the real tool pipeline, and holds
/// the run to reporting no finding and one error that names what broke.
fn verify_shipped_run_breaks(probe: &ShippedBrokenRun) {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let repo = tempfile::tempdir().unwrap();
    let content = probe.source.unwrap_or_default();
    stage_probe_files(repo.path(), probe.source.map(|bytes| (probe.path, bytes)));
    stage_probe_files(repo.path(), probe.support.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let work = tool_rule_work(
        probe.change_purpose,
        CODE_HYGIENE_SET,
        [probe.prompt_rule.to_string(), probe.run.rule.to_string()],
        [(probe.path, content)],
    );
    let plan = plan_tool_rules(&work, &loader, probe.run.project_types, None);
    let run = required_run(&plan, probe.run.rule);

    let outcome = execute_tool_runs(std::slice::from_ref(run), &repo_root, None);

    assert!(
        outcome.findings().is_empty(),
        "a file the tool never judged must report no finding; got {:?}",
        outcome.findings()
    );
    let details: Vec<&str> = outcome
        .errors()
        .iter()
        .map(|error| error.detail())
        .collect();
    assert_eq!(
        details.len(),
        1,
        "the run must report exactly one tool error; got {details:?}"
    );
    for fragment in probe.run.expected {
        assert!(
            details[0].contains(fragment),
            "the error must carry '{fragment}'; got '{}'",
            details[0]
        );
    }
}

/// A tool rule's script driven with NO file, and the tree it must not read.
///
/// A `files`-scope script judges the files it is given as its arguments. Given
/// none, a script that hands `"$@"` straight to its tool hands the tool an
/// empty argument list, and a tool that falls back to a default target of its
/// own then reads the whole tree. The run answers for files the review never
/// gave it, and it exits 0, so the answer reads as a measured result. This
/// shape measures the other behaviour: the script reports nothing and exits 0.
struct ShippedEmptyRun {
    /// The run whose script is driven with no file. Its `expected` names each
    /// finding the script must report, and a script given no file reports none.
    run: ShippedRun,

    /// Each file staged in the probe repository, with the bytes it holds. The
    /// script is given none of them, and a tool that reads a default target of
    /// its own finds every one.
    staged: &'static [(&'static str, &'static str)],

    /// Why the staged files stay silent.
    reason: &'static str,
}

/// The `run` script the shipped tool rule `rule` carries.
///
/// The script is read where the set ships it, so the run measures the SHIPPED
/// bytes rather than a copy a test wrote.
fn shipped_run_script(loader: &ValidatorLoader, rule: &str) -> String {
    loader
        .list_rulesets()
        .iter()
        .flat_map(|ruleset| ruleset.rules.iter())
        .find(|candidate| candidate.name == rule)
        .and_then(|candidate| candidate.tool.as_ref())
        .unwrap_or_else(|| panic!("the shipped tool rule `{rule}` must carry a tool block"))
        .run
        .clone()
}

/// Stages `files` in a temporary repository, drives the shipped script of
/// `rule` there with the argument list a run of `scope` carries, and answers
/// each finding it reported as `path:line`.
///
/// The arguments come from [`script_args`], the one function the engine builds
/// them with, so a `files`-scope rule reads the argument list it would really
/// receive with no matched file, and a `workspace`-scope rule reads the empty
/// list it always receives.
///
/// The findings are the SCRIPT's own, before the engine keeps only the ones in
/// the changed files. A script that names a file the author cannot edit is
/// visible here and nowhere else.
fn shipped_script_findings(
    loader: &ValidatorLoader,
    rule: &str,
    scope: ToolScope,
    staged: &[(&str, &str)],
) -> Result<Vec<String>, ScriptFailure> {
    let repo = tempfile::tempdir().unwrap();
    stage_probe_files(repo.path(), staged.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let script = shipped_run_script(loader, rule);
    let no_files: [&str; 0] = [];
    let args = script_args(scope, &no_files);

    let findings = run_script_findings(&script, &repo_root, &args)?;

    Ok(findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{}",
                normalize_tool_path(&finding.file, &repo_root),
                finding.line
            )
        })
        .collect())
}

/// The entries of `expected` as the owned strings [`shipped_script_findings`]
/// answers with.
fn expected_script_findings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|entry| (*entry).to_string()).collect()
}

/// Drives the shipped script of `probe` with no file argument at all, over a
/// probe repository the script was never given, and holds it to reporting
/// exactly the entries the probe names.
fn verify_shipped_run_reads_only_its_arguments(probe: &ShippedEmptyRun) {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);

    let reported = shipped_script_findings(&loader, probe.run.rule, ToolScope::Files, probe.staged)
        .expect("a script given no file must judge nothing and exit 0");

    assert_eq!(
        reported,
        expected_script_findings(probe.run.expected),
        "{}",
        probe.reason
    );
}

/// Names the prompt rules a roster row expects, for a failure message.
fn supersedes_label(expected: &[&str]) -> String {
    match expected.is_empty() {
        true => "nothing".to_string(),
        false => expected.join(", "),
    }
}

/// The project types a Swift workspace carries, as the plan holds them.
const SWIFT_PROJECT_TYPES: &[&str] = &["swift"];

/// Where a project states its own swiftlint settings, as the work-list holds
/// the path.
const SWIFT_PROJECT_CONFIG_PATH: &str = ".swiftlint.yml";

/// A project `.swiftlint.yml` that excludes the directory its generator writes
/// into.
///
/// The three shipped swiftlint rules name this file as the PARENT of the
/// configuration each of them writes, so each of the three is measured against
/// the same project settings.
const SWIFT_EXCLUDING_PROJECT_CONFIG: &str = concat!("excluded:\n", "  - Generated\n");

/// The project configuration staged beside a Swift probe's positions, which
/// the work-list does NOT name.
const SWIFT_EXCLUDING_SUPPORT_FILES: &[(&str, &str)] =
    &[(SWIFT_PROJECT_CONFIG_PATH, SWIFT_EXCLUDING_PROJECT_CONFIG)];

/// Where a project names a child configuration of its own, as the work-list
/// holds the path.
const SWIFT_OTHER_CONFIG_PATH: &str = "other.yml";

/// A project `.swiftlint.yml` that names a child configuration of its own.
///
/// swiftlint reads a list of `--config` paths as one parent-child hierarchy,
/// and a parent that names a child of its own makes that hierarchy ambiguous:
/// swiftlint aborts with exit 134 and writes `Could not read configuration` to
/// stderr. Each of the three shipped swiftlint rules then runs a second time
/// with its own configuration alone, so the run still measures. The
/// `excluded:` list stands here to show it is dropped for that second run.
const SWIFT_CHILD_CONFIG_PROJECT_CONFIG: &str = concat!(
    "child_config: other.yml\n",
    "excluded:\n",
    "  - Generated\n",
);

/// What the child configuration a project names holds. swiftlint reads this
/// file only when it accepts the hierarchy, and it never accepts one here.
const SWIFT_OTHER_CONFIG: &str = concat!("only_rules:\n", "  - todo\n");

/// The project configuration that names a child of its own, with that child
/// beside it, staged for a Swift probe. The work-list names neither.
const SWIFT_CHILD_CONFIG_SUPPORT_FILES: &[(&str, &str)] = &[
    (SWIFT_PROJECT_CONFIG_PATH, SWIFT_CHILD_CONFIG_PROJECT_CONFIG),
    (SWIFT_OTHER_CONFIG_PATH, SWIFT_OTHER_CONFIG),
];

/// A project `.swiftlint.yml` that states a warning threshold of one finding.
///
/// swiftlint counts the warnings of the whole run against this number. At the
/// number, and over it, swiftlint adds one `warning_threshold` entry of error
/// severity to the report and exits 2. Every finding of the run stands on
/// stdout beside that entry. A script that reads each nonzero status as a
/// broken tool answers no finding, so one line in the project file switches
/// the gate off. Each of the three shipped swiftlint rules reads status 2 as a
/// measured run.
const SWIFT_WARNING_THRESHOLD_PROJECT_CONFIG: &str = "warning_threshold: 1\n";

/// The warning-threshold project configuration staged beside a Swift probe's
/// positions, which the work-list does NOT name.
const SWIFT_WARNING_THRESHOLD_SUPPORT_FILES: &[(&str, &str)] = &[(
    SWIFT_PROJECT_CONFIG_PATH,
    SWIFT_WARNING_THRESHOLD_PROJECT_CONFIG,
)];

/// A project `.swiftlint.yml` that names a swiftlint version that is not
/// installed.
///
/// swiftlint compares this value with the version it is. At a difference it
/// writes one warning line to stderr, writes 0 bytes to stdout, runs no lint,
/// and exits 2. Measured with swiftlint 0.65.0: a run beside this file writes
/// 0 bytes and exits 2, and a run beside `warning_threshold: 1` writes 608
/// bytes and exits 2. So the status alone cannot tell a broken run from a
/// measured one, and the report tells them apart. Each of the three shipped
/// swiftlint rules accepts status 2 only when the report holds a JSON array of
/// one entry or more.
const SWIFT_VERSION_MISMATCH_PROJECT_CONFIG: &str = "swiftlint_version: 99.0.0\n";

/// The version-mismatch project configuration staged beside a Swift probe's
/// file, which the work-list does NOT name.
const SWIFT_VERSION_MISMATCH_SUPPORT_FILES: &[(&str, &str)] = &[(
    SWIFT_PROJECT_CONFIG_PATH,
    SWIFT_VERSION_MISMATCH_PROJECT_CONFIG,
)];

/// What the one error of a version-mismatch run must name: the swiftlint
/// warning line, which carries the version the project stated.
const SWIFT_VERSION_MISMATCH_ERROR: &[&str] = &["configuration specified version 99.0.0"];

/// The head a Swift staged file carries: none. The project's `excluded:` list
/// decides on the path alone, so every position holds the same bytes.
const SWIFT_NO_HEAD: &[&str] = &[];

/// The generated position: a file under the directory the project excludes.
const SWIFT_GENERATED_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "Generated/Staged.swift",
    head: SWIFT_NO_HEAD,
};

/// The ordinary position, which the project's exclude list does not name.
const SWIFT_ORDINARY_POSITION: ShippedStagedFile = ShippedStagedFile {
    path: "Sources/Staged.swift",
    head: SWIFT_NO_HEAD,
};

/// Both Swift positions, in the order the work-list holds them.
const SWIFT_EXCLUDE_POSITIONS: &[ShippedStagedFile] =
    &[SWIFT_GENERATED_POSITION, SWIFT_ORDINARY_POSITION];

/// The generated position alone, which is then every file of the run.
const SWIFT_EXCLUDED_POSITION_ONLY: &[ShippedStagedFile] = &[SWIFT_GENERATED_POSITION];

/// The ordinary position alone, for a probe that stages no excluded file.
const SWIFT_ORDINARY_POSITION_ONLY: &[ShippedStagedFile] = &[SWIFT_ORDINARY_POSITION];

/// What a run whose every file the project excludes must report: nothing.
const NO_STAGED_REPORTS: &[&str] = &[];

/// What a script given no file must report: nothing.
const NO_FINDINGS: &[&str] = &[];

/// The name of the SwiftPM manifest, and of the fixture template that
/// carries one.
const SWIFT_MANIFEST: &str = "Package.swift";

/// A Swift package root as the process working directory, held until the
/// returned pair drops.
///
/// The guard comes first and the directory second, because a tuple drops
/// its fields in declaration order. The guard therefore restores the
/// working directory before [`tempfile::TempDir`] removes the root. The
/// other order removes the root while the process still stands in it, and
/// `getcwd` fails for every call until the guard runs.
///
/// `dead-code-swift` checks `which periphery swift jq && test -f
/// Package.swift`, because periphery scans a built SPM package and reports
/// itself missing outside one. That half of the check is a working-directory
/// precondition rather than a tool, so no install command can satisfy it and
/// a roster test that requires every row has to supply it. The manifest is
/// the shipped fixture template, so the test states no manifest of its own.
///
/// The fixture runs are unaffected: doctor materializes each pair into its
/// own scratch directory, `Package.swift.tmpl` included, and runs the script
/// there.
fn swift_package_root(loader: &ValidatorLoader) -> (CurrentDirGuard, tempfile::TempDir) {
    let manifest = shipped_asset(loader, &FIXTURE_TEMPLATE_ASSET, SWIFT_MANIFEST);
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::copy(&manifest, root.path().join(SWIFT_MANIFEST))
        .expect("copy the shipped Package.swift template");
    let guard = CurrentDirGuard::new(root.path()).expect("cwd guard");
    (guard, root)
}

/// The pair [`swift_package_root`] returns restores the working directory
/// before it removes that directory.
///
/// A tuple drops its fields in declaration order, so the first element runs
/// first. A `TempDir` in that position removes the package root while the
/// root is still the process working directory, and `getcwd` then fails for
/// the whole window until the guard runs. The guard therefore has to be the
/// first element.
#[test]
fn the_swift_package_root_restores_the_directory_before_it_removes_it() {
    let loader = builtin_loader();
    let outside = std::env::current_dir().expect("a working directory before the guard");

    let (first, second) = swift_package_root(&loader);
    assert_ne!(
        std::env::current_dir().expect("the guard entered the package root"),
        outside,
        "the guard must enter the package root"
    );

    drop(first);

    let restored = std::env::current_dir();
    assert_eq!(
        restored.as_ref().ok().map(PathBuf::as_path),
        Some(outside.as_path()),
        "the first element must restore the working directory; instead the working \
         directory was removed while the process still stood in it"
    );
    drop(second);
}

/// Drives every rule in `rules` through the real install, doctor and
/// fixture path, and holds each one to the fixture contract.
///
/// Each row names a project type, the tool rule that serves it, and the
/// prompt rules that rule must supersede — empty for a rule that must leave
/// its prompt rule running. For each row, the helper reads the doctor row
/// and asserts what the row supersedes. The list belongs to the row rather
/// than to the call because one roster — the complexity rules — mixes rules
/// that replace one prompt rule with a rule that replaces two.
///
/// Every row keeps one contract, the same one the single-rule acceptance
/// tests keep: [`require_tool_installed`] gets the tool through the rule's
/// own declared install commands, and the fixture assertion then runs for
/// every row rather than for the rows this machine happens to carry. A row
/// whose tool cannot be obtained fails the test, naming the binary and the
/// command that installs it.
///
/// The degradation contract — a missing tool falls the rule back to its
/// prompt rule and never blocks a review — is held by
/// [`plan_reports_a_fallback_when_the_tool_is_missing_and_suppresses_nothing`]
/// and [`a_missing_tool_whose_installs_all_fail_stays_on_the_prompt_fallback`],
/// which state it over built specs and need no tool at all.
///
/// `rule_kind` names the group in the failure messages — the prompt rule the
/// group is named for, whether the group replaces that rule or runs beside
/// it — so a failing run says which roster broke.
fn verify_shipped_tool_rules_pass_fixtures(rules: &[(&str, &str, &[&str])], rule_kind: &str) {
    let loader = builtin_loader();
    let _package_root = swift_package_root(&loader);

    for (project_type, rule_name, expected_supersedes) in rules {
        let project_types = [*project_type];
        require_tool_installed(&loader, &project_types, rule_name);

        let status = crate::doctor::check_review_engine_with(&loader, &project_types, None);
        let row = status
            .tool_rules
            .iter()
            .find(|row| row.rule_name == *rule_name)
            .unwrap_or_else(|| panic!("{rule_name} must be reported for a {project_type} project"));
        assert_eq!(
            row.supersedes.names(),
            *expected_supersedes,
            "{rule_name} must supersede {}, the contract every {rule_kind} tool rule keeps",
            supersedes_label(expected_supersedes)
        );
        assert!(
            row.usable(),
            "{rule_name}'s tool is installed, so its fixtures must pass; doctor says: {}",
            row.degraded_detail()
        );
    }
}
