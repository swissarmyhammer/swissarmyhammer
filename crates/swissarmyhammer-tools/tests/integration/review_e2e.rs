//! End-to-end acceptance test for the local multi-agent review feature.
//!
//! This mirrors `tests/integration/semantic_search_e2e.rs` (real indexer → real
//! query → real result) for the review tool: a **real temp git repo** with a diff
//! that PLANTS specific defects on purpose, a **real on-disk code_context index**
//! (production schema), and the **registered production `review` tool** driven
//! over a deterministic/playback ACP agent and a mock embedder so CI needs no live
//! model. It asserts pipeline BEHAVIOR — scope → fan-out → guard → verify →
//! synthesize through the registered tool — not canned output strings.
//!
//! The repo / index / scripted-agent / embedder plumbing lives in the shared
//! [`review_fixture`](super::review_fixture) module so it can be reused by the
//! global-subscriber observability test binary; this file holds only the
//! behavioral acceptance tests.
//!
//! ## What the planted diff exercises
//!
//! | # | Plant                                              | Caught by      | Severity | Verdict                 |
//! |---|----------------------------------------------------|----------------|----------|-------------------------|
//! | 1 | copy-pasted block duplicating an existing function | `duplication`  | blocker  | confirmed (`duplicates`)|
//! | 2 | helper reimplementing an existing shared util      | `reuse`        | warning  | confirmed (`similar`)   |
//! | 3 | hardcoded if-chain over a known set (→ table)      | `data-driven`  | warning  | confirmed (in-file)     |
//! | 4 | new function with zero inbound callers              | `dead-code`    | blocker  | confirmed (`callers`)   |
//! | 5 | a planted secret                                   | `no-secrets`   | blocker  | confirmed (in-file)     |
//! | 6 | an agent red herring (looks buggy, is correct)     | `rust`         | warning  | REFUTED by the agent    |
//! | 7 | a guard red herring (claimed dead, but IS called)  | `dead-code`    | blocker  | REFUTED by the guard    |
//! | 8 | a Rust idiom issue                                 | `rust`         | warning  | confirmed (language)    |
//!
//! Items 1–5 + 8 must be confirmed at the right validator + severity; item 6 must
//! be refuted by the adversarial verifier (the agent runs, says "disproven"); item
//! 7 must be refuted by the deterministic guard (the `callers` fact contradicts the
//! "dead" claim) WITHOUT any verifier agent ever seeing it — proving the two-layer
//! verify. The driver runs across `review working`, `review sha`, and `review
//! file`, all of which share the same dispatch → driver → engine path.

use serde_json::json;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_kanban::{
    board::InitBoard, task::AddTask, task::GetTask, Execute, KanbanContext,
};

use super::review_fixture::{
    plant_diff, report_has_claim, run_review_op, seed_on_disk_index, working_args, TestRepo,
    CLAIM_DATA, CLAIM_DEAD_ORPHAN, CLAIM_DUP, CLAIM_GUARD_HERRING, CLAIM_RED_HERRING, CLAIM_REUSE,
    CLAIM_RUST_IDIOM, CLAIM_SECRET,
};

// ---------------------------------------------------------------------------
// The acceptance tests.
// ---------------------------------------------------------------------------

/// `review working` over the planted diff: items 1–5 + 8 are confirmed at the
/// right severity, item 6 is agent-refuted, item 7 is guard-refuted, and neither
/// refuted finding appears in the report.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_working_confirms_real_defects_and_refutes_both_red_herrings() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let parsed = run_review_op(&repo, working_args()).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    // --- the dated GFM checklist format renders ---
    assert!(
        markdown.contains("## Review Findings ("),
        "report must render the dated GFM section header: {markdown}"
    );

    // --- items 1–5 + 8: confirmed at the right severity ---
    assert!(
        markdown.contains("### Blockers"),
        "blockers section: {markdown}"
    );
    assert!(
        markdown.contains("### Warnings"),
        "warnings section: {markdown}"
    );

    assert!(
        report_has_claim(markdown, CLAIM_DUP),
        "item 1 duplication: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_SECRET),
        "item 5 secret: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "item 4 dead orphan: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_REUSE),
        "item 2 reuse: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DATA),
        "item 3 data-driven: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_RUST_IDIOM),
        "item 8 rust idiom: {markdown}"
    );

    // The three confirmed blockers (1, 4, 5) render under Blockers, before Warnings.
    let blockers_at = markdown.find("### Blockers").unwrap();
    let warnings_at = markdown.find("### Warnings").unwrap();
    assert!(
        blockers_at < warnings_at,
        "blockers render before warnings: {markdown}"
    );
    for blocker_claim in [CLAIM_DUP, CLAIM_DEAD_ORPHAN, CLAIM_SECRET] {
        let at = markdown.find(blocker_claim).unwrap();
        assert!(
            at > blockers_at && at < warnings_at,
            "blocker `{blocker_claim}` must render in the Blockers section: {markdown}"
        );
    }

    // --- item 6: agent-refuted → NOT in the report ---
    assert!(
        !report_has_claim(markdown, CLAIM_RED_HERRING),
        "item 6 (agent red herring) must be refuted and absent: {markdown}"
    );
    // --- item 7: guard-refuted → NOT in the report ---
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "item 7 (guard red herring) must be refuted and absent: {markdown}"
    );

    // --- counts: confirmed vs refuted, blockers vs warnings ---
    let counts = &parsed["counts"];
    assert_eq!(
        counts["blockers"],
        json!(3),
        "items 1,4,5 are confirmed blockers: {counts}"
    );
    assert_eq!(
        counts["warnings"],
        json!(3),
        "items 2,3,8 are confirmed warnings: {counts}"
    );
    assert_eq!(
        counts["confirmed"],
        json!(6),
        "six confirmed findings (1,2,3,4,5,8): {counts}"
    );
    assert_eq!(
        counts["refuted"],
        json!(2),
        "two refuted findings (item 6 by agent, item 7 by guard): {counts}"
    );
}

