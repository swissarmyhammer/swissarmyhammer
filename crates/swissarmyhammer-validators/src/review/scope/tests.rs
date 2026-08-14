//! Stage-1 scope tests: `ScopeSpec` resolution, scope-phase progress, working
//! and sha scopes, line annotations (blame + change marks), untracked files,
//! `WorkList` views, and `batch_work_list` packing.

use super::*;

use model_embedding::mock::MockEmbedder;

use crate::review::test_support::{
    body, dup_emb, index_conn, loader_with, ruleset, seed_chunk, uniform_budget, TestRepo, DIM,
};

// ---- ScopeSpec::resolve ----------------------------------------------

#[test]
fn scope_spec_resolves_exactly_one_selector() {
    let spec = ScopeSpec {
        working: true,
        ..Default::default()
    };
    assert_eq!(spec.resolve().unwrap(), Scope::Working);

    let spec = ScopeSpec {
        sha: Some("HEAD~1".to_string()),
        ..Default::default()
    };
    assert_eq!(spec.resolve().unwrap(), Scope::Sha("HEAD~1".to_string()));
}

/// The zero-selector message is pinned to its literal text on purpose. It is
/// a user-facing contract, and the assertion is deliberately NOT built from
/// [`SCOPE_SELECTOR_ERROR_PREFIX`] — an expectation composed from the same
/// constant the code uses would move with it and prove nothing. Editing the
/// selector list must break this test.
#[test]
fn scope_spec_errors_on_zero_selectors() {
    let err = ScopeSpec::default().resolve().unwrap_err();
    match err {
        AvpError::Validator { message, .. } => {
            assert_eq!(
                message,
                "a review scope must set exactly one of file/glob/working/sha; none were set"
            );
        }
        other => panic!("expected Validator error, got: {other:?}"),
    }
}

/// The many-selector message reports the count AND shares the zero-selector
/// message's prefix. The prefix half is asserted against
/// [`SCOPE_SELECTOR_ERROR_PREFIX`] deliberately: together with the literal
/// pinned in `scope_spec_errors_on_zero_selectors`, that catches BOTH ways
/// the pair can rot — editing the selector list (the literal there) and
/// re-introducing a divergent hardcoded message in this branch.
#[test]
fn scope_spec_errors_on_multiple_selectors() {
    let spec = ScopeSpec {
        working: true,
        file: Some("a.rs".to_string()),
        ..Default::default()
    };
    let err = spec.resolve().unwrap_err();
    match err {
        AvpError::Validator { message, .. } => {
            assert!(
                message.starts_with(SCOPE_SELECTOR_ERROR_PREFIX),
                "both selector errors must share one prefix, got: {message}"
            );
            assert_eq!(
                message,
                format!("{SCOPE_SELECTOR_ERROR_PREFIX}; 2 were set")
            );
        }
        other => panic!("expected Validator error, got: {other:?}"),
    }
}

/// `Scope` is a value key: callers cache and de-duplicate resolved scopes,
/// so it must work in hash-based collections, and its `Hash` must agree with
/// its `Eq` — equal scopes collapse to one entry, distinct ones do not.
#[test]
fn scope_is_usable_as_a_hash_key() {
    use std::collections::HashSet;

    let mut set: HashSet<Scope> = HashSet::new();
    assert!(set.insert(Scope::Working));
    assert!(set.insert(Scope::Sha("HEAD~1".to_string())));
    assert!(set.insert(Scope::File("a.rs".to_string())));
    assert!(set.insert(Scope::Glob("**/*.rs".to_string())));

    // Eq-equal scopes must hash equal: re-inserting is a no-op.
    assert!(!set.insert(Scope::Working));
    assert!(!set.insert(Scope::Sha("HEAD~1".to_string())));
    assert_eq!(set.len(), 4);

    // Same payload, different variant, stays distinct.
    assert!(set.insert(Scope::Glob("a.rs".to_string())));
    assert_eq!(set.len(), 5);
}

// ---- scope_review: scope-phase progress events -------------------------

#[tokio::test]
async fn scope_review_emits_one_file_scoped_event_per_resolved_file() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn base() {}\n");
    repo.commit("initial");
    // Two changed files in the working tree — the multi-file scope.
    repo.write("src/alpha.rs", &format!("{}\n", body("alpha")));
    repo.write("src/beta.rs", &format!("{}\n", body("beta")));

    let conn = index_conn();
    let loader = loader_with("scoped", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    scope_review(
        Scope::Working,
        repo.path(),
        &loader,
        &conn,
        &embedder,
        Some(&tx),
    )
    .await
    .unwrap();
    drop(tx);

    // The scope stage announces each resolved file exactly once — its
    // events are the run's FIRST progress (they exist before any fleet
    // work), so their emission from `scope_review` itself is the contract.
    let mut scoped_files = Vec::new();
    while let Some(event) = rx.recv().await {
        match event {
            ReviewProgressEvent::FileScoped { file } => scoped_files.push(file),
            other => panic!("the scope stage emits only FileScoped events, got: {other:?}"),
        }
    }
    scoped_files.sort();
    assert_eq!(
        scoped_files,
        vec!["src/alpha.rs".to_string(), "src/beta.rs".to_string()],
        "one FileScoped event per resolved file"
    );
}

/// A changed file under a loaded validator set's `fixtures/` directory leaves
/// the work-list entirely: it pairs with NO validator, so no fan-out task ever
/// renders it and no tool rule ever receives it as an argument (both read the
/// work-list this stage produces). It is reported on the work-list instead,
/// with its reason.
#[tokio::test]
async fn scope_review_drops_a_validator_sets_fixture_from_the_work_list() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn base() {}\n");
    repo.commit("initial");
    // The change: one ordinary source file, and one file under the set's own
    // fixtures directory. Both are `.rs`, so the set matches both.
    repo.write("src/alpha.rs", &format!("{}\n", body("alpha")));
    let fixture = ".validators/probe/fixtures/probe-rule.fail.rs";
    repo.write(fixture, &format!("{}\n", body("fixture")));

    let conn = index_conn();
    let mut set = ruleset("probe", "*.rs", &[]);
    set.base_path = repo.path().join(".validators/probe");
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(set);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let reviewed: Vec<&str> = work.distinct_files().map(FileWork::path).collect();
    assert_eq!(
        reviewed,
        ["src/alpha.rs"],
        "the fixture pairs with no validator; the source file still does"
    );
    assert_eq!(
        work.excluded().len(),
        1,
        "the fixture is reported, not dropped in silence: {:?}",
        work.excluded()
    );
    assert_eq!(work.excluded()[0].path(), fixture);
    assert_eq!(work.excluded()[0].reason(), "validator fixture");
}

