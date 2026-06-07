//! Engine-run code_context probes bound from the semantic diff.
//!
//! A **probe** is a [`code_context`](swissarmyhammer_code_context) operation the
//! review *engine* runs on the agent's behalf and injects as ground-truth
//! evidence — never a "please call this tool" instruction the agent can skip
//! (the failure mode of the prose-only review). This module is the catalog of
//! the probes the engine knows how to run, plus the runner that executes a
//! named subset against a real code_context index.
//!
//! # The catalog is data, not branching
//!
//! There are exactly three probes — [`callers`], [`duplicates`], [`similar`] —
//! described by a single static table ([`CATALOG`]) of [`ProbeCatalogEntry`]
//! rows. Each row binds a semantic probe name to the [`ProbeOp`] the runner
//! interprets and the [`ProbeKind`] (`fact` vs `candidate`) the verify guard
//! uses to decide which probes can deterministically refute a claim. The runner
//! is **one** code path parameterized by the entry — there is no per-probe match
//! arm with copy-pasted call code.
//!
//! | Probe        | code_context op            | Subject (from the diff)          | Kind        |
//! |--------------|----------------------------|----------------------------------|-------------|
//! | `callers`    | `get callgraph` (inbound)  | each **added** symbol            | `fact`      |
//! | `duplicates` | `find duplicates`          | each changed file + changed set  | `fact`      |
//! | `similar`    | `search code` (semantic)   | each **added** function body     | `candidate` |
//!
//! # Reuse, never reimplement
//!
//! The runner calls [`swissarmyhammer_code_context::find_duplicates`],
//! [`swissarmyhammer_code_context::get_callgraph`], and
//! [`swissarmyhammer_code_context::search_code`] as a library. It does not
//! reimplement duplicate detection, call-graph traversal, or semantic search.
//! The one piece of logic that lives here — and must — is the **changed-set**
//! duplicate comparison: a HEAD-based index does not contain another
//! just-changed file, so the same block pasted into two new unindexed files
//! would be missed by an index-only `find_duplicates`. The `duplicates` probe
//! therefore also compares the changed blocks against each other.
//!
//! # Resolving the index
//!
//! Probes are read-only, indexed, and bounded. The runner never calls
//! `current_dir()`: the caller resolves the code_context connection and CWD from
//! the session/work-dir and passes the connection in, alongside an
//! [`TextEmbedder`] used to embed query bodies (for `similar`) and changed
//! blocks (for the changed-set `duplicates` comparison).

use std::collections::BTreeSet;

use model_embedding::{cosine_similarity, TextEmbedder};
use rusqlite::Connection;
use serde::Serialize;

use swissarmyhammer_code_context::{
    find_duplicates_in, get_callgraph, load_all_embedded_chunks, search_loaded, CallGraphDirection,
    CallGraphOptions, FindDuplicatesOptions, LoadedChunk, SearchCodeOptions,
};

use crate::error::AvpError;

/// Whether a probe yields a deterministically checkable fact or an
/// agent-interpreted candidate.
///
/// The verify guard uses this to decide which probes can *refute* a finding
/// (only [`ProbeKind::Fact`]) and which only *inform* it
/// ([`ProbeKind::Candidate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeKind {
    /// A guard-able fact: the probe's rows either confirm or refute a claim.
    Fact,
    /// A reuse candidate: informative context the agent interprets, never a
    /// deterministic refutation.
    Candidate,
}

/// The code_context operation a catalog entry binds to.
///
/// This is the discriminator the runner interprets to derive arguments from the
/// diff and dispatch the right library op — the *only* thing that varies between
/// catalog entries' execution. It exists so the catalog stays a data table and
/// the runner stays a single parameterized code path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOp {
    /// `get callgraph` (inbound) bound to each added symbol.
    Callers,
    /// `find duplicates` bound to each changed file, plus the changed-set
    /// comparison.
    Duplicates,
    /// `search code` (semantic) bound to each added function body, self
    /// excluded.
    Similar,
}

