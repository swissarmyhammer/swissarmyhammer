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
use crate::types::{
    A11yNode, DbState, FileState, HttpState, Observation, SurfaceState, VerdictTier,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::OnceLock;
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

/// The substring marking an HTTP status-code reference in a criterion.
const STATUS_CUE: &str = "status";

/// The word marking an HTTP header reference in a criterion; the header name is
/// the word immediately before it (`content-type header is …`).
const HEADER_CUE: &str = "header";

/// Grammatical punctuation trimmed from the edges of a whitespace token when
/// extracting an http header name or value, leaving interior `-`, `/`, and `.`
/// (so `content-type` and `application/json` survive intact).
const TOKEN_TRIM: &[char] = &['`', '\'', '"', '.', ',', ';', ':', '(', ')', '!', '?'];

/// Floating-point tolerance for numeric equality (exact integers compare clean).
const EPSILON: f64 = 1e-9;

/// Relation keywords that introduce an invariant, longest-first so a longer
/// phrase is matched before a substring of it.
const RELATION_KEYWORDS: &[&str] = &["is equal to", "equal to", "equals", "matches", "=="];

/// The keywords introducing a tree-relationship constraint in the a11y locator
/// dialect (`ideas/expect.md` §"Locators are a per-surface dialect"): the node
/// before the keyword must descend from the node after it. `within` and
/// `ancestor` are accepted synonyms.
const A11Y_RELATIONSHIP_KEYWORDS: &[&str] = &["within", "ancestor"];

/// The comparison keywords recognized between an a11y locator and its expected
/// value, longest-first so a longer phrase matches before a substring of it.
const A11Y_RELATION_KEYWORDS: &[&str] = &[
    "is equal to",
    "equal to",
    "equals",
    "matches",
    "shows",
    "==",
    "is",
];

/// The regex matching one `role[name=…]` selector of the a11y dialect: a role
/// identifier followed by a bracketed, optionally quoted accessible name. The
/// required brackets keep the selector unambiguous against surrounding prose.
const A11Y_SELECTOR_PATTERN: &str = r#"(?P<role>[A-Za-z][A-Za-z0-9_-]*)\s*\[\s*name\s*=\s*(?:"(?P<dq>[^"]*)"|'(?P<sq>[^']*)'|(?P<bare>[^\]]*?))\s*\]"#;

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
    /// The HTTP response status code (the most durable http locator).
    Status,
    /// An HTTP response header value, addressed by case-insensitive name (the
    /// http `header:<name>` dialect).
    Header {
        /// The header name; resolution matches it case-insensitively.
        name: String,
    },
    /// A SQL query projecting one scalar from a captured database snapshot — the
    /// db dialect, where the locator *is* SQL (the most durable surface locator).
    ///
    /// Resolution reloads the snapshot into an ephemeral in-memory database and
    /// returns the first column of the query's first row; a query that no longer
    /// binds (a dropped table, a renamed column) reports structural drift.
    Sql {
        /// The `SELECT` projecting one value (first column of the first row).
        query: String,
    },
    /// The textual content of a captured file at `path` — the file dialect's
    /// `path + content` locator.
    FileContent {
        /// The captured file's path, relative to the scratch root.
        path: String,
    },
    /// A json-path into the JSON parsed from a captured file — the file dialect's
    /// structured **sub-locator** (`path + content (+ sub-locator if structured)`).
    FileJsonPath {
        /// The captured file's path, relative to the scratch root.
        path: String,
        /// The `$.a.b` json-path resolved against that file's parsed JSON.
        pointer: String,
    },
    /// An accessibility-tree node addressed by `role[name=…]` plus tree
    /// relationship — the browser/gui dialect (`ideas/expect.md` §"Locators are a
    /// per-surface dialect"), a11y-stable rather than pixel-based.
    ///
    /// Resolution walks the captured a11y tree for the first node matching
    /// [`target`](Locator::A11y::target) whose ancestor chain satisfies
    /// [`ancestors`](Locator::A11y::ancestors) (nearest first), and returns that
    /// node's value (or its accessible name when it has no distinct value). A
    /// renamed control no longer binds and surfaces as structural drift — the
    /// honest signal a screenshot diff cannot give.
    A11y {
        /// The selector for the node whose value is read.
        target: A11ySelector,
        /// Ancestor selectors the target must descend from, nearest first; empty
        /// for an unconstrained `role[name=…]` lookup.
        ancestors: Vec<A11ySelector>,
    },
}

