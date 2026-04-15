//! Drag-drop performance benchmark for `MoveTask` on a large board.
//!
//! Seeds a `.kanban/` directory with 2000 tasks spread across four columns
//! (50/500/1000/450 distribution), opens a `KanbanContext` (which primes the
//! `EntityCache` via `load_all`), and then measures `MoveTask::execute`
//! iterations — each one shuffling a task into a new column with a
//! `with_before` neighbor placement, mirroring what the drag-drop handler
//! does on the frontend.
//!
//! Target per the driving card (`entity-cache 2/4`, which supersedes the
//! earlier drag-perf card): median <20ms per iteration. The full drag-drop
//! budget end-to-end is <300ms on a 2000-task board; the <20ms target
//! leaves plenty of headroom for front-end render cost.
//!
//! The `EntityCache` short-circuits the `read_entity_dir` call on
//! `list`/`read`, the per-task `_changelog` / `_file_created` I/O that
//! `apply_compute` injects, and (as of Option B) the `ComputeEngine::derive_all`
//! output itself — the simple and aggregate computed-field values are
//! memoized on the cache alongside the inputs. All three caches are
//! invalidated by the normal mutation paths (`write`, `delete`, `evict`,
//! `archive`, `unarchive`, `refresh_from_disk`). The remaining cost on a
//! warm cache is a `HashMap` lookup plus a `Value::clone` per cached
//! field — the entity-file scan, pseudo-field injection, and compute-engine
//! derivation are all skipped on the warm path.
//!
//! Run manually:
//!
//! ```bash
//! cargo bench -p swissarmyhammer-kanban --bench move_task_bench
//! ```

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_kanban::task::MoveTask;
use swissarmyhammer_kanban::KanbanContext;
use swissarmyhammer_operations::Execute;
use tempfile::TempDir;

/// Column distribution for the seeded 2000-task board.
///
/// Deliberately uneven so the benchmark exercises different ordinal-insert
/// neighborhoods: most drops land in the 1000-task column, but we still
/// need correctness on the tail of the 50-task column.
const COLUMN_DISTRIBUTION: &[(&str, usize)] = &[
    ("todo", 50),
    ("doing", 500),
    ("review", 1000),
    ("done", 450),
];

/// Build a kanban directory containing the seeded 2000-task board.
///
/// Writes column entities and task entities directly via the entity context
/// so the setup cost does not pollute the timed region. The returned
/// `TempDir` must outlive the returned `KanbanContext` — dropping it first
/// would yank the on-disk files.
async fn seed_board() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().expect("tempdir");
    let kanban_dir = temp.path().join(".kanban");
    std::fs::create_dir_all(&kanban_dir).expect("mkdir .kanban");

    let ctx = KanbanContext::open(&kanban_dir).await.expect("open kanban");
    ctx.create_directories().await.expect("create_directories");

    // Grab the entity context first — this primes the cache with whatever is
    // on disk (currently nothing, so it's cheap). We then write entities
    // through the cache-wired context so the cache stays authoritative.
    let ectx = ctx.entity_context().await.expect("entity_context");

    // Seed columns.
    for (order, (slug, _)) in COLUMN_DISTRIBUTION.iter().enumerate() {
        let mut col = Entity::new("column", *slug);
        col.set(
            "name",
            serde_json::json!(slug.chars().next().unwrap().to_uppercase().to_string() + &slug[1..]),
        );
        col.set("order", serde_json::json!(order));
        ectx.write(&col).await.expect("write column");
    }

    // Seed tasks — assign each into its column with monotonically-increasing
    // ordinals so `with_before` placements always have a valid neighbor.
    let mut task_index = 0usize;
    for (slug, count) in COLUMN_DISTRIBUTION.iter() {
        for i in 0..*count {
            let id = format!("01TASK{:020}", task_index);
            let mut t = Entity::new("task", id.as_str());
            t.set(
                "title",
                serde_json::json!(format!("Task #{} in {}", task_index, slug)),
            );
            t.set("position_column", serde_json::json!(*slug));
            // Use a simple hex-padded ordinal; the exact format doesn't
            // matter — MoveTask recomputes it on each move.
            t.set("position_ordinal", serde_json::json!(format!("a{i:06x}")));
            t.set("body", serde_json::json!(""));
            ectx.write(&t).await.expect("write task");
            task_index += 1;
        }
    }

    (temp, ctx)
}

