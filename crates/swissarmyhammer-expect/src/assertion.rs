//! Compiling a `Then` criterion into a typed, replayable [`CompiledAssertion`].
//!
//! Per `ideas/expect.md` §"How `evaluate` turns prose into a check": `evaluate`
//! never re-interprets a `Then` line every run. Each criterion is compiled once,
//! against a *real* [`Observation`], into an assertion bound to a checkpoint, a
//! [`Locator`] (where the value lives in that checkpoint's state), an
//! [`AssertOp`], and an [`Expected`]. The *kind* of assertion that compiles sets
//! the [`VerdictTier`] — a locator plus an exact/numeric comparison is Tier 1
//! ([`VerdictTier::Deterministic`]); the author never picks a tier.
//!
//! Two deterministic flavors compile here (the cli locator dialect, Tier 1):
//!
//! - **literal-match** — `$.total equals 40` — freezes a specific value parsed
//!   from the prose (the example-style fallback).
//! - **invariant-holds** — `for each X: a == count(b)` — freezes a *relationship*
//!   between two locators; the expected side is derived from the observation each
//!   run, so it does not drift on incidental data. Preferred where expressible.
//!
//! Compilation is **self-verifying**: you cannot write `$.total` without seeing
//! the output's shape, so a freshly compiled assertion must bind *and* pass
//! against the very observation it was compiled from — otherwise it is rejected
//! as a hallucinated locator ([`CompileError::HallucinatedLocator`]) before it
//! ever reaches the approve diff. The compiler prefers the most durable locator
//! that binds (json-path over text-regex). A locator that *stops* binding on a
//! later observation is structural drift, surfaced as the distinct
//! [`AssertionOutcome::Drifted`] outcome — never a silent mis-read.

use crate::spec::Criterion;
use crate::types::{Observation, SurfaceState, VerdictTier};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use thiserror::Error;

/// The json-path root segment every compiled json-path locator starts from.
const ROOT: &str = "$";

/// A regex fragment matching one non-negative integer or decimal number.
const NUMBER_PATTERN: &str = r"[0-9]+(?:\.[0-9]+)?";

/// A regex fragment capturing one number into group 1 (the stream-regex locator).
const CAPTURE_NUMBER: &str = r"([0-9]+(?:\.[0-9]+)?)";

/// A regex fragment matching any run of non-digit characters between a key word
/// and the value it labels (e.g. the `": $"` in `Total: $40`).
const NON_DIGIT_GAP: &str = r"\D*";

/// The substring marking an exit-code reference in a criterion.
const EXIT_CUE: &str = "exit";

/// Floating-point tolerance for numeric equality (exact integers compare clean).
const EPSILON: f64 = 1e-9;

/// Relation keywords that introduce an invariant, longest-first so a longer
/// phrase is matched before a substring of it.
const RELATION_KEYWORDS: &[&str] = &["is equal to", "equal to", "equals", "matches", "=="];

/// Cues whose suffix names the collection an invariant's right side counts.
const COUNT_CUES: &[&str] = &["number of", "count of", "cardinality of"];

/// Ordinal words mapping a `When`-step reference to a checkpoint index.
const ORDINALS: &[(&str, usize)] = &[
    ("first", 1),
    ("second", 2),
    ("third", 3),
    ("fourth", 4),
    ("fifth", 5),
];

/// Words carried for grammar but dropped from json-key matching hints.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "of", "its", "their", "it", "was", "were", "be", "to", "with",
    "and", "that", "this", "still", "no", "not", "does", "after", "before", "then", "when",
];

/// A per-surface locator: where in a checkpoint's state a value lives.
///
/// The cli dialect from `ideas/expect.md` §"Locators are a per-surface dialect":
/// a stream regex-capture, a json-path (when the output is structured JSON), or
/// the process exit code. Ranked by robustness — json-path is stable, a stream
/// regex is brittle — so the compiler prefers a json-path that binds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Locator {
    /// A json-path into a structured checkpoint state (the durable locator).
    JsonPath {
        /// The `$.a.b[0]` path resolved against the checkpoint's JSON.
        path: String,
    },
    /// The element count of the array at a json-path — the right side of an
    /// invariant such as `count(items)`.
    JsonPathCount {
        /// The `$.a.b` path whose array length is counted.
        path: String,
    },
    /// A regex over a captured stream, binding group 1 to the value (brittle).
    StreamRegex {
        /// Which stream the pattern runs against.
        stream: Stream,
        /// The regex; capture group 1 is the bound value.
        pattern: String,
    },
    /// The process exit code (the most durable cli locator).
    Exit,
}

/// A captured cli output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// The comparison a compiled assertion performs against its located value.
///
/// Tier 1 needs only exact/numeric equality; richer operators arrive with the
/// later tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssertOp {
    /// The located value equals the expected (numeric within tolerance, or exact
    /// text).
    Equals,
}

