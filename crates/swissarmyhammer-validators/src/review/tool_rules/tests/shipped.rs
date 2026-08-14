//! What every shipped-rule acceptance test shares.
//!
//! A shipped-rule test drives the SHIPPED script over a probe repository and
//! reads what the real tool reports. This module carries the shapes those
//! tests are written in — a planned run, a fail fixture, a staged position
//! set, a staged row set, one path the work-list names, and a run measured
//! with no file beside the same run measured with the files — and the helpers
//! that drive each shape.
//!
//! The tests themselves stand one module per rule family. A family whose rule
//! for one language answers shapes the other languages cannot show stands one
//! module more, named for that language — the Rust dead-code rule reads the
//! cargo report, and the Swift one builds the package's test targets; the
//! complexity family stands one module for each language it drives.
//! Each module then stays small enough for a reviewer, and for the review
//! engine, to read whole. `scope_roster`, `temp_directory` and `zero_argument`
//! are the three
//! modules that are not a rule family: each reads the shipped script of EVERY
//! rule, because the contract it holds is about the set and not about one
//! language. `scope_roster` states which of those set-wide guards reads which
//! rule, and it holds the two scope rosters to the whole set.

mod complexity;
mod complexity_go;
mod complexity_python;
mod complexity_rust;
mod complexity_swift;
mod complexity_typescript;
mod dead_code;
mod dead_code_rust;
mod dead_code_swift;
mod dead_code_typescript;
mod function_length_go;
mod magic_numbers;
mod missing_docs;
mod missing_docs_rust;
mod scope_roster;
mod stuttering_name_go;
mod temp_directory;
mod unused_dependencies;
mod zero_argument;

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

/// Writes `bytes` into `repo` at `path`, making the directory of the file
/// first.
///
/// The content is a byte slice rather than text, because a probe of a file the
/// tool cannot DECODE stages bytes that are not UTF-8, and no `&str` holds
/// those.
fn stage_probe_bytes(repo: &Path, path: &str, bytes: &[u8]) {
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().expect("a staged path has a parent")).unwrap();
    std::fs::write(&file, bytes).unwrap();
}

/// Writes each `(path, bytes)` pair of `files` into `repo`, making the
/// directory of each one first.
fn stage_probe_files<'a>(repo: &Path, files: impl IntoIterator<Item = (&'a str, &'a str)>) {
    for (path, content) in files {
        stage_probe_bytes(repo, path, content.as_bytes());
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

    assert_shipped_run_reports(&outcome, probe.run.expected, probe.reason);
}

/// Holds `outcome` to breaking no run, and to reporting exactly the files
/// `expected` names, in the order `expected` holds them.
///
/// `reason` states why those files report and the others stay silent.
fn assert_shipped_run_reports(outcome: &ToolOutcome, expected: &[&str], reason: &str) {
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
    assert_eq!(reported, expected, "{reason}");
}

/// One path the work-list names, what the probe stages around it, and what the
/// run over that path must answer.
///
/// Two behaviours stand in this one shape, because the staging and the
/// work-list are the same for both and only the answer differs.
///
/// A run that reports no finding and exits 0 for a file the tool never judged
/// reads exactly like a clean file, so [`verify_shipped_run_breaks`] holds
/// such a run to no finding and to one error that names what broke.
///
/// A script that tests each path with `[ ! -r "$path" ]` admits a DIRECTORY,
/// because a directory is readable, and the tool then reads that directory. A
/// directory that holds no file of the tool's own language leaves the tool
/// nothing to judge, and the tool states that rather than breaking, so
/// [`verify_shipped_hollow_directory_answers_clean`] holds such a run to no
/// error and to the findings the probe names.
struct ShippedNamedPath {
    /// The run the named path must produce. What one entry of its `expected`
    /// is stands with the function that drives the probe: one fragment of the
    /// single error detail for [`verify_shipped_run_breaks`], and the file of
    /// one finding for [`verify_shipped_hollow_directory_answers_clean`].
    run: ShippedRun,

    /// The prompt rule the work-list names beside the tool rule, which is the
    /// rule the tool rule supersedes.
    prompt_rule: &'static str,

