use super::*;

/// A run over `script` with `scope` and `files`, for the execute tests.
fn run_of(script: &str, scope: ToolScope, files: &[&str]) -> ToolRun {
    ToolRun {
        validator: "docs".to_string(),
        rule: "docs-tool".to_string(),
        spec: ToolSpec {
            scope,
            run: script.to_string(),
            doctor: None,
            install: None,
        },
        files: files.iter().map(|f| f.to_string()).collect(),
    }
}

/// A run reported under `validator` and `rule`, for the tests that read only
/// the names a run is reported under.
fn run_named(validator: &str, rule: &str) -> ToolRun {
    ToolRun {
        validator: validator.to_string(),
        rule: rule.to_string(),
        ..run_of("true", ToolScope::Files, &[])
    }
}

/// A tool-run task that does not finish reports one error for every run it
/// carried, each under that run's own names.
///
/// The runs overlap the fleet on a blocking task, so a panic in the run loop
/// is the one way the task can end without an outcome. Every rule the task
/// carried has to reach the report: a lost rule reads as a clean run.
#[tokio::test]
async fn a_tool_run_task_that_panics_reports_every_run_it_carried() {
    let runs = [
        run_named("docs", "docs-tool"),
        run_named("todo", "todo-tool"),
    ];
    let in_flight = ToolRunsInFlight {
        identities: runs.iter().map(RunIdentity::of).collect(),
        task: tokio::task::spawn_blocking(|| panic!("the run loop broke")),
    };

    let (findings, errors, diagnostics) = in_flight.finish().await.into_parts();

    assert!(
        findings.is_empty(),
        "a task that did not finish reports no findings: {findings:?}"
    );
    assert!(
        diagnostics.is_empty(),
        "a task that did not finish judged nothing, so it declined nothing: {diagnostics:?}"
    );
    let reported: Vec<(&str, &str)> = errors
        .iter()
        .map(|error| (error.validator(), error.rule()))
        .collect();
    assert_eq!(
        reported,
        [("docs", "docs-tool"), ("todo", "todo-tool")],
        "every run the task carried must be reported under its own names"
    );
}

#[test]
fn execute_passes_the_changed_files_as_arguments_and_tags_the_findings() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join("src/lib.rs"), "fn a() {}\n// TODO: fix\n").unwrap();
    let run = run_of(TODO_SCRIPT, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.errors().is_empty());
    assert_eq!(outcome.findings().len(), 1);
    let verified = &outcome.findings()[0];
    assert!(
        verified.confirmed,
        "tool findings are confirmed by construction"
    );
    assert_eq!(verified.finding.file, "src/lib.rs");
    assert_eq!(verified.finding.line, 2);
    assert_eq!(verified.finding.validator, "docs");
    assert_eq!(verified.finding.rule.as_deref(), Some("docs-tool"));
}

#[test]
fn execute_keeps_only_matched_file_findings_for_a_workspace_scope_run() {
    let repo = tempfile::tempdir().unwrap();
    // The script reports findings in a matched and an unmatched file.
    let script = r#"printf 'src/lib.rs:1: in scope\n./src/lib.rs:2: dot slash in scope\nsrc/unrelated.rs:1: out of scope\n'"#;
    let run = run_of(script, ToolScope::Workspace, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.errors().is_empty());
    let files: Vec<&str> = outcome
        .findings()
        .iter()
        .map(|v| v.finding.file.as_str())
        .collect();
    assert_eq!(files, ["src/lib.rs", "src/lib.rs"]);
}

#[test]
fn execute_reports_a_nonzero_exit_as_a_tool_error_with_the_raw_stderr() {
    let repo = tempfile::tempdir().unwrap();
    // The script prints a well-formed finding line but exits nonzero: the
    // exit code wins — a tool error, no findings read.
    let script = r#"echo "src/lib.rs:1: would-be finding"; echo "the linter exploded" >&2; exit 3"#;
    let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.findings().is_empty());
    assert_eq!(outcome.errors().len(), 1);
    assert_eq!(outcome.errors()[0].validator(), "docs");
    assert_eq!(outcome.errors()[0].rule(), "docs-tool");
    assert!(outcome.errors()[0].detail().contains("the linter exploded"));
}

#[test]
fn execute_reports_contract_breaking_stdout_as_a_tool_error() {
    let repo = tempfile::tempdir().unwrap();
    let script = r#"echo "this is not a finding line""#;
    let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.findings().is_empty());
    assert_eq!(outcome.errors().len(), 1);
    assert!(outcome.errors()[0]
        .detail()
        .contains("this is not a finding line"));
}

#[test]
fn a_tool_run_error_displays_the_rule_the_validator_and_the_detail() {
    let error = ToolRunError::for_test("docs", "docs-tool", "the linter exploded");

    assert_eq!(
        error.to_string(),
        "tool rule `docs-tool` in validator `docs` broke: the linter exploded"
    );
}

#[test]
fn a_tool_run_error_is_a_standard_error() {
    let error = ToolRunError::for_test("docs", "docs-tool", "the linter exploded");
    let standard: &dyn std::error::Error = &error;

    assert_eq!(standard.to_string(), error.to_string());
}

#[test]
fn a_tool_fallback_displays_the_rule_the_validator_and_the_detail() {
    let fallback = ToolFallback::for_test(
        "docs",
        "docs-tool",
        &["missing-docs"],
        "the tool is not installed",
    );

    assert_eq!(
        fallback.to_string(),
        "tool rule `docs-tool` in validator `docs` fell back: the tool is not installed"
    );
}