/// A value resolved from a checkpoint, or the right side of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoundValue {
    /// A numeric value (also how a count and an exit code are carried).
    Number(f64),
    /// A textual value (an exact string or a non-numeric capture).
    Text(String),
}

/// What a compiled assertion compares its located value against.
///
/// The discriminant is the deterministic flavor: a frozen [`Literal`] value, or
/// an [`Invariant`] whose expected side is a second locator resolved against the
/// observation each run (never frozen).
///
/// [`Literal`]: Expected::Literal
/// [`Invariant`]: Expected::Invariant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "flavor", rename_all = "snake_case")]
pub enum Expected {
    /// literal-match: a specific value frozen from the criterion prose.
    Literal {
        /// The frozen expected value.
        value: BoundValue,
    },
    /// invariant-holds: a relationship whose expected side is derived from the
    /// observation each run via this locator.
    Invariant {
        /// The locator whose resolved value the [primary locator] must equal.
        ///
        /// [primary locator]: CompiledAssertion::locator
        right: Locator,
    },
}

/// A `Then` criterion compiled into a typed, replayable assertion.
///
/// Bound to one checkpoint in the timeline and one [`Locator`], it is replayed
/// (never recompiled) against a later observation by [`evaluate`]. The
/// `criterion_text` keeps the assertion bound to the prose it was compiled from.
///
/// [`evaluate`]: CompiledAssertion::evaluate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledAssertion {
    /// Index into [`Observation::checkpoints`] this assertion reads.
    pub checkpoint: usize,
    /// Where in the checkpoint's state the located value lives.
    pub locator: Locator,
    /// The comparison performed.
    pub op: AssertOp,
    /// What the located value is compared against.
    pub expected: Expected,
    /// The verdict-ladder tier this assertion's kind selected.
    pub tier: VerdictTier,
    /// The criterion prose this assertion is bound to.
    pub criterion_text: String,
}

/// The outcome of replaying a [`CompiledAssertion`] against an observation.
///
/// Structural drift (the located value no longer binds, or the checkpoint is
/// gone) is a *distinct* outcome from a value mismatch — never a silent
/// mis-read, per `ideas/expect.md`.
#[derive(Debug, Clone, PartialEq)]
pub enum AssertionOutcome {
    /// The locator bound and the assertion held.
    Holds,
    /// The locator bound but the value did not satisfy the assertion.
    Violated {
        /// The value the locator resolved to.
        found: BoundValue,
        /// The value it was compared against.
        expected: BoundValue,
    },
    /// The locator no longer binds — structural drift, surfaced loudly.
    Drifted {
        /// The locator that failed to bind (for the drift report).
        locator: String,
    },
    /// The assertion's checkpoint index is absent from the observation.
    CheckpointMissing {
        /// The missing checkpoint index.
        index: usize,
    },
}

/// Why a criterion could not be compiled into a Tier 1 assertion.
#[derive(Debug, Error, PartialEq)]
pub enum CompileError {
    /// The prose carries no compilable Tier 1 assertion (no literal value, no
    /// invariant relation, no exit-code reference).
    #[error("criterion '{criterion}' has no compilable Tier 1 assertion (no literal value, invariant relation, or exit-code reference)")]
    Unrecognized {
        /// The criterion prose.
        criterion: String,
    },
    /// A locator was derived but does not bind and pass against the source
    /// observation — a hallucinated locator, rejected before approve.
    #[error("criterion '{criterion}' compiled to a locator that does not bind and pass against its source observation (hallucinated locator)")]
    HallucinatedLocator {
        /// The criterion prose.
        criterion: String,
    },
    /// The criterion targets a checkpoint index the observation does not have.
    #[error("criterion '{criterion}' targets checkpoint {index} but the observation has only {available}")]
    CheckpointOutOfRange {
        /// The criterion prose.
        criterion: String,
        /// The requested checkpoint index.
        index: usize,
        /// How many checkpoints the observation actually has.
        available: usize,
    },
    /// The observation has no checkpoints to bind against.
    #[error("criterion '{criterion}' cannot bind against an observation with no checkpoints")]
    EmptyObservation {
        /// The criterion prose.
        criterion: String,
    },
}