    /// What the work-list states the change is for.
    change_purpose: &'static str,

    /// Where the named path stands inside the probe repository, as the
    /// work-list holds it. The name meets the rule's own file pattern, because
    /// a path that pattern refuses reaches no run at all.
    path: &'static str,

    /// The bytes written at `path`, or `None` to write no file at all.
    ///
    /// `None` stages the path the tool cannot open, and it also leaves `path`
    /// free for the support files to make a directory of it. The work-list
    /// names the path either way, so the run reads the same file list, and the
    /// tool is the only thing that sees the difference.
    ///
    /// The type is a byte slice rather than text, because a probe of a file
    /// the tool cannot DECODE stages bytes that are not UTF-8, and no `&str`
    /// holds those.
    source: Option<&'static [u8]>,

    /// Each file staged beside `path` that the work-list does NOT name.
    ///
    /// A `files`-scope rule reads the files it is given and needs none. A
    /// `workspace`-scope rule loads a project rather than a file list, so the
    /// project manifest stands here, and the tool then breaks on the staged
    /// file rather than on a project it could not find. A probe of a DIRECTORY
    /// names the files inside `path` here, and those files make the directory.
    support: &'static [(&'static str, &'static str)],
}

/// Stages the probe repository of `probe`, plans the run its work-list asks
/// for, and drives that run through the real tool pipeline.
///
/// The probe repository comes back beside the outcome because
/// [`tempfile::TempDir`] removes the tree as it drops, and an assertion that
/// reads the staged tree needs the tree standing.
fn drive_shipped_named_path(probe: &ShippedNamedPath) -> (tempfile::TempDir, ToolOutcome) {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let repo = tempfile::tempdir().unwrap();
    let content = probe
        .source
        .map(String::from_utf8_lossy)
        .unwrap_or_default();
    if let Some(bytes) = probe.source {
        stage_probe_bytes(repo.path(), probe.path, bytes);
    }
    stage_probe_files(repo.path(), probe.support.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let work = tool_rule_work(
        probe.change_purpose,
        CODE_HYGIENE_SET,
        [probe.prompt_rule.to_string(), probe.run.rule.to_string()],
        [(probe.path, content.as_ref())],
    );
    let plan = plan_tool_rules(&work, &loader, probe.run.project_types, None);
    let run = required_run(&plan, probe.run.rule);

    let outcome = execute_tool_runs(std::slice::from_ref(run), &repo_root, None);

    (repo, outcome)
}

/// Drives the named path of `probe` through the real tool pipeline, and holds
/// the run to reporting no finding and one error that names what broke.
fn verify_shipped_run_breaks(probe: &ShippedNamedPath) {
    let (_repo, outcome) = drive_shipped_named_path(probe);

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

/// Drives the directory of `probe` through the real tool pipeline, and holds
/// the run to reporting no error and exactly the findings the probe names.
///
/// The probe writes no file at `path` and stages its support files under that
/// path, so the run measures a DIRECTORY. The assertion states that, because a
/// probe that wrote a file there would measure the other behaviour and still
/// pass.
fn verify_shipped_hollow_directory_answers_clean(probe: &ShippedNamedPath) {
    let (repo, outcome) = drive_shipped_named_path(probe);

    assert!(
        probe_repository_root(&repo).join(probe.path).is_dir(),
        "the probe must stage {} as a directory, or the run measures a file",
        probe.path
    );
    assert_shipped_run_reports(
        &outcome,
        probe.run.expected,
        "the guard admits the directory, the tool finds no file of its own language under it, \
         and the script reads the tool's own message and answers clean",
    );
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

    /// Each finding the SAME script reports when it takes every staged file
    /// as its arguments, one `path:line` entry for each.
    ///
    /// The zero-argument half alone cannot tell a guard that answers nothing
    /// from a script that answers nothing whatever it is given. Each staged
    /// file trips the rule, so this list states what the guard must leave
    /// standing, and a guard that swallowed the real run breaks it.
    ///
    /// The list is the whole answer rather than a count, because a tool that
    /// moved its findings to other lines, or to other files, keeps the count.
    ///
    /// The ORDER of the entries is free. A tool that reads more than one file
    /// at one time answers in the order its own work finished. Measured on
    /// `missing-docs-swift`: two runs over the same two files gave the two
    /// files in opposite order.
    with_files: &'static [&'static str],

    /// Why the staged files stay silent.
    reason: &'static str,
}

/// One shipped rule that carries a `tool` block.
///
/// A guard over the whole set reads these four fields and no other, so one
/// shape serves every guard and one walk of the set answers them all.
struct ShippedToolRule {
    /// The name of the rule, for the failure messages.
    name: String,

    /// Which inputs the `run` script receives.
    scope: ToolScope,

    /// The `run` script the set ships for the rule.
    script: String,

    /// The `doctor.check_command` the set ships for the rule, or `None` when
    /// the rule carries no `doctor` block at all. A rule that names no check
    /// names no tool either.
    check_command: Option<String>,
}

/// Every shipped rule that carries a `tool` block.
///
/// The block is read where the set ships it, so a guard over the answer holds
/// the SHIPPED bytes rather than a copy a test wrote, and a rule added later
/// stands in the answer with no test edit.
fn shipped_tool_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    loader
        .list_rulesets()
        .iter()
        .flat_map(|ruleset| ruleset.rules.iter())
        .filter_map(|rule| rule.tool.as_ref().map(|tool| (&rule.name, tool)))
        .map(|(name, tool)| ShippedToolRule {
            name: name.clone(),
            scope: tool.scope,
            script: tool.run.clone(),
            check_command: tool
                .doctor
                .as_ref()
                .map(|doctor| doctor.check_command.clone()),
        })
        .collect()
}