/// One `role[name=…]` selector in the browser/gui accessibility locator dialect.
///
/// Matches an [`A11yNode`](crate::types::A11yNode) by its accessible `role` and,
/// when [`name`](A11ySelector::name) is set, its accessible name — never by
/// pixels or DOM position, so a control rename surfaces as structural drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11ySelector {
    /// The accessible role the node must have (e.g. `button`, `textbox`).
    pub role: String,
    /// The accessible name the node must have, or `None` to match any name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
            Locator::Status => http_state(state).map(|http| BoundValue::Number(http.status as f64)),
            Locator::Header { name } => http_state(state)
                .and_then(|http| http.headers.get(&name.to_ascii_lowercase()))
                .map(|value| parse_bound(value)),
            Locator::Sql { query } => {
                let db = db_state(state)?;
                query_snapshot(&db.snapshot, query)
            }
            Locator::FileContent { path } => file_state(state)
                .and_then(|file| file.files.get(path))
                .map(|content| parse_bound(content)),
            Locator::FileJsonPath { path, pointer } => {
                let file = file_state(state)?;
                let content = file.files.get(path)?;
                let json: Value = serde_json::from_str(content).ok()?;
                bound_from_value(resolve_json_path(&json, pointer)?)
            }
            Locator::A11y { target, ancestors } => {
                let tree = a11y_state(state)?;
                resolve_a11y(tree, target, ancestors)
            }
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
            Locator::Status => write!(f, "status"),
            Locator::Header { name } => write!(f, "header:{name}"),
            Locator::Sql { query } => write!(f, "sql:{query}"),
            Locator::FileContent { path } => write!(f, "file:{path}"),
            Locator::FileJsonPath { path, pointer } => write!(f, "file:{path}#{pointer}"),
            Locator::A11y { target, ancestors } => {
                write!(f, "{target}")?;
                for ancestor in ancestors {
                    write!(f, " within {ancestor}")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for A11ySelector {
    /// Render a selector in the dialect's `role[name="…"]` form (or bare `role`
    /// when it matches any name).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{}[name=\"{name}\"]", self.role),
            None => write!(f, "{}", self.role),
        }
    }
}

impl A11ySelector {
    /// Parse a single `role[name="…"]` selector from the start of `input`,
    /// ignoring any trailing text, or `None` when `input` does not begin with a
    /// selector. Shared with the browser surface adapter's drive dialect so the
    /// `role[name=…]` grammar has one parser.
    pub fn parse(input: &str) -> Option<Self> {
        parse_selector_prefix(input.trim_start()).map(|(selector, _)| selector)
    }

    /// Parse `input` as exactly one `role[name="…"]` selector and nothing else
    /// (trailing whitespace aside), or `None`.
    ///
    /// Unlike [`parse`](A11ySelector::parse), trailing text is rejected rather
    /// than ignored — the browser drive dialect uses this so a mistakenly-scoped
    /// step (`press button[name="Go"] within form[name="…"]`) does not silently
    /// drop the scope and press the wrong control; it fails to bind and routes to
    /// the agent fallback instead.
    pub fn parse_exact(input: &str) -> Option<Self> {
        let trimmed = input.trim_start();
        let (selector, consumed) = parse_selector_prefix(trimmed)?;
        if trimmed[consumed..].trim().is_empty() {
            Some(selector)
        } else {
            None
        }
    }

    /// Whether `node`'s role matches and, when [`name`](A11ySelector::name) is
    /// set, its accessible name matches too.
    fn matches(&self, node: &A11yNode) -> bool {
        node.role == self.role && self.name.as_ref().is_none_or(|name| &node.name == name)
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

impl fmt::Display for BoundValue {
    /// Render a bound value for an evidence snippet: a number in its shortest
    /// form (`40`, not `40.0`), text verbatim.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundValue::Number(number) => write!(f, "{number}"),
            BoundValue::Text(text) => f.write_str(text),
        }
    }
}

/// The parsed kind of a criterion, before a locator is bound to it.
enum Intent {
    /// An exit-code comparison against a literal code.
    Exit {
        /// The expected exit code.
        expected: f64,
    },
    /// An HTTP status-code comparison against a literal code.
    Status {
        /// The expected status code.
        expected: f64,
    },
    /// An HTTP header comparison against a literal value.
    Header {
        /// The header name (lowercased), addressing the `header:<name>` locator.
        name: String,
        /// The expected header value (numeric or textual).
        expected: BoundValue,
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
    /// An accessibility-node comparison: the criterion names an explicit
    /// `role[name=…]` locator (the browser/gui dialect) and the value it should
    /// hold.
    A11y {
        /// The selector for the node whose value is read.
        target: A11ySelector,
        /// Ancestor selectors the target must descend from, nearest first.
        ancestors: Vec<A11ySelector>,
        /// The expected value (numeric or textual).
        expected: BoundValue,
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
    // The a11y dialect is keyed by an explicit `role[name=…]` token that does not
    // occur in the other surfaces' prose, so it is recognized first (most
    // specific). It only binds against an a11y observation; against any other
    // state it falls through to the dialect that does.
    if let Some(intent) = a11y_intent(text) {
        intents.push(intent);
    }
    if let Some(expected) = exit_intent(text) {
        intents.push(Intent::Exit { expected });
    }
    if let Some(expected) = status_intent(text) {
        intents.push(Intent::Status { expected });
    }
    if let Some((name, expected)) = header_intent(text) {
        intents.push(Intent::Header { name, expected });
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
    if mentions(text, EXIT_CUE) {
        parse_number(text)
    } else {
        None
    }
}

/// The status-code intent's expected value when the criterion references the
/// HTTP response status and names a number, else `None`.
///
/// The cue is matched as a whole word (via [`word_present`]) rather than a bare
/// substring, so an unrelated token such as `statuses` does not trigger a status
/// binding. When it does fire and a body field also matches, `status` wins by the
/// dialect's robustness ranking (`ideas/expect.md`: status is the most durable
/// http locator); [`build_candidate`]'s self-verify still rejects it unless it
/// holds against the source observation.
fn status_intent(text: &str) -> Option<f64> {
    if word_present(&text.to_ascii_lowercase(), STATUS_CUE) {
        parse_number(text)
    } else {
        None
    }
}

/// The header intent when the criterion references a response header by name and
/// states the value it should hold, else `None`.
///
/// Recognizes the shape `… <name> header … <value>` (e.g. "the content-type
/// header is application/json"): the header name is the word immediately before
/// `header`, and the expected value is the criterion's final word. Header names
/// keep their `-`/`/`/`.` characters, so tokenization is on whitespace rather
/// than the alphanumeric split [`tokens`] uses. A mis-read name simply fails to
/// bind in [`build_candidate`] and the compiler falls through, so this stays a
/// cheap recognizer rather than a strict grammar.
fn header_intent(text: &str) -> Option<(String, BoundValue)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let header_pos = words
        .iter()
        .position(|word| trim_token(word).eq_ignore_ascii_case(HEADER_CUE))?;
    if header_pos == 0 || header_pos + 1 >= words.len() {
        return None;
    }
    let name = trim_token(words[header_pos - 1]);
    let value = trim_token(words[words.len() - 1]);
    if name.is_empty() || value.is_empty() {
        return None;
    }
    Some((name.to_ascii_lowercase(), parse_bound(value)))
}

/// Trim grammatical punctuation ([`TOKEN_TRIM`]) from a whitespace token's
/// edges, leaving header-name and value interiors (`content-type`,
/// `application/json`) intact.
fn trim_token(word: &str) -> &str {
    word.trim_matches(|c: char| TOKEN_TRIM.contains(&c))
}

/// The compiled a11y selector regex, built once.
fn a11y_selector_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(A11Y_SELECTOR_PATTERN).expect("valid a11y selector regex"))
}