/// Compile a `Then` criterion into a typed assertion bound against `observation`.
///
/// Resolves the criterion's checkpoint (by ordinal, else the final one), parses
/// its assertion kind, derives the most durable locator that binds, and
/// **self-verifies** that the freshly compiled assertion holds against the very
/// observation it was compiled from.
///
/// # Errors
///
/// Returns a [`CompileError`] when the observation has no checkpoints, the
/// referenced checkpoint is out of range, the prose carries no Tier 1 assertion,
/// or the derived locator does not bind and pass (a hallucinated locator).
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use swissarmyhammer_expect::{
///     compile, AssertOp, BoundValue, Criterion, Expected, Locator, Observation, VerdictTier,
/// };
///
/// let observation: Observation = serde_json::from_value(json!({
///     "path": "src/checkout/coupon",
///     "checkpoints": [
///         { "after": "initial cart", "state": { "kind": "json", "body": { "total": 50 } }, "duration_ms": 5 },
///         { "after": "apply SAVE10", "state": { "kind": "json", "body": { "total": 40 } }, "duration_ms": 7 }
///     ],
///     "trajectory": { "steps": [] }
/// }))?;
///
/// let criterion = Criterion {
///     text: "After the first apply, the total is $40".into(),
///     checked: false,
/// };
/// let assertion = compile(&criterion, &observation)?;
///
/// assert_eq!(assertion.checkpoint, 1);
/// assert_eq!(assertion.op, AssertOp::Equals);
/// assert_eq!(assertion.tier, VerdictTier::Deterministic);
/// assert_eq!(assertion.expected, Expected::Literal { value: BoundValue::Number(40.0) });
/// assert!(matches!(assertion.locator, Locator::JsonPath { path } if path == "$.total"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compile(
    criterion: &Criterion,
    observation: &Observation,
) -> Result<CompiledAssertion, CompileError> {
    let text = criterion.text.as_str();
    let checkpoint = resolve_checkpoint(text, observation)?;
    let state = &observation.checkpoints[checkpoint].state;

    let intents = candidate_intents(text);
    if intents.is_empty() {
        return Err(CompileError::Unrecognized {
            criterion: text.to_string(),
        });
    }

    // Try each recognized kind in preference order (most durable first) and
    // return the first that both binds and passes its own self-verification —
    // so a greedy mis-classification (e.g. an `exit` cue in prose that is really
    // a literal) falls through to the interpretation that actually holds.
    for intent in intents {
        if let Some(candidate) = build_candidate(intent, checkpoint, state, text) {
            if candidate.evaluate(observation) == AssertionOutcome::Holds {
                return Ok(candidate);
            }
        }
    }
    Err(CompileError::HallucinatedLocator {
        criterion: text.to_string(),
    })
}

impl CompiledAssertion {
    /// Replay this assertion against `observation`, returning the outcome.
    ///
    /// Structural drift (the locator no longer binds, or the checkpoint is gone)
    /// is reported as its own outcome, distinct from a value mismatch.
    pub fn evaluate(&self, observation: &Observation) -> AssertionOutcome {
        let Some(checkpoint) = observation.checkpoints.get(self.checkpoint) else {
            return AssertionOutcome::CheckpointMissing {
                index: self.checkpoint,
            };
        };
        let state = &checkpoint.state;

        let Some(found) = self.locator.resolve(state) else {
            return AssertionOutcome::Drifted {
                locator: self.locator.to_string(),
            };
        };
        let expected = match &self.expected {
            Expected::Literal { value } => value.clone(),
            Expected::Invariant { right } => match right.resolve(state) {
                Some(value) => value,
                None => {
                    return AssertionOutcome::Drifted {
                        locator: right.to_string(),
                    }
                }
            },
        };

        if bound_equals(&found, &expected) {
            AssertionOutcome::Holds
        } else {
            AssertionOutcome::Violated { found, expected }
        }
    }
}

impl Locator {
    /// Resolve this locator against a checkpoint's `state`, or `None` if it does
    /// not bind (the structural-drift signal).
    pub fn resolve(&self, state: &SurfaceState) -> Option<BoundValue> {
        match self {
            Locator::JsonPath { path } => {
                let json = checkpoint_json(state)?;
                bound_from_value(resolve_json_path(&json, path)?)
            }
            Locator::JsonPathCount { path } => {
                let json = checkpoint_json(state)?;
                let len = resolve_json_path(&json, path)?.as_array()?.len();
                Some(BoundValue::Number(len as f64))
            }
            Locator::StreamRegex { stream, pattern } => {
                let content = stream_content(state, *stream)?;
                let regex = Regex::new(pattern).ok()?;
                let captured = regex.captures(content)?.get(1)?.as_str();
                Some(parse_bound(captured))
            }
            Locator::Exit => checkpoint_exit(state).map(|code| BoundValue::Number(code as f64)),
        }
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Locator::JsonPath { path } => write!(f, "{path}"),
            Locator::JsonPathCount { path } => write!(f, "count({path})"),
            Locator::StreamRegex { stream, pattern } => write!(f, "{stream}:/{pattern}/"),
            Locator::Exit => write!(f, "exit"),
        }
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        };
        f.write_str(name)
    }
}

