//! SearchTasks command — relevance-ranked task search over a filter-scoped corpus.
//!
//! `search tasks` is the relevance-ranking sibling of `list tasks`: an optional
//! DSL `filter` SCOPES the corpus exactly as `list tasks` does (same
//! `parse_filter_expr` / `TaskFilterAdapter` path, same done-column exclusion),
//! and the required `query` string ranks the in-scope tasks. Filter narrows;
//! search ranks within.
//!
//! Each in-scope task becomes a [`Doc`] in an in-memory corpus: title (high
//! weight), description (low weight), and joined tags (mid weight) are lexical
//! fields, while the task's cached embedding feeds the cosine signal. The query
//! is embedded once and ranked against the corpus via
//! [`swissarmyhammer_search::search`]; each [`Hit`] maps back to the enriched
//! task JSON carrying its `score` and per-signal `signals`.
//!
//! Embeddings are never optional. The embedder is a process-lifetime singleton
//! loaded at most once (see [`shared_embedder`]); if it cannot load, the op
//! returns a [`KanbanError`] rather than silently degrading to a lexical-only
//! search. Per-task vectors are cached in the [`EmbeddingCache`] sidecar so a
//! second search over unchanged tasks pays no embedding cost.

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task::embedding_cache::{content_hash, task_embedding_text, EmbeddingCache};
use crate::task::shared::parse_filter_expr;
use crate::task_helpers::{
    enrich_all_task_entities, task_entity_to_rich_json, task_tags, EntitySlugRegistry,
    TaskFilterAdapter,
};
use crate::virtual_tags::default_virtual_tag_registry;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use swissarmyhammer_embedding::{Embedder, TextEmbedder};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};
use swissarmyhammer_search::{search, Doc, Field, Hit, Query, SignalWeights};
use tokio::sync::OnceCell;

/// Default number of ranked tasks returned when the caller does not specify
/// `top_k`. Mirrors `list tasks`' `DEFAULT_PAGE_SIZE` (10) to keep AI tool
/// results lean — at ~200 prompt tokens per enriched task, 10 hits stays well
/// under the budget a single tool call should consume.
pub const DEFAULT_TOP_K: usize = 10;

/// Lexical field weight for a task's title — the strongest relevance signal.
const TITLE_WEIGHT: f32 = 3.0;
/// Lexical field weight for a task's joined tags — a moderate signal.
const TAGS_WEIGHT: f32 = 2.0;
/// Lexical field weight for a task's description body — the weakest signal.
const DESCRIPTION_WEIGHT: f32 = 1.0;

/// Search tasks by relevance, optionally scoped by a DSL filter.
#[operation(
    verb = "search",
    noun = "tasks",
    description = "Search tasks by relevance, optionally scoped by a DSL filter"
)]
#[derive(Debug, Default, Deserialize)]
pub struct SearchTasks {
    /// Free-text relevance query. Required — ranks the in-scope corpus.
    pub query: String,
    /// Optional filter DSL expression (e.g. `#bug && @alice`) that SCOPES the
    /// corpus before ranking, using the same path as `list tasks`.
    pub filter: Option<String>,
    /// Maximum number of ranked hits to return. Defaults to [`DEFAULT_TOP_K`].
    pub top_k: Option<usize>,
}

impl SearchTasks {
    /// Create a new SearchTasks command for `query` with no filter.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            filter: None,
            top_k: None,
        }
    }

    /// Set a filter DSL expression to scope the corpus.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Override the maximum number of ranked hits returned.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = Some(top_k);
        self
    }
}

/// Process-lifetime embedder handle, loaded at most once.
///
/// Reloading the qwen-embedding model on every search is a multi-second cliff
/// that makes interactive search unusable, so the loaded [`Embedder`] is cached
/// in this `OnceCell` for the life of the process. The first caller pays the
/// load cost; every later search reuses the same handle.
static EMBEDDER: OnceCell<Arc<Embedder>> = OnceCell::const_new();

