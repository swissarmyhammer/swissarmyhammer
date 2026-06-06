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

/// Severity of a review [`Finding`].
///
/// Serializes to exactly `blocker` / `warning` / `nit`, matching the review
/// skill's checklist sections so a finding's severity maps straight onto the
/// section it is rendered under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Must be fixed before the change can merge.
    Blocker,
    /// Should be addressed but does not block.
    Warning,
    /// Minor / cosmetic; nice to fix.
    Nit,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Severity::Blocker => "blocker",
            Severity::Warning => "warning",
            Severity::Nit => "nit",
        };
        f.write_str(s)
    }
}

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
    pub validator: String,

    /// Which specific rule inside the validator fired, when known.
    ///
    /// Optional traceability: agents cite it when they can, but a finding is
    /// still valid without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,

    /// How serious the finding is.
    pub severity: Severity,

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
/// judgement).
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
    pub refuted_by: Option<RefutingLayer>,
}

/// Parse a `Vec<Finding>` out of a raw fleet-agent response.
///
/// Agents are prompted to emit a JSON array of findings, but real responses
/// wrap that array in explanatory prose and/or ```` ```json ```` fences. This
/// function strips that wrapping (reusing the same fence/bracket extraction the
/// validator response parser uses) and deserializes the array.
///
/// # Errors
///
/// Returns an [`AvpError::Json`] when no findings array can be located or the
/// extracted text is not a valid `Vec<Finding>`.
pub fn parse_findings(agent_text: &str) -> Result<Vec<Finding>, AvpError> {
    let json = extract_json_array(agent_text);
    let findings = serde_json::from_str(json)?;
    Ok(findings)
}

/// Extract the JSON array substring from an agent response.
///
/// Ported from the validator response parser's fence-stripping, generalized
/// from a single JSON object (`{ ... }`) to the JSON array (`[ ... ]`) a batch
/// of findings is emitted as. Tries, in order:
///
/// 1. A ```` ```json ```` fenced block.
/// 2. Any bare ```` ``` ```` fenced block.
/// 3. Bracket-counting from the first `[` to its matching `]`.
/// 4. The first `[` to the last `]` as a last resort.
///
/// Falls back to the trimmed input so the caller's `serde_json` error carries a
/// useful message when nothing array-shaped is present.
fn extract_json_array(response: &str) -> &str {
    let trimmed = response.trim();

    // 1. JSON within a ```json fenced block.
    if let Some(start) = trimmed.find("```json") {
        let after_marker = &trimmed[start + "```json".len()..];
        if let Some(end) = after_marker.find("```") {
            let content = after_marker[..end].trim();
            if looks_like_array(content) {
                return content;
            }
        }
    }

    // 2. JSON within a bare ``` fenced block.
    if let Some(start) = trimmed.find("```") {
        let after_marker = &trimmed[start + 3..];
        // Skip an optional language identifier on the fence's opening line.
        let content_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_marker[content_start..];
        if let Some(end) = content.find("```") {
            let inner = content[..end].trim();
            if looks_like_array(inner) {
                return inner;
            }
        }
    }

    // 3. Bracket-count from the first `[` to its matching `]`.
    if let Some(open) = trimmed.find('[') {
        if let Some(close) = matching_bracket(&trimmed[open..]) {
            return &trimmed[open..=open + close];
        }
    }

    // 4. Last resort: first `[` to last `]`.
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Whether `s` is bracketed like a JSON array.
fn looks_like_array(s: &str) -> bool {
    s.starts_with('[') && s.ends_with(']')
}

/// Find the byte index (relative to `s`, which must start with `[`) of the `]`
/// that closes the opening bracket, honouring string literals and escapes.
fn matching_bracket(s: &str) -> Option<usize> {
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
            '[' if !in_string => depth += 1,
            ']' if !in_string => {
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
            severity: Severity::Warning,
            claim: "Duplicated logic with foo.rs — a future edit will fix only one copy."
                .to_string(),
            evidence: "`find_duplicates`: 0.94 match at `foo.rs:42`".to_string(),
            suggestion: suggestion.map(String::from),
        }
    }

    #[test]
    fn severity_serializes_to_lowercase_words() {
        assert_eq!(
            serde_json::to_string(&Severity::Blocker).unwrap(),
            "\"blocker\""
        );
        assert_eq!(
            serde_json::to_string(&Severity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(serde_json::to_string(&Severity::Nit).unwrap(), "\"nit\"");
    }

    #[test]
    fn severity_deserializes_from_lowercase_words() {
        assert_eq!(
            serde_json::from_str::<Severity>("\"blocker\"").unwrap(),
            Severity::Blocker
        );
        assert_eq!(
            serde_json::from_str::<Severity>("\"warning\"").unwrap(),
            Severity::Warning
        );
        assert_eq!(
            serde_json::from_str::<Severity>("\"nit\"").unwrap(),
            Severity::Nit
        );
    }

    #[test]
    fn severity_display_matches_serde() {
        assert_eq!(Severity::Blocker.to_string(), "blocker");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Nit.to_string(), "nit");
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
        assert_eq!(finding.severity, Severity::Warning);
    }

    #[test]
    fn verified_finding_round_trips() {
        let verified = VerifiedFinding {
            finding: sample_finding(Some("no-copy-paste"), None),
            confirmed: true,
            reason: "Confirmed: the 0.94 match is a real copy-paste.".to_string(),
            refuted_by: Some(RefutingLayer::Agent),
        };
        let json = serde_json::to_string(&verified).unwrap();
        let back: VerifiedFinding = serde_json::from_str(&json).unwrap();
        assert_eq!(verified, back);
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
        assert_eq!(findings[0].severity, Severity::Blocker);
        assert_eq!(findings[1].rule.as_deref(), Some("r2"));
        assert_eq!(findings[1].suggestion.as_deref(), Some("fix it"));
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
        assert_eq!(findings[0].severity, Severity::Warning);
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

    #[test]
    fn parse_findings_reads_array_embedded_in_prose_without_fence() {
        let text = "Result: [{\"file\": \"a.rs\", \"line\": 9, \"validator\": \"v\", \
                    \"severity\": \"blocker\", \"claim\": \"c\", \"evidence\": \"e\"}] done.";
        let findings = parse_findings(text).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 9);
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
}