/// The rules of `rules` that `holds` answers false for, by name.
fn tool_rules_that_deviate(
    rules: &[ShippedToolRule],
    holds: impl Fn(&ShippedToolRule) -> bool,
) -> Vec<&str> {
    rules
        .iter()
        .filter(|rule| !holds(rule))
        .map(|rule| rule.name.as_str())
        .collect()
}

/// The lines of `script`, each one with its leading and trailing space
/// removed.
///
/// A guard reads a script line by line, and a script indents a line that
/// stands inside a block. The trim is what lets a guard compare a line
/// against the one text the contract states, wherever the script writes it.
fn trimmed_script_lines(script: &str) -> Vec<&str> {
    script.lines().map(str::trim).collect()
}

/// The index of each line of `lines` that reads `head`.
fn script_lines_that_read(lines: &[&str], head: &str) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == head)
        .map(|(at, _)| at)
        .collect()
}

/// Whether the lines of `lines` directly under `at` read `under`, in order.
///
/// The lines stand together, so no statement between them can run before the
/// block ends, and the block cannot open something a later line closes. A
/// block that runs past the last line of the script answers false.
fn script_lines_under(lines: &[&str], at: usize, under: &[&str]) -> bool {
    under
        .iter()
        .enumerate()
        .all(|(step, text)| lines.get(at + 1 + step) == Some(text))
}

/// The shipped rules `keeps` answers true for, or a panic when the set ships
/// another number of them.
///
/// A guard over an empty list holds nothing and reports green, and a guard
/// over a list that shrank holds less than it held before. The size is
/// therefore an assertion of its own: a rule dropped from the set, a rule
/// whose `run` no longer meets `keeps`, and a rule added with no thought for
/// the contract each break it. `roster` names what the rules of the list have
/// in common, for the failure message.
fn required_tool_rules(
    loader: &ValidatorLoader,
    roster: &str,
    count: usize,
    keeps: impl Fn(&ShippedToolRule) -> bool,
) -> Vec<ShippedToolRule> {
    let rules: Vec<ShippedToolRule> = shipped_tool_rules(loader)
        .into_iter()
        .filter(|rule| keeps(rule))
        .collect();
    let names: Vec<&str> = rules.iter().map(|rule| rule.name.as_str()).collect();

    assert_eq!(
        rules.len(),
        count,
        "the set must ship {count} rules that {roster}, or this guard holds another \
         roster than the one it was measured against; it ships {names:?}"
    );

    rules
}

