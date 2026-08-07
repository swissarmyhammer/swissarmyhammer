//! Structured finding data model for the local multi-agent review pipeline.
//!
//! A [`Finding`] is the unit of currency that flows through the whole review
//! pipeline: it is *emitted* by the fleet review agents (as JSON), *consumed* by
//! the verifier (which wraps each one in a [`VerifiedFinding`]), and *rendered*
//! by synthesis into the human-facing report.
//!
//! The fleet-agent prompt instructs agents to emit a JSON array of findings;
//! [`parse_findings`] turns a raw agent response — prose and ```` ```json ````
//! fences and all — back into a `Vec<Finding>`.

use serde::{Deserialize, Serialize};

use crate::error::AvpError;

/// A single structured review finding.
///
/// This is the structured shape a fleet review agent emits and every later
/// stage consumes. The `claim`/`evidence`/`suggestion` triple deliberately
/// separates three different concerns — see the per-field docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// File the finding is about (path as the agent saw it).
    pub file: String,

    /// 1-based line number the finding points at.
    pub line: u32,

    /// The source validator — the shard/RuleSet that produced this finding.
    ///
    /// Optional in the agent's emitted JSON: the fan-out agent reviews against a
    /// single validator and need not echo its name, and the engine
    /// authoritatively re-tags every parsed finding with the shard's validator
    /// (see `fleet::tag_findings`). Defaulting here keeps a real agent's
    /// response — which routinely omits this redundant field — from failing to
    /// parse and silently dropping the whole batch.
    #[serde(default)]
    pub validator: String,

    /// Which specific rule inside the validator fired, when known.
    ///
    /// Optional traceability: agents cite it when they can, but a finding is
    /// still valid without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,

    /// What is wrong **and why it matters** — the human-facing sentence.
    ///
    /// This is the prose synthesis renders. It is *not* the proof the issue is
    /// real (that is [`Finding::evidence`]); it is the "what + why it matters".
    pub claim: String,

    /// The *proof* the issue is real — the probe hit or code citation.
    ///
    /// Verifier/audit-facing, e.g. `` "`find_duplicates`: 0.94 match at
    /// `bar.rs:88`" ``. This is the evidence the verifier checks the claim
    /// against; it is distinct from "why it matters", which lives in
    /// [`Finding::claim`].
    pub evidence: String,

    /// The fix, when the agent can offer one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Which verify layer reached a verdict on a [`VerifiedFinding`].
///
/// The verify stage has two layers (see [`crate::review::verify`]): a
/// deterministic probe guard and an adversarial agent. This records which one
/// produced the verdict so synthesis can report *how* each finding was decided
/// (a guard refutation is ground-truth-deterministic; an agent verdict is a
/// judgement). It names the *deciding* layer regardless of the verdict — a
/// confirmed finding records the layer that confirmed it just as a refuted one
/// records the layer that refuted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefutingLayer {
    /// The deterministic probe guard reached the verdict.
    Guard,
    /// The adversarial agent reached the verdict.
    Agent,
}

/// A [`Finding`] after the verifier has checked it.
///
/// The verifier confirms (or refutes) the claim against its evidence and
/// records why, plus which [layer](RefutingLayer) reached the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedFinding {
    /// The finding that was verified.
    pub finding: Finding,

    /// Whether the verifier confirmed the finding is real.
    pub confirmed: bool,

    /// Why the verifier confirmed or refuted the finding.
    pub reason: String,

    /// Which layer reached the verdict — the deterministic guard or the agent.
    ///
    /// This names the *deciding* layer, not whether the finding was refuted: a
    /// confirmed finding still records who confirmed it. `decided_by.is_some()`
    /// is therefore never the "was refuted" test — use [`Self::confirmed`].
    pub decided_by: Option<RefutingLayer>,
}

/// Parse a `Vec<Finding>` out of a raw fleet-agent response.
///
/// Agents are prompted to emit a JSON array of findings, but real responses
/// wrap that array in explanatory prose and/or ```` ```json ```` fences. This
/// function strips that wrapping (reusing the same fence/bracket extraction the
/// validator response parser uses) and deserializes the array.
///
/// Real local models also sometimes emit findings as bare JSON objects
/// instead of an array — a single `{...}`, or several consecutive
/// (NDJSON-ish) `{...}\n{...}`; when no array parses, this falls back to
/// collecting every consecutive bare [`Finding`] object so no part of the
/// batch is silently dropped.
///
/// # Errors
///
/// Returns an [`AvpError::Json`] when neither a findings array nor at least
/// one bare finding object can be located and deserialized.
pub fn parse_findings(agent_text: &str) -> Result<Vec<Finding>, AvpError> {
    let array = extract_json_value(agent_text, '[', ']');
    match serde_json::from_str(array) {
        Ok(findings) => Ok(findings),
        Err(array_err) => parse_bare_object_findings(agent_text).ok_or_else(|| array_err.into()),
    }
}