/// One row of the probe catalog: a semantic name bound to an op and a kind.
#[derive(Debug, Clone, Copy)]
pub struct ProbeCatalogEntry {
    /// The semantic name validators declare (`callers`, `duplicates`,
    /// `similar`).
    pub name: &'static str,
    /// The kind of evidence this probe yields.
    pub kind: ProbeKind,
    /// The code_context op this probe runs.
    pub op: ProbeOp,
}

/// The complete probe catalog — exactly three entries.
///
/// Adding a probe is adding a row here plus an arm in [`ProbeOp`]; the runner
/// does not change. This is the single source of truth for both
/// [`probe_exists`] and [`run_probes`].
pub static CATALOG: &[ProbeCatalogEntry] = &[
    ProbeCatalogEntry {
        name: "callers",
        kind: ProbeKind::Fact,
        op: ProbeOp::Callers,
    },
    ProbeCatalogEntry {
        name: "duplicates",
        kind: ProbeKind::Fact,
        op: ProbeOp::Duplicates,
    },
    ProbeCatalogEntry {
        name: "similar",
        kind: ProbeKind::Candidate,
        op: ProbeOp::Similar,
    },
];

/// Look up a catalog entry by its semantic name.
fn catalog_entry(name: &str) -> Option<&'static ProbeCatalogEntry> {
    CATALOG.iter().find(|e| e.name == name)
}

/// Whether `name` is a real probe in the catalog.
///
/// Called by the `check validators` linter to validate a validator's declared
/// `probes` against the catalog before the engine runs.
pub fn probe_exists(name: &str) -> bool {
    catalog_entry(name).is_some()
}

/// A changed entity derived from the git semantic diff.
///
/// Mirrors the subset of the git `get diff` tool's `ChangeEntry` the probe
/// runner needs to bind arguments: *what* changed ([`Self::entity_type`] /
/// [`Self::entity_name`]), *where* ([`Self::file_path`]), *how*
/// ([`Self::change_type`]), and the new source ([`Self::after_content`], the
/// added/modified body). The runner owns this minimal shape rather than
/// depending on the tool crate (which depends on this engine crate).
#[derive(Debug, Clone)]
pub struct ChangeEntry {
    /// `"added"`, `"modified"`, `"deleted"`, ... — the change classification.
    pub change_type: String,
    /// The kind of entity (`"function"`, `"struct"`, ...).
    pub entity_type: String,
    /// The entity's name (the symbol name for `callers`).
    pub entity_name: String,
    /// The file the entity lives in.
    pub file_path: String,
    /// The entity's source after the change, when available (the body
    /// `similar` and the changed-set `duplicates` comparison embed).
    pub after_content: Option<String>,
}

impl ChangeEntry {
    /// Whether this entry is an *added* entity (`change_type == "added"`).
    fn is_added(&self) -> bool {
        self.change_type.eq_ignore_ascii_case("added")
    }

    /// Whether this entry looks like a function/method (for `similar`, which
    /// binds to added *function bodies*).
    fn is_function(&self) -> bool {
        let t = self.entity_type.to_ascii_lowercase();
        t.contains("function") || t.contains("method")
    }
}

/// The change set a `run_probes` invocation is bound to.
///
/// Carries the diff's changed entities. The runner derives every probe's
/// arguments from these — there is no other input axis.
#[derive(Debug, Clone, Default)]
pub struct FileChange {
    /// The changed entities from the semantic diff.
    pub entities: Vec<ChangeEntry>,
}

impl FileChange {
    /// Build a change set from its entities.
    pub fn new(entities: Vec<ChangeEntry>) -> Self {
        Self { entities }
    }
}

