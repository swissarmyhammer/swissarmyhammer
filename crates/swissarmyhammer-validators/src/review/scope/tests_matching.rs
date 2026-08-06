//! Stage-1 scope tests: observability, project-type and validator matching,
//! probe dedupe and evidence, change purpose, glob scope, ignore exclusion,
//! `read_working`/`read_at_ref` discipline, repo-root containment, and the
//! typed parameter pairs.

use super::*;

use swissarmyhammer_sem::git_types::FileStatus;

use model_embedding::mock::MockEmbedder;

use crate::review::probes::ProbeKind;
use crate::review::test_support::{
    body, dup_emb, index_conn, loader_with, ruleset, seed_call_edge, seed_chunk, seed_symbol,
    TestRepo, DIM,
};
use crate::validators::ValidatorLoader;

// ---- scope_review: observability tracing -----------------------------

#[tokio::test]
#[tracing_test::traced_test]
async fn scope_review_logs_the_selected_validators_and_their_rules() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

    let conn = index_conn();
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);

    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    // The selection summary names the matched validator and file count.
    assert!(logs_contain("review scope resolved"));
    assert!(logs_contain("validators=[\"deduplicate\"]"));
    // The per-validator detail line names the validator, its files, and its
    // declared probes/rules.
    assert!(logs_contain("validator=deduplicate"));
    assert!(logs_contain("deduplicate-rule"));
    assert!(logs_contain("duplicates"));
}

#[tokio::test]
#[tracing_test::traced_test]
async fn scope_review_logs_a_summary_even_when_nothing_matches() {
    let repo = TestRepo::new();
    repo.write("Cargo.lock", "# lockfile\n");
    repo.commit("initial");
    repo.write("Cargo.lock", "# lockfile\nupdated = true\n");

    let conn = index_conn();
    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let _work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    // The summary still fires, reporting zero matched validators.
    assert!(logs_contain("review scope resolved"));
    assert!(logs_contain("validators=[]"));
}

// ---- scope_review: project-type matching ------------------------------

/// A single-rule RuleSet matching `file_glob` files in workspaces with any
/// of the named detected project types.
fn project_typed_ruleset(name: &str, file_glob: &str, project_types: &[&str]) -> RuleSet {
    let mut rs = ruleset(name, file_glob, &[]);
    rs.manifest
        .match_criteria
        .as_mut()
        .expect("the ruleset fixture always carries match criteria")
        .project_types = project_types.iter().map(|t| t.to_string()).collect();
    rs
}

#[tokio::test]
async fn scope_review_resolves_workspace_project_types_for_matching() {
    let repo = TestRepo::new();
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    );
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    repo.write("src/lib.rs", "fn placeholder() {}\n\nfn added() {}\n");

    let conn = index_conn();
    let embedder = MockEmbedder::new(DIM);

    let mut loader = ValidatorLoader::new();
    // The repo carries a Cargo.toml, so the rust-keyed validator applies.
    loader.add_builtin_ruleset(project_typed_ruleset("rust-keyed", "*.rs", &["rust"]));
    // No python markers exist, so the python-keyed validator does not.
    loader.add_builtin_ruleset(project_typed_ruleset("python-keyed", "*.rs", &["python"]));

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let names: Vec<&str> = work
        .validators
        .iter()
        .map(|v| v.validator_name.as_str())
        .collect();
    assert!(
        names.contains(&"rust-keyed"),
        "a rust-keyed validator must match in a rust workspace, got {names:?}"
    );
    assert!(
        !names.contains(&"python-keyed"),
        "a python-keyed validator must not match in a non-python workspace, got {names:?}"
    );
}

// ---- scope_review: probe dedupe --------------------------------------