/// Parse one or more consecutive top-level bare JSON objects as findings.
///
/// The fallback for the no-array shapes real local models emit: a single bare
/// `{...}`, or several NDJSON-ish consecutive objects `{...}\n{...}`. It
/// starts at the object [`extract_json_value`] locates (keeping its fence
/// stripping), then keeps collecting whitespace-separated balanced objects
/// until one is unbalanced, fails to deserialize as a [`Finding`], or the
/// text moves on to something that is not an object. Returns `None` when not
/// even one finding object parses, so the caller reports the array error.
fn parse_bare_object_findings(agent_text: &str) -> Option<Vec<Finding>> {
    let region = extract_json_value(agent_text, '{', '}');
    // `region` is always a subslice of `agent_text`, so this offset locates it
    // exactly; the scan then continues past the region's end for further
    // consecutive objects the single-value extractor cannot return.
    let offset = region.as_ptr() as usize - agent_text.as_ptr() as usize;
    let mut rest = agent_text[offset..].trim_start();

    let mut findings = Vec::new();
    while rest.starts_with('{') {
        let Some(end) = matching_delimiter(rest, '{', '}') else {
            break;
        };
        let Ok(finding) = serde_json::from_str::<Finding>(&rest[..=end]) else {
            break;
        };
        findings.push(finding);
        rest = rest[end + 1..].trim_start();
    }
    (!findings.is_empty()).then_some(findings)
}

/// How many `[` positions the repair pass tries as the findings array's start.
///
/// A reply's prose carries a handful of bracketed asides at most ("I reviewed
/// [src/lib.rs]"), so the real array is among the first few candidates. The cap
/// keeps a reply that is mostly brackets from turning the repair into a long
/// scan.
const MAX_REPAIR_ARRAY_STARTS: usize = 16;

/// Parse a fleet-agent reply into findings, repairing a reply [`parse_findings`]
/// rejects.
///
/// The reply is a JSON array by contract, so a rejected one earns a cheap repair
/// pass before the caller spends anything more expensive on it: strip the text
/// before an `[` and after the last `]`, and deserialize what is left.
/// [`extract_json_value`] already does that for the FIRST `[` and the delimiter
/// that balances it, which a bracketed aside in the prose captures instead of
/// the array; this walks the later `[` candidates too, up to
/// [`MAX_REPAIR_ARRAY_STARTS`] of them.
///
/// # Errors
///
/// Returns the [`parse_findings`] error when no candidate span deserializes into
/// a findings array. The repair never invents a message of its own — the error a
/// caller reports is the one the real parse produced.
pub(crate) fn parse_findings_repaired(agent_text: &str) -> Result<Vec<Finding>, AvpError> {
    let parse_error = match parse_findings(agent_text) {
        Ok(findings) => return Ok(findings),
        Err(err) => err,
    };
    repair_findings_array(agent_text).ok_or(parse_error)
}

/// Deserialize the first `[`…last `]` span of `agent_text` that yields a findings
/// array, or `None` when no candidate span does.
fn repair_findings_array(agent_text: &str) -> Option<Vec<Finding>> {
    let end = agent_text.rfind(']')?;
    agent_text
        .match_indices('[')
        .take_while(|(start, _)| *start < end)
        .take(MAX_REPAIR_ARRAY_STARTS)
        .find_map(|(start, _)| serde_json::from_str(&agent_text[start..=end]).ok())
}

/// The Markdown marker that opens and closes a fenced code block.
const CODE_FENCE: &str = "```";

/// The Markdown marker that opens a fenced code block tagged as JSON.
const JSON_CODE_FENCE: &str = "```json";