#[test]
fn execute_streams_planned_pair_and_findings_events() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(repo.path().join("src/lib.rs"), "// TODO: fix\n").unwrap();
    let run = run_of(TODO_SCRIPT, ToolScope::Files, &["src/lib.rs"]);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    execute_tool_runs(&[run], repo.path(), Some(&tx));

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    assert!(matches!(
        events[0],
        ReviewProgressEvent::Planned { total_pairs: 1 }
    ));
    assert!(
        matches!(&events[1], ReviewProgressEvent::PairStarted { validator, file }
        if validator == "docs" && file == "src/lib.rs")
    );
    assert!(
        matches!(&events[2], ReviewProgressEvent::Findings { validator, findings }
        if validator == "docs" && findings.len() == 1)
    );
    assert!(
        matches!(&events[3], ReviewProgressEvent::PairDone { validator, file }
        if validator == "docs" && file == "src/lib.rs")
    );
    assert_eq!(events.len(), 4);
}

/// An empty plan runs nothing, so it reports the default outcome and
/// raises no event at all.
#[test]
fn execute_emits_no_planned_event_when_there_are_no_runs() {
    let repo = tempfile::tempdir().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let outcome = execute_tool_runs(&[], repo.path(), Some(&tx));

    assert_eq!(outcome, ToolOutcome::default());
    assert!(rx.try_recv().is_err(), "no events for an empty plan");
}

/// A marked stderr line from a run that exited 0 reaches the outcome.
///
/// A rule that judged the code and could not judge ONE item exits 0 — failing
/// the whole run over one item is worse — so the marked line is the only
/// channel it has left.
#[test]
fn execute_reports_a_marked_stderr_line_as_a_diagnostic() {
    let repo = tempfile::tempdir().unwrap();
    let script = r#"printf 'sah-diagnostic: no compile database covers src/lib.rs\n' >&2"#;
    let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.errors().is_empty());
    assert!(outcome.findings().is_empty());
    assert_eq!(outcome.diagnostics().len(), 1);
    assert_eq!(outcome.diagnostics()[0].validator(), "docs");
    assert_eq!(outcome.diagnostics()[0].rule(), "docs-tool");
    assert_eq!(
        outcome.diagnostics()[0].message(),
        "no compile database covers src/lib.rs"
    );
}

/// A run that declined an item is observably different from a run that found
/// nothing. Both exit 0 with no finding, so without the diagnostic the
/// declined item reads exactly like a clean pass over the thing the rule
/// refused to judge.
#[test]
fn a_run_that_declines_an_item_differs_from_a_run_that_found_nothing() {
    let repo = tempfile::tempdir().unwrap();
    let clean = run_of("true", ToolScope::Files, &["src/lib.rs"]);
    let declining = run_of(
        r#"printf 'sah-diagnostic: no compile database covers src/lib.rs\n' >&2"#,
        ToolScope::Files,
        &["src/lib.rs"],
    );

    let found_nothing = execute_tool_runs(&[clean], repo.path(), None);
    let declined_one = execute_tool_runs(&[declining], repo.path(), None);

    assert!(found_nothing.findings().is_empty());
    assert!(declined_one.findings().is_empty());
    assert!(found_nothing.errors().is_empty());
    assert!(declined_one.errors().is_empty());
    assert_ne!(
        found_nothing, declined_one,
        "a declined item must not read as a clean pass"
    );
    assert!(found_nothing.diagnostics().is_empty());
    assert_eq!(declined_one.diagnostics().len(), 1);
}

/// Unmarked stderr is the tool's own chatter — progress, a deprecation notice,
/// a lock wait — and never a statement to the author. Three shipped rules
/// forward a linter's raw stderr on the success path, so reading every byte as
/// a diagnostic makes the report block a log dump.
#[test]
fn execute_reads_no_diagnostic_from_unmarked_stderr() {
    let repo = tempfile::tempdir().unwrap();
    let script = r#"printf 'Linting Swift files at paths\nDone linting!\n' >&2"#;
    let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.errors().is_empty());
    assert!(outcome.diagnostics().is_empty());
}

/// A `scope: workspace` run keeps only the findings in its matched files, and
/// a matched file is the changed-file list through the rule's OWN globs. The
/// natural anchor for a diagnostic is a manifest or a lint configuration, and
/// a rule that lints source never declares a glob for one, so a diagnostic
/// cannot pass through that filter at all.
#[test]
fn a_workspace_scope_diagnostic_survives_the_matched_file_filter() {
    let repo = tempfile::tempdir().unwrap();
    let script = r#"printf 'sah-diagnostic: tsconfig.json names no include list\n' >&2"#;
    let run = run_of(script, ToolScope::Workspace, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert!(outcome.errors().is_empty());
    assert_eq!(outcome.diagnostics().len(), 1);
    assert_eq!(
        outcome.diagnostics()[0].message(),
        "tsconfig.json names no include list"
    );
}

/// A broken run reports a tool error and reads no findings, and a marked line
/// on the same stderr is part of the failure detail rather than a diagnostic:
/// the run judged nothing, so it declined nothing.
#[test]
fn a_broken_run_reports_no_diagnostic() {
    let repo = tempfile::tempdir().unwrap();
    let script = r#"printf 'sah-diagnostic: the item was declined\n' >&2; exit 3"#;
    let run = run_of(script, ToolScope::Files, &["src/lib.rs"]);

    let outcome = execute_tool_runs(&[run], repo.path(), None);

    assert_eq!(outcome.errors().len(), 1);
    assert!(outcome.diagnostics().is_empty());
}

#[test]
fn a_tool_diagnostic_displays_the_rule_the_validator_and_the_message() {
    let diagnostic =
        ToolDiagnostic::for_test("docs", "docs-tool", "no compile database covers src/lib.rs");

    assert_eq!(
        diagnostic.to_string(),
        "tool rule `docs-tool` in validator `docs` declined an item: no compile database covers src/lib.rs"
    );
}