#[tokio::test]
async fn two_validators_share_one_probe_run_for_the_same_file() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

    let conn = index_conn();
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

    // Baseline: ONE validator declaring `duplicates` drives the embedder a
    // fixed number of times for this change set. The embedder call count is
    // the probe runner's observable execution count — a re-run repeats the
    // changed-set embedding work.
    let baseline_embedder = MockEmbedder::new(DIM);
    let single = loader_with("dedupe-a", "*.rs", &["duplicates"]);
    scope_review(
        Scope::Working,
        repo.path(),
        &single,
        &conn,
        &baseline_embedder,
        None,
    )
    .await
    .unwrap();
    let baseline = baseline_embedder.call_count();
    assert!(baseline > 0, "the duplicates probe must drive the embedder");

    // Two validators, both declaring `duplicates`, both matching *.rs.
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset("dedupe-a", "*.rs", &["duplicates"]));
    loader.add_builtin_ruleset(ruleset("dedupe-b", "*.rs", &["duplicates"]));
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    // Execution count: the shared (file, probe) run embeds exactly as often
    // as the single-validator baseline — a per-validator re-run would
    // multiply it.
    assert_eq!(
        embedder.call_count(),
        baseline,
        "two validators declaring the same probe must not re-run it"
    );

    let results_for = |name: &str| -> Vec<ProbeResult> {
        work.validators
            .iter()
            .find(|v| v.validator_name == name)
            .and_then(|v| v.files.iter().find(|f| f.path == "src/lib.rs"))
            .map(|f| f.probe_results.clone())
            .unwrap_or_default()
    };

    // Secondary check: the single shared run's result fans out to both
    // validators byte-for-byte.
    let a = results_for("dedupe-a");
    let b = results_for("dedupe-b");
    assert!(!a.is_empty(), "validator A should have probe results");
    assert_eq!(
        a, b,
        "both validators must receive the identical shared (file, probe) result"
    );
}

// ---- scope_review: symbol-targeted probes reach the work-list --------

#[tokio::test]
async fn symbol_targeted_probes_attach_to_the_file_bearing_the_symbol() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    // The working-tree change adds `compute` to src/lib.rs.
    let added = body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{added}\n"));

    let conn = index_conn();
    // `callers` evidence: an indexed inbound caller of `compute`.
    seed_symbol(&conn, "callee-1", "compute", "src/lib.rs");
    seed_symbol(&conn, "caller-1", "uses_compute", "src/caller.rs");
    seed_call_edge(&conn, "caller-1", "callee-1", "src/caller.rs", "src/lib.rs");
    // `similar` evidence: a reuse candidate in another file with the same
    // embedding as the mock embedder's constant query vector.
    let query_vec = vec![0.1_f32; DIM];
    seed_chunk(&conn, "src/util.rs", "existing_util", &added, &query_vec);

    // One validator declaring BOTH symbol-targeted probes on the .rs file.
    let loader = loader_with("reuse", "*.rs", &["callers", "similar"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let validator = work
        .validators
        .iter()
        .find(|v| v.validator_name == "reuse")
        .expect("the reuse validator matched the .rs change");
    let file = validator
        .files
        .iter()
        .find(|f| f.path == "src/lib.rs")
        .expect("the changed file appears under the validator");

    // The `callers` result (target = symbol name `compute`) must reach the
    // file whose changed entity bears that name.
    let callers = file
        .probe_results
        .iter()
        .find(|r| r.name == "callers")
        .expect("callers result attaches to the file bearing `compute`");
    assert_eq!(callers.target, "compute");
    assert!(
        callers
            .rows
            .iter()
            .any(|row| row.file_path == "src/caller.rs"),
        "callers should carry the inbound caller, got: {:?}",
        callers.rows
    );

    // The `similar` result (also symbol-targeted) must reach the same file.
    let similar = file
        .probe_results
        .iter()
        .find(|r| r.name == "similar")
        .expect("similar result attaches to the file bearing `compute`");
    assert_eq!(similar.target, "compute");
    assert!(
        similar
            .rows
            .iter()
            .any(|row| row.file_path == "src/util.rs"),
        "similar should carry the reuse candidate, got: {:?}",
        similar.rows
    );
}

// ---- scope_review: complexity probe evidence -------------------------

/// A file with one function far over the nesting gate and one well under it.
const MIXED_COMPLEXITY_SOURCE: &str = r#"fn deep(a: bool, b: bool, items: &[u8]) -> u8 {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    return 1;
                }
            }
        }
    }
    0
}

fn shallow(a: Option<u8>) -> u8 {
    match a {
        Some(v) => v,
        None => 0,
    }
}
"#;

