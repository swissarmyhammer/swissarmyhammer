//! Report the near-duplicate definitions a set of files holds.
//!
//! The unit is a whole named definition — a function, a method or a type —
//! and never a window sliding over a file's tokens. A window knows nothing
//! about where a definition starts or ends, so it pairs the tail of one
//! function with the head of another and reports runs of boilerplate that
//! span two definitions.
//!
//! The definitions, and the normalized token stream each one compares by,
//! come from this workspace's tree-sitter roster — see
//! [`duplication_source`](swissarmyhammer_sem::parser::plugins::code::duplication_source).
//! The same parse decides which definitions are test code and which
//! definitions a marker comment exempts.
//!
//! Two definitions are paired by the length of the longest subsequence their
//! two normalized streams share. That is exact integer arithmetic on a parse:
//! no similarity model, no embedding, no judgment.
//!
//! That comparison is quadratic in the length of one definition, and the
//! number of pairs is quadratic in how many definitions share a length. Three
//! bounds hold both down, and each one is stated where it is declared:
//! definitions that normalize to the same stream are grouped and answered
//! without any comparison at all, `MAXIMUM_DEFINITION_TOKENS` caps one
//! comparison, and `MAXIMUM_COMPARED_SHAPES` caps one length band.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use swissarmyhammer_sem::parser::plugins::code::{duplication_source, DuplicationSource};

use crate::ops::workspace_path::resolve_within;

/// The shortest definition this op compares.
///
/// Below it sit the shapes a language forces on every author — a one-line
/// accessor, a `From` impl that calls one constructor, a two-field newtype.
/// Those repeat because the language requires them to, and a report full of
/// them says nothing. The number is measured over this workspace; the rule
/// body records the counts it was chosen from.
const MINIMUM_DEFINITION_TOKENS: usize = 40;

/// The longest definition this op compares.
///
/// One comparison fills a table of `left * right` cells, so the cost of a
/// single pair grows with the square of the definition. An uncapped pair near
/// 28000 tokens costs 12.7 s on its own, which is more than a whole workspace
/// of ordinary definitions. The longest definition this workspace declares is
/// 1744 normalized tokens, so this limit clears every real definition twice
/// over and still holds one table under 17 million cells. A definition longer
/// than this is not a candidate, so it is never compared and never reported.
const MAXIMUM_DEFINITION_TOKENS: usize = 4096;

/// How alike two definitions must be, as a percentage, before the pair is a
/// finding.
///
/// The number is measured over this workspace; the rule body records the
/// counts it was chosen from.
const MINIMUM_SIMILARITY_PERCENT: usize = 90;

/// How many later shapes one shape compares against before its band scan
/// stops.
///
/// Two definitions of equal length never fail [`lengths_can_reach`], so a
/// length band has no natural end and the pairs inside it grow with the square
/// of its width. The widest band this workspace holds is 748 shapes, so this
/// limit clears it with room. A pair past the limit is not compared, so it is
/// never reported.
const MAXIMUM_COMPARED_SHAPES: usize = 1024;

/// A whole, as a percentage.
const PERCENT_SCALE: usize = 100;

/// A shared run of tokens is shared by both definitions, so it counts once on
/// each side of the ratio.
const SIDES: usize = 2;

/// One pair of near-duplicate definitions, ready to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicationFinding {
    /// The path the finding lands on, exactly as the caller spelled it, so
    /// the review engine matches the finding to the changed file it passed
    /// in.
    pub file: String,
    /// The one-based line the repeated definition starts on.
    pub line: usize,
    /// The word the language spells this kind of definition with.
    pub kind: &'static str,
    /// The name of the repeated definition.
    pub name: String,
    /// The path of the definition this one repeats.
    pub other_file: String,
    /// The one-based line that definition starts on.
    pub other_line: usize,
    /// The name of that definition.
    pub other_name: String,
    /// How many tokens the repeated definition normalizes to.
    pub tokens: usize,
    /// How alike the two normalized streams are, as a percentage.
    pub similarity: usize,
}

/// Renders as `path:line: message`, the shape the tool-rule contract parses.
impl fmt::Display for DuplicationFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} `{}` is a near-duplicate of `{}` at {}:{} ({} tokens, {}% alike)",
            self.file,
            self.line,
            self.kind,
            self.name,
            self.other_name,
            self.other_file,
            self.other_line,
            self.tokens,
            self.similarity
        )
    }
}

