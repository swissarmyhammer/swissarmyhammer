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