/// Collect the ids of the first N tasks in a given column (sorted by ordinal)
/// so the benchmark has deterministic source and neighbor tasks.
async fn collect_task_ids(ctx: &KanbanContext, column: &str, limit: usize) -> Vec<String> {
    let ectx = ctx.entity_context().await.expect("entity_context");
    let mut tasks = ectx.list("task").await.expect("list tasks");
    tasks.retain(|t| t.get_str("position_column") == Some(column));
    tasks.sort_by(|a, b| {
        let oa = a.get_str("position_ordinal").unwrap_or("");
        let ob = b.get_str("position_ordinal").unwrap_or("");
        oa.cmp(ob)
    });
    tasks
        .into_iter()
        .take(limit)
        .map(|t| t.id.to_string())
        .collect()
}

/// Run the move-task benchmark.
///
/// Each iteration:
/// 1. Picks a task from the source column (`todo`).
/// 2. Picks a neighbor in the target column (`doing`).
/// 3. Calls `MoveTask::to_column(id, "doing").with_before(neighbor).execute(&ctx)`.
///
/// Criterion times just the `execute` call; the runtime + KanbanContext +
/// seeded board are reused across iterations.
fn bench_move_task(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let (_temp, ctx) = runtime.block_on(seed_board());

    // Pre-collect source tasks and neighbors so the timed region doesn't
    // spend time enumerating the board.
    // 40 source tasks and 40 neighbors gives Criterion plenty of variety
    // across iterations while staying comfortably inside the 50-task
    // `todo` column the card-specified distribution seeds.
    let source_ids = runtime.block_on(async { collect_task_ids(&ctx, "todo", 40).await });
    let neighbors = runtime.block_on(async { collect_task_ids(&ctx, "doing", 40).await });
    assert!(
        source_ids.len() >= 40 && neighbors.len() >= 40,
        "seed data is too small: source={} neighbors={}",
        source_ids.len(),
        neighbors.len()
    );

    let mut group = c.benchmark_group("move_task_2000");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    group.bench_function("with_before", |b| {
        let mut i = 0usize;
        b.to_async(&runtime).iter(|| {
            let source = source_ids[i % source_ids.len()].clone();
            let neighbor = neighbors[i % neighbors.len()].clone();
            i += 1;
            let ctx_ref = &ctx;
            async move {
                let op =
                    MoveTask::to_column(source.as_str(), "doing").with_before(neighbor.as_str());
                let res = op.execute(ctx_ref).await.into_result();
                assert!(res.is_ok(), "MoveTask failed: {:?}", res.err());
            }
        });
    });

    group.finish();

    // Diagnostic companion benches — these do NOT count against the
    // <20ms target. They exist to show reviewers where the
    // per-iteration cost is going in isolation:
    //
    // - `list_task`: a single `ectx.list("task")` on the same 2000-task
    //   board. Dominates `MoveTask::execute`. The cache short-circuits
    //   the entity-file scan, the per-task `_changelog` /
    //   `_file_created` injection, and (as of Option B) the
    //   `ComputeEngine::derive_all` output — `derive-created` /
    //   `derive-updated` / `derive-started` / `derive-completed` do NOT
    //   run per-list on a warm cache; their outputs are memoized and
    //   cloned from the derived-output slot. The remaining warm-path
    //   cost is the per-task `HashMap` lookup + `Value::clone` of each
    //   memoized field.
    //
    // - `read_task`: a single `ectx.read("task", id)` from the cache.
    //   Three orders of magnitude faster than `list`, confirming the
    //   per-entity compute cost multiplies to the observed total.
    let mut diag = c.benchmark_group("move_task_components_2000");
    diag.warm_up_time(Duration::from_secs(1));
    diag.measurement_time(Duration::from_secs(5));
    diag.sample_size(20);

    diag.bench_function("list_task", |b| {
        let ctx_ref = &ctx;
        b.to_async(&runtime).iter(|| async move {
            let ectx = ctx_ref.entity_context().await.expect("ectx");
            let _tasks = ectx.list("task").await.expect("list task");
        });
    });

    diag.bench_function("read_task", |b| {
        let ctx_ref = &ctx;
        let id = source_ids[0].clone();
        b.to_async(&runtime).iter(|| {
            let id = id.clone();
            async move {
                let ectx = ctx_ref.entity_context().await.expect("ectx");
                let _t = ectx.read("task", &id).await.expect("read task");
            }
        });
    });

    diag.finish();
}

criterion_group!(benches, bench_move_task);
criterion_main!(benches);