/// The parsed kind of a criterion, before a locator is bound to it.
enum Intent {
    /// An exit-code comparison against a literal code.
    Exit {
        /// The expected exit code.
        expected: f64,
    },
    /// An invariant relating a scalar to the count of a collection.
    Invariant {
        /// Key hints for the scalar (left) side.
        left: Vec<String>,
        /// Key hints for the counted collection (right) side.
        right: Vec<String>,
    },
    /// A literal-match against a numeric value parsed from the prose.
    Literal {
        /// The expected number.
        expected: f64,
        /// Key hints used to prefer a matching json key / stream label.
        key_tokens: Vec<String>,
    },
}

/// Resolve which checkpoint a criterion binds to: the ordinal it names, else the
/// final checkpoint.
fn resolve_checkpoint(text: &str, observation: &Observation) -> Result<usize, CompileError> {
    let count = observation.checkpoints.len();
    if count == 0 {
        return Err(CompileError::EmptyObservation {
            criterion: text.to_string(),
        });
    }
    match ordinal_checkpoint(text) {
        Some(index) if index < count => Ok(index),
        Some(index) => Err(CompileError::CheckpointOutOfRange {
            criterion: text.to_string(),
            index,
            available: count,
        }),
        None => Ok(count - 1),
    }
}

/// All assertion kinds a criterion could plausibly be, in preference order
/// (most durable first): exit, then invariant, then literal. Empty when no Tier
/// 1 kind is recognized at all. The caller binds and self-verifies each in turn.
fn candidate_intents(text: &str) -> Vec<Intent> {
    let mut intents = Vec::new();
    if let Some(expected) = exit_intent(text) {
        intents.push(Intent::Exit { expected });
    }
    if let Some((left, right)) = split_count_invariant(text) {
        intents.push(Intent::Invariant { left, right });
    }
    if let Some(expected) = parse_number(text) {
        intents.push(Intent::Literal {
            expected,
            key_tokens: tokens(text),
        });
    }
    intents
}

/// The exit-code intent's expected value when the criterion references the exit
/// code and names a number, else `None`.
fn exit_intent(text: &str) -> Option<f64> {
    if mentions_exit(text) {
        parse_number(text)
    } else {
        None
    }
}

/// Derive the most durable locator that binds for `intent` against `state`,
/// building the candidate assertion (`None` if nothing binds).
fn build_candidate(
    intent: Intent,
    checkpoint: usize,
    state: &SurfaceState,
    text: &str,
) -> Option<CompiledAssertion> {
    match intent {
        Intent::Exit { expected } => {
            checkpoint_exit(state)?;
            Some(deterministic(
                checkpoint,
                Locator::Exit,
                Expected::Literal {
                    value: BoundValue::Number(expected),
                },
                text,
            ))
        }
        Intent::Invariant { left, right } => {
            let json = checkpoint_json(state)?;
            let (left_path, _) = find_scalar_path_by_keys(&json, &left)?;
            let (right_path, _) = find_array_path_by_keys(&json, &right)?;
            Some(deterministic(
                checkpoint,
                Locator::JsonPath { path: left_path },
                Expected::Invariant {
                    right: Locator::JsonPathCount { path: right_path },
                },
                text,
            ))
        }
        Intent::Literal {
            expected,
            key_tokens,
        } => {
            if let Some(json) = checkpoint_json(state) {
                if let Some(path) = find_scalar_path_by_value(&json, expected, &key_tokens) {
                    return Some(deterministic(
                        checkpoint,
                        Locator::JsonPath { path },
                        Expected::Literal {
                            value: BoundValue::Number(expected),
                        },
                        text,
                    ));
                }
            }
            for (stream, content) in checkpoint_streams(state) {
                if let Some(pattern) = build_capture_regex(content, &key_tokens, expected) {
                    return Some(deterministic(
                        checkpoint,
                        Locator::StreamRegex { stream, pattern },
                        Expected::Literal {
                            value: BoundValue::Number(expected),
                        },
                        text,
                    ));
                }
            }
            None
        }
    }
}

/// Assemble a Tier 1 ([`VerdictTier::Deterministic`]) equality assertion.
fn deterministic(
    checkpoint: usize,
    locator: Locator,
    expected: Expected,
    text: &str,
) -> CompiledAssertion {
    CompiledAssertion {
        checkpoint,
        locator,
        op: AssertOp::Equals,
        expected,
        tier: VerdictTier::Deterministic,
        criterion_text: text.to_string(),
    }
}

/// The structured JSON view of a checkpoint state: its body, or its stdout when
/// that parses as JSON (so a json-path is preferred over a stream regex).
fn checkpoint_json(state: &SurfaceState) -> Option<Value> {
    match state {
        SurfaceState::Json { body } => Some(body.clone()),
        SurfaceState::Cli(cli) => serde_json::from_str(cli.stdout.trim()).ok(),
    }
}

/// The capturable streams of a checkpoint state, in robustness order.
fn checkpoint_streams(state: &SurfaceState) -> Vec<(Stream, &str)> {
    match state {
        SurfaceState::Cli(cli) => {
            vec![
                (Stream::Stdout, cli.stdout.as_str()),
                (Stream::Stderr, cli.stderr.as_str()),
            ]
        }
        SurfaceState::Json { .. } => Vec::new(),
    }
}