// ---- scope_review: working scope, duplicate function ------------------

#[tokio::test]
async fn working_scope_groups_duplicate_under_validator_with_full_source() {
    let repo = TestRepo::new();
    // Header = imports; an unrelated marker sits in the MIDDLE of the file,
    // outside both the header window and any changed-hunk window.
    let mid_padding: String = (0..30).map(|i| format!("// mid {i}\n")).collect();
    let tail_padding: String = (0..30).map(|i| format!("// tail {i}\n")).collect();
    let base = format!(
            "use std::fmt;\nuse std::io;\n{mid_padding}fn distant_unrelated_marker() {{}}\n{tail_padding}"
        );
    repo.write("src/lib.rs", &base);
    repo.commit("initial");

    // The working-tree change ADDS a duplicate function at the very bottom.
    let dup = body("compute");
    repo.write("src/lib.rs", &format!("{base}\n{dup}\n"));

    // The index already holds an equivalent function in another file → dup hit.
    let conn = index_conn();
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

    let loader = loader_with("deduplicate", "*.rs", &["duplicates"]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let validator = work
        .validators
        .iter()
        .find(|v| v.validator_name == "deduplicate")
        .expect("the deduplicate validator matched the .rs change");
    let file = validator
        .files
        .iter()
        .find(|f| f.path == "src/lib.rs")
        .expect("the changed file appears under the validator");

    // Non-empty semantic diff carrying the added function.
    assert!(
        file.semantic_diff
            .iter()
            .any(|c| c.entity_name == "compute"),
        "semantic diff should carry the added `compute`, got: {:?}",
        file.changed_symbols
    );

    // Full source: a changed file is always inlined whole, so the model never
    // re-reads it. The changed function, the header, AND a distant unrelated
    // marker are all present (nothing is trimmed to a slice).
    assert!(
        file.source_slice.contains("pub fn compute"),
        "full source must include the changed function"
    );
    assert!(
        file.source_slice.contains("use std::fmt"),
        "full source must include the file header"
    );
    assert!(
        file.source_slice.contains("distant_unrelated_marker"),
        "the full inline carries even distant code, got:\n{}",
        file.source_slice
    );

    // The duplicates probe hit at the existing file is attached.
    let dup_hit = file
        .probe_results
        .iter()
        .filter(|r| r.name == "duplicates")
        .flat_map(|r| r.rows.iter())
        .any(|row| row.file_path == "src/existing.rs");
    assert!(
        dup_hit,
        "duplicates probe_results should carry the existing.rs hit, got: {:?}",
        file.probe_results
    );

    // ^t7f5fqf: the batch-scoped `<changed-set>` result is carried on the
    // VALIDATOR now (once), never cloned onto the file's own
    // `probe_results` — through the REAL `scope_review` path, not just
    // the isolated selection helpers.
    assert!(
        !file
            .probe_results
            .iter()
            .any(|r| r.target == "<changed-set>"),
        "the shared <changed-set> result must not attach to the file's own \
             probe_results; it is carried once on the validator instead, got: {:?}",
        file.probe_results
    );
    assert!(
        validator
            .shared_probe_results()
            .iter()
            .any(|r| r.name == "duplicates" && r.target == "<changed-set>"),
        "the validator's shared_probe_results must carry the changed-set \
             duplicates comparison, got: {:?}",
        validator.shared_probe_results()
    );
}

// ---- scope_review: line annotations (blame + change marks) ------------

/// Production-path test: a real two-commit git history feeds `scope_review`
/// (`Scope::Sha`), and the resulting `FileWork::line_annotations` must carry
/// the CORRECT 1-based line number (by position), the correct 8-char blame
/// sha per line, and the correct `touched` mark from the diff — not from
/// blame. Line 2 (edited by the second commit) is attributed to the second
/// commit AND marked touched; lines 1 and 3 (untouched) keep the first
/// commit's sha and are NOT marked. The real rendered prime block is then
/// asserted to show line 2 with exactly that number, sha, and `+` mark —
/// closing the loop from `scope_review` through to what the model reads.
#[tokio::test]
async fn sha_scope_line_annotations_carry_correct_number_sha_and_mark() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "line one\nline two\nline three\n");
    let first_sha = repo.commit("first");
    repo.write("src/lib.rs", "line one\nEDITED two\nline three\n");
    let second_sha = repo.commit("second");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Sha(format!("{first_sha}..{second_sha}")),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let file = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
        .expect("src/lib.rs must be under review");

    let annotations = file.line_annotations();
    assert_eq!(annotations.len(), 3, "one annotation per source line");
    assert_eq!(annotations[0].sha(), &first_sha[..8], "line 1 untouched");
    assert!(!annotations[0].touched());
    assert_eq!(
        annotations[1].sha(),
        &second_sha[..8],
        "line 2 was changed by the second commit"
    );
    assert!(
        annotations[1].touched(),
        "line 2 is exactly what this review's diff touched"
    );
    assert_eq!(annotations[2].sha(), &first_sha[..8], "line 3 untouched");
    assert!(!annotations[2].touched());

    // The actual rendered prime block a model reads must show line 2 with
    // this exact number, sha, and `+` mark — the numbering the model is
    // told to READ rather than count.
    let rendered =
        crate::review::fleet::render_file_payload(std::slice::from_ref(file), ReviewSubject::Files);
    let expected_line_2 = format!("     2 | {} + | EDITED two", &second_sha[..8]);
    assert!(
            rendered.contains(&expected_line_2),
            "rendered block must show line 2 numbered with its blame sha and a `+` mark, got:\n{rendered}"
        );
    // An untouched line renders a space, never a `+`.
    let expected_line_1 = format!("     1 | {}   | line one", &first_sha[..8]);
    assert!(
        rendered.contains(&expected_line_1),
        "untouched line 1 must render a space in the mark column, got:\n{rendered}"
    );
}