/// The `tool` block the shipped rule `rule` carries, or a panic naming it.
///
/// The block is read where the set ships it, so a run measures the SHIPPED
/// bytes rather than a copy a test wrote.
fn required_shipped_tool_rule(loader: &ValidatorLoader, rule: &str) -> ShippedToolRule {
    shipped_tool_rules(loader)
        .into_iter()
        .find(|candidate| candidate.name == rule)
        .unwrap_or_else(|| panic!("the shipped tool rule `{rule}` must carry a tool block"))
}

/// The argument list a `workspace`-scope script receives: none.
const NO_SCRIPT_FILES: &[&str] = &[];

/// Each finding of `findings` as the `path:line` row a probe states, with the
/// path of the probe repository taken off.
///
/// A tool reports an absolute path, and a probe states the path the work-list
/// holds. [`normalize_tool_path`] is the one function the engine attributes a
/// tool-reported path with, so a probe reads the same path the engine would.
fn finding_rows(findings: &[Finding], repo_root: &Path) -> Vec<String> {
    findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{}",
                normalize_tool_path(&finding.file, repo_root),
                finding.line
            )
        })
        .collect()
}

/// Stages `staged` in a temporary repository, drives the shipped script of
/// `rule` there with the argument list a run over `files` carries, and answers
/// each finding it reported as `path:line`.
///
/// The arguments come from [`script_args`], the one function the engine builds
/// them with, and the scope comes from the SHIPPED rule rather than from the
/// caller. So a `files`-scope rule reads the argument list it would really
/// receive for `files`, a `workspace`-scope rule reads the empty list it always
/// receives, and a probe cannot state a shape the rule does not carry.
///
/// The findings are the SCRIPT's own, before the engine keeps only the ones in
/// the changed files. A script that names a file the author cannot edit is
/// visible here and nowhere else.
fn shipped_script_findings(
    loader: &ValidatorLoader,
    rule: &str,
    staged: &[(&str, &str)],
    files: &[&str],
) -> Result<Vec<String>, ScriptFailure> {
    let shipped = required_shipped_tool_rule(loader, rule);
    let repo = tempfile::tempdir().unwrap();
    stage_probe_files(repo.path(), staged.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let args = script_args(shipped.scope, files);

    let findings = run_script_findings(&shipped.script, &repo_root, &args)?;

    Ok(finding_rows(&findings, &repo_root))
}

/// The entries of `expected` as the owned strings [`shipped_script_findings`]
/// answers with.
fn expected_script_findings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|entry| (*entry).to_string()).collect()
}

/// Sorted names, for a set comparison that does not depend on read order.
fn sorted_names(names: &[String]) -> Vec<String> {
    let mut sorted = names.to_vec();
    sorted.sort();
    sorted
}

/// Drives the shipped script of `run` `runs` times over ONE probe repository,
/// every run released together, and answers the sorted `path:line` rows each
/// run reported.
///
/// One repository is what makes the probe. A tool that takes a file lock, and a
/// tool whose cache directory is named for the workspace, can clash only with
/// another run standing in the same workspace, so runs in different
/// repositories could never measure it.
///
/// `files` is the file list the runs are given, which [`script_args`] reads
/// beside the SHIPPED scope: a `files`-scope rule takes the staged paths, and a
/// `workspace`-scope rule takes [`NO_SCRIPT_FILES`] because it receives an
/// empty list on every run.
fn rows_of_runs_started_together(
    run: &ShippedRun,
    staged: &[(&str, &str)],
    files: &[&str],
    runs: usize,
) -> Vec<Vec<String>> {
    let loader = builtin_loader();
    require_tool_installed(&loader, run.project_types, run.rule);
    let shipped = required_shipped_tool_rule(&loader, run.rule);
    let repo = tempfile::tempdir().expect("temp dir");
    stage_probe_files(repo.path(), staged.iter().copied());
    let repo_root = probe_repository_root(&repo);
    let args = script_args(shipped.scope, files);
    let released = std::sync::Barrier::new(runs);

    std::thread::scope(|threads| {
        let running: Vec<_> = (0..runs)
            .map(|_| {
                threads.spawn(|| {
                    released.wait();
                    let reported = run_script_findings(&shipped.script, &repo_root, &args)
                        .expect("each run must judge the probe repository and exit 0");
                    sorted_names(&finding_rows(&reported, &repo_root))
                })
            })
            .collect();
        running
            .into_iter()
            .map(|run| run.join().expect("each run of the probe must finish"))
            .collect()
    })
}