/// Extract the JSON value substring delimited by `open`/`close` from an agent
/// response.
///
/// The one fence-stripping extractor for every agent-emitted JSON shape in
/// the review pipeline: the findings array (`[ ... ]`), the bare-object
/// findings fallback (`{ ... }`), and the verify stage's verdict object.
///
/// The strategies are a ladder, each one a rung the next only reaches when it
/// returns `None`, in this order:
///
/// 1. [`json_fenced_value`] — a ```` ```json ```` fenced block.
/// 2. [`bare_fenced_value`] — any bare ```` ``` ```` fenced block.
/// 3. [`balanced_value`] — the first `open` to the `close` that balances it.
/// 4. [`spanning_value`] — the first `open` to the last `close`, as a last
///    resort.
///
/// Falls back to the trimmed input so the caller's `serde_json` error carries a
/// useful message when nothing delimited is present.
pub(crate) fn extract_json_value(response: &str, open: char, close: char) -> &str {
    let trimmed = response.trim();
    json_fenced_value(trimmed, open, close)
        .or_else(|| bare_fenced_value(trimmed, open, close))
        .or_else(|| balanced_value(trimmed, open, close))
        .or_else(|| spanning_value(trimmed, open, close))
        .unwrap_or(trimmed)
}

/// Return `content` trimmed when it is exactly one `open`…`close` value.
///
/// The shared acceptance test of both fenced rungs: a fence whose body is
/// something other than the wanted value is not an answer, so the caller falls
/// through to the next rung instead of returning prose.
fn delimited_value(content: &str, open: char, close: char) -> Option<&str> {
    let content = content.trim();
    (content.starts_with(open) && content.ends_with(close)).then_some(content)
}

/// Rung 1: the value inside a ```` ```json ```` fenced block.
fn json_fenced_value(trimmed: &str, open: char, close: char) -> Option<&str> {
    let marker = trimmed.find(JSON_CODE_FENCE)?;
    let body = &trimmed[marker + JSON_CODE_FENCE.len()..];
    delimited_value(&body[..body.find(CODE_FENCE)?], open, close)
}

/// Rung 2: the value inside any bare ```` ``` ```` fenced block.
fn bare_fenced_value(trimmed: &str, open: char, close: char) -> Option<&str> {
    let marker = trimmed.find(CODE_FENCE)?;
    let after_marker = &trimmed[marker + CODE_FENCE.len()..];
    // Skip an optional language identifier on the fence's opening line.
    let body_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
    let body = &after_marker[body_start..];
    delimited_value(&body[..body.find(CODE_FENCE)?], open, close)
}

/// Rung 3: the first `open` through the `close` that balances it.
fn balanced_value(trimmed: &str, open: char, close: char) -> Option<&str> {
    let start = trimmed.find(open)?;
    let end = matching_delimiter(&trimmed[start..], open, close)?;
    Some(&trimmed[start..=start + end])
}

/// Rung 4: the first `open` through the last `close`, whether or not they
/// balance.
fn spanning_value(trimmed: &str, open: char, close: char) -> Option<&str> {
    let start = trimmed.find(open)?;
    let end = trimmed.rfind(close)?;
    (start < end).then(|| &trimmed[start..=end])
}