/// Drive the real `scope_review` over a repo holding `source`, and return the
/// `complexity` probe evidence the pipeline attached to the file.
async fn complexity_evidence_for(source: &str) -> Vec<ProbeResult> {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    repo.write("src/lib.rs", source);

    let conn = index_conn();
    let loader = loader_with("complexity", "*.rs", &["complexity"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .expect("the working scope resolves");

    work.validators
        .iter()
        .find(|v| v.validator_name == "complexity")
        .expect("the complexity validator matched the changed .rs file")
        .files
        .iter()
        .find(|f| f.path == "src/lib.rs")
        .expect("the changed file is work")
        .probe_results
        .clone()
}

#[tokio::test]
async fn scope_review_attaches_computed_complexity_evidence_to_the_file() {
    // The production path: a real repo, the real loader, the real probe
    // runner. The agent must receive the measured numbers, not be asked for
    // them.
    let evidence = complexity_evidence_for(MIXED_COMPLEXITY_SOURCE).await;

    let result = evidence
        .iter()
        .find(|r| r.name == "complexity")
        .expect("the complexity probe result reaches the work item");
    let symbols: Vec<&str> = result
        .rows
        .iter()
        .filter_map(|row| row.symbol.as_deref())
        .collect();

    assert_eq!(
        symbols,
        vec!["deep"],
        "only the over-gate function is listed, got: {:?}",
        result.rows
    );
    assert!(
        result.rows[0]
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("max condition-nesting depth 4 (gate 4)")),
        "the row carries the measured depth and its gate, got: {:?}",
        result.rows[0].detail
    );
}

#[tokio::test]
async fn scope_review_reports_an_empty_complexity_result_for_a_simple_file() {
    // An empty result is the deterministic fact the verify guard refutes a
    // complexity claim with. It must survive the whole pipeline, not be
    // dropped as "no evidence".
    let evidence = complexity_evidence_for(
            "fn shallow(a: Option<u8>) -> u8 {\n    match a {\n        Some(v) => v,\n        None => 0,\n    }\n}\n",
        )
        .await;

    let result = evidence
        .iter()
        .find(|r| r.name == "complexity")
        .expect("a simple file still gets a complexity result");
    assert!(
        result.rows.is_empty(),
        "no function is over a gate, got: {:?}",
        result.rows
    );
}

// ---- scope_review: change_purpose from commit message (sha) ----------

#[tokio::test]
async fn sha_scope_sets_change_purpose_from_commit_message() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn base() {}\n");
    repo.commit("base commit");
    repo.write("src/lib.rs", "fn base() {}\n\nfn added() {}\n");
    repo.commit("Add the added function for review");

    let conn = index_conn();
    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Sha("HEAD~1..HEAD".to_string()),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    assert!(
        work.change_purpose
            .contains("Add the added function for review"),
        "change_purpose should be the commit message, got: {}",
        work.change_purpose
    );
}

// ---- scope_review: glob whole-content, no diff -----------------------

#[tokio::test]
async fn glob_scope_returns_matched_files_as_whole_content_work() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", &format!("{}\n", body("whole_file_fn")));
    repo.commit("initial");

    let conn = index_conn();
    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Glob("src/*.rs".to_string()),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let validator = work
        .validators
        .iter()
        .find(|v| v.validator_name == "deduplicate")
        .expect("validator matched the globbed .rs file");
    let file = validator
        .files
        .iter()
        .find(|f| f.path == "src/lib.rs")
        .expect("the globbed file is whole-content work");
    // Whole-content (no before side): the file's entity diffs as all-added.
    assert!(
        file.semantic_diff
            .iter()
            .any(|c| c.entity_name == "whole_file_fn"),
        "whole-content work should surface the file's entities as added"
    );
    assert!(
        file.source_slice.contains("whole_file_fn"),
        "the bounded slice should carry the whole-content function"
    );
}

// ---- scope_review: unmatched file yields no work ---------------------

#[tokio::test]
async fn unmatched_lock_file_yields_no_validator_work() {
    let repo = TestRepo::new();
    repo.write("Cargo.lock", "# lockfile\n");
    repo.commit("initial");
    repo.write("Cargo.lock", "# lockfile\nupdated = true\n");

    let conn = index_conn();
    // The only validator matches *.rs, never a .lock file.
    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    assert!(
        work.validators.is_empty(),
        "a changed .lock with no matching validator yields no work, got: {:?}",
        work.validators
    );
}

// ---- scope_review: .reviewignore + .gitignore exclusion --------------

/// The distinct file paths any validator carries in a resolved work-list.
fn work_paths(work: &WorkList) -> Vec<String> {
    work.validators
        .iter()
        .flat_map(|v| v.files.iter().map(|f| f.path.clone()))
        .collect()
}