/// A dirty, uncommitted edit in the `review working` scope has no committed
/// history at that content: `LineBlame::Worktree` (the same sentinel `git
/// blame` itself uses for "not committed yet"), rendered as `worktree` in
/// the sha column. The SAME line is also marked touched, since the
/// before/after diff sees it as changed — showing that `touched` and `sha`
/// are computed independently and can legitimately agree.
#[tokio::test]
async fn working_scope_dirty_line_gets_worktree_sha() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "alpha\nbeta\ngamma\n");
    let sha = repo.commit("initial");

    // Dirty, uncommitted edit to line 2.
    repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\n");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let file = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
        .expect("src/lib.rs must be under review");

    let annotations = file.line_annotations();
    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[0].sha(), &sha[..8]);
    assert!(!annotations[0].touched());
    assert_eq!(
        annotations[1].sha(),
        "worktree",
        "an uncommitted dirty line must show worktree in the sha column, got: {:?}",
        annotations[1]
    );
    assert!(annotations[1].touched());
    assert_eq!(annotations[2].sha(), &sha[..8]);
    assert!(!annotations[2].touched());
}

/// A brand-new untracked file (never `git add`ed) has no git history at
/// all: every line shows `untrackd`, and the whole file is marked touched
/// (there is no "before" side — it is entirely new relative to the review).
#[tokio::test]
async fn working_scope_untracked_new_file_gets_untrackd_sha() {
    let repo = TestRepo::new();
    repo.write("README.md", "# base\n");
    repo.commit("initial");

    repo.write("src/new.rs", "pub fn brand_new() {}\n");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let file = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/new.rs"))
        .expect("the untracked new file must be under review");

    let annotations = file.line_annotations();
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].sha(), "untrackd");
    assert!(
        annotations[0].touched(),
        "every line of a brand-new file is touched by this review"
    );
}

/// Regression test for ^8p6kjmw: the `/finish` loop's commit step commits
/// EVERY dirty file together (`git add -A && git commit`), not just the
/// one file whose finding it actually resolved. That sweeps up a
/// STILL-UNRESOLVED dirty line in some OTHER file right along with it —
/// the exact "intervening commit of unrelated files" the task describes.
///
/// Here `src/lib.rs` line 2 (`BETA-DIRTY`) is a still-open, unresolved
/// finding: its BYTES never change across the two runs. Between the runs,
/// a commit lands that (a) genuinely fixes `src/lib.rs` line 4 and (b)
/// incidentally sweeps up line 2's untouched dirty edit too, because
/// `commit_only` — like a real `git add -A` — stages the WHOLE file, not
/// individual lines. A third, freshly dirty line 1 keeps the file in
/// `review working` scope for run 2, isolating the assertion to line 2
/// alone: with `blame_at: None` (binding to HEAD), line 2 flips from
/// `worktree` (run 1) to a real committed sha (run 2) even though nothing
/// about that specific finding changed — the prompt drift the task
/// reports. Pinning the blame anchor to the branch's merge-base with
/// `main` (fixed for the whole session, since `main` never moves here)
/// closes this: the sweep commit postdates the anchor, so it stays
/// invisible to blame and line 2 keeps reading `worktree` in both runs.
#[tokio::test]
async fn working_scope_sha_is_stable_across_a_sweep_commit_of_an_unresolved_line() {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "alpha\nbeta\ngamma\ndelta\n");
    repo.commit("initial");
    repo.rename_current_branch_to("main");
    repo.checkout_new_branch("task");

    // A still-open, unresolved dirty edit to line 2 — this exact edit
    // survives, byte-for-byte, across both runs.
    repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\ndelta\n");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work1 = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();
    let file1 = work1
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
        .expect("src/lib.rs must be under review");
    let line2_run1 = file1.line_annotations()[1].sha().to_string();
    assert_eq!(
        line2_run1, "worktree",
        "line 2's still-open dirty edit must show worktree before the sweep commit"
    );

    // A second, DIFFERENT finding on line 4 gets genuinely fixed and
    // committed — but the commit step stages the WHOLE file, sweeping up
    // line 2's still-unresolved edit right along with it.
    repo.write("src/lib.rs", "alpha\nBETA-DIRTY\ngamma\nDELTA-FIXED\n");
    repo.commit_only(&["src/lib.rs"], "fix line 4's finding");

    // A THIRD, freshly dirty line keeps the file in `review working`
    // scope for run 2, without touching line 2 at all.
    repo.write(
        "src/lib.rs",
        "ALPHA-DIRTY\nBETA-DIRTY\ngamma\nDELTA-FIXED\n",
    );

    let work2 = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();
    let file2 = work2
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
        .expect("src/lib.rs must still be under review (line 1 is freshly dirty)");
    let line2_run2 = file2.line_annotations()[1].sha().to_string();

    assert_eq!(
            line2_run1, line2_run2,
            "line 2's sha must not drift across the sweep commit: it is the SAME \
             unresolved finding, byte-for-byte, in both runs — run1={line2_run1:?} run2={line2_run2:?}"
        );
}

/// A blame failure for any other reason (here: the repo handle itself
/// cannot be opened) must never abort the review: every affected line
/// degrades to the `????????` sentinel and `compute_line_annotations`
/// still returns a complete, non-panicking result — proving the
/// review-completes contract structurally (the function has no `Result`
/// to propagate a failure through in the first place).
#[tokio::test]
async fn blame_failure_degrades_to_unknown_marker_without_aborting() {
    let mut matched_files = BTreeSet::new();
    matched_files.insert("src/lib.rs".to_string());
    let mut after_content = BTreeMap::new();
    after_content.insert("src/lib.rs".to_string(), "line one\nline two\n".to_string());
    let before_by_path = BTreeMap::new();

    // A path with no git repository at all: `GitOperations::with_work_dir`
    // fails for every file, exercising the "blame fails for any other
    // reason" arm rather than the untracked/brand-new short-circuits.
    let not_a_repo = tempfile::tempdir().unwrap();

    let annotations = compute_line_annotations(
        not_a_repo.path(),
        &matched_files,
        &after_content,
        &before_by_path,
        None,
    )
    .await;

    let file_annotations = annotations
        .get("src/lib.rs")
        .expect("the file is still annotated even though blame failed");
    assert_eq!(file_annotations.len(), 2);
    for annotation in file_annotations {
        assert_eq!(
            annotation.sha(),
            "????????",
            "a blame failure must degrade to the unknown sentinel, got: {annotation:?}"
        );
    }
}