/// Parse one `role[name=…]` selector anchored at the start of `slice`, returning
/// it and the number of bytes consumed, or `None` when `slice` does not begin
/// with a selector.
fn parse_selector_prefix(slice: &str) -> Option<(A11ySelector, usize)> {
    let captures = a11y_selector_regex().captures(slice)?;
    let whole = captures.get(0)?;
    if whole.start() != 0 {
        return None;
    }
    let role = captures.name("role")?.as_str().to_string();
    let name = captures
        .name("dq")
        .or_else(|| captures.name("sq"))
        .or_else(|| captures.name("bare"))
        .map(|matched| matched.as_str().trim().to_string())
        .filter(|name| !name.is_empty());
    Some((A11ySelector { role, name }, whole.end()))
}

/// Recognize an a11y-dialect criterion: an explicit `role[name=…]` locator
/// (optionally with `within`/`ancestor` ancestor links) followed by a comparison
/// keyword and the expected value. `None` when the prose carries no such locator.
///
/// The locator's bracket syntax does not appear in the other surfaces' prose, so
/// this recognizer keys on it alone — leading filler ("the …") is ignored, and a
/// trailing value may be quoted (for a multi-word value) or a bare token.
///
/// Because the a11y intent is recognized first and keys purely on the bracket
/// token, count phrasing over an a11y locator (`the number of button[name="X"]
/// …`) is read as a value comparison, *not* a `count(...)` invariant — the
/// invariant dialect is not honored for a11y selectors.
fn a11y_intent(text: &str) -> Option<Intent> {
    let first = a11y_selector_regex().find(text)?;
    let (target, consumed) = parse_selector_prefix(&text[first.start()..])?;
    let mut cursor = &text[first.start() + consumed..];

    let mut ancestors = Vec::new();
    loop {
        let trimmed = cursor.trim_start();
        let Some(keyword_len) = leading_keyword(trimmed, A11Y_RELATIONSHIP_KEYWORDS) else {
            break;
        };
        let after_keyword = trimmed[keyword_len..].trim_start();
        let Some((selector, selector_len)) = parse_selector_prefix(after_keyword) else {
            break;
        };
        ancestors.push(selector);
        cursor = &after_keyword[selector_len..];
    }

    let trimmed = cursor.trim_start();
    let keyword_len = leading_keyword(trimmed, A11Y_RELATION_KEYWORDS)?;
    let expected_token = first_value_token(&trimmed[keyword_len..])?;
    let expected = parse_bound(&dequote(expected_token));

    Some(Intent::A11y {
        target,
        ancestors,
        expected,
    })
}

/// The byte length of the leading keyword from `keywords` that `text` begins with
/// (case-insensitively, as a whole word), or `None`. Keywords are tested in the
/// given order, so a caller listing them longest-first matches the longest.
fn leading_keyword(text: &str, keywords: &[&str]) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    keywords.iter().find_map(|keyword| {
        let rest = lower.strip_prefix(keyword)?;
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            Some(keyword.len())
        } else {
            None
        }
    })
}

/// The first value token of `text`: a quoted string (through its closing quote)
/// or the first whitespace-delimited word. `None` when `text` is blank.
fn first_value_token(text: &str) -> Option<&str> {
    let text = text.trim_start();
    let quote = text.chars().next()?;
    if quote == '"' || quote == '\'' {
        let close = text[1..].find(quote)? + 1;
        Some(&text[..=close])
    } else {
        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        Some(&text[..end])
    }
}