/// A tracked, edited `.kanban/board.md` — the finish-loop churn the default
/// `.reviewignore` exists to suppress — must be dropped from the working
/// scope even though a tracked non-code modification would otherwise stay.
#[tokio::test]
async fn working_scope_excludes_tracked_kanban_board_edits() {
    let repo = TestRepo::new();
    repo.write(".kanban/board.md", "# board\n");
    repo.write("src/lib.rs", "fn base() {}\n");
    repo.commit("initial");
    // A finish-loop comment edits the tracked board file.
    repo.write(".kanban/board.md", "# board\n- new comment\n");

    let conn = index_conn();
    // A match-everything validator would otherwise pull the board into scope.
    let loader = loader_with("everything", "*", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let paths = work_paths(&work);
    assert!(
        !paths.iter().any(|p| p.starts_with(".kanban/")),
        "the auto-ignored .kanban board must not enter working scope, got: {paths:?}"
    );
}

/// A committed, tracked file matched by the repo's `.gitignore` must be
/// excluded from a `Scope::Sha` range while a real source edit in the same
/// range stays — gitignored artifacts are not source even when tracked.
#[tokio::test]
async fn sha_scope_excludes_a_gitignored_committed_file() {
    let repo = TestRepo::new();
    // The generated file is committed BEFORE the ignore exists, so it is
    // tracked; git ignores only untracked paths, so it stays tracked after.
    repo.write("src/lib.rs", "fn base() {}\n");
    repo.write("src/schema.gen.rs", "fn generated() {}\n");
    repo.commit("initial");
    repo.write(".gitignore", "*.gen.rs\n");
    repo.write("src/lib.rs", "fn base() {}\n\nfn added() {}\n");
    repo.write("src/schema.gen.rs", "fn generated() {}\n\nfn more() {}\n");
    repo.commit("second");

    let conn = index_conn();
    let loader = loader_with("everything", "*", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Sha("HEAD~1..HEAD".to_string()),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let paths = work_paths(&work);
    assert!(
        !paths.iter().any(|p| p.ends_with(".gen.rs")),
        "a gitignored committed file must be excluded from sha scope, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p == "src/lib.rs"),
        "the real source edit must stay in sha scope, got: {paths:?}"
    );
}

/// A `Scope::Glob` result is filtered through the ignore matcher too: a
/// user's `.reviewignore` directory pattern drops vendored files a broad
/// glob would otherwise sweep in.
#[tokio::test]
async fn glob_scope_excludes_reviewignored_files() {
    let repo = TestRepo::new();
    repo.write(".reviewignore", "src/vendor/\n");
    repo.write("src/lib.rs", &format!("{}\n", body("real")));
    repo.write("src/vendor/dep.rs", &format!("{}\n", body("vendored")));
    repo.commit("initial");

    let conn = index_conn();
    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Glob("src/*.rs".to_string()),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let paths = work_paths(&work);
    assert!(
        paths.iter().any(|p| p == "src/lib.rs"),
        "the real source file must stay in glob scope, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("vendor")),
        "the reviewignored vendored file must be excluded from glob scope, got: {paths:?}"
    );
}

/// A `!` negation in `.reviewignore` re-includes a file a broader directory
/// pattern excluded — full gitignore semantics carry through the scope filter.
#[tokio::test]
async fn working_scope_negation_reincludes_a_broadly_excluded_file() {
    let repo = TestRepo::new();
    repo.write(".reviewignore", "src/gen/\n!src/gen/keep.rs\n");
    repo.write("src/lib.rs", "fn base() {}\n");
    repo.commit("initial");
    // Two brand-new untracked source files under the excluded directory.
    repo.write("src/gen/noise.rs", &format!("{}\n", body("noise")));
    repo.write("src/gen/keep.rs", &format!("{}\n", body("keep")));

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let paths = work_paths(&work);
    assert!(
        paths.iter().any(|p| p == "src/gen/keep.rs"),
        "the `!` negation must re-include keep.rs, got: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p == "src/gen/noise.rs"),
        "the broad directory pattern must still exclude noise.rs, got: {paths:?}"
    );
}

/// The first review of a repo without a `.reviewignore` auto-generates the
/// default (with `.kanban/`); a second review preserves a user-edited file
/// byte-for-byte.
#[tokio::test]
async fn scope_review_autogenerates_reviewignore_then_preserves_edits() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn base() {}\n");
    repo.commit("initial");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();
    let generated = std::fs::read_to_string(repo.path().join(".reviewignore"))
        .expect("the first review must auto-generate .reviewignore");
    assert!(
        generated.contains(".kanban/"),
        "the generated default must ignore .kanban/, got:\n{generated}"
    );

    // A user edits it; a second review must leave it byte-identical.
    let edited = "# custom rules\nvendor/\n";
    std::fs::write(repo.path().join(".reviewignore"), edited).unwrap();
    scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();
    let after = std::fs::read_to_string(repo.path().join(".reviewignore")).unwrap();
    assert_eq!(
        after, edited,
        "a second review must not rewrite the user's .reviewignore"
    );
}

/// A `Scope::File` naming an ignored path resolves to an empty scope — no
/// findings, no error — the same exclusion the other scopes apply.
#[tokio::test]
async fn file_scope_of_an_ignored_path_yields_empty_scope() {
    let repo = TestRepo::new();
    repo.write(".reviewignore", ".kanban/\n");
    repo.write(".kanban/board.md", "# board\n");
    repo.commit("initial");

    let conn = index_conn();
    let loader = loader_with("everything", "*", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::File(".kanban/board.md".to_string()),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    assert!(
        work.validators.is_empty(),
        "an explicitly named ignored file must resolve to an empty scope, got: {:?}",
        work.validators
    );
}

// ---- read_working / read_at_ref error discipline --------------------

/// A non-UTF8 byte sequence: a lone continuation byte that is invalid as
/// the start of a UTF-8 sequence, so `read_to_string` / `from_utf8` reject
/// it. Models a binary/unreadable tracked blob.
const BINARY_BYTES: &[u8] = &[0xff, 0xfe, 0x00, 0x01];

/// An absent working-tree path resolves to `Ok(None)` — the intended
/// deletion signal — not an error.
#[test]
fn read_working_maps_an_absent_path_to_ok_none() {
    let repo = TestRepo::new();
    let got = read_working(repo.path(), "src/does_not_exist.rs")
        .expect("an absent path must not be an error");
    assert_eq!(got, None, "an absent path is the deletion signal: Ok(None)");
}

/// A present, readable working-tree path resolves to `Ok(Some(content))`.
#[test]
fn read_working_reads_a_present_file() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn compute() {}\n");
    let got = read_working(repo.path(), "src/lib.rs").expect("a readable file must succeed");
    assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));
}

