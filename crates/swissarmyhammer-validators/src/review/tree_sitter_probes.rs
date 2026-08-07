//! Tree-sitter probes: review evidence computed from a file's own parse.
//!
//! The second probe family beside the code_context ops catalogued in
//! [`probes`](crate::review::probes). Where a
//! [`ProbeOp`](crate::review::probes::ProbeOp) probe binds a code_context
//! library op to the diff, a **tree-sitter probe** is plain logic over one
//! changed file and its parse: implement [`TreeSitterProbe`], and the file, its
//! parse, and the parse of its base revision arrive as a
//! [`TreeSitterProbeContext`].
//!
//! # One roster, one parse
//!
//! Grammars and language routing come from
//! [`swissarmyhammer_sem::parser::plugins::code::parse_code`] — the same table
//! the entity extractor and the `complexity` probe read. This module adds no
//! grammar list of its own.
//!
//! Each `(file, revision)` under review is parsed **once per review** by the
//! shared parse cache, before any probe runs, and every probe that reads that
//! file is handed the same parse. The sharing is structural rather than a
//! convention: [`TreeSitterProbe::run`] receives parses and has nothing to
//! parse with.
//!
//! # Registration
//!
//! The `TREE_SITTER_PROBES` roster holds the implementations. The probe catalog
//! builds a row from each one, reading the name and kind off the implementation
//! itself, so a validator declares a tree-sitter probe in its `probes:` list
//! exactly as it declares `complexity`, and `check validators` sees it through
//! the same [`probe_exists`](crate::review::probes::probe_exists).

use std::collections::BTreeMap;

use swissarmyhammer_sem::parser::plugins::code::{parse_code, ParsedCode};

use crate::review::probes::{
    is_function_entity_type, not_computed_row, per_file_results, FileChange, ProbeKind,
    ProbeResult, ProbeRow,
};

/// The detail a tree-sitter probe row carries when a file has no parse.
///
/// A file whose language is absent from the grammar roster must read as
/// **unknown**, never as "this probe found nothing here" — the same contract
/// the `complexity` probe holds with its own not-computed row. Emitting this
/// row keeps the result non-empty, so the verify guard cannot mistake an
/// unparsed file for a probe that came back clean.
pub const TREE_SITTER_NOT_PARSED: &str =
    "tree-sitter probe not computed — this language has no grammar mapping; judge from the source";

/// Which revision of a file under review a parse belongs to.
///
/// A diff-aware probe compares the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Revision {
    /// The file at the review's base revision.
    Before,
    /// The file as it stands under review.
    After,
}

/// One revision of one file: its source text and the parse of that text.
///
/// Copied freely — both halves are borrows of values the runner owns.
#[derive(Debug, Clone, Copy)]
pub struct ParsedRevision<'a> {
    source: &'a str,
    parsed: &'a ParsedCode,
}

impl<'a> ParsedRevision<'a> {
    /// The file's source text at this revision.
    ///
    /// The text the parse was made from, so node byte ranges index into it.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// The tree-sitter parse of [`Self::source`].
    pub fn parsed(&self) -> &'a ParsedCode {
        self.parsed
    }
}

/// The input to one [`TreeSitterProbe`] run: one file under review, parsed.
#[derive(Debug, Clone, Copy)]
pub struct TreeSitterProbeContext<'a> {
    path: &'a str,
    after: ParsedRevision<'a>,
    before: Option<ParsedRevision<'a>>,
}

impl<'a> TreeSitterProbeContext<'a> {
    /// The path of the file under review.
    pub fn path(&self) -> &'a str {
        self.path
    }

    /// The file as it stands under review, parsed.
    pub fn after(&self) -> ParsedRevision<'a> {
        self.after
    }

    /// The file at the review's base revision, parsed.
    ///
    /// `None` when the change added the file, when the scope carries no base
    /// revision at all (a glob or whole-file review), or when the base
    /// revision's source did not parse. A diff-aware probe must treat `None` as
    /// "there is nothing to compare against", never as "the file was empty".
    pub fn before(&self) -> Option<ParsedRevision<'a>> {
        self.before
    }
}