/// `review sha <range>` over the same planted change committed as a range — the
/// committed-scope path shares the dispatch → driver → engine path with `working`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_sha_range_confirms_the_same_defects() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    // Commit the planted change so a range scope sees it as committed work.
    repo.commit("plant the reviewable defects");
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review sha"));
    args.insert("sha".to_string(), json!("HEAD~1..HEAD"));
    args.insert("backend".to_string(), json!("local"));

    let parsed = run_review_op(&repo, args).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    assert!(
        report_has_claim(markdown, CLAIM_DUP),
        "duplication via sha: {markdown}"
    );
    assert!(
        report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "dead-code via sha: {markdown}"
    );
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "guard refutation holds via sha: {markdown}"
    );
    assert!(
        !report_has_claim(markdown, CLAIM_RED_HERRING),
        "agent refutation holds via sha: {markdown}"
    );
    assert_eq!(
        parsed["counts"]["refuted"],
        json!(2),
        "two refuted via sha: {parsed}"
    );
}

/// `review file <glob>` over the planted files — the file/glob scope shares the
/// same dispatch → driver → engine path and confirms its scoped defects.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_file_glob_confirms_scoped_defects() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    repo.commit("plant the reviewable defects");
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review file"));
    args.insert("path".to_string(), json!("src/*.rs"));
    args.insert("backend".to_string(), json!("local"));

    let parsed = run_review_op(&repo, args).await;
    let markdown = parsed["markdown"].as_str().expect("markdown string");

    // The glob whole-content scope still drives the engine to confirmed findings.
    assert!(
        report_has_claim(markdown, CLAIM_DUP) || report_has_claim(markdown, CLAIM_DEAD_ORPHAN),
        "file/glob scope must surface at least one confirmed defect: {markdown}"
    );
    // Refutations are scope-independent: the guard still kills item 7.
    assert!(
        !report_has_claim(markdown, CLAIM_GUARD_HERRING),
        "guard refutation holds via file/glob: {markdown}"
    );
}

/// Skill write-path contract: the report's `markdown` (the dated GFM section the
/// `review` tool returns) lands verbatim on a kanban task — the range-mode
/// tracking-task write the review skill performs (`builtin/skills/review/SKILL.md`
/// step: "embed the report's `markdown`"). The skill itself runs in the agent
/// harness, but its data contract — engine report markdown → task description,
/// byte-for-byte, in the documented dated format — is exercised here against a
/// real file-backed kanban board.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn review_e2e_report_lands_on_a_kanban_task_in_the_dated_gfm_format() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");

    let repo = TestRepo::new();
    plant_diff(&repo);
    seed_on_disk_index(repo.path());
    let _cwd = CurrentDirGuard::new(repo.path()).expect("chdir");

    // 1. Drive the production review tool to a report.
    let parsed = run_review_op(&repo, working_args()).await;
    let markdown = parsed["markdown"]
        .as_str()
        .expect("markdown string")
        .to_string();
    assert!(
        markdown.contains("## Review Findings ("),
        "precondition: the engine produced the dated section: {markdown}"
    );

    // 2. Write it onto a kanban task exactly as the skill's range-mode path does:
    //    a tracking task in the board, with `Scope: ...\n\n<report.markdown>` as
    //    the description.
    let kanban_dir = repo.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    InitBoard::new("Review E2E")
        .execute(&ctx)
        .await
        .into_result()
        .expect("init board");

    let description = format!("Scope: working\n\n{markdown}");
    let added = AddTask::new("Review of working")
        .with_description(&description)
        .execute(&ctx)
        .await
        .into_result()
        .expect("add tracking task");
    let task_id = added["id"].as_str().expect("new task id").to_string();

    // 3. Read it back: the dated section and a confirmed checklist item must have
    //    landed verbatim, and the refuted herrings must not be present.
    let task = GetTask::new(task_id)
        .execute(&ctx)
        .await
        .into_result()
        .expect("get tracking task");
    let stored = task["description"].as_str().expect("task description");

    assert!(
        stored.contains("## Review Findings ("),
        "the dated GFM section must land on the task verbatim: {stored}"
    );
    assert!(
        stored.contains("### Blockers"),
        "the severity checklist must land on the task: {stored}"
    );
    assert!(
        report_has_claim(stored, CLAIM_DUP),
        "a confirmed finding must land on the task: {stored}"
    );
    assert!(
        !report_has_claim(stored, CLAIM_GUARD_HERRING)
            && !report_has_claim(stored, CLAIM_RED_HERRING),
        "refuted findings must not land on the task: {stored}"
    );
}