/// A binary/non-UTF8 working-tree file is a genuine read failure, NOT the
/// deletion signal — it must surface as an error, never as `Ok(None)`.
#[test]
fn read_working_propagates_a_non_utf8_file_as_an_error() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("blob.bin"), BINARY_BYTES).unwrap();

    let err = read_working(repo.path(), "blob.bin")
        .expect_err("a non-UTF8 file must not be silently treated as absent");
    match err {
        AvpError::Context(msg) => {
            assert!(
                msg.contains("blob.bin"),
                "the error must name the path: {msg}"
            );
        }
        other => panic!("expected AvpError::Context, got: {other:?}"),
    }
}

// ---- read_working containment: reject paths escaping the repo root --

/// A `..` traversal path must be rejected — never read — with a typed
/// [`AvpError::Validator`] naming the full offending path. Without the
/// containment guard, `repo_path.join("../x")` climbs out of the repo and
/// reads an arbitrary file into the review agent's context.
#[test]
fn read_working_rejects_a_parent_traversal_path() {
    let repo = TestRepo::new();
    // A secret file just ABOVE the repo dir, named uniquely so a parallel
    // test or a leftover can never make this assertion flaky.
    let marker = format!(
        "outside_secret_{}.txt",
        repo.path().file_name().unwrap().to_string_lossy()
    );
    let outside = repo.path().parent().unwrap().join(&marker);
    std::fs::write(&outside, "TOP SECRET").unwrap();

    let got = read_working(repo.path(), &format!("../{marker}"));
    let _ = std::fs::remove_file(&outside);

    match got {
        Err(AvpError::Validator { message, .. }) => {
            assert!(
                message.contains(&format!("../{marker}")),
                "the error must carry the full offending path: {message}"
            );
            assert!(
                message.contains("escapes the repository root"),
                "the error must explain the escape: {message}"
            );
        }
        other => panic!("a `..` traversal must be a Validator error, got: {other:?}"),
    }
}