/// Return the process-lifetime embedder, loading it on first use.
///
/// Constructs the default embedder ([`Embedder::default`]) and `load`s it once,
/// caching the loaded handle in [`EMBEDDER`]. A load failure is surfaced as a
/// [`KanbanError`] — `search tasks` never falls back to a lexical-only mode.
async fn shared_embedder() -> Result<Arc<Embedder>, KanbanError> {
    EMBEDDER
        .get_or_try_init(|| async {
            let embedder = Embedder::default()
                .await
                .map_err(|e| KanbanError::parse(format!("failed to create embedder: {e}")))?;
            embedder
                .load()
                .await
                .map_err(|e| KanbanError::parse(format!("failed to load embedder: {e}")))?;
            tracing::info!(
                model = embedder.model_name(),
                "loaded process-lifetime task-search embedder"
            );
            Ok(Arc::new(embedder))
        })
        .await
        .cloned()
}

/// Select the in-scope task entities for a search, mirroring `list tasks`.
///
/// The `terminal` (done) column is excluded by default; when `expr` is set,
/// only tasks matching the DSL filter survive. Slug resolution for
/// `$project`/`@user`/`^task` predicates routes through `slug_registry`,
/// exactly as `ListTasks::execute` does. Returns owned clones of the matching
/// entities so the embedding/Doc-build steps can consume them independently.
///
/// Kept free of any embedding so the scoping logic is unit-testable without a
/// model.
fn in_scope_tasks(
    all_tasks: &[Entity],
    terminal: &str,
    slug_registry: &EntitySlugRegistry,
    expr: &Option<swissarmyhammer_filter_expr::Expr>,
) -> Vec<Entity> {
    all_tasks
        .iter()
        .filter(|t| {
            if t.get_str("position_column") == Some(terminal) {
                return false;
            }
            if let Some(e) = expr {
                if !e.matches(&TaskFilterAdapter::with_registry(t, slug_registry)) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Build the searchable [`Doc`] for one task.
///
/// Fields are weighted title > tags > description as lexical signals; the joined
/// tag string is a lexical field only. The optional `embedding` (the task's
/// cached vector) feeds the cosine signal. NOTE: the embedded text is
/// [`task_embedding_text`] (title + description, tags EXCLUDED) — tags are a
/// lexical-only field, deliberately kept out of the embedding.
///
/// Kept free of any embedding call so Doc construction is unit-testable without
/// a model — the caller supplies the (cached) embedding.
fn build_doc(entity: &Entity, embedding: Option<Vec<f32>>) -> Doc {
    let title = entity.get_str("title").unwrap_or("");
    let description = entity.get_str("body").unwrap_or("");
    let tags_joined = task_tags(entity).join(" ");
    Doc::new(
        entity.id.as_str(),
        vec![
            Field::new(TITLE_WEIGHT, title),
            Field::new(DESCRIPTION_WEIGHT, description),
            Field::new(TAGS_WEIGHT, tags_joined),
        ],
        embedding,
    )
}

/// Build the searchable corpus for `scoped`, lazy-filling the embedding cache.
///
/// For each in-scope task this embeds-or-reuses its vector — a cache hit on
/// `task_embedding_text(title, description)` reuses the stored vector, a miss
/// embeds via `embedder` and writes the vector back through `cache` — then turns
/// the task into a weighted [`Doc`] (see [`build_doc`]) and records its enriched
/// JSON. Returns the `Doc` corpus parallel to a `id → enriched JSON` map for the
/// later map-back. An embed failure propagates as a [`KanbanError`]; a cache
/// write failure is logged but non-fatal.
///
/// Isolated from [`SearchTasks::run`] so the embedding/cache machinery reads as a
/// single step and `run` stays a clear sequence. Takes `cache` by value because
/// [`EmbeddingCache`] is `Send` but not `Sync`: holding it by `&` across the
/// embedder `.await` would make this future non-`Send`, owning it does not.
async fn build_docs_with_embeddings(
    scoped: &[Entity],
    embedder: &Embedder,
    cache: EmbeddingCache,
) -> Result<(Vec<Doc>, std::collections::HashMap<String, Value>), KanbanError> {
    let mut docs: Vec<Doc> = Vec::with_capacity(scoped.len());
    let mut enriched_by_id: std::collections::HashMap<String, Value> =
        std::collections::HashMap::with_capacity(scoped.len());
    for entity in scoped {
        let title = entity.get_str("title").unwrap_or("");
        let description = entity.get_str("body").unwrap_or("");
        let embed_text = task_embedding_text(title, description);
        let hash = content_hash(&embed_text);
        let id = entity.id.as_str();

        let vector = match cache.get(id, &hash) {
            Some(v) => v,
            None => {
                let result = embedder
                    .embed_text(&embed_text)
                    .await
                    .map_err(|e| KanbanError::parse(format!("failed to embed task {id}: {e}")))?;
                let v = result.embedding().to_vec();
                if let Err(e) = cache.put(id, &hash, &v) {
                    tracing::warn!(task = id, error = %e, "failed to cache task embedding");
                }
                v
            }
        };

        docs.push(build_doc(entity, Some(vector)));
        enriched_by_id.insert(id.to_string(), task_entity_to_rich_json(entity));
    }
    Ok((docs, enriched_by_id))
}

/// Map ranked [`Hit`]s back to the `{ tasks, count }` response shape.
///
/// Each hit's id selects its enriched task JSON from `enriched_by_id`; the
/// hit's `score` and per-signal `signals` are attached to that JSON object.
/// Hits whose id is absent from the map are skipped (a task removed between
/// corpus build and map-back). The response mirrors `list tasks` plus the
/// per-hit `score`/`signals`.
///
/// Kept free of any embedding so the map-back is unit-testable without a model.
fn map_hits_to_response(
    hits: &[Hit],
    enriched_by_id: &std::collections::HashMap<String, Value>,
) -> Value {
    let tasks: Vec<Value> = hits
        .iter()
        .filter_map(|hit| {
            let enriched = enriched_by_id.get(&hit.id)?;
            let mut obj = enriched.clone();
            obj["score"] = json!(hit.score);
            obj["signals"] = serde_json::to_value(hit.signals).unwrap_or(Value::Null);
            Some(obj)
        })
        .collect();
    json!({
        "count": tasks.len(),
        "tasks": tasks,
    })
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for SearchTasks {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match self.run(ctx).await {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

impl SearchTasks {
    /// Run the search: scope the corpus, embed-or-cache each Doc, rank, map back.
    async fn run(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        let ectx = ctx.entity_context().await?;
        let all_columns = ectx.list("column").await?;
        let mut all_tasks = ectx.list("task").await?;

        let terminal = all_columns
            .iter()
            .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
            .map(|c| c.id.as_str())
            .unwrap_or("done")
            .to_string();

        let registry = default_virtual_tag_registry();
        enrich_all_task_entities(&mut all_tasks, &terminal, registry);

        let all_projects = ectx.list("project").await?;
        let all_actors = ectx.list("actor").await?;
        let slug_registry = EntitySlugRegistry::build(&all_projects, &all_actors, &all_tasks);

        let expr = parse_filter_expr(self.filter.as_deref())?;
        let scoped = in_scope_tasks(&all_tasks, &terminal, &slug_registry, &expr);

        // Empty corpus → no ranking work, no embedder load.
        if scoped.is_empty() {
            return Ok(json!({ "count": 0, "tasks": [] }));
        }

        // Process-lifetime embedder (loaded at most once). NO lexical-only
        // fallback: a load failure propagates as a KanbanError.
        let embedder = shared_embedder().await?;
        let model_name = embedder.model_name().to_string();
        let dim = embedder.embedding_dimension().unwrap_or(0);

        let cache = EmbeddingCache::open(ctx.search_cache_path(), &model_name, dim)
            .map_err(|e| KanbanError::parse(format!("failed to open embedding cache: {e}")))?;

        // Build a Doc per task, lazy-filling the embedding cache on misses.
        let (docs, enriched_by_id) = build_docs_with_embeddings(&scoped, &embedder, cache).await?;

        // Embed the query once.
        let query_vec = embedder
            .embed_text(&self.query)
            .await
            .map_err(|e| KanbanError::parse(format!("failed to embed query: {e}")))?
            .embedding()
            .to_vec();

        let query = Query::new(self.query.clone())
            .with_embedding(query_vec)
            .with_weights(SignalWeights::default())
            .with_top_k(self.top_k.unwrap_or(DEFAULT_TOP_K));

        let hits = search(&docs, &query);
        Ok(map_hits_to_response(&hits, &enriched_by_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::{AddTask, MoveTask};
    use std::collections::HashMap;
    use swissarmyhammer_search::Signals;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    /// Build the enriched, scoped corpus for a board the same way
    /// `SearchTasks::run` does, but stopping before any embedding. Returns the
    /// in-scope entities so scoping/Doc-build can be asserted without a model.
    async fn scoped_corpus(ctx: &KanbanContext, filter: Option<&str>) -> Vec<Entity> {
        let ectx = ctx.entity_context().await.unwrap();
        let all_columns = ectx.list("column").await.unwrap();
        let mut all_tasks = ectx.list("task").await.unwrap();
        let terminal = all_columns
            .iter()
            .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
            .map(|c| c.id.as_str())
            .unwrap_or("done")
            .to_string();
        let registry = default_virtual_tag_registry();
        enrich_all_task_entities(&mut all_tasks, &terminal, registry);
        let all_projects = ectx.list("project").await.unwrap();
        let all_actors = ectx.list("actor").await.unwrap();
        let slug_registry = EntitySlugRegistry::build(&all_projects, &all_actors, &all_tasks);
        let expr = parse_filter_expr(filter).unwrap();
        in_scope_tasks(&all_tasks, &terminal, &slug_registry, &expr)
    }

    fn titles(entities: &[Entity]) -> Vec<String> {
        entities
            .iter()
            .map(|e| e.get_str("title").unwrap_or("").to_string())
            .collect()
    }

    // --- Filter-scoping --------------------------------------------------

    #[tokio::test]
    async fn no_filter_scopes_to_whole_non_done_board() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Alpha")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Beta")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r3 = AddTask::new("Gamma")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r2["id"].as_str().unwrap(), "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r3["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let scoped = scoped_corpus(&ctx, None).await;
        let scoped_titles = titles(&scoped);
        assert_eq!(scoped.len(), 2, "done task excluded from corpus");
        assert!(scoped_titles.contains(&"Alpha".to_string()));
        assert!(scoped_titles.contains(&"Beta".to_string()));
        assert!(!scoped_titles.contains(&"Gamma".to_string()));
    }

    #[tokio::test]
    async fn tag_filter_narrows_the_corpus() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Bug task")
            .with_description("This is a #bug to fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Plain task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let scoped = scoped_corpus(&ctx, Some("#bug")).await;
        assert_eq!(scoped.len(), 1, "#bug restricts corpus to tagged tasks");
        assert_eq!(scoped[0].get_str("title"), Some("Bug task"));
    }

    // --- Doc construction ------------------------------------------------

    #[tokio::test]
    async fn build_doc_uses_weighted_fields_and_cached_embedding() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Search ranking")
            .with_description("Rank tasks by relevance")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let scoped = scoped_corpus(&ctx, None).await;
        let entity = &scoped[0];

        let embedding = vec![0.5_f32, 0.25, 0.0];
        let doc = build_doc(entity, Some(embedding.clone()));

        assert_eq!(doc.id(), entity.id.as_str());
        assert_eq!(doc.embedding(), Some(&embedding[..]));

        let fields = doc.fields();
        assert_eq!(fields.len(), 3, "title, description, tags");
        // Field 0 = title (high weight).
        assert_eq!(fields[0].text(), "Search ranking");
        assert_eq!(fields[0].weight(), TITLE_WEIGHT);
        // Field 1 = description (low weight).
        assert_eq!(fields[1].text(), "Rank tasks by relevance");
        assert_eq!(fields[1].weight(), DESCRIPTION_WEIGHT);
        // Field 2 = tags (mid weight).
        assert_eq!(fields[2].weight(), TAGS_WEIGHT);
        // Title weight strictly exceeds description weight.
        assert!(fields[0].weight() > fields[1].weight());
    }

    #[tokio::test]
    async fn embed_text_excludes_tags() {
        // The embedded text is task_embedding_text(title, description) — tags
        // are a lexical-only Doc field and must never enter the embedding.
        let (_temp, ctx) = setup().await;
        AddTask::new("Tagged title")
            .with_description("Body mentions #urgent inline")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let scoped = scoped_corpus(&ctx, None).await;
        let entity = &scoped[0];

        let title = entity.get_str("title").unwrap_or("");
        let description = entity.get_str("body").unwrap_or("");
        let embed_text = task_embedding_text(title, description);

        // The Doc's tag field carries the tag lexically...
        let doc = build_doc(entity, None);
        let tag_field = &doc.fields()[2];
        assert!(
            tag_field.text().contains("urgent"),
            "tags are a lexical Doc field"
        );
        // ...but the embedded text is title + description, never the tag list.
        assert_eq!(embed_text, format!("{title}\n{description}"));
        assert!(
            !embed_text.contains("\nurgent") && !embed_text.ends_with("urgent\n"),
            "embed text must be exactly title\\ndescription"
        );
    }

    // --- Map-back --------------------------------------------------------

    #[tokio::test]
    async fn map_hits_to_response_shape_carries_score_and_signals() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("First")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Second")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();
        let id2 = r2["id"].as_str().unwrap().to_string();

        let scoped = scoped_corpus(&ctx, None).await;
        let mut enriched_by_id: HashMap<String, Value> = HashMap::new();
        for e in &scoped {
            enriched_by_id.insert(e.id.as_str().to_string(), task_entity_to_rich_json(e));
        }

        let hits = vec![
            Hit {
                id: id1.clone(),
                score: 0.9,
                signals: Signals {
                    bm25: 0.5,
                    trigram: 0.4,
                    cosine: 0.8,
                },
            },
            Hit {
                id: id2.clone(),
                score: 0.3,
                signals: Signals {
                    bm25: 0.1,
                    trigram: 0.2,
                    cosine: 0.3,
                },
            },
        ];

        let response = map_hits_to_response(&hits, &enriched_by_id);
        assert_eq!(response["count"], 2);
        let tasks = response["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);

        // First hit ranked first, carries its enriched fields plus score/signals.
        assert_eq!(tasks[0]["id"].as_str().unwrap(), id1);
        assert_eq!(tasks[0]["title"], "First");
        assert!((tasks[0]["score"].as_f64().unwrap() - 0.9).abs() < 1e-6);
        assert!((tasks[0]["signals"]["cosine"].as_f64().unwrap() - 0.8).abs() < 1e-6);
        // Enriched fields from task_entity_to_rich_json are preserved.
        assert!(tasks[0].get("ready").is_some());
        assert!(tasks[0].get("filter_tags").is_some());

        assert_eq!(tasks[1]["id"].as_str().unwrap(), id2);
    }

    #[tokio::test]
    async fn map_hits_skips_unknown_ids() {
        // A hit whose task disappeared between corpus build and map-back is
        // dropped, not surfaced as a null entry.
        let enriched_by_id: HashMap<String, Value> = HashMap::new();
        let hits = vec![Hit {
            id: "gone".to_string(),
            score: 1.0,
            signals: Signals {
                bm25: 0.0,
                trigram: 0.0,
                cosine: 0.0,
            },
        }];
        let response = map_hits_to_response(&hits, &enriched_by_id);
        assert_eq!(response["count"], 0);
        assert!(response["tasks"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_board_returns_zero_without_embedder() {
        // No in-scope tasks → empty response, and crucially the embedder is
        // never loaded (the early return happens before shared_embedder()).
        let (_temp, ctx) = setup().await;
        let result = SearchTasks::new("anything")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 0);
        assert!(result["tasks"].as_array().unwrap().is_empty());
    }
}