/// One stream's content, or `None` when the state has no such stream.
fn stream_content(state: &SurfaceState, stream: Stream) -> Option<&str> {
    match state {
        SurfaceState::Cli(cli) => Some(match stream {
            Stream::Stdout => cli.stdout.as_str(),
            Stream::Stderr => cli.stderr.as_str(),
        }),
        SurfaceState::Json { .. } => None,
    }
}

/// The process exit code of a checkpoint state, or `None`.
fn checkpoint_exit(state: &SurfaceState) -> Option<i32> {
    match state {
        SurfaceState::Cli(cli) => cli.exit_code,
        SurfaceState::Json { .. } => None,
    }
}

/// Whether the criterion references the process exit code.
fn mentions_exit(text: &str) -> bool {
    text.to_ascii_lowercase().contains(EXIT_CUE)
}

/// Parse the last numeric literal in `text` (the expected value usually trails).
fn parse_number(text: &str) -> Option<f64> {
    let regex = Regex::new(NUMBER_PATTERN).ok()?;
    regex.find_iter(text).last()?.as_str().parse().ok()
}

/// Map a criterion's ordinal reference (`first`, `second`, …) to a checkpoint
/// index, or `None` when it names no ordinal.
fn ordinal_checkpoint(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    ORDINALS
        .iter()
        .find(|(word, _)| word_present(&lower, word))
        .map(|(_, index)| *index)
}

/// Whether `word` appears as a whole alphabetic token in `lower`.
fn word_present(lower: &str, word: &str) -> bool {
    lower
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|token| token == word)
}

/// Split an invariant criterion into (left scalar hints, right collection hints)
/// when it relates a value to the count of a collection, else `None`.
fn split_count_invariant(text: &str) -> Option<(Vec<String>, Vec<String>)> {
    let lower = text.to_ascii_lowercase();
    let (keyword_index, keyword_len) = RELATION_KEYWORDS
        .iter()
        .filter_map(|keyword| lower.find(keyword).map(|index| (index, keyword.len())))
        .min_by_key(|(index, _)| *index)?;

    let left = &text[..keyword_index];
    let right = &text[keyword_index + keyword_len..];
    let counted = strip_count_cue(right)?;
    Some((tokens(left), tokens(counted)))
}

/// Return the substring after the first count cue (`number of`, `count of`, …),
/// or `None` when the phrase counts nothing.
fn strip_count_cue(phrase: &str) -> Option<&str> {
    let lower = phrase.to_ascii_lowercase();
    COUNT_CUES
        .iter()
        .filter_map(|cue| lower.find(cue).map(|index| index + cue.len()))
        .min()
        .map(|offset| &phrase[offset..])
}

/// Lowercased alphanumeric word tokens of `phrase`, with stopwords, ordinals,
/// and pure-number tokens dropped — the json-key matching hints.
fn tokens(phrase: &str) -> Vec<String> {
    phrase
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .filter(|word| !is_stopword(word) && !is_ordinal_word(word) && !is_all_digits(word))
        .collect()
}

/// Whether `word` is a dropped stopword.
fn is_stopword(word: &str) -> bool {
    STOPWORDS.contains(&word)
}

/// Whether `word` is an ordinal (already consumed as a checkpoint reference).
fn is_ordinal_word(word: &str) -> bool {
    ORDINALS.iter().any(|(ordinal, _)| *ordinal == word)
}

/// Whether `word` is all ASCII digits (a value, not a key hint).
fn is_all_digits(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// All `(json-path, node)` pairs in `json`, depth-first from the root.
fn collect_nodes<'a>(value: &'a Value, prefix: &str, out: &mut Vec<(String, &'a Value)>) {
    out.push((prefix.to_string(), value));
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                collect_nodes(child, &format!("{prefix}.{key}"), out);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                collect_nodes(child, &format!("{prefix}[{index}]"), out);
            }
        }
        _ => {}
    }
}

/// Find the json-path of a numeric scalar equal to `target` to bind a literal.
///
/// Prefers a scalar that both matches a hint key *and* equals `target`. A bare
/// value match (any key) is used only when the hint is unanchored — no scalar's
/// key matches it at all. When a hint *does* name a field but that field holds a
/// different value, binding an unrelated same-valued field would be a silent
/// mis-read, so this returns `None` instead (rejecting as a hallucinated
/// locator) rather than guessing.
fn find_scalar_path_by_value(json: &Value, target: f64, key_tokens: &[String]) -> Option<String> {
    let mut nodes = Vec::new();
    collect_nodes(json, ROOT, &mut nodes);

    let mut keyed_match: Option<String> = None;
    let mut value_only: Option<String> = None;
    let mut hint_anchored = false;
    for (path, value) in &nodes {
        let Some(number) = value.as_f64() else {
            continue;
        };
        let key_hit = key_matches(final_key(path), key_tokens);
        hint_anchored |= key_hit;
        if !numbers_equal(number, target) {
            continue;
        }
        if key_hit && keyed_match.is_none() {
            keyed_match = Some(path.clone());
        }
        value_only.get_or_insert_with(|| path.clone());
    }

    if keyed_match.is_some() {
        return keyed_match;
    }
    if key_tokens.is_empty() || !hint_anchored {
        return value_only;
    }
    None
}