/// Every near-duplicate pair of definitions in `files`, inside one file and
/// across two.
///
/// `files` are paths as the caller spelled them, and every one of them is
/// resolved inside `working_dir` by
/// [`resolve_within`](crate::ops::workspace_path::resolve_within): a path that
/// climbs out of the workspace names no file this op will read. A file the
/// roster has no grammar for, and a file that cannot be read, are each skipped
/// without a finding — the same silence
/// [`find_commented_code`](crate::ops::find_commented_code) keeps, and the
/// reason the tool rule narrows its `match` to the roster's own extensions.
///
/// A definition is reported once, against the closest of the definitions that
/// come before it, so a cluster of copies costs one finding for each copy
/// rather than one for each pair.
///
/// Two definitions are paired only when they parse to the same language, so a
/// `.rs` file and a `.py` file are never compared.
///
/// Two bounds keep the work finite, and each one is silent rather than
/// reporting: a definition outside `MINIMUM_DEFINITION_TOKENS` and
/// `MAXIMUM_DEFINITION_TOKENS` is not compared, and one length band stops
/// after `MAXIMUM_COMPARED_SHAPES` distinct shapes. No file of this workspace
/// reaches either bound, and each constant records the measurement it came
/// from.
pub fn find_duplication(working_dir: &Path, files: &[&str]) -> Vec<DuplicationFinding> {
    let sources: Vec<ReadSource> = files
        .iter()
        .filter_map(|file| read_source(working_dir, file))
        .collect();
    let candidates = candidates_of(&sources);
    let partners = best_partners(&candidates);
    findings_of(&sources, &candidates, &partners)
}

/// How alike two normalized token streams are, as a percentage.
///
/// The measure is the length of the longest subsequence the two share,
/// counted once on each side: `100 * 2 * shared / (left + right)`. Two equal
/// streams answer 100, two streams that share nothing answer 0, and the
/// arithmetic is exact, so a run over the same two streams always answers the
/// same number.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_code_context::ops::find_duplication::similarity_percent;
///
/// let left = ["v1", "+", "#num"];
/// let right = ["v1".to_string(), "+".to_string(), "#num".to_string()];
/// assert_eq!(similarity_percent(&left, &right), 100);
/// assert_eq!(similarity_percent(&left, &[] as &[&str]), 0);
/// ```
pub fn similarity_percent(left: &[impl AsRef<str>], right: &[impl AsRef<str>]) -> usize {
    let mut symbols = SymbolTable::default();
    let left = symbols.intern(left);
    let right = symbols.intern(right);
    ratio_percent(
        longest_common_subsequence(&left, &right),
        left.len(),
        right.len(),
    )
}

/// One file the op read: the path the caller spelled, and the parse.
struct ReadSource {
    /// The path exactly as the caller spelled it.
    path: String,
    /// The definitions the parse reports.
    parsed: DuplicationSource,
}

/// Read one file, `None` when it cannot be read or the roster does not claim
/// it.
fn read_source(working_dir: &Path, file: &str) -> Option<ReadSource> {
    let text = std::fs::read_to_string(resolve_within(working_dir, file)?).ok()?;
    let parsed = duplication_source(file, &text)?;
    Some(ReadSource {
        path: file.to_string(),
        parsed,
    })
}

/// One definition the op will compare.
struct Candidate {
    /// Which source it came from.
    source: usize,
    /// Which definition of that source it is.
    definition: usize,
    /// The language the source parses to, which is the only grouping two
    /// definitions may be paired within.
    language: &'static str,
    /// The normalized stream, with each distinct token interned to a number
    /// so the comparison is over integers.
    symbols: Vec<u32>,
    /// The same numbers sorted, which is what the cheap bound reads.
    sorted: Vec<u32>,
}

/// Every definition of `sources` that sits between
/// [`MINIMUM_DEFINITION_TOKENS`] and [`MAXIMUM_DEFINITION_TOKENS`], in the
/// order the caller named the files and the order each file declares them.
///
/// A definition outside that range is not a candidate, so it is never
/// compared and never reported.
fn candidates_of(sources: &[ReadSource]) -> Vec<Candidate> {
    let mut symbols = SymbolTable::default();
    let mut candidates = Vec::new();
    for (source, read) in sources.iter().enumerate() {
        for (definition, declared) in read.parsed.definitions.iter().enumerate() {
            if !(MINIMUM_DEFINITION_TOKENS..=MAXIMUM_DEFINITION_TOKENS)
                .contains(&declared.shape.len())
            {
                continue;
            }
            let interned = symbols.intern(&declared.shape);
            let mut sorted = interned.clone();
            sorted.sort_unstable();
            candidates.push(Candidate {
                source,
                definition,
                language: read.parsed.language,
                symbols: interned,
                sorted,
            });
        }
    }
    candidates
}