/// A whole probe repository, and what the shipped script of one rule must
/// answer over it.
///
/// A `workspace`-scope script loads a project rather than a file list, so a
/// probe of such a rule stages a whole package — the manifest, the sources,
/// and the build script where the shape needs one — and the tool reads what it
/// finds there. One shape carries both answers a run can give, because the
/// staging is the same for both and only the answer differs:
/// [`verify_shipped_tree_breaks`] holds a run to no finding and to an error
/// that names what broke, and [`verify_shipped_tree_reports`] holds a run to
/// exactly the findings the probe names.
struct ShippedStagedTree {
    /// The run the staged tree must produce. What one entry of its `expected`
    /// is stands with the function that drives the probe: one fragment of the
    /// error detail for [`verify_shipped_tree_breaks`], and one `path:line`
    /// entry for [`verify_shipped_tree_reports`].
    run: ShippedRun,

    /// Each file of the probe repository, with the bytes it holds. The
    /// work-list names every one of them.
    staged: &'static [(&'static str, &'static str)],

    /// Why the run answers what the probe names, for the failure message.
    reason: &'static str,
}

/// Drives the shipped script of `probe` over the tree it stages, with `extra`
/// staged beside it, and answers what that run reported.
///
/// The work-list names the probe's own files and never `extra`, because a file
/// staged to shape the RUN is not a file the change touched.
fn drive_shipped_staged_tree_with(
    probe: &ShippedStagedTree,
    extra: &[(&str, &str)],
) -> Result<Vec<String>, ScriptFailure> {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let paths: Vec<&str> = probe.staged.iter().map(|(path, _)| *path).collect();
    let staged: Vec<(&str, &str)> = probe
        .staged
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect();

    shipped_script_findings(&loader, probe.run.rule, &staged, &paths)
}

/// Drives the shipped script of `probe` over the tree it stages, and answers
/// what that run reported.
fn drive_shipped_staged_tree(probe: &ShippedStagedTree) -> Result<Vec<String>, ScriptFailure> {
    drive_shipped_staged_tree_with(probe, &[])
}

/// Holds the run of `probe` to breaking with an error that names every
/// fragment the probe expects.
///
/// A run that reports no finding and exits 0 over a tree the tool never judged
/// reads exactly like a clean tree, so a broken run must state what broke.
fn verify_shipped_tree_breaks(probe: &ShippedStagedTree) {
    let failure = drive_shipped_staged_tree(probe).expect_err(probe.reason);

    assert_shipped_failure_names(&failure, probe.run.expected);
}

/// Holds `failure` to naming every fragment `expected` carries.
fn assert_shipped_failure_names(failure: &ScriptFailure, expected: &[&str]) {
    let detail = failure.to_string();
    for fragment in expected {
        assert!(
            detail.contains(fragment),
            "the run must break with '{fragment}'; got '{detail}'"
        );
    }
}

/// Holds the run of `probe` to reporting exactly the `path:line` entries the
/// probe names, and to exiting 0.
///
/// This is the control half of [`verify_shipped_tree_breaks`]: a gate that
/// broke every run it could not read at a glance would pass that assertion and
/// throw away the findings of a run the tool DID make.
fn verify_shipped_tree_reports(probe: &ShippedStagedTree) {
    let reported = drive_shipped_staged_tree(probe)
        .expect("the shipped script must judge the probe package and exit 0");

    assert_eq!(
        sorted_names(&reported),
        sorted_names(&expected_script_findings(probe.run.expected)),
        "{}",
        probe.reason
    );
}

/// The status a shell answers for a command it could not run.
const COMMAND_NOT_FOUND_STATUS: i32 = 127;

/// The mode that makes a file executable for its owner and readable for every
/// other user.
#[cfg(unix)]
const EXECUTABLE_MODE: u32 = 0o755;