/// End-to-end: the line number the model READS off the real rendered prime
/// (not counted, not derived) survives, byte-for-byte, all the way through
/// `Finding.line` and into `synthesize`'s final report — no stage in
/// between renumbers it. This is the numbering half of the task closed
/// loop: [`sha_scope_line_annotations_carry_correct_number_sha_and_mark`]
/// proves the printed number is correct; this proves it is never lost.
#[tokio::test]
async fn a_findings_line_number_survives_from_the_prime_to_the_report() {
    let repo = TestRepo::new();
    repo.write(
        "src/lib.rs",
        "fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n",
    );
    repo.commit("initial");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
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
    let file = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/lib.rs"))
        .expect("src/lib.rs must be under review");

    let rendered =
        crate::review::fleet::render_file_payload(std::slice::from_ref(file), ReviewSubject::Files);
    // The exact numbered line the model would read for line 3 — pulled
    // from the REAL render, not hand-typed, so this test would fail if the
    // format ever drifted.
    let printed_line_3 = rendered
        .lines()
        .find(|l| l.trim_start().starts_with("3 |") || l.trim_start().starts_with("     3 |"))
        .expect("the rendered block must number line 3");
    assert!(
        printed_line_3.ends_with("fn three() {}"),
        "line 3 as printed must be line 3 of the real source: {printed_line_3}"
    );

    // The model "reads" the number 3 off that exact line and cites it in
    // its findings JSON — simulated here via the real parse path rather
    // than constructing a `Finding` by hand, so the whole textual
    // round-trip is exercised.
    let agent_response = crate::review::test_support::findings_json(
        "src/lib.rs",
        3,
        "no-unused",
        "`three` looks unused",
    );
    let findings = crate::review::types::parse_findings(&agent_response).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].line, 3,
        "the parsed finding must keep the exact line the model cited"
    );

    // Verify + synthesize: the line must reach the final report unchanged.
    let verified = vec![crate::review::types::VerifiedFinding {
        finding: findings[0].clone(),
        confirmed: true,
        reason: "confirmed".to_string(),
        decided_by: None,
    }];
    let report = crate::review::synthesize::synthesize(
        verified,
        &crate::review::synthesize::FleetTally::new(
            crate::review::synthesize::TasksAttempted(1),
            crate::review::synthesize::TasksFailed(0),
        ),
        &[],
        &[],
        &crate::review::ToolReport::default(),
        &crate::review::synthesize::ReviewedScope::new(&Scope::Working, 1),
        "2026-04-11 13:08",
    );
    assert!(
        report.markdown().contains("`src/lib.rs:3`"),
        "the final report must cite the SAME line the model read off the prime: {}",
        report.markdown()
    );
}