/// The definition one candidate repeats, and how alike the two are.
struct Match {
    /// The candidate this one repeats.
    partner: usize,
    /// How alike the two are, as a percentage.
    similarity: usize,
}

/// The candidates grouped by the exact stream they normalize to.
///
/// The group keeps the pair count away from the square of the candidate
/// count. A stream an earlier candidate already holds needs no comparison at
/// all: two equal streams are [`PERCENT_SCALE`] alike, which is the highest
/// answer the ratio gives, so each later candidate of a shape reads its answer
/// off the earliest one and is compared against nothing.
struct Shapes {
    /// The shape each candidate belongs to.
    of_candidate: Vec<usize>,
    /// The earliest candidate of each shape. That candidate is the one the
    /// shape is compared as, and the one every later member repeats.
    first: Vec<usize>,
}

impl Shapes {
    /// Group `candidates` by language and stream. The language is part of the
    /// key because two languages are never paired, however equal their
    /// streams read.
    fn of(candidates: &[Candidate]) -> Self {
        let mut seen: HashMap<(&'static str, &[u32]), usize> = HashMap::new();
        let mut shapes = Shapes {
            of_candidate: Vec::with_capacity(candidates.len()),
            first: Vec::new(),
        };
        for (index, candidate) in candidates.iter().enumerate() {
            let key = (candidate.language, candidate.symbols.as_slice());
            let shape = *seen.entry(key).or_insert_with(|| {
                shapes.first.push(index);
                shapes.first.len() - 1
            });
            shapes.of_candidate.push(shape);
        }
        shapes
    }

    /// The match an earlier candidate of the same shape already answers. The
    /// two streams are equal, so the pair is a whole and the op makes no
    /// comparison.
    fn repeated(&self, candidate: usize) -> Option<Match> {
        let first = self.first[self.of_candidate[candidate]];
        (first < candidate).then_some(Match {
            partner: first,
            similarity: PERCENT_SCALE,
        })
    }
}

/// The closest earlier definition each candidate repeats, `None` for a
/// candidate that repeats none.
fn best_partners(candidates: &[Candidate]) -> Vec<Option<Match>> {
    let shapes = Shapes::of(candidates);
    let mut best: Vec<Option<Match>> = (0..candidates.len())
        .map(|candidate| shapes.repeated(candidate))
        .collect();
    scan_bands(candidates, &shapes, MAXIMUM_COMPARED_SHAPES, &mut best);
    best
}

/// Compare each shape against the later shapes of its length band, and keep
/// the closest partner each comparison finds.
///
/// The scan walks the shapes in order of length, so a shape too long to reach
/// the gate against the current one ends the inner loop rather than being
/// compared. Equal lengths always reach, so a band has no natural end; the
/// inner loop therefore stops after `maximum` shapes whatever the lengths say.
/// `best_partners` passes [`MAXIMUM_COMPARED_SHAPES`].
///
/// Only the earliest candidate of a shape needs the scan. Every later member
/// already holds a partner at [`PERCENT_SCALE`], which no comparison beats.
fn scan_bands(
    candidates: &[Candidate],
    shapes: &Shapes,
    maximum: usize,
    best: &mut [Option<Match>],
) {
    let mut order: Vec<usize> = (0..shapes.first.len()).collect();
    order.sort_by_key(|&shape| candidates[shapes.first[shape]].symbols.len());
    for (position, &shape) in order.iter().enumerate() {
        let left = shapes.first[shape];
        for &other in order[position + 1..].iter().take(maximum) {
            let right = shapes.first[other];
            if !lengths_can_reach(&candidates[left], &candidates[right]) {
                break;
            }
            if let Some(similarity) = similarity_of(&candidates[left], &candidates[right]) {
                keep_best(best, left, right, similarity);
            }
        }
    }
}

/// Keep the pair on the later of the two definitions, which is the copy, when
/// it is closer than anything kept for that copy already.
fn keep_best(best: &mut [Option<Match>], left: usize, right: usize, similarity: usize) {
    let (partner, copy) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    let closer = best[copy]
        .as_ref()
        .is_none_or(|found| similarity > found.similarity);
    if closer {
        best[copy] = Some(Match {
            partner,
            similarity,
        });
    }
}

/// Whether two lengths alone leave the gate reachable.
///
/// Two streams share at most the shorter of them, so the ratio can reach no
/// higher than `2 * shorter / (shorter + longer)`. A pair that fails here
/// cannot pass whatever its contents are.
fn lengths_can_reach(left: &Candidate, right: &Candidate) -> bool {
    let shorter = left.symbols.len().min(right.symbols.len());
    ratio_percent(shorter, left.symbols.len(), right.symbols.len()) >= MINIMUM_SIMILARITY_PERCENT
}

/// How alike two candidates are, `None` when they parse to different
/// languages or do not reach the gate.
///
/// The multiset the two share bounds the subsequence they share from above,
/// and it costs one merge rather than a whole matrix, so it is read first.
fn similarity_of(left: &Candidate, right: &Candidate) -> Option<usize> {
    if left.language != right.language {
        return None;
    }
    let shared = shared_tokens(&left.sorted, &right.sorted);
    if ratio_percent(shared, left.symbols.len(), right.symbols.len()) < MINIMUM_SIMILARITY_PERCENT {
        return None;
    }
    let common = longest_common_subsequence(&left.symbols, &right.symbols);
    let similarity = ratio_percent(common, left.symbols.len(), right.symbols.len());
    (similarity >= MINIMUM_SIMILARITY_PERCENT).then_some(similarity)
}

/// One finding for each candidate that repeats an earlier one, in the order
/// the caller named the files.
fn findings_of(
    sources: &[ReadSource],
    candidates: &[Candidate],
    partners: &[Option<Match>],
) -> Vec<DuplicationFinding> {
    candidates
        .iter()
        .zip(partners)
        .filter_map(|(copy, found)| {
            let found = found.as_ref()?;
            Some(finding_of(
                sources,
                copy,
                &candidates[found.partner],
                found.similarity,
            ))
        })
        .collect()
}

/// The finding one pair is. The finding lands on the copy and names the
/// definition it repeats, so the message reads the way a reader meets the
/// code: this definition repeats that one.
fn finding_of(
    sources: &[ReadSource],
    copy: &Candidate,
    original: &Candidate,
    similarity: usize,
) -> DuplicationFinding {
    let copied = &sources[copy.source].parsed.definitions[copy.definition];
    let repeated = &sources[original.source].parsed.definitions[original.definition];
    DuplicationFinding {
        file: sources[copy.source].path.clone(),
        line: copied.line,
        kind: copied.kind,
        name: copied.name.clone(),
        other_file: sources[original.source].path.clone(),
        other_line: repeated.line,
        other_name: repeated.name.clone(),
        tokens: copy.symbols.len(),
        similarity,
    }
}

/// A shared run of `common` tokens, as a percentage of two streams.
fn ratio_percent(common: usize, left: usize, right: usize) -> usize {
    let total = left + right;
    if total == 0 {
        return 0;
    }
    PERCENT_SCALE * SIDES * common / total
}

/// How many tokens two sorted streams share, counting a token repeated in
/// both as often as the shorter of the two repeats it.
fn shared_tokens(left: &[u32], right: &[u32]) -> usize {
    let (mut index, mut other, mut shared) = (0, 0, 0);
    while index < left.len() && other < right.len() {
        match left[index].cmp(&right[other]) {
            std::cmp::Ordering::Less => index += 1,
            std::cmp::Ordering::Greater => other += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                index += 1;
                other += 1;
            }
        }
    }
    shared
}