/// The bound that seals [`TreeSitterProbe`].
///
/// Private, so only this module and its submodules can name [`Sealed`] and
/// therefore only they can implement the probe trait. That is where every probe
/// belongs: a probe is useful only once [`TREE_SITTER_PROBES`] registers it, and
/// that roster lives here. Sealing also keeps the trait free to gain a method
/// without breaking a downstream implementation that could never have worked.
///
/// [`Sealed`]: sealed::Sealed
mod sealed {
    /// The private supertrait of [`TreeSitterProbe`](super::TreeSitterProbe).
    pub trait Sealed {}
}

/// A review probe whose evidence is computed from a file's tree-sitter parse.
///
/// An implementation declares its [`name`](Self::name) and
/// [`kind`](Self::kind); the probe catalog builds its row from those, so the
/// name a validator declares and the name the probe answers to are one value.
///
/// The runner owns parsing. `run` is handed the parses it needs and must never
/// parse anything itself, because that is what keeps the per-review parse count
/// at one per file per revision however many probes read the file.
///
/// **Sealed.** A new probe is written in this module or a submodule of it, and
/// implements `sealed::Sealed` beside `TreeSitterProbe`; no other crate can
/// implement it.
pub trait TreeSitterProbe: sealed::Sealed + std::fmt::Debug + Send + Sync {
    /// The semantic name validators declare in their `probes:` list.
    ///
    /// Unique across the whole probe catalog, code_context ops included.
    fn name(&self) -> &'static str;

    /// Whether this probe's rows are guard-able facts or agent-read candidates.
    fn kind(&self) -> ProbeKind;

    /// The evidence rows this probe finds in one parsed file.
    ///
    /// An empty result is a positive measurement — "this probe found nothing in
    /// this file" — which the verify guard may use to refute a claim, so return
    /// rows only for what the parse actually shows.
    fn run(&self, context: &TreeSitterProbeContext<'_>) -> Vec<ProbeRow>;
}

/// The per-review tree-sitter parse cache: one parse per file per revision.
///
/// [`Self::prime`] fills the cache in a single pass BEFORE any probe runs, and
/// probes only ever read it. That ordering is what makes the sharing
/// structural: there is no path by which a probe can cause a second parse.
#[derive(Debug, Default)]
pub(crate) struct ParseCache {
    before: BTreeMap<String, Option<ParsedCode>>,
    after: BTreeMap<String, Option<ParsedCode>>,
}

impl ParseCache {
    /// Parse every revision of every file in `file_change`, once each.
    pub(crate) fn prime(&mut self, file_change: &FileChange) {
        Self::fill(&mut self.after, &file_change.sources, Revision::After);
        Self::fill(
            &mut self.before,
            &file_change.before_sources,
            Revision::Before,
        );
    }

    /// The parse of `path` at `revision`.
    ///
    /// `None` when the review does not carry that revision of the file, or when
    /// its language has no grammar in the shared roster.
    pub(crate) fn parsed_at(&self, path: &str, revision: Revision) -> Option<&ParsedCode> {
        self.revision(revision).get(path)?.as_ref()
    }

    /// How many `(file, revision)` pairs this cache holds a parse attempt for.
    ///
    /// Every entry cost exactly one call into the grammar roster.
    pub(crate) fn parse_count(&self) -> usize {
        self.before.len() + self.after.len()
    }

    /// The map holding one revision's parses.
    fn revision(&self, revision: Revision) -> &BTreeMap<String, Option<ParsedCode>> {
        match revision {
            Revision::Before => &self.before,
            Revision::After => &self.after,
        }
    }