/// An absolute input path must be rejected outright: `Path::join` with an
/// absolute argument REPLACES the repo root entirely, so a naive join reads
/// straight from the given absolute path (e.g. `/etc/passwd`).
#[test]
fn read_working_rejects_an_absolute_path() {
    let repo = TestRepo::new();
    let mut secret = std::env::temp_dir();
    secret.push(format!(
        "abs_secret_{}.txt",
        repo.path().file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&secret, "TOP SECRET").unwrap();

    let got = read_working(repo.path(), secret.to_str().unwrap());
    let _ = std::fs::remove_file(&secret);

    match got {
        Err(AvpError::Validator { message, .. }) => {
            assert!(
                message.contains(secret.to_str().unwrap()),
                "the error must carry the full offending path: {message}"
            );
        }
        other => panic!("an absolute path must be a Validator error, got: {other:?}"),
    }
}

/// A repo-internal symlink whose target lies OUTSIDE the repo root must be
/// rejected: canonicalization resolves the link to its real path, which the
/// containment check finds is not under the root.
#[cfg(unix)]
#[test]
fn read_working_rejects_a_symlink_escaping_the_repo_root() {
    let repo = TestRepo::new();
    let outside_dir = tempfile::TempDir::new().unwrap();
    let secret = outside_dir.path().join("secret.txt");
    std::fs::write(&secret, "TOP SECRET").unwrap();
    // A link living INSIDE the repo but pointing at the outside secret.
    std::os::unix::fs::symlink(&secret, repo.path().join("link.txt")).unwrap();

    let got = read_working(repo.path(), "link.txt");
    match got {
        Err(AvpError::Validator { message, .. }) => {
            assert!(
                message.contains("link.txt"),
                "the error must name the offending path: {message}"
            );
            assert!(
                message.contains("escapes the repository root"),
                "the error must explain the escape: {message}"
            );
        }
        other => panic!("an escaping symlink must be a Validator error, got: {other:?}"),
    }
}

/// A normal NESTED relative path under the repo root is read exactly as
/// before the containment guard — the guard must not reject legitimate
/// repo-relative targets.
#[test]
fn read_working_reads_a_nested_relative_path_unchanged() {
    let repo = TestRepo::new();
    repo.write("src/deep/nested.rs", "pub fn nested() {}\n");
    let got = read_working(repo.path(), "src/deep/nested.rs")
        .expect("a nested repo-relative file must read normally");
    assert_eq!(got.as_deref(), Some("pub fn nested() {}\n"));
}

/// A path absent at the requested ref resolves to `Ok(None)`.
#[test]
fn read_at_ref_maps_a_path_absent_at_ref_to_ok_none() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn compute() {}\n");
    repo.commit("initial");
    let git = open_repo(repo.path()).unwrap();

    let got = read_at_ref(
        &git,
        GitRefSpec::head(),
        FilePath::new("src/never_committed.rs"),
    )
    .expect("a path absent at the ref must not be an error");
    assert_eq!(got, None, "absent at the ref is Ok(None)");
}

/// A blob present at the ref resolves to `Ok(Some(content))`.
#[test]
fn read_at_ref_reads_a_committed_blob() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn compute() {}\n");
    repo.commit("initial");
    let git = open_repo(repo.path()).unwrap();

    let got = read_at_ref(&git, GitRefSpec::head(), FilePath::new("src/lib.rs"))
        .expect("a committed blob must succeed");
    assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));
}

/// The two halves of a `refspec:path` blob address are DISTINCT types, so a
/// call site cannot transpose them by accident — the compiler rejects it.
/// This pins the order the types carry: the refspec selects the revision,
/// the path selects the file within it, and the transposition addresses
/// nothing.
#[test]
fn read_at_ref_addresses_the_path_within_the_refspec_never_the_transposition() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn compute() {}\n");
    repo.commit("initial");
    let git = open_repo(repo.path()).unwrap();

    let got = read_at_ref(&git, GitRefSpec::head(), FilePath::new("src/lib.rs"))
        .expect("the path within the refspec must read");
    assert_eq!(got.as_deref(), Some("pub fn compute() {}\n"));

    // Swapping the halves yields the meaningless spec `src/lib.rs:HEAD`,
    // whose revision half resolves to nothing — the absent-at-ref signal,
    // never this file's content.
    let transposed = read_at_ref(&git, GitRefSpec::new("src/lib.rs"), FilePath::new("HEAD"));
    assert!(
        matches!(transposed, Ok(None)),
        "a transposed refspec/path addresses nothing: {transposed:?}"
    );
}