/// The length of the longest subsequence two streams share.
///
/// The table is two rows rather than a matrix, because each row reads only
/// the row before it.
fn longest_common_subsequence(left: &[u32], right: &[u32]) -> usize {
    let mut previous = vec![0usize; right.len() + 1];
    let mut current = vec![0usize; right.len() + 1];
    for token in left {
        for (index, other) in right.iter().enumerate() {
            current[index + 1] = if token == other {
                previous[index] + 1
            } else {
                current[index].max(previous[index + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// The number each distinct token of the run is interned to.
#[derive(Default)]
struct SymbolTable {
    /// The number already handed to each token.
    ids: HashMap<String, u32>,
}

impl SymbolTable {
    /// The numbers `shape` interns to.
    fn intern<T: AsRef<str>>(&mut self, shape: &[T]) -> Vec<u32> {
        shape.iter().map(|token| self.id(token.as_ref())).collect()
    }

    /// The number `token` interns to — the same one every time it repeats.
    fn id(&mut self, token: &str) -> u32 {
        if let Some(id) = self.ids.get(token) {
            return *id;
        }
        let next = u32::try_from(self.ids.len()).unwrap_or(u32::MAX);
        self.ids.insert(token.to_string(), next);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{workspace_beside_an_outside_file, WORKSPACE_OUTSIDE_RUST_FILE};

    /// A Rust function long enough to clear [`MINIMUM_DEFINITION_TOKENS`],
    /// written once with the name, the accumulator and the seed the caller
    /// passes, so two copies can be made to differ ONLY by a renamed variable
    /// or ONLY by one literal.
    fn long_function(name: &str, band: &str, seed: &str) -> String {
        format!(
            concat!(
                "pub fn {name}(grid: &[Vec<i32>], limit: i32) -> i32 {{\n",
                "    let mut {band} = {seed};\n",
                "    let mut seen = 0;\n",
                "    for row in grid {{\n",
                "        for cell in row {{\n",
                "            if *cell < limit {{\n",
                "                {band} += *cell;\n",
                "                seen += 1;\n",
                "            }} else {{\n",
                "                {band} -= *cell;\n",
                "            }}\n",
                "        }}\n",
                "        if seen > limit {{\n",
                "            {band} = limit;\n",
                "        }}\n",
                "    }}\n",
                "    {band}\n",
                "}}\n",
            ),
            name = name,
            band = band,
            seed = seed
        )
    }

    /// One function that is valid JavaScript and valid TypeScript, so the two
    /// files normalize to the same stream.
    ///
    /// The roster routes `.js` and `.ts` to different grammars, so the pair is
    /// what proves the detector groups by language: put both files in one
    /// group and this function matches itself across them.
    const ECMASCRIPT_FUNCTION: &str = concat!(
        "export function foldedBand(grid, limit) {\n",
        "    let band = 0;\n",
        "    let seen = 0;\n",
        "    for (const row of grid) {\n",
        "        for (const cell of row) {\n",
        "            if (cell < limit) {\n",
        "                band += cell;\n",
        "                seen += 1;\n",
        "            } else {\n",
        "                band -= cell;\n",
        "            }\n",
        "        }\n",
        "        if (seen > limit) {\n",
        "            band = limit;\n",
        "        }\n",
        "    }\n",
        "    return band;\n",
        "}\n",
    );

    /// A Rust function under [`MINIMUM_DEFINITION_TOKENS`].
    fn short_function(name: &str) -> String {
        format!("pub fn {name}(limit: i32) -> i32 {{\n    limit + 1\n}}\n")
    }

    /// How many statements [`counting_function`] writes for a definition meant
    /// to clear [`MAXIMUM_DEFINITION_TOKENS`]. One statement normalizes to
    /// several tokens, so this clears the gate several times over.
    const ENORMOUS_STATEMENTS: usize = 1_500;

    /// How many statements [`counting_function`] writes for a definition that
    /// clears [`MINIMUM_DEFINITION_TOKENS`] and stays well under
    /// [`MAXIMUM_DEFINITION_TOKENS`].
    const MODEST_STATEMENTS: usize = 40;

    /// A Rust function of `statements` accumulator statements. Two copies
    /// under two names normalize to the same stream, so the pair is a finding
    /// whenever both copies are candidates.
    fn counting_function(name: &str, statements: usize) -> String {
        let mut text = format!("pub fn {name}(limit: i32) -> i32 {{\n    let mut band = 0;\n");
        for step in 0..statements {
            text.push_str(&format!("    band += {step} * limit;\n"));
        }
        text.push_str("    band\n}\n");
        text
    }

    /// How many copies of one shape the pair-count test writes. The old scan
    /// paid a full comparison for each of the 719400 pairs these copies make.
    const REPEATED_SHAPE_COPIES: usize = 1_200;

    /// The wall time those copies must stay under. The old scan spent about
    /// 47 s on them; the scan that answers an equal stream without comparing
    /// spends a fraction of a second, so the budget separates the two by a
    /// wide margin either way.
    const REPEATED_SHAPE_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

    /// How long each stream [`three_shapes_in_one_band`] builds is. At this
    /// length one differing token scores 95 percent and two score 90, so the
    /// three streams sit at three distinct distances above the gate.
    const BAND_STREAM_TOKENS: u32 = 20;

    /// Three candidates of one length, one language and three distinct
    /// streams, so all three sit in one length band.
    ///
    /// The first is the base stream. The second differs from it by one token
    /// and is 95 percent alike. The third differs by two tokens and is 90
    /// percent alike to each of the other two.
    fn three_shapes_in_one_band() -> Vec<Candidate> {
        let base: Vec<u32> = (0..BAND_STREAM_TOKENS).collect();
        let mut near = base.clone();
        let mut far = base.clone();
        near[base.len() - 1] = BAND_STREAM_TOKENS;
        far[base.len() - 2] = BAND_STREAM_TOKENS + 1;
        far[base.len() - 1] = BAND_STREAM_TOKENS + 2;
        [base, near, far]
            .into_iter()
            .enumerate()
            .map(|(definition, symbols)| {
                let mut sorted = symbols.clone();
                sorted.sort_unstable();
                Candidate {
                    source: 0,
                    definition,
                    language: "rust",
                    symbols,
                    sorted,
                }
            })
            .collect()
    }

    /// Write `contents` at `name` under a fresh temporary directory.
    fn workspace_with(name: &str, contents: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create a scratch workspace");
        std::fs::write(dir.path().join(name), contents).expect("write the probe file");
        dir
    }

    #[test]
    fn two_functions_that_differ_only_by_a_renamed_variable_are_reported() {
        let source = format!(
            "{}\n{}",
            long_function("folded_band", "band", "0"),
            long_function("mirrored_band", "total", "0")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].name, "mirrored_band");
        assert_eq!(findings[0].other_name, "folded_band");
        assert_eq!(findings[0].kind, "fn");
        assert_eq!(
            findings[0].similarity, 100,
            "a renamed variable leaves the two shapes equal"
        );
    }

    #[test]
    fn two_functions_that_differ_only_by_a_literal_are_reported() {
        let source = format!(
            "{}\n{}",
            long_function("folded_band", "band", "0"),
            long_function("mirrored_band", "band", "4096")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(
            findings[0].similarity, 100,
            "a substituted literal leaves the two shapes equal"
        );
    }

    #[test]
    fn two_structs_with_the_same_field_types_are_reported() {
        let source = concat!(
            "pub struct Row {\n",
            "    pub width: usize,\n    pub height: usize,\n    pub label: String,\n",
            "    pub title: String,\n    pub depth: usize,\n    pub note: String,\n",
            "    pub span: usize,\n    pub rise: usize,\n    pub head: String,\n",
            "    pub tail: String,\n    pub deep: usize,\n    pub near: String,\n",
            "}\n",
            "pub struct Band {\n",
            "    pub one: usize,\n    pub two: usize,\n    pub three: String,\n",
            "    pub four: String,\n    pub five: usize,\n    pub six: String,\n",
            "    pub seven: usize,\n    pub eight: usize,\n    pub nine: String,\n",
            "    pub ten: String,\n    pub eleven: usize,\n    pub twelve: String,\n",
            "}\n",
        );
        let dir = workspace_with("probe.rs", source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].kind, "struct");
        assert_eq!(findings[0].name, "Band");
        assert_eq!(findings[0].other_name, "Row");
    }

    #[test]
    fn a_finding_renders_as_the_tool_rule_contract_line() {
        let finding = DuplicationFinding {
            file: "src/lib.rs".to_string(),
            line: 40,
            kind: "fn",
            name: "fold_grid".to_string(),
            other_file: "src/other.rs".to_string(),
            other_line: 12,
            other_name: "fold_band".to_string(),
            tokens: 84,
            similarity: 96,
        };

        assert_eq!(
            finding.to_string(),
            "src/lib.rs:40: fn `fold_grid` is a near-duplicate of `fold_band` at src/other.rs:12 \
             (84 tokens, 96% alike)"
        );
    }

    #[test]
    fn a_definition_under_the_minimum_size_is_not_reported() {
        let source = format!("{}\n{}", short_function("first"), short_function("second"));
        let dir = workspace_with("probe.rs", &source);

        assert!(find_duplication(dir.path(), &["probe.rs"]).is_empty());
    }

    #[test]
    fn two_functions_that_only_share_a_run_of_tokens_are_not_reported() {
        let source = concat!(
            "pub fn folded_band(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
            "    let mut band = 0;\n",
            "    for row in grid {\n",
            "        for cell in row {\n",
            "            if *cell < limit {\n",
            "                band += *cell;\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "    band\n",
            "}\n",
            "pub fn counted_rows(grid: &[Vec<i32>], limit: i32) -> i32 {\n",
            "    let mut band = 0;\n",
            "    for row in grid {\n",
            "        for cell in row {\n",
            "            if *cell < limit {\n",
            "                band += *cell;\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "    let mut seen = 0;\n",
            "    while seen < limit {\n",
            "        seen += 1;\n",
            "        band -= seen;\n",
            "        if band < 0 {\n",
            "            band = 0;\n",
            "        }\n",
            "    }\n",
            "    let mut extra = band;\n",
            "    for step in 0..limit {\n",
            "        extra += step;\n",
            "    }\n",
            "    extra\n",
            "}\n",
        );
        let dir = workspace_with("probe.rs", source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(
            findings.is_empty(),
            "a run shared by two definitions of different length is not a near-duplicate: \
             {findings:?}"
        );
    }

    #[test]
    fn a_marker_suppressed_copy_is_not_reported() {
        let source = format!(
            "{}\n// sah:allow duplication the two shapes fork next week\n{}",
            long_function("folded_band", "band", "0"),
            long_function("mirrored_band", "total", "0")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_duplicate_pair_inside_a_test_module_is_not_reported() {
        let source = format!(
            "#[cfg(test)]\nmod tests {{\n{}\n{}\n}}\n",
            long_function("folded_band", "band", "0"),
            long_function("mirrored_band", "total", "0")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_definition_pasted_into_two_files_is_reported() {
        let dir = workspace_with("first.rs", &long_function("one", "band", "0"));
        std::fs::write(
            dir.path().join("second.rs"),
            long_function("two", "total", "0"),
        )
        .expect("write the second probe file");

        let findings = find_duplication(dir.path(), &["first.rs", "second.rs"]);

        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file, "second.rs");
        assert_eq!(findings[0].other_file, "first.rs");
    }

    #[test]
    fn a_cluster_of_copies_costs_one_finding_for_each_copy() {
        let source = format!(
            "{}\n{}\n{}",
            long_function("one", "band", "0"),
            long_function("two", "total", "0"),
            long_function("three", "sum", "0")
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(findings.len(), 2, "{findings:?}");
        assert_eq!(findings[0].name, "two");
        assert_eq!(findings[1].name, "three");
    }

    #[test]
    fn two_languages_are_never_paired() {
        let dir = workspace_with("probe.js", ECMASCRIPT_FUNCTION);
        std::fs::write(dir.path().join("probe.ts"), ECMASCRIPT_FUNCTION)
            .expect("write the typescript probe file");

        let findings = find_duplication(dir.path(), &["probe.js", "probe.ts"]);

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_file_the_roster_does_not_claim_reports_nothing() {
        let source = format!(
            "{}\n{}",
            long_function("first", "band", "0"),
            long_function("second", "total", "0")
        );
        let dir = workspace_with("notes.txt", &source);

        assert!(find_duplication(dir.path(), &["notes.txt"]).is_empty());
    }

    #[test]
    fn a_missing_file_reports_nothing_and_does_not_break_the_run() {
        let dir = tempfile::tempdir().expect("create a scratch workspace");

        assert!(find_duplication(dir.path(), &["gone.rs"]).is_empty());
    }

    #[test]
    fn an_absolute_path_inside_the_working_directory_is_read() {
        let source = format!(
            "{}\n{}",
            long_function("first", "band", "0"),
            long_function("second", "total", "0")
        );
        let dir = workspace_with("probe.rs", &source);
        let absolute = dir.path().join("probe.rs").to_string_lossy().to_string();

        let findings = find_duplication(dir.path(), &[absolute.as_str()]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, absolute);
    }

    #[test]
    fn a_relative_path_that_climbs_out_of_the_working_directory_is_refused() {
        let source = format!(
            "{}\n{}",
            long_function("first", "band", "0"),
            long_function("second", "total", "0")
        );
        let (_dir, workspace) =
            workspace_beside_an_outside_file(WORKSPACE_OUTSIDE_RUST_FILE, &source);
        let climbing = format!("../{WORKSPACE_OUTSIDE_RUST_FILE}");

        assert!(find_duplication(&workspace, &[climbing.as_str()]).is_empty());
    }

    #[test]
    fn an_absolute_path_outside_the_working_directory_is_refused() {
        let source = format!(
            "{}\n{}",
            long_function("first", "band", "0"),
            long_function("second", "total", "0")
        );
        let (dir, workspace) =
            workspace_beside_an_outside_file(WORKSPACE_OUTSIDE_RUST_FILE, &source);
        let outside = dir
            .path()
            .join(WORKSPACE_OUTSIDE_RUST_FILE)
            .to_string_lossy()
            .to_string();

        assert!(find_duplication(&workspace, &[outside.as_str()]).is_empty());
    }

    #[test]
    fn a_definition_over_the_maximum_size_is_not_compared() {
        let source = format!(
            "{}\n{}",
            counting_function("first", ENORMOUS_STATEMENTS),
            counting_function("second", ENORMOUS_STATEMENTS)
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert!(
            findings.is_empty(),
            "a definition over {MAXIMUM_DEFINITION_TOKENS} tokens is not compared: {findings:?}"
        );
    }

    #[test]
    fn a_definition_under_the_maximum_size_is_compared() {
        let source = format!(
            "{}\n{}",
            counting_function("first", MODEST_STATEMENTS),
            counting_function("second", MODEST_STATEMENTS)
        );
        let dir = workspace_with("probe.rs", &source);

        let findings = find_duplication(dir.path(), &["probe.rs"]);

        assert_eq!(
            findings.len(),
            1,
            "the same generator under the maximum reports the pair: {findings:?}"
        );
    }

    #[test]
    fn a_shape_repeated_many_times_does_not_cost_a_comparison_for_each_pair() {
        let mut source = String::new();
        for copy in 0..REPEATED_SHAPE_COPIES {
            source.push_str(&long_function(&format!("folded_{copy}"), "band", "0"));
            source.push('\n');
        }
        let dir = workspace_with("probe.rs", &source);

        let started = std::time::Instant::now();
        let findings = find_duplication(dir.path(), &["probe.rs"]);
        let spent = started.elapsed();

        assert_eq!(
            findings.len(),
            REPEATED_SHAPE_COPIES - 1,
            "{}",
            findings.len()
        );
        assert!(
            findings
                .iter()
                .all(|found| found.other_name == "folded_0" && found.similarity == PERCENT_SCALE),
            "every copy repeats the first definition at a whole"
        );
        assert!(
            spent < REPEATED_SHAPE_BUDGET,
            "{REPEATED_SHAPE_COPIES} copies of one shape took {spent:?}, over the \
             {REPEATED_SHAPE_BUDGET:?} budget: the pair count is quadratic again"
        );
    }

    #[test]
    fn a_band_stops_after_the_maximum_number_of_shapes() {
        let candidates = three_shapes_in_one_band();
        let shapes = Shapes::of(&candidates);
        let (mut stopped, mut whole) = (vec![None, None, None], vec![None, None, None]);

        scan_bands(&candidates, &shapes, 1, &mut stopped);
        scan_bands(&candidates, &shapes, 2, &mut whole);

        assert_eq!(
            stopped[2].as_ref().map(|found| found.partner),
            Some(1),
            "a scan that stops after one shape never compares the third against the first"
        );
        assert_eq!(
            whole[2].as_ref().map(|found| found.partner),
            Some(0),
            "a scan that reaches two shapes keeps the earlier of the two equal answers"
        );
    }

    #[test]
    fn the_similarity_of_two_equal_streams_is_a_whole() {
        let stream = ["v1".to_string(), "+".to_string(), "#num".to_string()];

        assert_eq!(similarity_percent(&stream, &stream), PERCENT_SCALE);
    }

    #[test]
    fn the_similarity_falls_as_the_streams_diverge() {
        let left = ["v1".to_string(), "+".to_string(), "#num".to_string()];
        let right = ["v1".to_string(), "-".to_string(), "#num".to_string()];

        let similarity = similarity_percent(&left, &right);

        assert!(similarity > 0 && similarity < PERCENT_SCALE, "{similarity}");
    }

    #[test]
    fn the_similarity_of_two_streams_that_share_nothing_is_zero() {
        let left = ["v1".to_string()];
        let right = ["#str".to_string()];

        assert_eq!(similarity_percent(&left, &right), 0);
    }

    #[test]
    fn the_similarity_reads_order_and_not_only_the_tokens() {
        let left = ["a".to_string(), "b".to_string(), "c".to_string()];
        let reversed = ["c".to_string(), "b".to_string(), "a".to_string()];

        assert!(similarity_percent(&left, &reversed) < PERCENT_SCALE);
    }
}