/// The name the shipped scripts call the report filter by.
const FILTER_BINARY_NAME: &str = "jq";

/// The file a probe stages to make the stubbed command break for its own run
/// alone.
const BROKEN_COMMAND_MARKER: &str = ".sah-broken-command";

/// The one directory every stubbed command of this test binary stands in.
///
/// The directory outlives each test that leads `PATH` with it, on purpose. A
/// run that read the stubbed `PATH` before that test finished still has to find
/// a command there, and a directory removed under such a run makes the shell
/// answer `No such file or directory` for a tool the machine has. Measured with
/// a directory of its own for each stub: `complexity-rust` broke that way in
/// the whole-suite run, on `.tmp06q4QT/jq: No such file or directory`.
#[cfg(unix)]
fn stub_directory() -> &'static Path {
    static STUBS: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();

    STUBS
        .get_or_init(|| tempfile::tempdir().expect("make the directory the stubs stand in"))
        .path()
}

/// The path of `binary` on this machine, or a panic naming it.
///
/// The stub built by [`verify_shipped_tree_breaks_without`] hands every other
/// run through to this path, so it is resolved BEFORE the stub leads `PATH`.
#[cfg(unix)]
fn resolved_binary(binary: &str) -> String {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("command -v \"$1\"")
        .arg("sh")
        .arg(binary)
        .output()
        .unwrap_or_else(|error| panic!("ask the shell where `{binary}` stands: {error}"));
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        !path.is_empty(),
        "`{binary}` must stand on PATH for this probe to stub it"
    );
    path
}