/// One evidence row a probe produced.
///
/// Structured so the same value renders as human-facing evidence *and* is
/// machine-checkable by the verify guard. Fields are optional because the three
/// ops surface different shapes (a duplicate has a similarity; a caller has a
/// call site; a reuse candidate has both).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeRow {
    /// The file the evidence points at (the duplicate's location, the caller's
    /// file, the similar chunk's file).
    pub file_path: String,
    /// The symbol/qualified path at that location, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The starting line of the evidence, when known (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Cosine similarity, for the embedding-backed probes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    /// A short human-readable detail (e.g. the matched chunk text, truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The result of running one probe against one bound target.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeResult {
    /// The probe's semantic name (`callers` / `duplicates` / `similar`).
    pub name: String,
    /// The probe's kind, copied from the catalog so consumers don't re-look-up.
    pub kind: ProbeKind,
    /// What the probe was bound to (the added symbol, the changed file, the
    /// added body) — the diff-derived subject.
    pub target: String,
    /// The evidence rows the probe produced (possibly empty — an empty
    /// `callers` result is itself a guard-able fact: "no inbound callers").
    pub rows: Vec<ProbeRow>,
}

/// The full set of probe results from one [`run_probes`] call.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ProbeResults {
    /// One entry per (probe, bound target) executed.
    pub results: Vec<ProbeResult>,
}

impl ProbeResults {
    /// Whether any probe produced any evidence row.
    pub fn is_empty(&self) -> bool {
        self.results.iter().all(|r| r.rows.is_empty())
    }
}

/// Truncate a chunk of source to a short single-line detail string.
fn snippet(text: &str) -> String {
    const MAX: usize = 120;
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > MAX {
        let truncated: String = one_line.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        one_line
    }
}

/// Embed one text, returning an [`AvpError`] on embedder failure.
async fn embed(embedder: &dyn TextEmbedder, text: &str) -> Result<Vec<f32>, AvpError> {
    let result = embedder
        .embed_text(text)
        .await
        .map_err(|e| AvpError::Context(format!("probe embedding failed: {e}")))?;
    Ok(result.embedding().to_vec())
}

/// Run the named probes against the code_context index, binding each probe's
/// arguments from the diff's changed entities.
///
/// `conn` is a connection to the code_context index the *caller* resolved from
/// the session/work-dir (never `current_dir()`). `embedder` embeds query bodies
/// and changed blocks; tests pass a deterministic mock, production passes the
/// real model.
///
/// # Errors
///
/// Returns [`AvpError::Validator`] if any requested name is not a real probe
/// (validated against [`CATALOG`]), or [`AvpError::Context`] if the index or
/// embedder fails. A probe that simply finds nothing is *not* an error — it
/// yields a [`ProbeResult`] with empty `rows`.
pub async fn run_probes(
    probe_names: &[String],
    file_change: &FileChange,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
) -> Result<ProbeResults, AvpError> {
    // Validate every requested name against the catalog up front so an unknown
    // probe is a clear error rather than a silent no-op.
    let entries = resolve_entries(probe_names)?;

    // The embedding-backed probes (`duplicates`, `similar`) compare against the
    // whole indexed corpus. Load it ONCE here and share it across every changed
    // file and every added body, rather than re-materializing the entire
    // `ts_chunks` embedding table inside `find_duplicates`/`search_code` per
    // call — for a large index that repeated multi-hundred-MB load is what OOMed
    // the review. The `callers` probe is index-graph-only and needs no corpus, so
    // a callers-only run skips the load entirely.
    let needs_corpus = entries
        .iter()
        .any(|e| matches!(e.op, ProbeOp::Duplicates | ProbeOp::Similar));
    let corpus: Vec<LoadedChunk> = if needs_corpus {
        load_all_embedded_chunks(conn)
            .map_err(|e| AvpError::Context(format!("failed to load embedding corpus: {e}")))?
    } else {
        Vec::new()
    };

    let mut results = Vec::new();
    for entry in entries {
        let mut probe_results = match entry.op {
            ProbeOp::Callers => run_callers(entry, file_change, conn)?,
            ProbeOp::Duplicates => run_duplicates(entry, file_change, &corpus, embedder).await?,
            ProbeOp::Similar => run_similar(entry, file_change, &corpus, embedder).await?,
        };
        results.append(&mut probe_results);
    }

    Ok(ProbeResults { results })
}