/// Strip a matching pair of surrounding quotes from `token`, else trim the
/// grammatical punctuation [`TOKEN_TRIM`] from its edges.
fn dequote(token: &str) -> String {
    let first = token.chars().next();
    let last = token.chars().next_back();
    if token.len() >= 2
        && matches!(
            (first, last),
            (Some('"'), Some('"')) | (Some('\''), Some('\''))
        )
    {
        return token[1..token.len() - 1].to_string();
    }
    trim_token(token).to_string()
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
        Intent::Status { expected } => {
            // Only an http observation carries a status code to bind against.
            http_state(state)?;
            Some(deterministic(
                checkpoint,
                Locator::Status,
                Expected::Literal {
                    value: BoundValue::Number(expected),
                },
                text,
            ))
        }
        Intent::Header { name, expected } => {
            // The named header must be present in the http observation to bind.
            http_state(state)?.headers.get(&name)?;
            Some(deterministic(
                checkpoint,
                Locator::Header { name },
                Expected::Literal { value: expected },
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
        Intent::A11y {
            target,
            ancestors,
            expected,
        } => {
            let locator = Locator::A11y { target, ancestors };
            // The locator must bind against this (a11y) observation; against any
            // other surface it resolves to nothing and the candidate is dropped.
            locator.resolve(state)?;
            Some(deterministic(
                checkpoint,
                locator,
                Expected::Literal { value: expected },
                text,
            ))
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

/// The structured JSON view of a checkpoint state: its body, or its stdout/body
/// when that parses as JSON (so a json-path is preferred over a stream regex).
///
/// db and file states have no single canonical JSON body — db is queried by SQL
/// ([`Locator::Sql`]) and a structured file is reached by its own
/// [`Locator::FileJsonPath`] sub-locator — so they yield `None` here.
fn checkpoint_json(state: &SurfaceState) -> Option<Value> {
    match state {
        SurfaceState::Json { body } => Some(body.clone()),
        SurfaceState::Cli(cli) => serde_json::from_str(cli.stdout.trim()).ok(),
        SurfaceState::Http(http) => serde_json::from_str(http.body.trim()).ok(),
        SurfaceState::Db(_) | SurfaceState::File(_) | SurfaceState::A11y { .. } => None,
    }
}

/// The a11y view of a checkpoint state, or `None` when it is not a browser/gui
/// accessibility read.
fn a11y_state(state: &SurfaceState) -> Option<&A11yNode> {
    match state {
        SurfaceState::A11y { tree } => Some(tree),
        SurfaceState::Cli(_)
        | SurfaceState::Http(_)
        | SurfaceState::Db(_)
        | SurfaceState::File(_)
        | SurfaceState::Json { .. } => None,
    }
}

/// Resolve an a11y locator against a captured tree: the value of the first node
/// matching `target` whose ancestor chain satisfies `ancestors` (nearest first),
/// or `None` (structural drift) when no such node exists.
fn resolve_a11y(
    tree: &A11yNode,
    target: &A11ySelector,
    ancestors: &[A11ySelector],
) -> Option<BoundValue> {
    let mut path: Vec<&A11yNode> = Vec::new();
    find_a11y_value(tree, target, ancestors, &mut path)
}

/// Depth-first search for the first node matching `target` (with its ancestor
/// chain satisfied), carrying the root-to-node `path` so ancestor constraints can
/// be checked. Returns that node's resolved value.
fn find_a11y_value<'a>(
    node: &'a A11yNode,
    target: &A11ySelector,
    ancestors: &[A11ySelector],
    path: &mut Vec<&'a A11yNode>,
) -> Option<BoundValue> {
    if target.matches(node) && ancestors_satisfied(ancestors, path) {
        return Some(a11y_node_value(node));
    }
    path.push(node);
    let mut found = None;
    for child in &node.children {
        if let Some(value) = find_a11y_value(child, target, ancestors, path) {
            found = Some(value);
            break;
        }
    }
    path.pop();
    found
}

/// Whether the `ancestors` chain holds along `path` (root first): each selector
/// is matched, in order, by a strictly-higher ancestor than the previous one
/// matched — so `a within b` reads "an `a` somewhere below a `b`".
fn ancestors_satisfied(ancestors: &[A11ySelector], path: &[&A11yNode]) -> bool {
    let mut idx = path.len();
    for selector in ancestors {
        let mut matched = false;
        while idx > 0 {
            idx -= 1;
            if selector.matches(path[idx]) {
                matched = true;
                break;
            }
        }
        if !matched {
            return false;
        }
    }
    true
}

/// The value an a11y locator resolves a node to: its computed value when it has
/// one, else its accessible name — parsed as a number when it looks numeric.
fn a11y_node_value(node: &A11yNode) -> BoundValue {
    match &node.value {
        Some(value) => parse_bound(value),
        None => parse_bound(&node.name),
    }
}

/// The http view of a checkpoint state, or `None` when it is not an http read.
fn http_state(state: &SurfaceState) -> Option<&HttpState> {
    match state {
        SurfaceState::Http(http) => Some(http),
        SurfaceState::Cli(_)
        | SurfaceState::Db(_)
        | SurfaceState::File(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => None,
    }
}

/// The db view of a checkpoint state, or `None` when it is not a db read.
fn db_state(state: &SurfaceState) -> Option<&DbState> {
    match state {
        SurfaceState::Db(db) => Some(db),
        SurfaceState::Cli(_)
        | SurfaceState::Http(_)
        | SurfaceState::File(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => None,
    }
}

/// The file view of a checkpoint state, or `None` when it is not a file read.
fn file_state(state: &SurfaceState) -> Option<&FileState> {
    match state {
        SurfaceState::File(file) => Some(file),
        SurfaceState::Cli(_)
        | SurfaceState::Http(_)
        | SurfaceState::Db(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => None,
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
        SurfaceState::Http { .. }
        | SurfaceState::Db(_)
        | SurfaceState::File(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => Vec::new(),
    }
}

/// One stream's content, or `None` when the state has no such stream.
fn stream_content(state: &SurfaceState, stream: Stream) -> Option<&str> {
    match state {
        SurfaceState::Cli(cli) => Some(match stream {
            Stream::Stdout => cli.stdout.as_str(),
            Stream::Stderr => cli.stderr.as_str(),
        }),
        SurfaceState::Http { .. }
        | SurfaceState::Db(_)
        | SurfaceState::File(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => None,
    }
}

/// The process exit code of a checkpoint state, or `None`.
fn checkpoint_exit(state: &SurfaceState) -> Option<i32> {
    match state {
        SurfaceState::Cli(cli) => cli.exit_code,
        SurfaceState::Http { .. }
        | SurfaceState::Db(_)
        | SurfaceState::File(_)
        | SurfaceState::A11y { .. }
        | SurfaceState::Json { .. } => None,
    }
}

/// Resolve a SQL-projection locator against a captured database `snapshot`.
///
/// Reloads the snapshot (a SQL script of `CREATE`/`INSERT` statements) into a
/// fresh in-memory database and returns the first column of `query`'s first row as
/// a [`BoundValue`]. Any failure — a malformed/no-longer-binding query, an empty
/// result, or a non-scalar/NULL cell — yields `None`, which the caller surfaces as
/// structural drift rather than a silent mis-read. This keeps the db locator pure
/// SQL while `evaluate` touches no external system (only an ephemeral in-process
/// database built from the captured bytes).
fn query_snapshot(snapshot: &str, query: &str) -> Option<BoundValue> {
    let connection = rusqlite::Connection::open_in_memory().ok()?;
    connection.execute_batch(snapshot).ok()?;
    let value = connection
        .query_row(query, [], |row| row.get::<_, rusqlite::types::Value>(0))
        .ok()?;
    bound_from_sql_value(value)
}

/// Convert a SQLite cell into a [`BoundValue`], or `None` for NULL / blob cells
/// (which carry no comparable scalar).
fn bound_from_sql_value(value: rusqlite::types::Value) -> Option<BoundValue> {
    match value {
        rusqlite::types::Value::Integer(integer) => Some(BoundValue::Number(integer as f64)),
        rusqlite::types::Value::Real(real) => Some(BoundValue::Number(real)),
        rusqlite::types::Value::Text(text) => Some(BoundValue::Text(text)),
        rusqlite::types::Value::Null | rusqlite::types::Value::Blob(_) => None,
    }
}

/// Whether `text` mentions `cue` as a case-insensitive substring (the exit cue
/// test; the status cue uses a stricter whole-word match, see [`status_intent`]).
fn mentions(text: &str, cue: &str) -> bool {
    text.to_ascii_lowercase().contains(cue)
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

    /// An http checkpoint at `after` with `status`, `headers`, and a raw `body`.
    fn http_checkpoint(
        after: &str,
        status: u16,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Checkpoint {
        Checkpoint {
            after: after.to_string(),
            state: SurfaceState::Http(HttpState {
                status,
                headers: headers
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
                body: body.to_string(),
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

    /// One http observation shared by the locator-dialect tests, so each locator
    /// is asserted against the same source of truth.
    fn http_observation() -> Observation {
        observation(vec![http_checkpoint(
            "final",
            200,
            &[("content-type", "application/json")],
            "{\"total\": 40}",
        )])
    }

    #[test]
    fn http_status_header_and_json_path_locators_bind_and_evaluate_at_tier_one() {
        let observation = http_observation();
        let state = &observation.checkpoints[0].state;

        // The three http-dialect locators, each with the value it should resolve.
        let cases: [(Locator, BoundValue); 3] = [
            (Locator::Status, BoundValue::Number(200.0)),
            (
                Locator::Header {
                    name: "content-type".to_string(),
                },
                BoundValue::Text("application/json".to_string()),
            ),
            (
                Locator::JsonPath {
                    path: "$.total".to_string(),
                },
                BoundValue::Number(40.0),
            ),
        ];

        for (locator, expected) in cases {
            // It binds against the observed http state.
            assert_eq!(
                locator.resolve(state).as_ref(),
                Some(&expected),
                "locator `{locator}` should bind"
            );
            // And a deterministic literal assertion over it holds at Tier 1.
            let assertion = deterministic(
                0,
                locator.clone(),
                Expected::Literal {
                    value: expected.clone(),
                },
                "criterion",
            );
            assert_eq!(assertion.tier, VerdictTier::Deterministic);
            assert_eq!(
                assertion.evaluate(&observation),
                AssertionOutcome::Holds,
                "locator `{locator}` should evaluate Holds"
            );
        }
    }

    #[test]
    fn http_header_locator_resolves_case_insensitively() {
        let state = SurfaceState::Http(HttpState {
            status: 204,
            headers: BTreeMap::from([("x-cache".to_string(), "HIT".to_string())]),
            body: String::new(),
        });
        let locator = Locator::Header {
            name: "X-Cache".to_string(),
        };
        assert_eq!(
            locator.resolve(&state),
            Some(BoundValue::Text("HIT".to_string()))
        );
    }

    #[test]
    fn compiles_an_http_status_criterion_to_the_status_locator() {
        let observation = http_observation();
        let assertion =
            compile(&criterion("the response status is 200"), &observation).expect("compiles");

        assert_eq!(assertion.locator, Locator::Status);
        assert_eq!(assertion.tier, VerdictTier::Deterministic);
        assert_eq!(
            assertion.expected,
            Expected::Literal {
                value: BoundValue::Number(200.0)
            }
        );
    }

    #[test]
    fn compiles_an_http_header_criterion_to_the_header_locator() {
        let observation = http_observation();
        let assertion = compile(
            &criterion("the content-type header is application/json"),
            &observation,
        )
        .expect("compiles");

        assert_eq!(
            assertion.locator,
            Locator::Header {
                name: "content-type".to_string()
            }
        );
        assert_eq!(assertion.tier, VerdictTier::Deterministic);
        assert_eq!(
            assertion.expected,
            Expected::Literal {
                value: BoundValue::Text("application/json".to_string())
            }
        );
    }

    #[test]
    fn compiles_an_http_body_literal_to_a_json_path() {
        let observation = http_observation();
        let assertion = compile(&criterion("the total is 40"), &observation).expect("compiles");

        assert_eq!(
            assertion.locator,
            Locator::JsonPath {
                path: "$.total".to_string()
            }
        );
    }

    #[test]
    fn a_status_criterion_binds_the_status_locator_over_a_same_named_body_field() {
        // The HTTP status is 200 and the body also carries a `status` field;
        // `status` is the most durable http locator, so the criterion binds the
        // response status rather than `$.status`. This pins the deliberate
        // precedence rather than leaving it incidental.
        let observation = observation(vec![http_checkpoint(
            "final",
            200,
            &[],
            "{\"status\": 500}",
        )]);
        let assertion =
            compile(&criterion("the response status is 200"), &observation).expect("compiles");

        assert_eq!(assertion.locator, Locator::Status);
        assert_eq!(
            assertion.expected,
            Expected::Literal {
                value: BoundValue::Number(200.0)
            }
        );
    }

    #[test]
    fn a_status_locator_reports_drift_against_a_non_http_state() {
        // A status assertion compiled against http no longer binds when replayed
        // over a cli observation — structural drift, not a value mismatch.
        let assertion = compile(
            &criterion("the response status is 200"),
            &http_observation(),
        )
        .expect("compiles");
        let cli = observation(vec![cli_checkpoint("final", "done\n", 0)]);

        assert_eq!(
            assertion.evaluate(&cli),
            AssertionOutcome::Drifted {
                locator: "status".to_string()
            }
        );
    }

    /// A db checkpoint whose snapshot is a one-table fixture with a single row.
    fn db_checkpoint() -> Checkpoint {
        Checkpoint {
            after: "final".to_string(),
            state: SurfaceState::Db(crate::types::DbState {
                snapshot: "CREATE TABLE orders (id INTEGER, total INTEGER, label TEXT);\n\
                           INSERT INTO orders VALUES (1, 40, 'SAVE10');\n"
                    .to_string(),
            }),
            duration: Duration::from_millis(1),
        }
    }

    #[test]
    fn sql_locator_projects_numbers_and_text_from_a_db_snapshot() {
        let observation = observation(vec![db_checkpoint()]);
        let state = &observation.checkpoints[0].state;

        let total = Locator::Sql {
            query: "SELECT total FROM orders WHERE id = 1".to_string(),
        };
        assert_eq!(total.resolve(state), Some(BoundValue::Number(40.0)));

        let label = Locator::Sql {
            query: "SELECT label FROM orders WHERE id = 1".to_string(),
        };
        assert_eq!(
            label.resolve(state),
            Some(BoundValue::Text("SAVE10".to_string()))
        );

        // A Tier-1 literal assertion over the SQL projection holds.
        let assertion = deterministic(
            0,
            total,
            Expected::Literal {
                value: BoundValue::Number(40.0),
            },
            "the order total is 40",
        );
        assert_eq!(assertion.evaluate(&observation), AssertionOutcome::Holds);
    }

    #[test]
    fn a_sql_locator_reports_drift_when_the_query_no_longer_binds() {
        let observation = observation(vec![db_checkpoint()]);
        let assertion = deterministic(
            0,
            Locator::Sql {
                query: "SELECT total FROM gone".to_string(),
            },
            Expected::Literal {
                value: BoundValue::Number(40.0),
            },
            "the order total is 40",
        );
        assert!(matches!(
            assertion.evaluate(&observation),
            AssertionOutcome::Drifted { .. }
        ));
    }

    /// A file checkpoint with a json file and a plain-text file.
    fn file_checkpoint() -> Checkpoint {
        Checkpoint {
            after: "final".to_string(),
            state: SurfaceState::File(crate::types::FileState {
                files: BTreeMap::from([
                    (
                        "config/app.json".to_string(),
                        "{\"total\": 40, \"items\": [\"a\"]}".to_string(),
                    ),
                    ("notes.txt".to_string(), "hello".to_string()),
                ]),
                dirs: vec!["config".to_string()],
            }),
            duration: Duration::from_millis(1),
        }
    }

    #[test]
    fn file_content_and_json_sublocators_resolve_against_a_file_state() {
        let observation = observation(vec![file_checkpoint()]);
        let state = &observation.checkpoints[0].state;

        // path + content.
        assert_eq!(
            Locator::FileContent {
                path: "notes.txt".to_string()
            }
            .resolve(state),
            Some(BoundValue::Text("hello".to_string()))
        );
        // json-path sub-locator into a structured file (scalar and array index).
        assert_eq!(
            Locator::FileJsonPath {
                path: "config/app.json".to_string(),
                pointer: "$.total".to_string()
            }
            .resolve(state),
            Some(BoundValue::Number(40.0))
        );
        assert_eq!(
            Locator::FileJsonPath {
                path: "config/app.json".to_string(),
                pointer: "$.items[0]".to_string()
            }
            .resolve(state),
            Some(BoundValue::Text("a".to_string()))
        );
    }

    #[test]
    fn a_file_locator_reports_drift_when_the_path_is_absent() {
        let observation = observation(vec![file_checkpoint()]);
        let assertion = deterministic(
            0,
            Locator::FileContent {
                path: "missing.txt".to_string(),
            },
            Expected::Literal {
                value: BoundValue::Text("hello".to_string()),
            },
            "the notes file says hello",
        );
        assert!(matches!(
            assertion.evaluate(&observation),
            AssertionOutcome::Drifted { .. }
        ));
    }

    // --- a11y (browser/gui) locator dialect ---------------------------------

    /// The single source of truth for the fixture a11y tree's node values, so the
    /// tests assert against the same constants the tree is built from.
    const EMAIL_IN_LOGIN: &str = "user@example.test";
    const EMAIL_IN_SEARCH: &str = "query@example.test";
    const RESULT_VALUE: &str = "clicked";
    const COUNT_VALUE: &str = "3";

    /// A fixture a11y tree whose `status` node is named `status_name`, so a
    /// "renamed control" can be modeled by passing a different name.
    ///
    /// Two `textbox[name="Email"]` nodes live under differently-named `form`s, so
    /// the `within` constraint has something to disambiguate beyond DFS order.
    fn fixture_tree(status_name: &str) -> A11yNode {
        A11yNode {
            role: "RootWebArea".to_string(),
            name: "Fixture".to_string(),
            value: None,
            children: vec![
                form_with_email("Login", EMAIL_IN_LOGIN),
                form_with_email("Search", EMAIL_IN_SEARCH),
                node("status", status_name, Some(RESULT_VALUE)),
                node("spinbutton", "count", Some(COUNT_VALUE)),
            ],
        }
    }

    /// A `form` named `form_name` containing a `textbox[name="Email"]` whose value
    /// is `email`, plus a bare `button`.
    fn form_with_email(form_name: &str, email: &str) -> A11yNode {
        A11yNode {
            role: "form".to_string(),
            name: form_name.to_string(),
            value: None,
            children: vec![
                node("textbox", "Email", Some(email)),
                node("button", "Submit", None),
            ],
        }
    }

    /// A childless a11y node.
    fn node(role: &str, name: &str, value: Option<&str>) -> A11yNode {
        A11yNode {
            role: role.to_string(),
            name: name.to_string(),
            value: value.map(str::to_string),
            children: Vec::new(),
        }
    }

    /// A single-checkpoint a11y observation over `tree`.
    fn a11y_observation(tree: A11yNode) -> Observation {
        observation(vec![Checkpoint {
            after: "final".to_string(),
            state: SurfaceState::A11y { tree },
            duration: Duration::from_millis(1),
        }])
    }

    fn selector(role: &str, name: Option<&str>) -> A11ySelector {
        A11ySelector {
            role: role.to_string(),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn a11y_intent_parses_target_ancestors_and_expected() {
        let Some(Intent::A11y {
            target,
            ancestors,
            expected,
        }) = a11y_intent("the textbox[name=\"Email\"] within form[name=\"Login\"] equals user@x")
        else {
            panic!("expected an a11y intent");
        };
        assert_eq!(target, selector("textbox", Some("Email")));
        assert_eq!(ancestors, vec![selector("form", Some("Login"))]);
        assert_eq!(expected, BoundValue::Text("user@x".to_string()));
    }

    #[test]
    fn a11y_relationship_keywords_are_synonyms() {
        // `within` and `ancestor` parse to the same ancestor constraint.
        for keyword in A11Y_RELATIONSHIP_KEYWORDS {
            let text = format!("textbox[name=\"Email\"] {keyword} form[name=\"Login\"] is x");
            let Some(Intent::A11y { ancestors, .. }) = a11y_intent(&text) else {
                panic!("`{keyword}` should parse as a relationship");
            };
            assert_eq!(
                ancestors,
                vec![selector("form", Some("Login"))],
                "{keyword}"
            );
        }
    }

    #[test]
    fn a11y_locator_resolves_a_node_value_over_a_snapshot_tree() {
        let state = SurfaceState::A11y {
            tree: fixture_tree("result"),
        };
        // A textual value resolves as text; a numeric value resolves as a number.
        let result = Locator::A11y {
            target: selector("status", Some("result")),
            ancestors: Vec::new(),
        };
        assert_eq!(
            result.resolve(&state),
            Some(BoundValue::Text(RESULT_VALUE.to_string()))
        );
        let count = Locator::A11y {
            target: selector("spinbutton", Some("count")),
            ancestors: Vec::new(),
        };
        assert_eq!(count.resolve(&state), Some(BoundValue::Number(3.0)));
    }

    #[test]
    fn a11y_locator_falls_back_to_the_accessible_name_without_a_value() {
        // A value-less node (a button) resolves to its accessible name.
        let state = SurfaceState::A11y {
            tree: fixture_tree("result"),
        };
        let locator = Locator::A11y {
            target: selector("button", Some("Submit")),
            ancestors: Vec::new(),
        };
        assert_eq!(
            locator.resolve(&state),
            Some(BoundValue::Text("Submit".to_string()))
        );
    }

    #[test]
    fn a_within_constraint_selects_the_node_under_the_named_container() {
        // Both forms hold a `textbox[name="Email"]`; the ancestor constraint picks
        // the one under the named form, not merely the first in DFS order.
        let state = SurfaceState::A11y {
            tree: fixture_tree("result"),
        };
        for (form_name, expected) in [("Login", EMAIL_IN_LOGIN), ("Search", EMAIL_IN_SEARCH)] {
            let locator = Locator::A11y {
                target: selector("textbox", Some("Email")),
                ancestors: vec![selector("form", Some(form_name))],
            };
            assert_eq!(
                locator.resolve(&state),
                Some(BoundValue::Text(expected.to_string())),
                "Email within form[{form_name}]"
            );
        }
    }

    #[test]
    fn a_role_only_selector_matches_any_name() {
        let status = node("status", "result", Some(RESULT_VALUE));
        assert!(selector("status", None).matches(&status));
        assert!(!selector("button", None).matches(&status));
        let locator = Locator::A11y {
            target: selector("status", None),
            ancestors: Vec::new(),
        };
        assert_eq!(
            locator.resolve(&SurfaceState::A11y {
                tree: fixture_tree("result"),
            }),
            Some(BoundValue::Text(RESULT_VALUE.to_string()))
        );
    }

    #[test]
    fn an_unsatisfied_ancestor_constraint_does_not_bind() {
        let state = SurfaceState::A11y {
            tree: fixture_tree("result"),
        };
        let locator = Locator::A11y {
            target: selector("textbox", Some("Email")),
            ancestors: vec![selector("form", Some("Nonexistent"))],
        };
        assert_eq!(locator.resolve(&state), None);
    }

    #[test]
    fn compiles_an_a11y_criterion_to_tier_one_and_holds() {
        let obs = a11y_observation(fixture_tree("result"));
        let assertion = compile(&criterion("status[name=\"result\"] equals clicked"), &obs)
            .expect("a11y criterion compiles");
        assert_eq!(assertion.tier, VerdictTier::Deterministic);
        assert_eq!(
            assertion.locator,
            Locator::A11y {
                target: selector("status", Some("result")),
                ancestors: Vec::new(),
            }
        );
        assert_eq!(assertion.evaluate(&obs), AssertionOutcome::Holds);
    }

    #[test]
    fn compiles_an_a11y_within_criterion_against_the_observed_tree() {
        let obs = a11y_observation(fixture_tree("result"));
        let text = format!(
            "textbox[name=\"Email\"] within form[name=\"Search\"] equals {EMAIL_IN_SEARCH}"
        );
        let assertion = compile(&criterion(&text), &obs).expect("within criterion compiles");
        assert_eq!(
            assertion.locator,
            Locator::A11y {
                target: selector("textbox", Some("Email")),
                ancestors: vec![selector("form", Some("Search"))],
            }
        );
        assert_eq!(assertion.evaluate(&obs), AssertionOutcome::Holds);
    }

    #[test]
    fn a_renamed_a11y_control_surfaces_as_structural_drift() {
        // Compile against a tree whose status node is named "result"...
        let compiled_against = a11y_observation(fixture_tree("result"));
        let assertion = compile(
            &criterion("status[name=\"result\"] equals clicked"),
            &compiled_against,
        )
        .expect("compiles");
        // ...then replay against a tree where that control was renamed: the
        // locator no longer binds, surfacing as honest structural drift.
        let renamed = a11y_observation(fixture_tree("outcome"));
        assert!(matches!(
            assertion.evaluate(&renamed),
            AssertionOutcome::Drifted { .. }
        ));
    }

    #[test]
    fn an_a11y_locator_reports_drift_against_a_non_a11y_state() {
        let cli = observation(vec![cli_checkpoint("final", "done\n", 0)]);
        let assertion = deterministic(
            0,
            Locator::A11y {
                target: selector("status", Some("result")),
                ancestors: Vec::new(),
            },
            Expected::Literal {
                value: BoundValue::Text(RESULT_VALUE.to_string()),
            },
            "status[name=\"result\"] equals clicked",
        );
        assert!(matches!(
            assertion.evaluate(&cli),
            AssertionOutcome::Drifted { .. }
        ));
    }

    #[test]
    fn an_a11y_locator_displays_in_the_role_name_within_dialect() {
        let locator = Locator::A11y {
            target: selector("textbox", Some("Email")),
            ancestors: vec![selector("form", Some("Login"))],
        };
        assert_eq!(
            locator.to_string(),
            "textbox[name=\"Email\"] within form[name=\"Login\"]"
        );
    }
}