/// A binary/non-UTF8 blob committed at the ref is a genuine read failure,
/// NOT a missing-path signal — it must surface as an error so the file is
/// never silently diffed as wholly added/removed.
#[test]
fn read_at_ref_propagates_a_non_utf8_blob_as_an_error() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("blob.bin"), BINARY_BYTES).unwrap();
    repo.commit("add a binary blob");
    let git = open_repo(repo.path()).unwrap();

    let err = read_at_ref(&git, GitRefSpec::head(), FilePath::new("blob.bin"))
        .expect_err("a non-UTF8 blob must not be silently treated as absent");
    match err {
        AvpError::Context(msg) => {
            assert!(
                msg.contains("blob.bin"),
                "the error must name the path: {msg}"
            );
        }
        other => panic!("expected AvpError::Context, got: {other:?}"),
    }
}

// ---- typed parameter pairs: a transposition must not compile ----------

/// The two sides of a file's change are DISTINCT types ([`BeforeContent`]
/// and [`AfterContent`]) carried by one named-field [`FileVersions`], so no
/// call site can transpose them: writing
/// `FileVersions { before: AfterContent::new(a), after: BeforeContent::new(b) }`
/// is `error[E0308]: mismatched types`, never a silently INVERTED diff.
/// This pins the direction each side carries into the semantic differ.
#[test]
fn file_change_builder_records_the_before_side_as_before_never_the_transposition() {
    let mut builder = FileChangeBuilder::new();
    builder.push(
        FilePath::new("src/lib.rs"),
        FileVersions {
            before: BeforeContent::new(Some("fn old() {}\n".to_string())),
            after: AfterContent::new(Some("fn new() {}\n".to_string())),
        },
    );
    let resolved = builder.finish(vec!["src/lib.rs".to_string()], "purpose".to_string(), None);

    let change = &resolved.file_changes[0];
    assert_eq!(change.file_path, "src/lib.rs");
    assert_eq!(change.before_content.as_deref(), Some("fn old() {}\n"));
    assert_eq!(change.after_content.as_deref(), Some("fn new() {}\n"));
    assert_eq!(change.status, FileStatus::Modified);
    // Only the AFTER side is the file's current content.
    assert_eq!(
        resolved.after_content.get("src/lib.rs").map(String::as_str),
        Some("fn new() {}\n")
    );
}

/// A file absent on the before side is an ADDITION; one absent on the after
/// side is a DELETION. Transposing the two sides inverts exactly this pair
/// — a plausible-looking but wholly wrong diff — which is why the sides are
/// separate types.
#[test]
fn an_absent_before_side_is_an_addition_and_an_absent_after_side_a_deletion() {
    let mut builder = FileChangeBuilder::new();
    builder.push(
        FilePath::new("added.rs"),
        FileVersions {
            before: BeforeContent::absent(),
            after: AfterContent::new(Some("fn added() {}\n".to_string())),
        },
    );
    builder.push(
        FilePath::new("deleted.rs"),
        FileVersions {
            before: BeforeContent::new(Some("fn deleted() {}\n".to_string())),
            // A deleted file has no post-change content.
            after: AfterContent::new(None),
        },
    );
    let resolved = builder.finish(
        vec!["added.rs".to_string(), "deleted.rs".to_string()],
        "purpose".to_string(),
        None,
    );

    assert_eq!(resolved.file_changes[0].status, FileStatus::Added);
    assert_eq!(resolved.file_changes[1].status, FileStatus::Deleted);
    // A deleted file has no current content to inline.
    assert!(!resolved.after_content.contains_key("deleted.rs"));
}

/// A validator's rule names and probe names are DISTINCT types
/// ([`RuleNames`], [`ProbeNames`]), so [`ValidatorWork::new`] cannot take
/// them transposed — the compiler rejects it. This pins which list each
/// accessor reports.
#[test]
fn validator_work_names_its_rules_and_probes_never_the_transposition() {
    let validator = ValidatorWork::new(
        "dedup".to_string(),
        RuleNames::new(["dedup-rule".to_string()]),
        ProbeNames::new(["similar".to_string()]),
        vec![],
    );
    assert_eq!(validator.rules(), ["dedup-rule".to_string()]);
    assert_eq!(validator.probes(), ["similar".to_string()]);
}