/// Find the json-path and value of a numeric scalar whose final key matches a
/// hint (the left side of an invariant).
fn find_scalar_path_by_keys(json: &Value, key_tokens: &[String]) -> Option<(String, f64)> {
    let mut nodes = Vec::new();
    collect_nodes(json, ROOT, &mut nodes);
    nodes.iter().find_map(|(path, value)| {
        let number = value.as_f64()?;
        key_matches(final_key(path), key_tokens).then(|| (path.clone(), number))
    })
}

/// Find the json-path and length of an array whose final key matches a hint (the
/// right side of an invariant).
fn find_array_path_by_keys(json: &Value, key_tokens: &[String]) -> Option<(String, usize)> {
    let mut nodes = Vec::new();
    collect_nodes(json, ROOT, &mut nodes);
    nodes.iter().find_map(|(path, value)| {
        let array = value.as_array()?;
        key_matches(final_key(path), key_tokens).then(|| (path.clone(), array.len()))
    })
}

/// The trailing object key of a json-path (`$.a.b` → `b`, `$.items[0]` → `items`).
fn final_key(path: &str) -> &str {
    let after_dot = path.rsplit('.').next().unwrap_or(path);
    match after_dot.find('[') {
        Some(bracket) => &after_dot[..bracket],
        None => after_dot,
    }
}

/// Whether a json `key` matches the hint `tokens` (any single token, or their
/// concatenation, e.g. `item_count` ↔ `["item", "count"]`).
fn key_matches(key: &str, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let normalized = normalize_key(key);
    if normalized.is_empty() {
        return false;
    }
    tokens.contains(&normalized) || tokens.concat() == normalized
}

/// Lowercase a key to its ASCII-alphanumeric form (`item_count` → `itemcount`).
fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Build a stream-regex pattern capturing the value `target` near a key word,
/// falling back to the first bare number; `None` when none captures `target`.
fn build_capture_regex(content: &str, key_tokens: &[String], target: f64) -> Option<String> {
    for token in key_tokens {
        let pattern = format!(
            "(?i){}{NON_DIGIT_GAP}{CAPTURE_NUMBER}",
            regex::escape(token)
        );
        if capture_matches(&pattern, content, target) {
            return Some(pattern);
        }
    }
    if capture_matches(CAPTURE_NUMBER, content, target) {
        return Some(CAPTURE_NUMBER.to_string());
    }
    None
}

/// Whether `pattern`'s capture group 1 binds the number `target` in `content`.
fn capture_matches(pattern: &str, content: &str, target: f64) -> bool {
    let captured = Regex::new(pattern)
        .ok()
        .and_then(|regex| regex.captures(content))
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .and_then(|text| text.parse::<f64>().ok());
    match captured {
        Some(number) => numbers_equal(number, target),
        None => false,
    }
}

/// Resolve a `$.a.b[0]` json-path against `root`, or `None` if any segment is
/// absent.
fn resolve_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    let mut rest = path.strip_prefix(ROOT)?;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            current = current.get(&after[..end])?;
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix('[') {
            let close = after.find(']')?;
            let index: usize = after[..close].parse().ok()?;
            current = current.get(index)?;
            rest = &after[close + 1..];
        } else {
            return None;
        }
    }
    Some(current)
}

/// Convert a JSON scalar into a [`BoundValue`], or `None` for non-scalars.
fn bound_from_value(value: &Value) -> Option<BoundValue> {
    if let Some(number) = value.as_f64() {
        Some(BoundValue::Number(number))
    } else {
        value
            .as_str()
            .map(|text| BoundValue::Text(text.to_string()))
    }
}

/// Parse a captured string as a number, else keep it as text.
fn parse_bound(captured: &str) -> BoundValue {
    match captured.parse::<f64>() {
        Ok(number) => BoundValue::Number(number),
        Err(_) => BoundValue::Text(captured.to_string()),
    }
}

/// Whether two bound values are equal (numeric within tolerance, text exact).
fn bound_equals(left: &BoundValue, right: &BoundValue) -> bool {
    match (left, right) {
        (BoundValue::Number(a), BoundValue::Number(b)) => numbers_equal(*a, *b),
        (BoundValue::Text(a), BoundValue::Text(b)) => a == b,
        _ => false,
    }
}