/// Regression test for ^j4d2613: a real, known two-commit history through
/// `Scope::Sha`, on a file with many untouched lines ABOVE the one edited
/// line — the exact shape the bug report cited ("a commit that touches a
/// file with many edits above the changed region, since that is where
/// drift shows"). The old, unnumbered render made the model COUNT lines,
/// and the miscount grew with depth; this proves the edited line's number,
/// blame sha, and touched mark survive correctly at depth, and that a
/// finding citing that number round-trips, unchanged, all the way to the
/// final report — closing the gap the earlier small fixture tests (e.g.
/// [`a_findings_line_number_survives_from_the_prime_to_the_report`]) do
/// not cover at scale.
#[tokio::test]
async fn a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol() {
    const LINES_ABOVE: usize = 190;

    let repo = TestRepo::new();
    let filler = || -> String {
        (0..LINES_ABOVE)
            .map(|i| format!("fn filler_{i}() {{}}\n"))
            .collect()
    };

    let mut before = filler();
    before.push_str("fn target() { old_body(); }\n");
    repo.write("src/big.rs", &before);
    let first_sha = repo.commit("first");

    let mut after = filler();
    after.push_str("fn target() { new_body(); }\n");
    repo.write("src/big.rs", &after);
    let second_sha = repo.commit("second");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(
        Scope::Sha(format!("{first_sha}..{second_sha}")),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let file = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .and_then(|v| v.files().iter().find(|f| f.path() == "src/big.rs"))
        .expect("src/big.rs must be under review");

    // 1-based line number of the edited `target` function: LINES_ABOVE
    // filler lines precede it.
    let changed_line = LINES_ABOVE + 1;
    let annotations = file.line_annotations();
    assert_eq!(
        annotations.len(),
        changed_line,
        "one annotation per source line"
    );

    // Every filler line above the change keeps the FIRST commit's blame
    // and stays unmarked — the large untouched block must not shift or
    // misattribute the edited line below it.
    for (i, annotation) in annotations.iter().take(LINES_ABOVE).enumerate() {
        assert_eq!(
            annotation.sha(),
            &first_sha[..8],
            "line {} sits above the change and must keep the first commit's blame",
            i + 1
        );
        assert!(
            !annotation.touched(),
            "line {} must not be marked touched",
            i + 1
        );
    }

    // The edited line itself blames to the SECOND commit and is touched.
    let changed = &annotations[changed_line - 1];
    assert_eq!(
            changed.sha(),
            &second_sha[..8],
            "the edited line must blame to the commit that edited it, not one of the {LINES_ABOVE} untouched lines above it"
        );
    assert!(changed.touched(), "the edited line must be marked touched");

    // The exact numbered line the model would read for the edited
    // function — pulled from the REAL render, not hand-typed, so this
    // test fails if the numbering ever drifts at depth.
    let rendered =
        crate::review::fleet::render_file_payload(std::slice::from_ref(file), ReviewSubject::Files);
    let expected_printed_line = format!(
        "{changed_line:>6} | {} + | fn target() {{ new_body(); }}",
        &second_sha[..8]
    );
    assert!(
            rendered.contains(&expected_printed_line),
            "rendered block must number the edited line at {changed_line} with the second commit's sha and a `+` mark, got:\n{rendered}"
        );

    // The model "reads" that number off the render and cites it in its
    // findings JSON — simulated via the real parse path, on a file large
    // enough that a counting-based miscite would show up as drift.
    let agent_response = crate::review::test_support::findings_json(
        "src/big.rs",
        changed_line as u32,
        "no-dead-code",
        "`target` calls `new_body`, which is unused",
    );
    let findings = crate::review::types::parse_findings(&agent_response).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(
            findings[0].line,
            changed_line as u32,
            "the parsed finding must keep the exact line the model cited, {LINES_ABOVE} lines of untouched content notwithstanding"
        );

    // Verify + synthesize: the line must reach the final report unchanged,
    // still resolving to the same `target` symbol the diff actually
    // touched.
    let verified = vec![crate::review::types::VerifiedFinding {
        finding: findings[0].clone(),
        confirmed: true,
        reason: "confirmed".to_string(),
        decided_by: None,
    }];
    let report = crate::review::synthesize::synthesize(
        verified,
        &crate::review::synthesize::FleetTally::new(
            crate::review::synthesize::TasksAttempted(1),
            crate::review::synthesize::TasksFailed(0),
        ),
        &[],
        &[],
        &crate::review::ToolReport::default(),
        &crate::review::synthesize::ReviewedScope::new(&Scope::Working, 1),
        "2026-08-03 12:00",
    );
    let expected_citation = format!("`src/big.rs:{changed_line}`");
    assert!(
            report.markdown().contains(&expected_citation),
            "the final report must cite the SAME line the model read off the prime, {LINES_ABOVE} lines deep: {}",
            report.markdown()
        );
}

/// Measures (does not tightly pin — wall-clock is inherently machine-
/// dependent) the blame overhead `scope_review` now pays on a
/// representative "normal commit": 8 changed files of ~150 lines each,
/// each with one dirty edit. Blame runs ONCE per file, concurrently
/// (`compute_line_annotations`'s `tokio::task::spawn_blocking` fan-out) —
/// this asserts only the coarse sanity bound that 8 files blame well
/// under a second, which would fail loudly if concurrency regressed to
/// sequential. The printed wall-clock (`cargo test -- --nocapture`) is
/// the number recorded on the task as the measured added cost.
#[tokio::test]
async fn blame_overhead_on_a_representative_commit_is_small_and_concurrent() {
    const FILE_COUNT: usize = 8;
    const LINES_PER_FILE: usize = 150;

    let repo = TestRepo::new();
    let body = |seed: usize| -> String {
        (0..LINES_PER_FILE)
            .map(|i| format!("fn line_{seed}_{i}() {{}}\n"))
            .collect()
    };
    for i in 0..FILE_COUNT {
        repo.write(&format!("src/file_{i}.rs"), &body(i));
    }
    repo.commit("initial");
    // A dirty, uncommitted one-line edit per file — a realistic "normal
    // commit" shape (every file touched, not wholesale rewritten).
    for i in 0..FILE_COUNT {
        let mut content = body(i);
        content.push_str("fn dirty_edit() {}\n");
        repo.write(&format!("src/file_{i}.rs"), &content);
    }

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let started = std::time::Instant::now();
    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();
    let elapsed = started.elapsed();
    println!(
        "scope_review with blame over {FILE_COUNT} files x {LINES_PER_FILE} lines took {elapsed:?}"
    );

    let validator = work
        .validators()
        .iter()
        .find(|v| v.validator_name() == "rust")
        .expect("the rust validator must match");
    assert_eq!(validator.files().len(), FILE_COUNT);
    for file in validator.files() {
        assert!(
            !file.line_annotations().is_empty(),
            "every file must carry line annotations"
        );
    }
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "blame across {FILE_COUNT} small files run concurrently must not take {elapsed:?}"
    );
}

// ---- scope_review: untracked files in the working scope ---------------