/// [`select_probe_results`] filters by PROBE NAME and attaches by changed
/// SYMBOL. The two lists are distinct types ([`ProbeNames`] against a plain
/// symbol slice), so they cannot be transposed; this pins which list each
/// filter reads.
#[test]
fn select_probe_results_filters_by_probe_name_and_attaches_by_changed_symbol() {
    let probe = |name: &str, target: &str| ProbeResult {
        name: name.to_string(),
        kind: ProbeKind::Fact,
        target: target.to_string(),
        rows: vec![],
    };
    let cache = vec![probe("similar", "compute"), probe("callers", "compute")];
    let declared = ProbeNames::new(["similar".to_string()]);
    let changed = vec!["compute".to_string()];

    let got = select_probe_results(&cache, "src/lib.rs", &changed, &declared);
    let names: Vec<&str> = got.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["similar"],
        "only the validator's DECLARED probe is selected"
    );

    // A symbol-targeted probe never attaches to a file whose changed
    // symbols do not name that target.
    let got = select_probe_results(&cache, "src/lib.rs", &[], &declared);
    assert!(
        got.is_empty(),
        "an unrelated symbol target attaches to no file: {got:?}"
    );
}

/// ^t7f5fqf: the batch-scoped `<changed-set>` `duplicates` comparison must
/// NEVER attach to an individual file's `probe_results` — it used to
/// match every file unconditionally, which multiplied its (potentially
/// ~1.43 MB) bytes by the batch's file count. It is shared evidence,
/// selected once per validator by [`select_shared_probe_results`] instead.
#[test]
fn probe_result_for_file_never_matches_the_shared_changed_set_result() {
    let changed_set = ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![],
    };
    assert!(
        !probe_result_for_file(&changed_set, "src/lib.rs", &[]),
        "the shared changed-set result must not attach to any individual file"
    );
    assert!(
        !probe_result_for_file(&changed_set, "src/lib.rs", &["lib".to_string()]),
        "a changed symbol name must not accidentally match the <changed-set> \
             sentinel target either"
    );
}

/// [`select_shared_probe_results`] is the sole path a validator's
/// batch-scoped shared evidence reaches [`ValidatorWork`] through: it
/// selects only the DECLARED probe's `<changed-set>` result, never a
/// per-file one, and never an undeclared probe's shared result.
#[test]
fn select_shared_probe_results_selects_only_the_declared_probes_changed_set_result() {
    let per_file = ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "src/a.rs".to_string(),
        rows: vec![],
    };
    let shared = ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![],
    };
    let undeclared_shared = ProbeResult {
        name: "similar".to_string(),
        kind: ProbeKind::Candidate,
        target: "<changed-set>".to_string(),
        rows: vec![],
    };
    let cache = vec![per_file, shared.clone(), undeclared_shared];
    let declared = ProbeNames::new(["duplicates".to_string()]);

    let got = select_shared_probe_results(&cache, &declared);
    assert_eq!(
        got,
        vec![shared],
        "only the declared probe's <changed-set> result is selected, never a \
             per-file result and never an undeclared probe's shared result"
    );
}

/// [`WorkList::shared_probe_results`] dedups the shared evidence across
/// validators by `(probe name, target)`, so two validators that both
/// declare `duplicates` do not double the shared block in the rendered
/// prime.
#[test]
fn work_list_shared_probe_results_dedups_across_validators() {
    let shared = ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![],
    };
    let v1 = ValidatorWork::new(
        "one".to_string(),
        RuleNames::default(),
        ProbeNames::new(["duplicates".to_string()]),
        vec![],
    )
    .with_shared_probe_results(vec![shared.clone()]);
    let v2 = ValidatorWork::new(
        "two".to_string(),
        RuleNames::default(),
        ProbeNames::new(["duplicates".to_string()]),
        vec![],
    )
    .with_shared_probe_results(vec![shared.clone()]);

    let work = WorkList::new("purpose".to_string(), vec![v1, v2]);

    assert_eq!(
        work.shared_probe_results(),
        vec![shared],
        "the identical shared result declared by two validators is carried once"
    );
}