/// Holds the run of `probe` to breaking when the command `binary` cannot run,
/// with an error that names every fragment the probe expects.
///
/// A step that ends in a pipe takes the status of the last command of that
/// pipe, so a step whose own tool broke reads as a step that found nothing.
/// The probe leads `PATH` with a directory holding a command of that name which
/// answers nothing and exits [`COMMAND_NOT_FOUND_STATUS`], so the SHIPPED
/// script runs its own step and finds it broken.
///
/// `PATH` is process state, and every other test of this binary drives a
/// shipped script through the same commands. So the stub breaks for ONE run:
/// it exits nonzero only when [`BROKEN_COMMAND_MARKER`] stands in the working
/// directory, which this probe alone stages, and it hands every other run
/// through to the real binary. Measured with the plain stub instead: the whole
/// tool-rule suite reported 8 failures, among them four `complexity-go` tests
/// whose fixture pair broke on `exit status: 127` and three `complexity-rust`
/// tests whose fixtures broke on `jq could not read the clippy report`.
///
/// The caller still stands under `#[serial_test::serial(env)]`, because the
/// `PATH` it leads is process state whatever the stub then does.
#[cfg(unix)]
fn verify_shipped_tree_breaks_without(probe: &ShippedStagedTree, binary: &str) {
    use std::os::unix::fs::PermissionsExt;
    use swissarmyhammer_common::test_utils::PathGuard;

    let real = resolved_binary(binary);
    let stubs = stub_directory();
    let stub = stubs.join(binary);
    std::fs::write(
        &stub,
        format!(
            "#!/bin/sh\nif [ -e \"./{BROKEN_COMMAND_MARKER}\" ]; then\n  \
             exit {COMMAND_NOT_FOUND_STATUS}\nfi\nexec \"{real}\" \"$@\"\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(EXECUTABLE_MODE)).unwrap();
    let _path = PathGuard::prepend(stubs);

    let failure = drive_shipped_staged_tree_with(probe, &[(BROKEN_COMMAND_MARKER, "")])
        .expect_err(probe.reason);

    assert_shipped_failure_names(&failure, probe.run.expected);
}

/// Drives the shipped script of `probe` two times over the same probe
/// repository, and holds each run to what the probe names for it.
///
/// The first run takes NO file argument, and it must report exactly the
/// entries the probe names, which is none.
///
/// The second run takes every staged file, and it must report exactly the
/// entries the probe names for it. The first run alone cannot tell a guard
/// that stops a run with nothing to judge from a guard that stops every run: a
/// script that reported nothing whatever it was given would pass the first
/// assertion. The second assertion is what makes the guard, and not the whole
/// script, the thing the first one measures.
///
/// The rule of the probe must state `scope: files`, because that scope is what
/// makes the two runs different. A `workspace`-scope script takes an empty
/// argument list for both runs, so the two halves would measure one run two
/// times.
fn verify_shipped_run_reads_only_its_arguments(probe: &ShippedEmptyRun) {
    let loader = builtin_loader();
    require_tool_installed(&loader, probe.run.project_types, probe.run.rule);
    let shipped = required_shipped_tool_rule(&loader, probe.run.rule);

    assert_eq!(
        shipped.scope,
        ToolScope::Files,
        "the probe for `{}` must name a rule that states `scope: files`, or \
         the two runs below take the same empty argument list and the pair \
         measures one run two times",
        probe.run.rule
    );

    let staged_files: Vec<&str> = probe.staged.iter().map(|(path, _)| *path).collect();

    let unread = shipped_script_findings(&loader, probe.run.rule, probe.staged, &[])
        .expect("a script given no file must judge nothing and exit 0");
    let read = shipped_script_findings(&loader, probe.run.rule, probe.staged, &staged_files)
        .expect("a script given its own files must judge them and exit 0");

    assert_eq!(
        sorted_names(&unread),
        sorted_names(&expected_script_findings(probe.run.expected)),
        "{}",
        probe.reason
    );
    assert_eq!(
        sorted_names(&read),
        sorted_names(&expected_script_findings(probe.with_files)),
        "the same script must report exactly these findings over the staged files, or \
         the guard swallows the run it is meant to leave standing"
    );
}

/// Staged files, and the exact ROWS one tool rule's real pipeline must report
/// over them.
///
/// [`ShippedStagedPositions`] names the FILE of each finding, so it cannot tell
/// one definition of a file from another. A carve-out a script decides by the
/// NAME of a definition needs the row: the file reports either way, and the row
/// states which definition the script kept and which one it dropped.
struct ShippedStagedRows {
    /// The run the staged files must produce. Each entry of its `expected` is
    /// one `path:line` the script must report.
    run: ShippedRun,

    /// Each file staged in the probe repository, with the bytes it holds. The
    /// script is given every one of them.
    staged: &'static [(&'static str, &'static str)],

    /// Why those rows report and the others stay silent.
    reason: &'static str,
}

/// Drives the shipped script of `probe` over every file it stages, and holds
/// the run to reporting exactly the rows the probe names.
fn verify_shipped_staged_rows_report(probe: &ShippedStagedRows) {
    verify_staged_rows_report(
        probe.run.project_types,
        probe.run.rule,
        probe.staged,
        probe.run.expected,
        probe.reason,
    );
}

/// Drives the shipped script of `rule` over every file of `staged`, and holds
/// the run to reporting exactly the `path:line` rows `expected` names.
///
/// `staged` and `expected` are borrowed rather than `'static`, because a probe
/// whose shape is a function of several hundred lines builds its source, and
/// the line each finding stands on, at run time. A probe whose files fit in the
/// binary states them as a [`ShippedStagedRows`] and reaches this function
/// through [`verify_shipped_staged_rows_report`].
fn verify_staged_rows_report(
    project_types: &[&str],
    rule: &str,
    staged: &[(&str, &str)],
    expected: &[&str],
    reason: &str,
) {
    let loader = builtin_loader();
    require_tool_installed(&loader, project_types, rule);
    let staged_files: Vec<&str> = staged.iter().map(|(path, _)| *path).collect();

    let reported = shipped_script_findings(&loader, rule, staged, &staged_files)
        .expect("a script given its own files must judge them and exit 0");

    assert_eq!(
        sorted_names(&reported),
        sorted_names(&expected_script_findings(expected)),
        "{reason}"
    );
}

/// Names the prompt rules a roster row expects, for a failure message.
fn supersedes_label(expected: &[&str]) -> String {
    match expected.is_empty() {
        true => "nothing".to_string(),
        false => expected.join(", "),
    }
}

/// The project types a Python workspace carries, as the plan holds them.
const PYTHON_PROJECT_TYPES: &[&str] = &["python"];

/// The project types a Go workspace carries, as the plan holds them.
const GO_PROJECT_TYPES: &[&str] = &["go"];

/// The project types a Node.js workspace carries, as the plan holds them.
const NODEJS_PROJECT_TYPES: &[&str] = &["nodejs"];

/// The project types a Flutter workspace carries, as the plan holds them.
const FLUTTER_PROJECT_TYPES: &[&str] = &["flutter"];

/// The project types a Rust workspace carries, as the plan holds them.
///
/// The two rules whose tool IS sah match a path by its extension and name no
/// project type of their own, so any project type reaches them.
const RUST_PROJECT_TYPES: &[&str] = &["rust"];

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

/// The position of the file whose NAME holds the words of swiftlint's decode
/// message, under the directory the project excludes.
///
/// The name ends in `.swift`, so the rule's own file pattern claims it and the
/// run carries it. The project excludes the directory, so swiftlint reads no
/// file and writes the path into a message of its own. Each of the three
/// shipped swiftlint rules tests stderr for the decode message, so each is
/// measured over this name.
const SWIFT_DECODE_NAME_POSITION_ONLY: &[ShippedStagedFile] = &[ShippedStagedFile {
    path: "Generated/Could not read contents of.swift",
    head: SWIFT_NO_HEAD,
}];

/// The position of the file whose NAME holds the words of swiftlint's
/// configuration message, under the directory the project excludes.
///
/// The same cause reaches the configuration test, and there it makes a WRONG
/// FINDING rather than a break: the script drops the project configuration and
/// runs swiftlint a second time without the `excluded:` list.
const SWIFT_CONFIG_NAME_POSITION_ONLY: &[ShippedStagedFile] = &[ShippedStagedFile {
    path: "Generated/Could not read configuration.swift",
    head: SWIFT_NO_HEAD,
}];

/// Where the directory that holds no Swift file stands inside a Swift probe
/// repository.
///
/// The name ends in `.swift` because each of the three shipped swiftlint rules
/// matches a path by that suffix, and a path the pattern refuses reaches no
/// run at all.
const SWIFT_HOLLOW_PATH: &str = "Sources/Hollow.swift";

/// The one file inside that directory. Its name ends in `.txt`, so swiftlint
/// finds no Swift file under the directory it is given, and the file makes the
/// directory the probe stages.
const SWIFT_HOLLOW_FILES: &[(&str, &str)] = &[("Sources/Hollow.swift/Notes.txt", "notes\n")];

/// What the work-list of a hollow-directory probe states the change is for.
const SWIFT_HOLLOW_PURPOSE: &str = "a directory that holds no Swift file";

/// What a run whose every file the project excludes must report: nothing.
const NO_STAGED_REPORTS: &[&str] = &[];

/// What a script given no file must report: nothing.
const NO_FINDINGS: &[&str] = &[];

/// Why the staged tree of a [`ShippedEmptyRun`] probe stays silent.
///
/// Every rule keeps the same contract, so every probe states it in the same
/// words and one sentence carries them all.
const READS_ONLY_ITS_ARGUMENTS: &str =
    "the script judges the files it is given and no other: given none, it reports none \
     and exits 0, and the staged tree stays unread";

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
///
/// The working directory is one value that every thread of the test binary
/// shares. [`CurrentDirGuard`] holds a global lock, so no two guards stand at
/// the same time. That lock does not cover what a test does BEFORE it takes
/// the guard: a test that reads the working directory first reads the value
/// another test set. Each test that calls this helper therefore stands under
/// `#[serial_test::serial(cwd)]`. That key holds the whole test body apart
/// from every other test in the binary that moves the working directory —
/// the tests of `validators::loader` and of `review::drive` already use it.
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
///
/// The test reads the working directory before it takes the guard, so it
/// stands under the `cwd` key [`swift_package_root`] states.
#[test]
#[serial_test::serial(cwd)]
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
///
/// Each caller stands under `#[serial_test::serial(cwd)]`, because
/// [`swift_package_root`] moves the process working directory.
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
