//! Diagnostic benchmark for `read_entity_dir` at scale.
//!
//! Builds a temporary `.kanban/`-style directory containing N synthetic task
//! YAML+frontmatter files and times how long it takes `read_entity_dir` to
//! enumerate, parse, and return them as `Entity` values.
//!
//! This benchmark exists to validate the hypothesis that disk I/O on the
//! per-file `read_entity` step is the dominant cost when listing tasks on
//! a large board (~2000 entities). It is **not** a CI gate — run it manually
//! when changing `read_entity_dir` or related helpers:
//!
//! ```bash
//! cargo bench -p swissarmyhammer-entity --bench list_entities
//! ```
//!
//! The benchmark prints two figures:
//! - `read_entity_dir/2000` — wall time for the bare I/O path on 2000 tasks.
//! - `read_entity_dir/500` — same path at a smaller scale, for sanity.
//!
//! Compare baseline numbers to the numbers after a parallelization change to
//! verify the speedup matches the hypothesis (>= 3x for the 2000-task case,
//! per the kanban card driving this work).
//!
//! ## Recorded baseline (macOS, 18-core, page cache hot)
//!
//! Serial implementation (pre-parallelization, commit prior to introducing
//! `buffer_unordered`):
//! - 500 tasks: ~19.7 ms
//! - 2000 tasks: ~79.7 ms
//!
//! Parallel implementation (`buffer_unordered(64)`):
//! - 500 tasks: ~6.5 ms (~3.0x faster)
//! - 2000 tasks: ~26 ms (~3.0x faster)
//!
//! These numbers are page-cache-hot (worst case for parallelization). On a
//! cold cache, the speedup is expected to be substantially larger because
//! serial reads each block on disk while concurrent reads overlap.

use std::path::Path;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use swissarmyhammer_entity::io::{entity_file_path, read_entity_dir, write_entity};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_fields::EntityDef;
use tempfile::TempDir;

/// Build a minimal task-shaped `EntityDef` (frontmatter + body).
///
/// Mirrors the shape used by the kanban app's `task` entity so that the
/// per-file parse cost is representative.
fn task_entity_def() -> EntityDef {
    EntityDef {
        name: "task".into(),
        icon: None,
        body_field: Some("body".into()),
        fields: vec![
            "title".into(),
            "body".into(),
            "assignees".into(),
            "tags".into(),
            "depends_on".into(),
            "position_column".into(),
            "position_ordinal".into(),
        ],
        sections: vec![],
        validate: None,
        mention_prefix: None,
        mention_display_field: None,
        mention_slug_field: None,
        search_display_field: None,
        commands: vec![],
    }
}

/// Populate `dir` with `count` synthetic task files, each ~realistic in shape.
///
/// Each task carries title, assignees, tags, dependencies, position metadata,
/// and a multi-line markdown body — enough that the YAML parse cost per file
/// is representative of real boards rather than vanishingly small.
async fn populate_tasks(dir: &Path, count: usize) {
    let entity_def = task_entity_def();
    for i in 0..count {
        let id = format!("01TASK{:020}", i);
        let path = entity_file_path(dir, &id, &entity_def);
        let mut entity = Entity::new("task", id.as_str());
        entity.set("title", serde_json::json!(format!("Synthetic task #{i}")));
        entity.set("assignees", serde_json::json!([format!("actor-{}", i % 8)]));
        entity.set(
            "tags",
            serde_json::json!([format!("tag-{}", i % 16), format!("category-{}", i % 4),]),
        );
        let depends_on = if i > 0 && i % 7 == 0 {
            serde_json::json!([format!("01TASK{:020}", i - 1)])
        } else {
            serde_json::json!([])
        };
        entity.set("depends_on", depends_on);
        entity.set("position_column", serde_json::json!("todo"));
        entity.set("position_ordinal", serde_json::json!(format!("a{i:04x}")));
        entity.set(
            "body",
            serde_json::json!(format!(
                "## Task body #{i}\n\nThis is a synthetic body intended to approximate the parse cost of\n\
                 a real kanban task description. It spans several lines so that the YAML\n\
                 frontmatter + markdown body split is exercised end-to-end.\n\n\
                 - subitem one\n- subitem two\n- subitem three\n"
            )),
        );
        write_entity(&path, &entity, &entity_def)
            .await
            .expect("write synthetic task");
    }
}

/// Time `read_entity_dir` over a freshly-populated directory of `count` tasks.
fn bench_read_entity_dir(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for bench");

    let mut group = c.benchmark_group("read_entity_dir");
    // Disk I/O is the cost we're measuring; a long warmup gives the page cache
    // time to settle so we measure steady-state throughput rather than first-touch.
    //
    // Note: criterion repeatedly calls the bench routine, so the OS page cache
    // will be hot for every iteration after the first. That makes this the
    // *worst case* for the parallelization — disk seeks are removed, leaving
    // only syscall overhead and tokio scheduling. The cold-cache speedup
    // (e.g. cold app launch) will be dramatically larger than what this prints.
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    let entity_def = task_entity_def();

    for &count in &[500usize, 2000usize] {
        // Build the fixture once per scale; criterion times only the read.
        let temp = TempDir::new().expect("tempdir");
        runtime.block_on(populate_tasks(temp.path(), count));

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _count| {
            b.to_async(&runtime).iter(|| async {
                let entities = read_entity_dir(temp.path(), "task", &entity_def)
                    .await
                    .expect("read_entity_dir");
                assert_eq!(entities.len(), count);
                entities
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_read_entity_dir);
criterion_main!(benches);