/// Find the byte index (relative to `s`, which must start with `open`) of the
/// `close` that balances the opening delimiter, honouring string literals and
/// escapes.
fn matching_delimiter(s: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            c if c == open && !in_string => depth += 1,
            c if c == close && !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_finding(rule: Option<&str>, suggestion: Option<&str>) -> Finding {
        Finding {
            file: "src/bar.rs".to_string(),
            line: 88,
            validator: "deduplicate".to_string(),
            rule: rule.map(String::from),
            claim: "Duplicated logic with foo.rs — a future edit will fix only one copy."
                .to_string(),
            evidence: "`find_duplicates`: 0.94 match at `foo.rs:42`".to_string(),
            suggestion: suggestion.map(String::from),
        }
    }

    #[test]
    fn finding_round_trips_with_rule_some() {
        let finding = sample_finding(Some("no-copy-paste"), Some("Extract a shared helper."));
        let json = serde_json::to_string(&finding).unwrap();
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(finding, back);
        assert_eq!(back.rule.as_deref(), Some("no-copy-paste"));
    }

    #[test]
    fn finding_round_trips_with_rule_none() {
        let finding = sample_finding(None, None);
        let json = serde_json::to_string(&finding).unwrap();
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(finding, back);
        assert_eq!(back.rule, None);
        assert_eq!(back.suggestion, None);
    }

    #[test]
    fn finding_serializes_without_a_severity_field() {
        // Review is a binary pass/fail model: a finding carries no severity tier.
        // The on-the-wire shape must never include a `severity` key.
        let json = serde_json::to_value(sample_finding(None, None)).unwrap();
        assert!(
            json.get("severity").is_none(),
            "a finding must not serialize a severity field: {json}"
        );
    }

    #[test]
    fn finding_parses_without_a_severity_field() {
        // A contract-shaped finding the fan-out agent emits carries no severity.
        let json = r#"{
            "file": "src/bar.rs",
            "line": 88,
            "validator": "deduplicate",
            "claim": "Duplicated logic.",
            "evidence": "0.94 match"
        }"#;
        let finding: Finding = serde_json::from_str(json).unwrap();
        assert_eq!(finding.file, "src/bar.rs");
        assert_eq!(finding.line, 88);
    }

    #[test]
    fn finding_omits_none_rule_and_suggestion_from_json() {
        let json = serde_json::to_string(&sample_finding(None, None)).unwrap();
        assert!(!json.contains("rule"), "rule should be omitted: {json}");
        assert!(
            !json.contains("suggestion"),
            "suggestion should be omitted: {json}"
        );
    }

    #[test]
    fn finding_deserializes_when_rule_and_suggestion_absent() {
        // An agent that does not know the rule omits the field entirely.
        let json = r#"{
            "file": "src/bar.rs",
            "line": 88,
            "validator": "deduplicate",
            "severity": "warning",
            "claim": "Duplicated logic.",
            "evidence": "0.94 match"
        }"#;
        let finding: Finding = serde_json::from_str(json).unwrap();
        assert_eq!(finding.rule, None);
        assert_eq!(finding.suggestion, None);
    }

    #[test]
    fn verified_finding_round_trips() {
        let verified = VerifiedFinding {
            finding: sample_finding(Some("no-copy-paste"), None),
            confirmed: true,
            reason: "Confirmed: the 0.94 match is a real copy-paste.".to_string(),
            decided_by: Some(RefutingLayer::Agent),
        };
        let json = serde_json::to_string(&verified).unwrap();
        let back: VerifiedFinding = serde_json::from_str(&json).unwrap();
        assert_eq!(verified, back);
    }

    #[test]
    fn a_confirmed_finding_does_not_serialize_a_refuted_named_field() {
        // The verdict-layer field records WHO decided, not that the finding was
        // refuted. A *confirmed* finding still names the deciding layer, so the
        // field must not be called `refuted_by` (a confirmed-yet-"refuted_by"
        // object is self-contradictory on the wire).
        let confirmed = VerifiedFinding {
            finding: sample_finding(None, None),
            confirmed: true,
            reason: "the agent positively substantiated the claim".to_string(),
            decided_by: Some(RefutingLayer::Agent),
        };
        let value: serde_json::Value = serde_json::to_value(&confirmed).unwrap();

        assert!(
            value.get("refuted_by").is_none(),
            "a confirmed finding must not carry a `refuted_by` field: {value}"
        );
        assert_eq!(
            value.get("decided_by").and_then(|v| v.as_str()),
            Some("agent"),
            "the deciding layer must serialize under `decided_by`: {value}"
        );
        assert_eq!(value.get("confirmed").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn parse_findings_reads_clean_json_array() {
        let text = r#"[
            {"file": "a.rs", "line": 1, "validator": "v1", "severity": "blocker",
             "claim": "c1", "evidence": "e1"},
            {"file": "b.rs", "line": 2, "validator": "v2", "rule": "r2",
             "severity": "nit", "claim": "c2", "evidence": "e2",
             "suggestion": "fix it"}
        ]"#;
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, None);
        assert_eq!(findings[1].rule.as_deref(), Some("r2"));
        assert_eq!(findings[1].suggestion.as_deref(), Some("fix it"));
    }

    #[test]
    fn parse_findings_tolerates_a_finding_without_validator() {
        // The fan-out output contract does NOT ask the agent for `validator`
        // (the engine knows the shard and re-tags every finding), so a real
        // agent omits it. Before `#[serde(default)]` on `Finding::validator`,
        // this failed with "missing field `validator`" and the WHOLE batch
        // degraded to zero findings — a real review silently found nothing.
        let text = r#"[
            {"file": "lib.rs", "line": 5, "severity": "blocker",
             "claim": "dead fn", "evidence": "no inbound callers"}
        ]"#;
        let findings = parse_findings(text).expect("a contract-shaped finding must parse");
        assert_eq!(findings.len(), 1);
        // The agent left it empty; the fleet stage fills the authoritative name.
        assert_eq!(findings[0].validator, "");
        assert_eq!(findings[0].file, "lib.rs");
    }

    #[test]
    fn parse_findings_reads_fenced_json_with_surrounding_prose() {
        let text = r#"I reviewed the changed files and found one issue.

Here are my findings:

```json
[
  {
    "file": "src/bar.rs",
    "line": 88,
    "validator": "deduplicate",
    "rule": "no-copy-paste",
    "severity": "warning",
    "claim": "Duplicated logic with foo.rs.",
    "evidence": "`find_duplicates`: 0.94 match at `foo.rs:42`",
    "suggestion": "Extract a shared helper."
  }
]
```

Let me know if you want more detail."#;
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/bar.rs");
        assert_eq!(findings[0].rule.as_deref(), Some("no-copy-paste"));
    }

    #[test]
    fn parse_findings_reads_bare_fenced_block() {
        let text = "Findings below:\n\n```\n[{\"file\": \"a.rs\", \"line\": 3, \
                    \"validator\": \"v\", \"severity\": \"nit\", \"claim\": \"c\", \
                    \"evidence\": \"e\"}]\n```\n";
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
    }

    /// The rung-1 marker is the lowercase ```` ```json ````, so a model that
    /// spells the language identifier in upper case misses it and the reply
    /// must still parse.
    ///
    /// This pins the outcome, not a rung. Measured by mutation: disabling any
    /// one of `bare_fenced_value`, `balanced_value`, or `spanning_value`
    /// leaves this test — and every other parse test — green, because each
    /// remaining rung finds the same array. The ladder is a fallback chain by
    /// design, so its rungs are redundant on well-formed input; what a test
    /// can hold is that the reply parses, whichever rung gets there.
    #[test]
    fn parse_findings_reads_a_fence_whose_language_is_upper_case() {
        let text = "Findings:\n\n```JSON\n[{\"file\": \"a.rs\", \"line\": 7, \
                    \"validator\": \"v\", \"severity\": \"nit\", \"claim\": \"c\", \
                    \"evidence\": \"e\"}]\n```\n";
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 7);
    }

    #[test]
    fn parse_findings_reads_array_embedded_in_prose_without_fence() {
        let text = "Result: [{\"file\": \"a.rs\", \"line\": 9, \"validator\": \"v\", \
                    \"severity\": \"blocker\", \"claim\": \"c\", \"evidence\": \"e\"}] done.";
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 9);
    }

    #[test]
    fn parse_findings_reads_a_single_bare_object() {
        // Real local models sometimes emit ONE finding as a bare JSON object
        // instead of a one-element array. Seen in a real qwen review run: the
        // magic-numbers shard emitted `{...}`, the parse failed with
        // "invalid type: map, expected a sequence", and the whole batch
        // degraded to zero findings plus a failed fleet task.
        let text = r#"
{
  "file": "src/orders.rs",
  "line": 10,
  "rule": "no-magic-numbers",
  "severity": "warning",
  "claim": "The literal 0.0825 is a tax rate with no named constant.",
  "evidence": "orders.rs:10: `total + total * 0.0825`",
  "suggestion": "Define `SALES_TAX_RATE` and use it."
}
"#;
        let findings = parse_findings(text).expect("a single bare finding object must parse");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule.as_deref(), Some("no-magic-numbers"));
    }

    #[test]
    fn parse_findings_reads_consecutive_bare_objects() {
        // NDJSON-ish sibling of the single-bare-object shape: multiple
        // findings as consecutive bare objects with no enclosing array. The
        // fallback must collect ALL of them, not parse the first and
        // silently drop the rest.
        let text = "{\"file\": \"a.rs\", \"line\": 1, \"validator\": \"v\", \
                    \"severity\": \"warning\", \"claim\": \"first\", \"evidence\": \"e1\"}\n\
                    {\"file\": \"b.rs\", \"line\": 2, \"validator\": \"v\", \
                    \"severity\": \"nit\", \"claim\": \"second\", \"evidence\": \"e2\"}";
        let findings = parse_findings(text).expect("consecutive bare objects must parse");
        assert_eq!(findings.len(), 2, "got: {findings:?}");
        assert_eq!(findings[0].claim, "first");
        assert_eq!(findings[1].claim, "second");
    }

    #[test]
    fn parse_findings_reads_a_single_fenced_object() {
        let text = "One issue:\n\n```json\n{\"file\": \"a.rs\", \"line\": 7, \
                    \"severity\": \"nit\", \"claim\": \"c\", \"evidence\": \"e\"}\n```\n";
        let findings = parse_findings(text).expect("a single fenced finding object must parse");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 7);
    }

    #[test]
    fn parse_findings_reads_empty_array() {
        let findings = parse_findings("No issues found.\n\n```json\n[]\n```").unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_findings_tolerates_bracket_inside_string_value() {
        // A `]` inside a JSON string must not be mistaken for the array close.
        let text = "[{\"file\": \"a.rs\", \"line\": 1, \"validator\": \"v\", \
                    \"severity\": \"nit\", \"claim\": \"array index a[0] is off\", \
                    \"evidence\": \"e\"}]";
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].claim, "array index a[0] is off");
    }

    #[test]
    fn parse_findings_errors_on_malformed_input() {
        let err = parse_findings("this is not json at all").unwrap_err();
        assert!(matches!(err, AvpError::Json(_)), "got: {err:?}");
    }

    #[test]
    fn parse_findings_errors_on_truncated_array() {
        let err = parse_findings("```json\n[{\"file\": \"a.rs\"\n```").unwrap_err();
        assert!(matches!(err, AvpError::Json(_)), "got: {err:?}");
    }

    /// A bracketed aside in the prose captures the strict parse's array
    /// extraction — it balances `[src/lib.rs]` and never reaches the real array
    /// — and the schema the model restates keeps the bare-object fallback from
    /// rescuing it. The repair walks on to the next `[` and reads the array.
    #[test]
    fn parse_findings_repaired_reads_past_a_bracketed_aside() {
        let text = "Each finding is {\"file\": <path>, \"line\": <n>}. I reviewed \
                    [src/lib.rs] and found:\n\
                    [{\"file\": \"src/lib.rs\", \"line\": 4, \"claim\": \"c1\", \"evidence\": \"e1\"},\
                    {\"file\": \"src/lib.rs\", \"line\": 9, \"claim\": \"c2\", \"evidence\": \"e2\"}]";
        parse_findings(text).expect_err("the strict parse cannot read past the aside");

        let findings = parse_findings_repaired(text).expect("the repair must read the array");
        assert_eq!(findings.len(), 2, "got: {findings:?}");
        assert_eq!(findings[0].claim, "c1");
        assert_eq!(findings[1].claim, "c2");
    }

    /// A reply the strict parse already reads is returned unchanged — the repair
    /// is a fallback, never a second interpretation of a good reply.
    #[test]
    fn parse_findings_repaired_leaves_a_clean_reply_alone() {
        let text = "```json\n[{\"file\": \"a.rs\", \"line\": 1, \"claim\": \"c\", \
                    \"evidence\": \"e\"}]\n```";
        let findings = parse_findings_repaired(text).unwrap();
        assert_eq!(findings, parse_findings(text).unwrap());
    }

    /// A reply cut off mid-object has no `]` to strip back to, so no candidate
    /// span exists and the repair reports the parse's own error. This is the
    /// reply shape that earns a re-ask.
    #[test]
    fn parse_findings_repaired_errors_on_a_reply_with_no_array_close() {
        let err =
            parse_findings_repaired("Here are my findings:\n\n[{\"file\": \"a.rs\", ").unwrap_err();
        assert!(matches!(err, AvpError::Json(_)), "got: {err:?}");
    }

    /// Delimiter counting balances the FIRST `[`, so an opening bracket that
    /// nothing closes leaves it with no answer. The last-resort span — the
    /// first `[` through the last `]` — is the rung that still returns text.
    #[test]
    fn extract_json_value_spans_first_open_to_last_close_when_nothing_balances() {
        // The inner `[` is never closed, so no `]` balances the outer one.
        assert_eq!(
            extract_json_value("prose [[1, 2] more", '[', ']'),
            "[[1, 2]"
        );
    }

    /// A response with no delimiter at all returns the trimmed input, so the
    /// caller's `serde_json` error quotes the real text.
    #[test]
    fn extract_json_value_falls_back_to_the_trimmed_response() {
        assert_eq!(
            extract_json_value("  no json here  ", '[', ']'),
            "no json here"
        );
    }
}