    /// Parse each source into `target`, skipping any path already parsed.
    ///
    /// Each real parse emits [`PARSE_EVENT`]. That event is the review's parse
    /// ledger: one line per call into the grammar roster, so a run that started
    /// parsing per probe rather than per review shows up as extra lines instead
    /// of hiding behind a per-cache total.
    fn fill(
        target: &mut BTreeMap<String, Option<ParsedCode>>,
        sources: &BTreeMap<String, String>,
        revision: Revision,
    ) {
        for (path, source) in sources {
            target.entry(path.clone()).or_insert_with(|| {
                tracing::debug!(file = %path, ?revision, "{PARSE_EVENT}");
                parse_code(path, source)
            });
        }
    }
}

/// The message every real tree-sitter parse logs, once.
///
/// Named because two readers depend on the exact text: an operator counting a
/// review's parse cost, and the guard test that asserts the count.
const PARSE_EVENT: &str = "tree-sitter probe parsed a file revision";

/// Build the shared parse cache for one review, and report what it cost.
pub(crate) fn prime_parse_cache(file_change: &FileChange) -> ParseCache {
    let mut cache = ParseCache::default();
    cache.prime(file_change);
    tracing::debug!(
        parses = cache.parse_count(),
        files = file_change.sources.len(),
        "tree-sitter probe parse cache primed"
    );
    cache
}

/// Run one [`TreeSitterProbe`] over every file under review, reading the shared
/// parse cache — one [`ProbeResult`] per file, in path order.
pub(crate) fn run_tree_sitter_probe(
    probe: &'static dyn TreeSitterProbe,
    file_change: &FileChange,
    cache: &ParseCache,
) -> Vec<ProbeResult> {
    per_file_results(probe.name(), probe.kind(), file_change, |path, source| {
        probe_rows(probe, path, source, file_change, cache)
    })
}

/// One probe's rows for one file, or the single [`TREE_SITTER_NOT_PARSED`] row
/// when the file has no parse to read.
fn probe_rows(
    probe: &'static dyn TreeSitterProbe,
    path: &str,
    source: &str,
    file_change: &FileChange,
    cache: &ParseCache,
) -> Vec<ProbeRow> {
    let Some(parsed) = cache.parsed_at(path, Revision::After) else {
        return vec![not_computed_row(path, TREE_SITTER_NOT_PARSED)];
    };
    let before = file_change
        .before_sources
        .get(path)
        .and_then(|before_source| {
            cache
                .parsed_at(path, Revision::Before)
                .map(|before_parsed| ParsedRevision {
                    source: before_source,
                    parsed: before_parsed,
                })
        });
    probe.run(&TreeSitterProbeContext {
        path,
        after: ParsedRevision { source, parsed },
        before,
    })
}

/// The `functions` probe: how many function definitions each file under review
/// holds, counted from its parse.
///
/// The reference implementation of [`TreeSitterProbe`] — the smallest probe
/// that is still a real fact, so the trait's whole contract (a file, its parse,
/// rows out, through the catalog and into the prompt) is exercised by the
/// engine and not only by the richer probes built on it.
#[derive(Debug)]
struct FunctionCountProbe;

impl sealed::Sealed for FunctionCountProbe {}

impl TreeSitterProbe for FunctionCountProbe {
    fn name(&self) -> &'static str {
        "functions"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Fact
    }

    fn run(&self, context: &TreeSitterProbeContext<'_>) -> Vec<ProbeRow> {
        let count = function_count(context.path(), context.after());
        vec![ProbeRow {
            file_path: context.path().to_string(),
            symbol: None,
            line: None,
            similarity: None,
            detail: Some(format!("{count} function definitions")),
        }]
    }
}

/// How many function definitions one revision of `path` defines.
///
/// Reads the definitions off the parse the runner already made, so counting a
/// file's functions never costs a second parse.
fn function_count(path: &str, revision: ParsedRevision<'_>) -> usize {
    revision
        .parsed()
        .entities(path, revision.source())
        .iter()
        .filter(|entity| is_function_entity_type(&entity.entity_type))
        .count()
}

/// The one [`FunctionCountProbe`] the catalog registers.
static FUNCTION_COUNT_PROBE: FunctionCountProbe = FunctionCountProbe;