/// Whether two numbers are equal within [`EPSILON`].
fn numbers_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Checkpoint, CliState, Trajectory};
    use std::collections::BTreeMap;
    use std::time::Duration;

    /// A JSON-bodied checkpoint at `after` carrying `body`.
    fn json_checkpoint(after: &str, body: Value) -> Checkpoint {
        Checkpoint {
            after: after.to_string(),
            state: SurfaceState::Json { body },
            duration: Duration::from_millis(1),
        }
    }

    /// A cli checkpoint at `after` with `stdout` and exit code `exit`.
    fn cli_checkpoint(after: &str, stdout: &str, exit: i32) -> Checkpoint {
        Checkpoint {
            after: after.to_string(),
            state: SurfaceState::Cli(CliState {
                stdout: stdout.to_string(),
                stderr: String::new(),
                exit_code: Some(exit),
                files: BTreeMap::new(),
            }),
            duration: Duration::from_millis(1),
        }
    }

    /// An observation over `checkpoints` for the coupon spec identity.
    fn observation(checkpoints: Vec<Checkpoint>) -> Observation {
        Observation {
            path: "src/checkout/coupon".to_string(),
            checkpoints,
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// An unchecked criterion from `text`.
    fn criterion(text: &str) -> Criterion {
        Criterion {
            text: text.to_string(),
            checked: false,
        }
    }

    #[test]
    fn compiles_a_literal_total_to_a_json_path_at_the_named_checkpoint() {
        let observation = observation(vec![
            json_checkpoint("initial cart", serde_json::json!({ "total": 50 })),
            json_checkpoint("apply SAVE10", serde_json::json!({ "total": 40 })),
            json_checkpoint("apply SAVE10 again", serde_json::json!({ "total": 40 })),
        ]);
        let criterion = criterion("After the first apply, the total is $40");

        let assertion = compile(&criterion, &observation).expect("compiles");

        assert_eq!(assertion.checkpoint, 1);
        assert_eq!(assertion.op, AssertOp::Equals);
        assert_eq!(assertion.tier, VerdictTier::Deterministic);
        assert_eq!(
            assertion.expected,
            Expected::Literal {
                value: BoundValue::Number(40.0)
            }
        );
        assert_eq!(
            assertion.locator,
            Locator::JsonPath {
                path: "$.total".to_string()
            }
        );
        assert_eq!(assertion.criterion_text, criterion.text);
    }

    #[test]
    fn compiles_an_invariant_to_a_derived_count_relationship() {
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "item_count": 3, "items": [{}, {}, {}] }),
        )]);
        let criterion = criterion("the item count equals the number of items");

        let assertion = compile(&criterion, &observation).expect("compiles");

        assert_eq!(assertion.op, AssertOp::Equals);
        assert_eq!(assertion.tier, VerdictTier::Deterministic);
        assert_eq!(
            assertion.locator,
            Locator::JsonPath {
                path: "$.item_count".to_string()
            }
        );
        // The expected is a derived locator (a relationship), not a frozen literal.
        assert_eq!(
            assertion.expected,
            Expected::Invariant {
                right: Locator::JsonPathCount {
                    path: "$.items".to_string()
                }
            }
        );
    }

    #[test]
    fn an_invariant_holds_against_a_different_observation_with_new_data() {
        let source = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "item_count": 3, "items": [{}, {}, {}] }),
        )]);
        let assertion = compile(
            &criterion("the item count equals the number of items"),
            &source,
        )
        .expect("compiles");

        // Different incidental data, same relationship — the derived invariant
        // still holds, where a frozen literal `3` would have failed.
        let next = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "item_count": 5, "items": [{}, {}, {}, {}, {}] }),
        )]);

        assert_eq!(assertion.evaluate(&next), AssertionOutcome::Holds);
    }

    #[test]
    fn rejects_a_hallucinated_locator_that_does_not_bind() {
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "total": 40 }),
        )]);
        let criterion = criterion("the total is $999");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::HallucinatedLocator {
                criterion: criterion.text.clone()
            }
        );
    }

    #[test]
    fn prefers_a_json_path_over_a_stream_regex_when_output_is_json() {
        let observation = observation(vec![cli_checkpoint("final", "{\"total\": 40}\n", 0)]);

        let assertion = compile(&criterion("the total is $40"), &observation).expect("compiles");

        assert_eq!(
            assertion.locator,
            Locator::JsonPath {
                path: "$.total".to_string()
            }
        );
    }

    #[test]
    fn falls_back_to_a_stream_regex_for_plain_text_output() {
        let observation = observation(vec![cli_checkpoint("final", "Total: $40\n", 0)]);

        let assertion = compile(&criterion("the total is $40"), &observation).expect("compiles");

        let Locator::StreamRegex { stream, pattern } = &assertion.locator else {
            panic!(
                "expected a stream-regex locator, got {:?}",
                assertion.locator
            );
        };
        assert_eq!(*stream, Stream::Stdout);
        // The captured value re-binds to 40 against the source stream.
        assert_eq!(
            assertion.locator.resolve(&observation.checkpoints[0].state),
            Some(BoundValue::Number(40.0))
        );
        assert!(pattern.contains("total"));
    }

    #[test]
    fn compiles_an_exit_code_criterion_to_the_exit_locator() {
        let observation = observation(vec![cli_checkpoint("final", "done\n", 0)]);

        let assertion =
            compile(&criterion("the command exits with code 0"), &observation).expect("compiles");

        assert_eq!(assertion.locator, Locator::Exit);
        assert_eq!(
            assertion.expected,
            Expected::Literal {
                value: BoundValue::Number(0.0)
            }
        );
    }

    #[test]
    fn defaults_to_the_final_checkpoint_without_an_ordinal() {
        let observation = observation(vec![
            json_checkpoint("apply", serde_json::json!({ "total": 50 })),
            json_checkpoint("final", serde_json::json!({ "total": 40 })),
        ]);

        let assertion = compile(&criterion("the total is $40"), &observation).expect("compiles");

        assert_eq!(assertion.checkpoint, 1);
    }

    #[test]
    fn reports_structural_drift_when_a_locator_stops_binding() {
        let source = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "total": 40 }),
        )]);
        let assertion = compile(&criterion("the total is $40"), &source).expect("compiles");

        // The `total` field is gone — the locator no longer binds: drift, not a
        // value mismatch.
        let drifted = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "sum": 40 }),
        )]);

        assert_eq!(
            assertion.evaluate(&drifted),
            AssertionOutcome::Drifted {
                locator: "$.total".to_string()
            }
        );
    }

    #[test]
    fn rejects_an_unrecognized_criterion_with_no_assertion() {
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "total": 40 }),
        )]);
        let criterion = criterion("the experience feels delightful");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::Unrecognized {
                criterion: criterion.text.clone()
            }
        );
    }

    #[test]
    fn rejects_compiling_against_an_observation_with_no_checkpoints() {
        let observation = observation(Vec::new());
        let criterion = criterion("the total is $40");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::EmptyObservation {
                criterion: criterion.text.clone()
            }
        );
    }

    #[test]
    fn compiled_assertion_round_trips_through_serde_json() {
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "total": 40 }),
        )]);
        let assertion = compile(&criterion("the total is $40"), &observation).expect("compiles");

        let json = serde_json::to_string(&assertion).expect("serialize");
        let parsed: CompiledAssertion = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed, assertion);
    }

    #[test]
    fn rejects_an_invariant_that_does_not_hold_in_its_source_observation() {
        // The named scalar binds and the array binds, but the relationship is
        // false in the source — only the `evaluate`-based self-verify (not the
        // value search) can catch this. This is the load-bearing rejection path.
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "item_count": 3, "items": [{}, {}, {}, {}] }),
        )]);
        let criterion = criterion("the item count equals the number of items");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::HallucinatedLocator {
                criterion: criterion.text.clone()
            }
        );
    }

    #[test]
    fn rejects_binding_an_unrelated_field_when_the_named_field_disagrees() {
        // `total` is named but holds 35; `discount` happens to be 40. Binding
        // `$.discount` would be a silent mis-read, so compilation rejects.
        let observation = observation(vec![json_checkpoint(
            "final",
            serde_json::json!({ "discount": 40, "total": 35 }),
        )]);
        let criterion = criterion("the total is $40");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::HallucinatedLocator {
                criterion: criterion.text.clone()
            }
        );
    }

    #[test]
    fn falls_through_a_misleading_exit_cue_to_the_literal_interpretation() {
        // "exit" appears in the prose, but the criterion is really a literal
        // about a JSON field; the exit interpretation fails self-verify and the
        // compiler falls through to the binding that holds.
        let observation = observation(vec![cli_checkpoint("final", "{\"total\": 40}\n", 7)]);
        let criterion = criterion("after exit, the total is $40");

        let assertion = compile(&criterion, &observation).expect("compiles");

        assert_eq!(
            assertion.locator,
            Locator::JsonPath {
                path: "$.total".to_string()
            }
        );
    }

    #[test]
    fn rejects_an_ordinal_checkpoint_beyond_the_timeline() {
        let observation = observation(vec![
            json_checkpoint("apply", serde_json::json!({ "total": 50 })),
            json_checkpoint("final", serde_json::json!({ "total": 40 })),
        ]);
        let criterion = criterion("after the third apply, the total is $40");

        let error = compile(&criterion, &observation).expect_err("must reject");

        assert_eq!(
            error,
            CompileError::CheckpointOutOfRange {
                criterion: criterion.text.clone(),
                index: 3,
                available: 2,
            }
        );
    }
}