/// Resolve probe names to catalog entries, erroring on the first unknown name.
fn resolve_entries(probe_names: &[String]) -> Result<Vec<&'static ProbeCatalogEntry>, AvpError> {
    probe_names
        .iter()
        .map(|name| {
            catalog_entry(name).ok_or_else(|| AvpError::Validator {
                validator: name.clone(),
                message: format!(
                    "unknown probe '{name}'; the catalog defines: {}",
                    CATALOG
                        .iter()
                        .map(|e| e.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })
        })
        .collect()
}

/// `callers`: `get callgraph` (inbound) on each added symbol.
///
/// An added symbol that the index cannot resolve (e.g. a brand-new, not-yet-
/// indexed symbol) has, by definition, no inbound callers — that resolution
/// miss is the guard-able *fact* "no callers", not an error.
fn run_callers(
    entry: &ProbeCatalogEntry,
    file_change: &FileChange,
    conn: &Connection,
) -> Result<Vec<ProbeResult>, AvpError> {
    let mut out = Vec::new();
    for added in file_change.entities.iter().filter(|e| e.is_added()) {
        let options = CallGraphOptions {
            symbol: added.entity_name.clone(),
            direction: CallGraphDirection::Inbound,
            max_depth: 1,
        };
        let rows = match get_callgraph(conn, &options) {
            Ok(graph) => graph
                .edges
                .iter()
                .map(|edge| ProbeRow {
                    file_path: edge.caller.file_path.clone(),
                    symbol: Some(edge.caller.name.clone()),
                    line: None,
                    similarity: None,
                    detail: None,
                })
                .collect(),
            // Unresolvable symbol → no inbound callers (a fact), not an error.
            Err(_) => Vec::new(),
        };
        out.push(ProbeResult {
            name: entry.name.to_string(),
            kind: entry.kind,
            target: added.entity_name.clone(),
            rows,
        });
    }
    Ok(out)
}

/// `duplicates`: `find duplicates` on each changed file PLUS a changed-set
/// comparison of the changed blocks against each other.
///
/// The index-backed `find_duplicates` only sees files already in the index; the
/// changed-set pass catches a block pasted into two new unindexed files.
async fn run_duplicates(
    entry: &ProbeCatalogEntry,
    file_change: &FileChange,
    corpus: &[LoadedChunk],
    embedder: &dyn TextEmbedder,
) -> Result<Vec<ProbeResult>, AvpError> {
    let options = FindDuplicatesOptions::default();
    let mut out = Vec::new();

    // One result per changed file: index-backed duplicates of that file's
    // blocks, found via the library op (never reimplemented) against the shared
    // pre-loaded corpus.
    let changed_files: BTreeSet<&str> = file_change
        .entities
        .iter()
        .map(|e| e.file_path.as_str())
        .collect();
    for file in &changed_files {
        let result = find_duplicates_in(corpus, file, &options);
        let rows = result
            .groups
            .iter()
            .flat_map(|group| {
                group.duplicates.iter().map(|dup| ProbeRow {
                    file_path: dup.chunk.file_path.clone(),
                    symbol: dup.chunk.symbol_path.clone(),
                    line: Some(dup.chunk.start_line),
                    similarity: Some(dup.similarity),
                    detail: Some(snippet(&dup.chunk.text)),
                })
            })
            .collect();
        out.push(ProbeResult {
            name: entry.name.to_string(),
            kind: entry.kind,
            target: (*file).to_string(),
            rows,
        });
    }

    // Changed-set comparison: embed each changed block and compare blocks
    // against each other so a paste into two new unindexed files is flagged.
    out.push(changed_set_duplicates(entry, file_change, embedder, options.min_similarity).await?);

    Ok(out)
}

/// Compare the changed blocks against each other, flagging near-identical
/// pairs the index cannot have seen (two brand-new files).
async fn changed_set_duplicates(
    entry: &ProbeCatalogEntry,
    file_change: &FileChange,
    embedder: &dyn TextEmbedder,
    min_similarity: f32,
) -> Result<ProbeResult, AvpError> {
    // Embed every changed block that carries source.
    let mut blocks: Vec<(&ChangeEntry, Vec<f32>)> = Vec::new();
    for entity in &file_change.entities {
        if let Some(body) = &entity.after_content {
            let embedding = embed(embedder, body).await?;
            blocks.push((entity, embedding));
        }
    }

    let mut rows = Vec::new();
    for i in 0..blocks.len() {
        for j in (i + 1)..blocks.len() {
            let (a, emb_a) = &blocks[i];
            let (b, emb_b) = &blocks[j];
            // A block is only a *changed-set* duplicate if it sits in a
            // different file — two entities in one file are not a paste.
            if a.file_path == b.file_path {
                continue;
            }
            let sim = cosine_similarity(emb_a, emb_b);
            if sim >= min_similarity {
                rows.push(ProbeRow {
                    file_path: b.file_path.clone(),
                    symbol: Some(b.entity_name.clone()),
                    line: None,
                    similarity: Some(sim),
                    detail: Some(format!(
                        "changed-set duplicate of {} in {}",
                        a.entity_name, a.file_path
                    )),
                });
            }
        }
    }

    Ok(ProbeResult {
        name: entry.name.to_string(),
        kind: entry.kind,
        target: "<changed-set>".to_string(),
        rows,
    })
}

/// `similar`: `search code` (semantic) on each added function body, self
/// excluded.
async fn run_similar(
    entry: &ProbeCatalogEntry,
    file_change: &FileChange,
    corpus: &[LoadedChunk],
    embedder: &dyn TextEmbedder,
) -> Result<Vec<ProbeResult>, AvpError> {
    let mut out = Vec::new();
    for added in file_change
        .entities
        .iter()
        .filter(|e| e.is_added() && e.is_function())
    {
        let Some(body) = &added.after_content else {
            continue;
        };
        let query = embed(embedder, body).await?;
        // top_k+1 so we can drop the self hit and still return up to top_k.
        let options = SearchCodeOptions {
            top_k: DEFAULT_SIMILAR_TOP_K + 1,
            ..Default::default()
        };
        let matches = search_loaded(corpus, &query, &options);

        let rows: Vec<ProbeRow> = matches
            .iter()
            .filter(|m| !is_self_match(m, added))
            .take(DEFAULT_SIMILAR_TOP_K)
            .map(|m| ProbeRow {
                file_path: m.file_path.clone(),
                symbol: m.symbol_path.clone(),
                line: Some(m.start_line),
                similarity: Some(m.similarity),
                detail: Some(snippet(&m.text)),
            })
            .collect();

        out.push(ProbeResult {
            name: entry.name.to_string(),
            kind: entry.kind,
            target: added.entity_name.clone(),
            rows,
        });
    }
    Ok(out)
}

/// Default number of reuse candidates `similar` returns after self-exclusion.
const DEFAULT_SIMILAR_TOP_K: usize = 5;

/// Whether a search hit is the added entity itself (so `similar` excludes it).
///
/// The self hit is the chunk in the same file whose symbol path matches the
/// added entity's name (semantic search will rank the body against itself
/// highest once it is indexed).
fn is_self_match(m: &swissarmyhammer_code_context::SearchCodeMatch, added: &ChangeEntry) -> bool {
    if m.file_path != added.file_path {
        return false;
    }
    match &m.symbol_path {
        Some(path) => path == &added.entity_name || path.ends_with(&added.entity_name),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use model_embedding::mock::MockEmbedder;
    use rusqlite::Connection;
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};
    use swissarmyhammer_code_context::serialize_embedding;

    /// Embedding dimension used by the seeded index and the mock embedder.
    const DIM: usize = 4;

    /// Open a real, schema-applied, in-memory code_context index connection.
    ///
    /// This is the production schema (`create_schema`) on a real SQLite
    /// connection — the ops run against exactly what they run against in
    /// production, just seeded deterministically instead of via the 600MB
    /// embedder.
    fn index_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    /// Register a file in `indexed_files`. `ts_chunks` / `lsp_symbols` carry a
    /// foreign key onto this table (and `configure_connection` enforces it), so
    /// every seeded chunk/symbol needs its file registered first.
    fn seed_file(conn: &Connection, file_path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
             VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
            rusqlite::params![file_path],
        )
        .unwrap();
    }

    /// Seed a `ts_chunks` row with an embedding so `find_duplicates` /
    /// `search_code` (which filter on `embedding IS NOT NULL`) can see it.
    fn seed_chunk(
        conn: &Connection,
        file_path: &str,
        symbol_path: &str,
        text: &str,
        embedding: &[f32],
    ) {
        seed_file(conn, file_path);
        let blob = serialize_embedding(embedding);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
            rusqlite::params![file_path, text.len() as i64, symbol_path, text, blob],
        )
        .unwrap();
    }

    /// Seed an `lsp_symbols` row.
    fn seed_symbol(conn: &Connection, id: &str, name: &str, file_path: &str) {
        seed_file(conn, file_path);
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
             VALUES (?1, ?2, 12, ?3, 1, 0, 5, 0, NULL)",
            rusqlite::params![id, name, file_path],
        )
        .unwrap();
    }

    /// Seed an `lsp_call_edges` row (caller -> callee).
    fn seed_call_edge(
        conn: &Connection,
        caller_id: &str,
        callee_id: &str,
        caller_file: &str,
        callee_file: &str,
    ) {
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
             VALUES (?1, ?2, ?3, ?4, 'lsp', '[]')",
            rusqlite::params![caller_id, callee_id, caller_file, callee_file],
        )
        .unwrap();
    }

    /// A long-enough body that clears the default `min_chunk_bytes` (100).
    fn body(label: &str) -> String {
        format!(
            "pub fn {label}(input: &[f64]) -> f64 {{ \
             let mut total = 0.0; for value in input {{ total += value * value; }} \
             total / input.len() as f64 }}"
        )
    }

    fn added_fn(name: &str, file: &str) -> ChangeEntry {
        ChangeEntry {
            change_type: "added".to_string(),
            entity_type: "function".to_string(),
            entity_name: name.to_string(),
            file_path: file.to_string(),
            after_content: Some(body(name)),
        }
    }

    // --- catalog --------------------------------------------------------

    #[test]
    fn catalog_has_exactly_the_three_probes_with_their_kinds() {
        let names: Vec<_> = CATALOG.iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["callers", "duplicates", "similar"]);

        assert_eq!(catalog_entry("callers").unwrap().kind, ProbeKind::Fact);
        assert_eq!(catalog_entry("duplicates").unwrap().kind, ProbeKind::Fact);
        assert_eq!(catalog_entry("similar").unwrap().kind, ProbeKind::Candidate);
    }

    #[test]
    fn probe_exists_is_true_for_catalog_names_and_false_otherwise() {
        assert!(probe_exists("callers"));
        assert!(probe_exists("duplicates"));
        assert!(probe_exists("similar"));
        assert!(!probe_exists("search_symbol"));
        assert!(!probe_exists("blastradius"));
        assert!(!probe_exists("nonsense"));
    }

    // --- run_probes errors ---------------------------------------------

    #[tokio::test]
    async fn run_probes_errors_on_unknown_name() {
        let conn = index_conn();
        let embedder = MockEmbedder::new(DIM);
        let change = FileChange::default();

        let err = run_probes(&["bogus".to_string()], &change, &conn, &embedder)
            .await
            .unwrap_err();

        match err {
            AvpError::Validator { validator, message } => {
                assert_eq!(validator, "bogus");
                assert!(message.contains("unknown probe"), "got: {message}");
            }
            other => panic!("expected Validator error, got: {other:?}"),
        }
    }

    // --- callers --------------------------------------------------------

    #[tokio::test]
    async fn callers_returns_empty_inbound_for_a_new_uncalled_symbol() {
        let conn = index_conn();
        // The added symbol is not in the index at all — no inbound callers.
        let embedder = MockEmbedder::new(DIM);
        let change = FileChange::new(vec![added_fn("brand_new_fn", "src/new.rs")]);

        let results = run_probes(&["callers".to_string()], &change, &conn, &embedder)
            .await
            .unwrap();

        let callers: Vec<_> = results
            .results
            .iter()
            .filter(|r| r.name == "callers")
            .collect();
        assert_eq!(callers.len(), 1, "one result per added symbol");
        assert_eq!(callers[0].target, "brand_new_fn");
        assert_eq!(callers[0].kind, ProbeKind::Fact);
        assert!(
            callers[0].rows.is_empty(),
            "a new uncalled symbol has no inbound callers"
        );
    }

    #[tokio::test]
    async fn callers_reports_inbound_call_sites_when_the_symbol_is_called() {
        let conn = index_conn();
        seed_symbol(&conn, "callee-1", "target_fn", "src/lib.rs");
        seed_symbol(&conn, "caller-1", "uses_target", "src/caller.rs");
        seed_call_edge(&conn, "caller-1", "callee-1", "src/caller.rs", "src/lib.rs");

        let embedder = MockEmbedder::new(DIM);
        let change = FileChange::new(vec![ChangeEntry {
            change_type: "added".to_string(),
            entity_type: "function".to_string(),
            entity_name: "target_fn".to_string(),
            file_path: "src/lib.rs".to_string(),
            after_content: Some(body("target_fn")),
        }]);

        let results = run_probes(&["callers".to_string()], &change, &conn, &embedder)
            .await
            .unwrap();

        let callers = results
            .results
            .iter()
            .find(|r| r.name == "callers")
            .unwrap();
        assert_eq!(callers.rows.len(), 1, "one inbound caller");
        assert_eq!(callers.rows[0].file_path, "src/caller.rs");
        assert_eq!(callers.rows[0].symbol.as_deref(), Some("uses_target"));
    }

    // --- duplicates (index-backed) -------------------------------------

    #[tokio::test]
    async fn duplicates_returns_the_index_hit_for_a_duplicated_function() {
        let conn = index_conn();
        let shared = body("compute");
        // The changed file and an existing indexed file share the same body
        // (identical embedding) → a duplicate.
        seed_chunk(
            &conn,
            "src/new.rs",
            "compute",
            &shared,
            &[1.0, 0.0, 0.0, 0.0],
        );
        seed_chunk(
            &conn,
            "src/existing.rs",
            "old_compute",
            &shared,
            &[1.0, 0.0, 0.0, 0.0],
        );

        let embedder = MockEmbedder::new(DIM);
        let change = FileChange::new(vec![ChangeEntry {
            change_type: "added".to_string(),
            entity_type: "function".to_string(),
            entity_name: "compute".to_string(),
            file_path: "src/new.rs".to_string(),
            after_content: Some(shared.clone()),
        }]);

        let results = run_probes(&["duplicates".to_string()], &change, &conn, &embedder)
            .await
            .unwrap();

        // The per-file result for src/new.rs must surface the existing dup.
        let file_result = results
            .results
            .iter()
            .find(|r| r.name == "duplicates" && r.target == "src/new.rs")
            .expect("a duplicates result bound to the changed file");
        assert!(
            file_result
                .rows
                .iter()
                .any(|row| row.file_path == "src/existing.rs"),
            "expected the index hit at src/existing.rs, got: {:?}",
            file_result.rows
        );
    }

    // --- duplicates (changed set) --------------------------------------

    #[tokio::test]
    async fn duplicates_flags_a_block_pasted_into_two_new_unindexed_files() {
        // Neither file is in the index — only the changed set carries them.
        let conn = index_conn();
        let pasted = body("helper");
        let embedder = MockEmbedder::new(DIM);

        let change = FileChange::new(vec![
            ChangeEntry {
                change_type: "added".to_string(),
                entity_type: "function".to_string(),
                entity_name: "helper_a".to_string(),
                file_path: "src/a.rs".to_string(),
                after_content: Some(pasted.clone()),
            },
            ChangeEntry {
                change_type: "added".to_string(),
                entity_type: "function".to_string(),
                entity_name: "helper_b".to_string(),
                file_path: "src/b.rs".to_string(),
                after_content: Some(pasted.clone()),
            },
        ]);

        let results = run_probes(&["duplicates".to_string()], &change, &conn, &embedder)
            .await
            .unwrap();

        let changed_set = results
            .results
            .iter()
            .find(|r| r.name == "duplicates" && r.target == "<changed-set>")
            .expect("a changed-set duplicates result");
        assert!(
            !changed_set.rows.is_empty(),
            "the same block in two new files must be flagged despite neither being indexed"
        );
        assert!(
            changed_set
                .rows
                .iter()
                .any(|row| row.file_path == "src/b.rs"),
            "the changed-set dup should point at the sibling file, got: {:?}",
            changed_set.rows
        );
    }

    // --- similar --------------------------------------------------------

    #[tokio::test]
    async fn similar_returns_an_existing_util_and_excludes_self() {
        let conn = index_conn();
        let reimplemented = body("my_mse");
        // The added function's own (already-indexed) chunk — must be excluded.
        seed_chunk(
            &conn,
            "src/new.rs",
            "my_mse",
            &reimplemented,
            &[1.0, 0.0, 0.0, 0.0],
        );
        // An existing util with the same embedding — the reuse candidate.
        seed_chunk(
            &conn,
            "src/util.rs",
            "mean_squared_error",
            &reimplemented,
            &[1.0, 0.0, 0.0, 0.0],
        );

        // Mock embedder returns a constant vector for every query; seed the
        // chunks with that same vector so cosine == 1.0 and both rank top.
        let embedder = MockEmbedder::new(DIM);
        // MockEmbedder returns [0.1; DIM]; re-seed chunks to match so the
        // query (also [0.1; DIM]) is maximally similar to both.
        conn.execute("DELETE FROM ts_chunks", []).unwrap();
        let query_vec = vec![0.1_f32; DIM];
        seed_chunk(&conn, "src/new.rs", "my_mse", &reimplemented, &query_vec);
        seed_chunk(
            &conn,
            "src/util.rs",
            "mean_squared_error",
            &reimplemented,
            &query_vec,
        );

        let change = FileChange::new(vec![ChangeEntry {
            change_type: "added".to_string(),
            entity_type: "function".to_string(),
            entity_name: "my_mse".to_string(),
            file_path: "src/new.rs".to_string(),
            after_content: Some(reimplemented.clone()),
        }]);

        let results = run_probes(&["similar".to_string()], &change, &conn, &embedder)
            .await
            .unwrap();

        let similar = results
            .results
            .iter()
            .find(|r| r.name == "similar" && r.target == "my_mse")
            .expect("a similar result bound to the added body");
        assert_eq!(similar.kind, ProbeKind::Candidate);
        assert!(
            similar
                .rows
                .iter()
                .any(|row| row.file_path == "src/util.rs"),
            "expected the existing util as a reuse candidate, got: {:?}",
            similar.rows
        );
        assert!(
            !similar.rows.iter().any(|row| row.file_path == "src/new.rs"),
            "similar must exclude the added entity itself, got: {:?}",
            similar.rows
        );
    }
}