/// Every registered [`TreeSitterProbe`].
///
/// The single source of truth the probe catalog builds its tree-sitter rows
/// from: a probe's name and kind are read off the implementation, never
/// restated beside it. Adding a probe is adding one entry here.
pub(crate) static TREE_SITTER_PROBES: &[&'static dyn TreeSitterProbe] = &[&FUNCTION_COUNT_PROBE];

#[cfg(test)]
mod tests {
    use model_embedding::mock::MockEmbedder;

    use super::{
        function_count, prime_parse_cache, run_tree_sitter_probe, sealed, ParseCache, Revision,
        TreeSitterProbe, TreeSitterProbeContext, PARSE_EVENT, TREE_SITTER_NOT_PARSED,
    };
    use crate::review::probes::{
        catalog, run_probes, FileChange, ProbeKind, ProbeOp, ProbeResult, ProbeRow,
    };
    use crate::review::test_support::{index_conn, loader_with, TestRepo, DIM};
    use crate::review::{render_file_payload, scope_review, Scope};

    /// A two-function Rust file, so a count row reads `2` rather than `0` or `1`.
    const TWO_FUNCTIONS: &str = "pub fn one() {}\npub fn two() {}\n";

    /// The same file one function earlier — the base revision of [`TWO_FUNCTIONS`].
    const ONE_FUNCTION: &str = "pub fn one() {}\n";

    /// The changed file every probe test here is bound to.
    const CHANGED_FILE: &str = "src/lib.rs";

    /// How many revisions of one changed file a review parses: the base one and
    /// the changed one, once each however many probes read the file.
    const EXPECTED_REVISIONS_PER_FILE: usize = 2;

    /// A one-file change set carrying both revisions of [`CHANGED_FILE`].
    fn changed_file() -> FileChange {
        FileChange::default()
            .with_sources([(CHANGED_FILE.to_string(), TWO_FUNCTIONS.to_string())])
            .with_before_sources([(CHANGED_FILE.to_string(), ONE_FUNCTION.to_string())])
    }

    /// Run the named probes over a change set through the real `run_probes`
    /// entry point, so catalog resolution and dispatch are exercised too.
    async fn probe_results(names: &[&str], change: &FileChange) -> Vec<ProbeResult> {
        let conn = index_conn();
        let embedder = MockEmbedder::new(DIM);
        run_probes(names, change, &conn, &embedder)
            .await
            .expect("the tree-sitter probes run")
            .results
    }

    #[test]
    fn the_catalog_registers_each_tree_sitter_probe_under_its_own_name_and_kind() {
        let entry = catalog()
            .iter()
            .find(|entry| entry.name == "functions")
            .expect("the `functions` probe is registered in the catalog");

        assert_eq!(entry.kind, ProbeKind::Fact);
        match entry.op {
            ProbeOp::TreeSitter(probe) => {
                // The catalog row is BUILT from the implementation, so the name
                // and kind a validator declares cannot drift from the probe's.
                assert_eq!(probe.name(), entry.name);
                assert_eq!(probe.kind(), entry.kind);
            }
            other => panic!("`functions` must be a tree-sitter probe, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_trait_probe_counts_the_function_definitions_in_each_file() {
        let results = probe_results(&["functions"], &changed_file()).await;

        assert_eq!(results.len(), 1, "one result per file, got: {results:?}");
        assert_eq!(results[0].name, "functions");
        assert_eq!(results[0].target, CHANGED_FILE);
        assert_eq!(
            results[0].rows[0].detail.as_deref(),
            Some("2 function definitions")
        );
    }

    #[tokio::test]
    async fn a_file_whose_language_has_no_grammar_reports_one_not_computed_row() {
        let change = FileChange::default()
            .with_sources([("notes.txt".to_string(), "plain prose\n".to_string())]);

        let results = probe_results(&["functions"], &change).await;

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].rows.len(),
            1,
            "exactly one row, got: {results:?}"
        );
        assert_eq!(
            results[0].rows[0].detail.as_deref(),
            Some(TREE_SITTER_NOT_PARSED)
        );
    }

    /// Two probe runs over the same changed file must share ONE parse per
    /// revision. The probe list is deliberately the same name twice: that is
    /// the smallest input that makes the engine execute two tree-sitter probes
    /// over one file, so a per-probe parse shows up as four parse events rather
    /// than two.
    ///
    /// The count comes from the parse ledger ([`PARSE_EVENT`], one line per
    /// real parse) rather than from any single cache's total, because a run
    /// that built one cache per probe would report the right total twice.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn every_file_revision_is_parsed_once_for_the_whole_review() {
        let probes = ["functions", "functions"];

        let results = probe_results(&probes, &changed_file()).await;

        assert_eq!(
            results.len(),
            probes.len(),
            "every probe run produced a result"
        );
        logs_assert(|lines: &[&str]| {
            let parses = lines
                .iter()
                .filter(|line| line.contains(PARSE_EVENT))
                .count();
            if parses == EXPECTED_REVISIONS_PER_FILE {
                Ok(())
            } else {
                Err(format!(
                    "the review must parse the file once before and once after, \
                     however many probes read it; got {parses} parses in:\n{lines:#?}"
                ))
            }
        });
    }

    #[test]
    fn the_cache_holds_both_revisions_of_a_changed_file() {
        let mut cache = ParseCache::default();
        cache.prime(&changed_file());

        let before = cache
            .parsed_at(CHANGED_FILE, Revision::Before)
            .expect("the base revision parses");
        let after = cache
            .parsed_at(CHANGED_FILE, Revision::After)
            .expect("the changed revision parses");

        assert_eq!(before.entities(CHANGED_FILE, ONE_FUNCTION).len(), 1);
        assert_eq!(after.entities(CHANGED_FILE, TWO_FUNCTIONS).len(), 2);
        assert_eq!(cache.parse_count(), EXPECTED_REVISIONS_PER_FILE);
    }

    /// A probe that reports how many functions the change ADDED, so the runner
    /// hands it both revisions. Registering it in the catalog would ship a
    /// probe no validator wants; running it through the real
    /// [`run_tree_sitter_probe`] proves the diff-aware half of the context
    /// without one.
    #[derive(Debug)]
    struct FunctionDeltaProbe;

    impl sealed::Sealed for FunctionDeltaProbe {}

    impl TreeSitterProbe for FunctionDeltaProbe {
        fn name(&self) -> &'static str {
            "function-delta"
        }

        fn kind(&self) -> ProbeKind {
            ProbeKind::Fact
        }

        fn run(&self, context: &TreeSitterProbeContext<'_>) -> Vec<ProbeRow> {
            let before = context
                .before()
                .map_or(0, |revision| function_count(context.path(), revision));
            let after = function_count(context.path(), context.after());
            vec![ProbeRow {
                file_path: context.path().to_string(),
                symbol: None,
                line: None,
                similarity: None,
                detail: Some(format!("{before} before, {after} after")),
            }]
        }
    }

    static FUNCTION_DELTA_PROBE: FunctionDeltaProbe = FunctionDeltaProbe;

    #[test]
    fn the_runner_hands_a_probe_the_before_parse_as_well_as_the_after() {
        let change = changed_file();
        let cache = prime_parse_cache(&change);

        let results = run_tree_sitter_probe(&FUNCTION_DELTA_PROBE, &change, &cache);

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].rows[0].detail.as_deref(),
            Some("1 before, 2 after")
        );
    }

    #[test]
    fn a_file_the_change_added_reaches_a_probe_with_no_before_parse() {
        let change = FileChange::default()
            .with_sources([(CHANGED_FILE.to_string(), TWO_FUNCTIONS.to_string())]);
        let cache = prime_parse_cache(&change);

        let results = run_tree_sitter_probe(&FUNCTION_DELTA_PROBE, &change, &cache);

        assert_eq!(
            results[0].rows[0].detail.as_deref(),
            Some("0 before, 2 after"),
            "an added file has no base revision to compare against"
        );
    }

    /// The whole path a validator's declared probe travels: a real repository,
    /// a validator that declares `probes: [functions]`, the real `scope_review`,
    /// and the real prompt renderer the model reads.
    #[tokio::test]
    async fn a_trait_probes_rows_reach_the_rendered_validator_prompt() {
        let repo = TestRepo::new();
        repo.write(CHANGED_FILE, ONE_FUNCTION);
        repo.commit("initial");
        repo.write(CHANGED_FILE, TWO_FUNCTIONS);

        let conn = index_conn();
        let loader = loader_with("census", "*.rs", &["functions"]);
        let embedder = MockEmbedder::new(DIM);

        let work = scope_review(Scope::Working, repo.path(), &loader, &conn, &embedder, None)
            .await
            .expect("the working scope resolves");
        let file = work
            .validators()
            .iter()
            .find(|validator| validator.validator_name() == "census")
            .and_then(|validator| {
                validator
                    .files()
                    .iter()
                    .find(|file| file.path() == CHANGED_FILE)
            })
            .expect("the changed file is under the census validator");

        let rendered = render_file_payload(std::slice::from_ref(file));

        assert!(
            rendered.contains("probe `functions` on `src/lib.rs`"),
            "the prompt must carry the probe's header, got:\n{rendered}"
        );
        assert!(
            rendered.contains("2 function definitions"),
            "the prompt must carry the probe's row, got:\n{rendered}"
        );
    }
}