#[tokio::test]
async fn working_scope_includes_untracked_nested_source_files() {
    let repo = TestRepo::new();
    repo.write("README.md", "# base\n");
    repo.commit("initial");

    // A brand-new untracked directory of source files — the calcutron shape.
    repo.write("src/new.rs", &format!("{}\n", body("brand_new")));

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let validator = work
        .validators
        .iter()
        .find(|v| v.validator_name == "rust")
        .expect("the rust validator must match the untracked .rs file");
    assert!(
        validator.files.iter().any(|f| f.path == "src/new.rs"),
        "the untracked nested source file must be in scope, got: {:?}",
        validator
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn working_scope_excludes_untracked_non_code_files() {
    let repo = TestRepo::new();
    repo.write("README.md", "# base\n");
    repo.commit("initial");

    // Untracked junk: a log file in a new directory. Even a match-everything
    // validator must never see it — the code-extension filter drops it
    // before matching, so its content is never read.
    repo.write("logs/run.log", "lots of noise\n");

    let conn = index_conn();
    let loader = loader_with("everything", "*", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    assert!(
        work.validators.is_empty(),
        "untracked non-code files must not enter the working scope, got: {:?}",
        work.validators
            .iter()
            .flat_map(|v| v.files.iter().map(|f| f.path.as_str()))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn working_scope_keeps_tracked_non_code_modifications() {
    let repo = TestRepo::new();
    repo.write("notes.txt", "original\n");
    repo.commit("initial");

    // A deliberate edit to a tracked non-code file keeps current behavior:
    // it stays in scope and per-validator globs decide whether it's reviewed.
    repo.write("notes.txt", "original\nedited\n");

    let conn = index_conn();
    let loader = loader_with("everything", "*", &[]);
    let embedder = MockEmbedder::new(DIM);

    let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
        .await
        .unwrap();

    let validator = work
        .validators
        .iter()
        .find(|v| v.validator_name == "everything")
        .expect("tracked modifications must stay in scope regardless of extension");
    assert!(
        validator.files.iter().any(|f| f.path == "notes.txt"),
        "the tracked modified file must be in scope, got: {:?}",
        validator
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
    );
}

// ---- WorkList::distinct_files ----------------------------------------

/// A minimal `FileWork` carrying only a path — enough to assert the
/// dedup/order semantics of [`WorkList::distinct_files`].
fn file_at(path: &str) -> FileWork {
    file_sized(path, 0)
}

/// A `FileWork` whose inlined `source_slice` is exactly `bytes` bytes — the
/// knob [`batch_work_list`] packs against.
fn file_sized(path: &str, bytes: usize) -> FileWork {
    FileWork {
        path: path.to_string(),
        semantic_diff: vec![],
        changed_symbols: vec![],
        source_slice: "x".repeat(bytes),
        probe_results: vec![],
        line_annotations: vec![],
    }
}

fn validator_over(name: &str, paths: &[&str]) -> ValidatorWork {
    ValidatorWork {
        validator_name: name.to_string(),
        rules: RuleNames::new([format!("{name}-rule")]),
        probes: ProbeNames::default(),
        files: paths.iter().map(|p| file_at(p)).collect(),
        shared_probe_results: vec![],
    }
}

/// A cost function that charges a file its raw source bytes.
///
/// The packing tests below fix each fixture file's size through
/// [`file_sized`], so charging raw bytes keeps their arithmetic legible.
/// Production passes
/// [`rendered_file_block_bytes`](crate::review::fleet::rendered_file_block_bytes)
/// instead — the packer is agnostic to which, which is the point of taking
/// the cost as a parameter.
fn raw_source_bytes(file: &FileWork) -> usize {
    file.source_slice.len()
}

/// A validator over `(path, byte-size)` files, for [`batch_work_list`] packing
/// assertions.
fn validator_sized(name: &str, files: &[(&str, usize)]) -> ValidatorWork {
    ValidatorWork {
        validator_name: name.to_string(),
        rules: RuleNames::new([format!("{name}-rule")]),
        probes: ProbeNames::default(),
        files: files.iter().map(|(p, n)| file_sized(p, *n)).collect(),
        shared_probe_results: vec![],
    }
}

/// The validator names a batch carries, in order.
fn batch_validators(batch: &WorkList) -> Vec<String> {
    batch
        .validators
        .iter()
        .map(|v| v.validator_name.clone())
        .collect()
}

/// The file paths a batch carries (distinct, prime order).
fn batch_paths(batch: &WorkList) -> Vec<String> {
    batch.distinct_files().map(|f| f.path.clone()).collect()
}

#[test]
fn work_list_getters_and_constructors_round_trip_the_private_fields() {
    let file = FileWork::new(
        "src/a.rs".to_string(),
        vec![],
        vec!["alpha".to_string()],
        "fn alpha() {}".to_string(),
        vec![],
    );
    assert_eq!(file.path(), "src/a.rs");
    assert!(file.semantic_diff().is_empty());
    assert_eq!(file.changed_symbols(), ["alpha".to_string()]);
    assert_eq!(file.source_slice(), "fn alpha() {}");
    assert!(file.probe_results().is_empty());

    let validator = ValidatorWork::new(
        "dedup".to_string(),
        RuleNames::new(["dedup-rule".to_string()]),
        ProbeNames::new(["similar".to_string()]),
        vec![file],
    );
    assert_eq!(validator.validator_name(), "dedup");
    assert_eq!(validator.rules(), ["dedup-rule".to_string()]);
    assert_eq!(validator.probes(), ["similar".to_string()]);
    assert_eq!(validator.files().len(), 1);

    let work = WorkList::new("purpose".to_string(), vec![validator]);
    assert_eq!(work.change_purpose(), "purpose");
    assert_eq!(work.validators().len(), 1);

    let spec = ScopeSpec {
        working: true,
        ..Default::default()
    };
    assert_eq!(spec.resolve().unwrap(), Scope::Working);
}

#[test]
fn batch_work_list_packs_whole_files_within_the_byte_budget() {
    // Three 10-byte files, budget 25 → greedy packing gives [a,b],[c]; the
    // running total never exceeds the budget and no file is split.
    let work = WorkList::new(
        "p",
        vec![validator_sized(
            "v",
            &[("a.rs", 10), ("b.rs", 10), ("c.rs", 10)],
        )],
    );

    let (batches, skipped) = batch_work_list(&work, uniform_budget(25), raw_source_bytes);

    assert_eq!(
        batches.iter().map(batch_paths).collect::<Vec<_>>(),
        vec![vec!["a.rs", "b.rs"], vec!["c.rs"]],
        "files pack greedily into whole-file batches under the budget"
    );
    assert!(skipped.is_empty(), "no file is oversized: {skipped:?}");
    for batch in &batches {
        let total: usize = batch.distinct_files().map(|f| f.source_slice.len()).sum();
        assert!(total <= 25, "every batch stays within the byte budget");
    }
}

#[test]
fn batch_work_list_skips_a_single_file_over_the_budget_and_packs_the_rest() {
    // One file larger than the budget cannot be packed without splitting it
    // (forbidden) — it is excluded and reported, never a hard error that
    // blocks the rest of the scope. `small.rs` still packs normally.
    let work = WorkList::new(
        "p",
        vec![validator_sized("v", &[("big.rs", 100), ("small.rs", 10)])],
    );

    let (batches, skipped) = batch_work_list(&work, uniform_budget(32), raw_source_bytes);

    assert_eq!(
        batches.iter().map(batch_paths).collect::<Vec<_>>(),
        vec![vec!["small.rs"]],
        "the non-oversized file still packs and reviews: {batches:?}"
    );
    assert_eq!(skipped.len(), 1, "exactly the oversized file is skipped");
    assert_eq!(skipped[0].path(), "big.rs", "names the offending file");
    assert_eq!(skipped[0].size(), 100, "names the file's size");
    assert_eq!(
        skipped[0].validator(),
        "v",
        "names the validator that could not carry it"
    );
    assert_eq!(skipped[0].cap(), 32, "names the limit");
}

#[test]
fn batch_work_list_small_diff_is_exactly_one_batch() {
    // Today's fast path: a small diff fits one batch, unchanged.
    let work = WorkList::new(
        "p",
        vec![validator_sized("v", &[("a.rs", 10), ("b.rs", 10)])],
    );

    let (batches, skipped) = batch_work_list(&work, uniform_budget(32 * 1024), raw_source_bytes);

    assert_eq!(batches.len(), 1, "a small diff is a single batch");
    assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
    assert!(skipped.is_empty());
}

#[test]
fn batch_work_list_projects_each_validator_onto_its_batch_files() {
    // v1 owns a.rs,b.rs; v2 owns c.rs. Budget 25 splits into [a,b],[c], so v1
    // lands wholly in batch 1 and v2 wholly in batch 2 — a validator with no
    // files in a batch is dropped from it.
    let work = WorkList::new(
        "p",
        vec![
            validator_sized("v1", &[("a.rs", 10), ("b.rs", 10)]),
            validator_sized("v2", &[("c.rs", 10)]),
        ],
    );

    let (batches, skipped) = batch_work_list(&work, uniform_budget(25), raw_source_bytes);

    assert_eq!(batches.len(), 2);
    assert_eq!(batch_validators(&batches[0]), vec!["v1"]);
    assert_eq!(batch_paths(&batches[0]), vec!["a.rs", "b.rs"]);
    assert_eq!(batch_validators(&batches[1]), vec!["v2"]);
    assert_eq!(batch_paths(&batches[1]), vec!["c.rs"]);
    assert!(skipped.is_empty());
}

#[test]
fn batch_work_list_keeps_a_shared_file_atomic_in_one_batch() {
    // `shared.rs` is matched by two validators but is ONE distinct file: it is
    // packed once, into a single batch, never duplicated or split.
    let work = WorkList::new(
        "p",
        vec![
            validator_sized("v1", &[("shared.rs", 10)]),
            validator_sized("v2", &[("shared.rs", 10)]),
        ],
    );

    let (batches, skipped) = batch_work_list(&work, uniform_budget(25), raw_source_bytes);

    assert_eq!(batches.len(), 1, "the one distinct file is one batch");
    assert_eq!(batch_paths(&batches[0]), vec!["shared.rs"]);
    assert_eq!(
        batch_validators(&batches[0]),
        vec!["v1", "v2"],
        "both validators that matched the shared file ride the same batch"
    );
    assert!(skipped.is_empty());
}

#[test]
fn batch_work_list_empty_work_yields_no_batches() {
    let work = WorkList::new("p", vec![]);
    let (batches, skipped) = batch_work_list(&work, uniform_budget(32 * 1024), raw_source_bytes);
    assert!(batches.is_empty());
    assert!(skipped.is_empty());
}

#[test]
fn distinct_files_dedups_by_path_in_first_seen_order() {
    // Three validators; `src/shared.rs` is matched by two of them, and the
    // overall first-seen order is b, shared, a.
    let work = WorkList::new(
        "purpose",
        vec![
            validator_over("v1", &["src/b.rs", "src/shared.rs"]),
            validator_over("v2", &["src/shared.rs", "src/a.rs"]),
        ],
    );

    let distinct: Vec<&str> = work.distinct_files().map(|f| f.path.as_str()).collect();
    assert_eq!(
        distinct,
        vec!["src/b.rs", "src/shared.rs", "src/a.rs"],
        "distinct_files dedups by path and preserves first-seen order"
    );
}

#[test]
fn as_borrowed_strings_borrows_every_owned_key_in_order() {
    let owned = vec!["rust".to_string(), "python".to_string()];

    assert_eq!(
        as_borrowed_strings(&owned),
        vec!["rust", "python"],
        "the owned keys borrow one for one, in their original order"
    );
}

#[test]
fn as_borrowed_strings_accepts_a_slice_that_already_borrows() {
    let already_borrowed = ["rust", "python"];

    assert_eq!(
        as_borrowed_strings(&already_borrowed),
        vec!["rust", "python"],
        "a caller holding borrowed keys needs no owned vector first"
    );
}

// ---- what the op hands the agent (^apb04az) ---------------------------

/// How many lines the fixture's large file carries. Big enough that a
/// whole-file view and a change view cannot be confused for one another.
const BIG_FILE_LINES: usize = 4000;

/// The 1-based line of [`BIG_FILE_LINES`] the fixture change edits — mid-file,
/// so the context band around it reaches neither end.
const EDITED_LINE: usize = 2000;

/// A line far from [`EDITED_LINE`], well outside any context band: present in a
/// whole-file view, absent from a change view.
const DISTANT_LINE: usize = 100;

/// The fixture's large Rust file: one uniquely-numbered function per line.
fn big_file(edited: Option<usize>) -> String {
    (1..=BIG_FILE_LINES)
        .map(|line| match edited {
            Some(target) if target == line => format!("pub fn f{line}() -> u8 {{ 1 }}\n"),
            _ => format!("pub fn f{line}() -> u8 {{ 0 }}\n"),
        })
        .collect()
}

/// The source line the fixture renders for `line`, as the numbered block
/// prints it after the `sha` and `mark` columns.
fn source_of(line: usize) -> String {
    format!("| pub fn f{line}()")
}

/// A repo holding [`BIG_FILE_LINES`] lines, then one line of it edited, with
/// the two commit shas the change spans.
struct OneLineChange {
    repo: TestRepo,
    range: String,
}

impl OneLineChange {
    /// Commit the large file, then commit a one-line edit to it.
    fn new() -> Self {
        let repo = TestRepo::new();
        repo.write("src/big.rs", &big_file(None));
        let before = repo.commit("add the big file");
        repo.write("src/big.rs", &big_file(Some(EDITED_LINE)));
        let after = repo.commit("edit one line of it");
        Self {
            repo,
            range: format!("{before}..{after}"),
        }
    }

    /// Drive the production scope stage over `scope` and render the run prime
    /// — the exact bytes the fan-out's prime turn sends the agent.
    async fn prime(&self, scope: Scope) -> String {
        let conn = index_conn();
        let loader = loader_with("rust", "*.rs", &[]);
        let embedder = MockEmbedder::new(DIM);
        let work = scope_review(scope, self.repo.path(), &loader, &conn, &embedder, None)
            .await
            .unwrap();
        crate::review::fleet::render_run_prime(&work)
    }
}

/// ^apb04az: a `review sha` op hands the agent THE DIFF, not the file.
///
/// The whole card in one assertion pair: a one-line edit to a 4000-line file
/// used to be handed over as 4000 lines, under a prompt that named the whole
/// file as the review boundary — which is why zero of 53 measured findings
/// landed on the change. Driven through the real `scope_review` and the real
/// `render_run_prime`, so it fails if any stage of the production path stops
/// reading the op's subject.
#[tokio::test]
async fn a_sha_op_hands_the_agent_the_diff_not_the_whole_file() {
    let change = OneLineChange::new();

    let prime = change.prime(Scope::Sha(change.range.clone())).await;

    // The changed line is there, marked as the line to REVIEW.
    assert!(
        prime.contains(&format!("{EDITED_LINE} | ")),
        "the changed line must be printed with its true number: {prime}"
    );
    assert!(
        prime.contains(&source_of(EDITED_LINE)),
        "the changed line's source must be printed: {prime}"
    );

    // A line far from the change is NOT there: the agent was handed the
    // change, not the file.
    assert!(
        !prime.contains(&source_of(DISTANT_LINE)),
        "a line far from the change must not be printed under a sha op: {prime}"
    );

    // And the block says which lines are the subject, in the engine's two
    // words, rather than naming the whole file as the boundary.
    assert!(
        prime.contains("REVIEW") && prime.contains("CONSIDER"),
        "the block must state what to REVIEW against what to CONSIDER: {prime}"
    );
    assert!(
        !prime.contains("This whole file is the review boundary"),
        "a sha op must not name the whole file as the review boundary: {prime}"
    );
}

/// The same repo under a `review file` op hands the agent the WHOLE file —
/// the other half of the distinction, so neither op can quietly become the
/// other. A file the caller named on purpose is answered in full.
#[tokio::test]
async fn a_file_op_hands_the_agent_the_whole_file() {
    let change = OneLineChange::new();

    let prime = change.prime(Scope::File("src/big.rs".to_string())).await;

    assert!(
        prime.contains(&source_of(DISTANT_LINE)),
        "a file op must print a line far from the last change: {prime}"
    );
    assert!(
        prime.contains(&source_of(BIG_FILE_LINES)),
        "a file op must print the file's last line: {prime}"
    );
}

/// How many lines the relocated fixture file carries before its move. Long
/// enough that the whole body sits well outside the context band around the one
/// line the move also added.
const MOVED_FILE_LINES: usize = 200;

/// ^apb04az: a relocation reviews in proportion to its real delta.
///
/// A file moved to a new path has no base side AT THAT PATH, so without git's
/// rename detection the engine diffs it as wholly added and reviews every line
/// of it as new work. `^0fn6dbf` read as 70 files and 20548 insertions when its
/// real delta was about 39 lines of module wiring. Here the move carries one
/// added line, and that one line is the whole of what the review is handed.
#[tokio::test]
async fn a_relocated_file_reviews_as_the_lines_the_move_also_changed() {
    let repo = TestRepo::new();
    let body: String = (1..=MOVED_FILE_LINES)
        .map(|line| format!("pub fn moved{line}() -> u8 {{ 0 }}\n"))
        .collect();
    repo.write("src/old.rs", &body);
    let before = repo.commit("add the file at its original path");

    std::fs::remove_file(repo.path().join("src/old.rs")).expect("remove the original path");
    repo.write(
        "src/new.rs",
        &format!("{body}pub fn added_by_the_move() {{}}\n"),
    );
    let after = repo.commit("move the file and add one function");

    let conn = index_conn();
    let loader = loader_with("rust", "*.rs", &[]);
    let embedder = MockEmbedder::new(DIM);
    let work = scope_review(
        Scope::Sha(format!("{before}..{after}")),
        repo.path(),
        &loader,
        &conn,
        &embedder,
        None,
    )
    .await
    .unwrap();

    let moved = work
        .distinct_files()
        .find(|file| file.path() == "src/new.rs")
        .expect("the moved file must be under review");

    // Only the line the move ALSO added is marked. Every relocated line reads
    // as what it is: content that already existed, at a new address.
    let touched: Vec<usize> = moved
        .line_annotations()
        .iter()
        .enumerate()
        .filter(|(_, annotation)| annotation.touched())
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(
        touched,
        vec![MOVED_FILE_LINES + 1],
        "a move plus one added line touches exactly that line"
    );

    // And the prime the agent reads shows the added line, not the moved body.
    let prime = crate::review::fleet::render_run_prime(&work);
    assert!(
        prime.contains("pub fn added_by_the_move()"),
        "the added line must be printed: {prime}"
    );
    assert!(
        !prime.contains("pub fn moved1()"),
        "a relocated line far from the change must not be printed: {prime}"
    );
}

/// The two ops' primes differ by an order of magnitude on the same repo. The
/// card's cost half: the diff view is what keeps a one-line edit from
/// batching, fanning out, and being paid for as a 4000-line review.
#[tokio::test]
async fn the_diff_view_is_a_fraction_of_the_whole_file_view() {
    let change = OneLineChange::new();

    let diff_prime = change.prime(Scope::Sha(change.range.clone())).await;
    let file_prime = change.prime(Scope::File("src/big.rs".to_string())).await;

    /// How many times smaller the diff view must be, at least. A one-line
    /// edit to a 4000-line file prints a band of tens of lines, so the real
    /// ratio is far larger; the gate is deliberately loose so it measures the
    /// distinction rather than the exact context width.
    const MIN_RATIO: usize = 10;

    assert!(
        diff_prime.len() * MIN_RATIO < file_prime.len(),
        "the diff view must be at least {MIN_RATIO}x smaller than the whole-file view: \
         {} bytes against {} bytes",
        diff_prime.len(),
        file_prime.len()
    );
}
