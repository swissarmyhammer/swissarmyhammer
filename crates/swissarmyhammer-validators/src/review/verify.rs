//! Engine stage 3 — verify: an adversarial refute pass over candidate findings.
//!
//! Fan-out (stage 2) emits *candidate* [`Finding`]s. This stage reports a
//! candidate only after it survives refutation against ground-truth evidence.
//! There are two layers, and both are **pipelined on the shared
//! [`AgentPool`](crate::validators::AgentPool)** — there is no separate batch
//! after fan-out:
//!
//! 1. **Probe guard (deterministic, no worker).** Run inline as each finding
//!    returns. It *reuses* the [`ProbeResult`]s already attached to the work item
//!    (never re-runs a probe) and acts only on **`fact`** probes
//!    ([`ProbeKind::Fact`]):
//!    - [`callers`](ProbeKind::Fact) auto-refutes a `dead-code` finding whose
//!      symbol has non-empty inbound callers.
//!    - [`duplicates`](ProbeKind::Fact) auto-refutes a `duplication` finding with
//!      no matching duplicate block.
//!
//!    `similar` is a [`candidate`](ProbeKind::Candidate) probe, not a fact, so a
//!    reuse-miss finding gets no deterministic guard and goes straight to the
//!    agent. Anything the guard cannot decide passes through.
//! 2. **Adversarial verifier (agent, pipelined).** For each guard survivor,
//!    submit a verify task to the same `AgentPool`. The prompt is the inverse of
//!    fan-out — the finding + that file's `source_slice` + the relevant probe
//!    results + "try to DISPROVE this claim." It returns a verdict that DEFAULTS
//!    TO refuted (`confirmed = false`) on uncertainty or tool failure, so only
//!    positively-substantiated findings survive.
//!
//! Each [`VerifiedFinding`] records which layer refuted it
//! ([`VerifiedFinding::refuted_by`]) so synthesis can report confirmed/refuted
//! counts and reasons.

use std::fmt::Write as _;

use serde::Deserialize;

use crate::review::probes::{ProbeKind, ProbeResult};
use crate::review::types::{Finding, RefutingLayer, VerifiedFinding};
use crate::validators::AgentPool;

/// One candidate finding plus the ground-truth context the verify stage checks
/// it against.
///
/// The fan-out stage emits a [`Finding`]; the verify stage pairs it with the
/// `source_slice` and `probe_results` of the work item it came from (the same
/// data already on the [`FileWork`](crate::review::FileWork) — never re-derived).
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The candidate finding fan-out emitted.
    pub finding: Finding,
    /// The bounded source slice of the finding's file (the work item's slice).
    pub source_slice: String,
    /// The probe results already attached to the work item — reused, never re-run.
    pub probe_results: Vec<ProbeResult>,
}

/// The outcome of the deterministic guard pass: the survivors that must go to
/// the agent, and the findings the guard refuted outright.
#[derive(Debug, Default)]
pub struct GuardOutcome {
    /// Candidates the guard could not refute — they go to the adversarial agent.
    pub survivors: Vec<Candidate>,
    /// Findings the guard refuted deterministically via a `fact` probe.
    pub refuted: Vec<VerifiedFinding>,
}

/// A deterministic guard rule: a finding *class* it applies to, the `fact` probe
/// it consults, and the condition under which that probe refutes the finding.
///
/// The guard is a data table interpreted by one code path — adding a guard is
/// adding a row here, never a new branch. Each rule names the probe whose rows
/// it inspects and a [`Refute`] predicate that, given those rows, decides
/// whether the fact contradicts the claim.
struct GuardRule {
    /// Substring that identifies the finding class in its validator or rule
    /// name (e.g. `"dead-code"`, `"duplicat"`).
    class: &'static str,
    /// The `fact` probe this rule consults (by [`ProbeResult::name`]).
    probe: &'static str,
    /// What about the probe's rows refutes the finding.
    refute: Refute,
    /// The verdict reason recorded when this rule refutes a finding.
    reason: &'static str,
}

/// The condition under which a guard rule's `fact` probe refutes a finding.
#[derive(Clone, Copy)]
enum Refute {
    /// The probe found rows — the claimed absence is contradicted (e.g.
    /// dead-code claimed, but `callers` shows inbound callers).
    WhenRowsPresent,
    /// The probe found no rows — the claimed presence is contradicted (e.g.
    /// duplication claimed, but `duplicates` shows no matching block).
    WhenRowsAbsent,
}

impl Refute {
    /// Whether `rows_present` triggers this refutation.
    fn refutes(self, rows_present: bool) -> bool {
        match self {
            Refute::WhenRowsPresent => rows_present,
            Refute::WhenRowsAbsent => !rows_present,
        }
    }
}

/// The complete deterministic-guard table.
///
/// Only `fact` probes appear here; `similar` (a `candidate` probe) is absent by
/// design, so reuse-miss findings are never guarded — they pass straight to the
/// agent. A finding whose class matches no row, or whose matched rule finds no
/// corresponding `fact` probe on the work item, is undecidable and passes
/// through.
static GUARD_RULES: &[GuardRule] = &[
    GuardRule {
        class: "dead-code",
        probe: "callers",
        refute: Refute::WhenRowsPresent,
        reason: "refuted by `callers` fact: the symbol has inbound callers, so it is not dead code",
    },
    GuardRule {
        class: "duplicat",
        probe: "duplicates",
        refute: Refute::WhenRowsAbsent,
        reason: "refuted by `duplicates` fact: no matching duplicate block exists",
    },
];

/// Run the deterministic probe guard over a batch of candidates.
///
/// For each candidate this consults [`GUARD_RULES`], reusing the candidate's own
/// `probe_results` (never re-running a probe) and acting only on
/// [`ProbeKind::Fact`] probes. A candidate a `fact` probe contradicts is refuted
/// outright (recorded with [`RefutingLayer::Guard`]); every other candidate —
/// `similar`-backed, or one no `fact` probe can decide — survives to the agent.
pub fn run_guard(candidates: &[Candidate]) -> GuardOutcome {
    let mut outcome = GuardOutcome::default();
    for candidate in candidates {
        match guard_verdict(candidate) {
            Some(rule) => {
                tracing::debug!(
                    file = %candidate.finding.file,
                    validator = %candidate.finding.validator,
                    rule = ?candidate.finding.rule,
                    probe = %rule.probe,
                    "guard auto-refuted finding"
                );
                outcome.refuted.push(VerifiedFinding {
                    finding: candidate.finding.clone(),
                    confirmed: false,
                    reason: rule.reason.to_string(),
                    refuted_by: Some(RefutingLayer::Guard),
                });
            }
            None => outcome.survivors.push(candidate.clone()),
        }
    }
    outcome
}

/// The guard's verdict for one candidate: `Some(rule)` — the [`GuardRule`] whose
/// `fact` probe deterministically refuted it — when refuted, `None` when it is
/// undecidable and must go to the agent.
fn guard_verdict(candidate: &Candidate) -> Option<&'static GuardRule> {
    for rule in GUARD_RULES {
        if !finding_is_class(&candidate.finding, rule.class) {
            continue;
        }
        // Reuse the work item's probe result for this rule's `fact` probe. A
        // missing probe (the validator did not declare it) is undecidable, not a
        // refutation, so the finding passes through.
        let Some(fact) = fact_probe(&candidate.probe_results, rule.probe) else {
            continue;
        };
        if rule.refute.refutes(!fact.rows.is_empty()) {
            return Some(rule);
        }
    }
    None
}

/// Whether a finding belongs to a guard `class`, matched against its validator
/// name or its rule name (case-insensitive substring).
fn finding_is_class(finding: &Finding, class: &str) -> bool {
    finding.validator.to_ascii_lowercase().contains(class)
        || finding
            .rule
            .as_deref()
            .is_some_and(|r| r.to_ascii_lowercase().contains(class))
}

/// Find the `fact` probe named `name` among a work item's probe results.
///
/// Only [`ProbeKind::Fact`] probes can refute, so a candidate-kind probe of the
/// same name (there is none today, but the kind is the guard's authority) is
/// never returned.
fn fact_probe<'a>(results: &'a [ProbeResult], name: &str) -> Option<&'a ProbeResult> {
    results
        .iter()
        .find(|r| r.name == name && r.kind == ProbeKind::Fact)
}

/// The result of the whole verify stage: every candidate's verdict, whether the
/// guard or the agent reached it.
///
/// The order interleaves guard-refuted findings (decided inline as fan-out
/// returned) and agent-decided findings (decided as each verify task drained);
/// callers that need a stable order should sort. The confirmed/refuted counts
/// are exposed for synthesis/summary.
#[derive(Debug, Default)]
pub struct VerifyOutcome {
    /// Every candidate's verdict.
    pub verified: Vec<VerifiedFinding>,
}

impl VerifyOutcome {
    /// How many findings survived verification (positively substantiated).
    pub fn confirmed_count(&self) -> usize {
        self.verified.iter().filter(|v| v.confirmed).count()
    }

    /// How many findings were refuted (by either layer).
    pub fn refuted_count(&self) -> usize {
        self.verified.iter().filter(|v| !v.confirmed).count()
    }

    /// The confirmed findings only — what synthesis reports.
    pub fn confirmed(&self) -> impl Iterator<Item = &VerifiedFinding> {
        self.verified.iter().filter(|v| v.confirmed)
    }
}

/// Verify a batch of candidate findings: deterministic guard, then adversarial
/// agent, both pipelined on the shared `pool`.
///
/// Each candidate first runs through [`run_guard`]; a candidate a `fact` probe
/// refutes is recorded immediately and never reaches the agent. Every survivor
/// is submitted to `pool` as a verify task (the inverse-of-fan-out "try to
/// DISPROVE this" prompt), so verification pipelines alongside any fan-out tasks
/// still draining the same pool. A verify task that errors, is dropped, or
/// returns an unparseable verdict resolves to **refuted** (`confirmed = false`):
/// only a positively-substantiated verdict confirms a finding.
///
/// The returned [`VerifyOutcome`] carries every candidate's verdict with the
/// [layer](RefutingLayer) that decided it.
pub async fn verify_findings(candidates: Vec<Candidate>, pool: &AgentPool) -> VerifyOutcome {
    let GuardOutcome { survivors, refuted } = run_guard(&candidates);

    // Submit every guard survivor to the shared pool up front, so the verify
    // tasks pipeline (they queue alongside any fan-out tasks still in flight).
    struct Pending {
        finding: Finding,
        rx: tokio::sync::oneshot::Receiver<crate::validators::PromptResult>,
    }
    let pending: Vec<Pending> = survivors
        .into_iter()
        .map(|candidate| {
            let prompt = render_verify_prompt(&candidate);
            Pending {
                finding: candidate.finding,
                rx: pool.submit(prompt),
            }
        })
        .collect();

    let guard_refuted = refuted.len();

    // The guard-refuted findings carry straight through.
    let mut verified = refuted;

    // Collect each verify task, refuting by default on any failure.
    for task in pending {
        let verdict = collect_verdict(task.rx.await);
        tracing::debug!(
            file = %task.finding.file,
            validator = %task.finding.validator,
            rule = ?task.finding.rule,
            confirmed = verdict.confirmed,
            "verify: agent verdict"
        );
        verified.push(VerifiedFinding {
            finding: task.finding,
            confirmed: verdict.confirmed,
            reason: verdict.reason,
            refuted_by: Some(RefutingLayer::Agent),
        });
    }

    let confirmed = verified.iter().filter(|v| v.confirmed).count();
    tracing::info!(
        candidates = verified.len(),
        confirmed,
        refuted = verified.len() - confirmed,
        guard_refuted,
        "review verify complete"
    );

    VerifyOutcome { verified }
}

/// One verify task's resolved verdict.
struct Verdict {
    confirmed: bool,
    reason: String,
}

/// Resolve one delivered verify-task result into a [`Verdict`], refuting by
/// default on a task error, a dropped channel, or an unparseable response.
fn collect_verdict(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
) -> Verdict {
    let response = match delivered {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "verify task failed; refuting by default");
            return Verdict {
                confirmed: false,
                reason: format!("verify task failed; refuted by default ({err})"),
            };
        }
        Err(_) => {
            tracing::warn!("verify task result was dropped; refuting by default");
            return Verdict {
                confirmed: false,
                reason: "verify task result was dropped; refuted by default".to_string(),
            };
        }
    };
    parse_verdict(&response.content)
}

/// The verdict object the verifier agent emits.
#[derive(Debug, Deserialize)]
struct VerdictJson {
    confirmed: bool,
    #[serde(default)]
    reason: String,
}

/// Parse the agent's verify response into a [`Verdict`], refuting by default
/// when no well-formed verdict object can be read.
///
/// Reuses the same fence/bracket extraction the findings parser uses, retargeted
/// to a JSON *object*. An uncertain or malformed response — anything that is not
/// an explicit `{"confirmed": true, ...}` — resolves to refuted, so the agent
/// must positively substantiate a finding for it to survive.
fn parse_verdict(agent_text: &str) -> Verdict {
    let json = extract_json_object(agent_text);
    match serde_json::from_str::<VerdictJson>(json) {
        Ok(parsed) => Verdict {
            confirmed: parsed.confirmed,
            reason: if parsed.reason.is_empty() {
                "agent verdict (no reason given)".to_string()
            } else {
                parsed.reason
            },
        },
        Err(err) => {
            tracing::warn!(error = %err, "verify response did not parse; refuting by default");
            Verdict {
                confirmed: false,
                reason: "verifier response was not a well-formed verdict; refuted by default"
                    .to_string(),
            }
        }
    }
}

/// Extract the JSON object substring from an agent response.
///
/// The object analog of the findings parser's array extractor: a ```` ```json ````
/// fenced block, then any bare fenced block, then brace-counting from the first
/// `{` to its match, then first `{` to last `}`. Falls back to the trimmed input
/// so a `serde_json` error carries a useful message.
fn extract_json_object(response: &str) -> &str {
    let trimmed = response.trim();

    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + "```json".len()..];
        if let Some(end) = after.find("```") {
            let content = after[..end].trim();
            if looks_like_object(content) {
                return content;
            }
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let content_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after[content_start..];
        if let Some(end) = content.find("```") {
            let inner = content[..end].trim();
            if looks_like_object(inner) {
                return inner;
            }
        }
    }

    if let Some(open) = trimmed.find('{') {
        if let Some(close) = matching_brace(&trimmed[open..]) {
            return &trimmed[open..=open + close];
        }
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Whether `s` is bracketed like a JSON object.
fn looks_like_object(s: &str) -> bool {
    s.starts_with('{') && s.ends_with('}')
}

/// Find the byte index (relative to `s`, which must start with `{`) of the `}`
/// that closes the opening brace, honouring string literals and escapes.
fn matching_brace(s: &str) -> Option<usize> {
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
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
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

/// Render the adversarial verify prompt for one candidate — the inverse of the
/// fan-out prompt.
///
/// Where fan-out asks the agent to *find* issues, this hands the agent one
/// specific finding plus the same ground-truth context (the file's
/// `source_slice` and the relevant `probe_results`) and instructs it to **try to
/// DISPROVE the claim**, emitting a `{"confirmed", "reason"}` verdict. The
/// contract is refute-by-default: only a verdict that positively substantiates
/// the finding confirms it.
pub fn render_verify_prompt(candidate: &Candidate) -> String {
    let finding = &candidate.finding;
    let mut out = String::new();

    out.push_str("# Adversarial verification\n\n");
    out.push_str(
        "You are the adversary. A reviewer has made the claim below. Your job is to \
         try to DISPROVE it using only the ground-truth evidence provided — the file's \
         source and the engine-run probe results. Do not take the claim on faith.\n\n",
    );

    out.push_str("# The claim under test\n\n");
    let _ = writeln!(out, "- File: `{}`", finding.file);
    let _ = writeln!(out, "- Line: {}", finding.line);
    let _ = writeln!(out, "- Validator: {}", finding.validator);
    if let Some(rule) = &finding.rule {
        let _ = writeln!(out, "- Rule: {rule}");
    }
    let _ = writeln!(out, "- Severity: {}", finding.severity);
    let _ = writeln!(out, "- Claim: {}", finding.claim);
    let _ = writeln!(out, "- Cited evidence: {}", finding.evidence);
    out.push('\n');

    out.push_str("# Source slice\n\n```\n");
    out.push_str(candidate.source_slice.trim_end());
    out.push_str("\n```\n\n");

    out.push_str("# Probe evidence (ground truth)\n\n");
    render_probe_evidence(&mut out, &candidate.probe_results);

    out.push_str(VERIFY_OUTPUT_CONTRACT);

    out
}

/// Render a candidate's probe results as ground-truth evidence blocks.
///
/// Mirrors the fan-out renderer's probe block so the adversary sees the same
/// evidence shape, annotated with each probe's `fact`/`candidate` kind so the
/// agent knows which rows are deterministic facts.
fn render_probe_evidence(out: &mut String, results: &[ProbeResult]) {
    if results.is_empty() {
        out.push_str("_No probe evidence._\n\n");
        return;
    }
    for result in results {
        let kind = match result.kind {
            ProbeKind::Fact => "fact",
            ProbeKind::Candidate => "candidate",
        };
        let _ = writeln!(
            out,
            "- probe `{}` ({kind}) on `{}`:",
            result.name, result.target
        );
        if result.rows.is_empty() {
            out.push_str("  - (no rows)\n");
            continue;
        }
        for row in &result.rows {
            out.push_str("  - ");
            out.push_str(&row.file_path);
            if let Some(line) = row.line {
                let _ = write!(out, ":{line}");
            }
            if let Some(symbol) = &row.symbol {
                let _ = write!(out, " `{symbol}`");
            }
            if let Some(similarity) = row.similarity {
                let _ = write!(out, " @ {similarity:.2}");
            }
            if let Some(detail) = &row.detail {
                let _ = write!(out, " — {detail}");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

/// The verify output contract, shared verbatim by every verify prompt.
const VERIFY_OUTPUT_CONTRACT: &str = "\
# Output contract

Emit exactly one JSON object:

- `confirmed`: `true` only if the evidence positively substantiates the claim; \
`false` if you can disprove it OR cannot positively substantiate it.
- `reason`: one sentence — what in the evidence confirmed or disproved the claim.

If you are uncertain, set `confirmed` to `false`. The claim must be proven, not \
merely plausible.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::probes::ProbeRow;
    use crate::review::types::Severity;

    /// A `dead-code` finding about `symbol` in `file`.
    fn dead_code_finding(file: &str, symbol: &str) -> Finding {
        Finding {
            file: file.to_string(),
            line: 10,
            validator: "dead-code".to_string(),
            rule: Some("no-unused".to_string()),
            severity: Severity::Warning,
            claim: format!("`{symbol}` is dead code — nothing calls it."),
            evidence: format!("`callers` on `{symbol}`: no inbound callers"),
            suggestion: Some("Remove it.".to_string()),
        }
    }

    /// A `callers` (fact) probe result for `symbol` with the given caller rows.
    fn callers_probe(symbol: &str, callers: &[&str]) -> ProbeResult {
        ProbeResult {
            name: "callers".to_string(),
            kind: ProbeKind::Fact,
            target: symbol.to_string(),
            rows: callers
                .iter()
                .map(|c| ProbeRow {
                    file_path: format!("{c}.rs"),
                    symbol: Some((*c).to_string()),
                    line: None,
                    similarity: None,
                    detail: None,
                })
                .collect(),
        }
    }

    #[test]
    fn guard_auto_refutes_dead_code_when_callers_show_inbound_callers() {
        // A dead-code finding whose `callers` fact says the symbol IS called.
        let candidate = Candidate {
            finding: dead_code_finding("src/lib.rs", "target"),
            source_slice: "fn target() {}".to_string(),
            probe_results: vec![callers_probe("target", &["uses_target"])],
        };

        let outcome = run_guard(&[candidate]);

        // The guard refuted it deterministically — no survivor reaches the agent.
        assert!(
            outcome.survivors.is_empty(),
            "callers fact must refute dead-code"
        );
        assert_eq!(outcome.refuted.len(), 1);
        assert!(!outcome.refuted[0].confirmed);
        assert_eq!(outcome.refuted[0].refuted_by, Some(RefutingLayer::Guard));
    }

    #[test]
    #[tracing_test::traced_test]
    fn guard_logs_the_finding_and_fact_probe_when_it_auto_refutes() {
        // A dead-code finding the `callers` fact refutes — the guard must log it.
        let candidate = Candidate {
            finding: dead_code_finding("src/lib.rs", "target"),
            source_slice: "fn target() {}".to_string(),
            probe_results: vec![callers_probe("target", &["uses_target"])],
        };

        let outcome = run_guard(&[candidate]);
        assert_eq!(outcome.refuted.len(), 1);

        // The guard auto-refute is logged, naming the finding (its file/validator)
        // and the `callers` fact probe that decided it.
        assert!(logs_contain("guard auto-refuted finding"));
        assert!(logs_contain("probe=callers"));
        assert!(logs_contain("validator=dead-code"));
    }

    #[test]
    fn guard_passes_real_dead_code_through_to_the_agent() {
        // A dead-code finding whose `callers` fact confirms NO inbound callers.
        let candidate = Candidate {
            finding: dead_code_finding("src/lib.rs", "orphan"),
            source_slice: "fn orphan() {}".to_string(),
            probe_results: vec![callers_probe("orphan", &[])],
        };

        let outcome = run_guard(&[candidate]);

        // The guard cannot refute it — it survives to the adversarial agent.
        assert_eq!(
            outcome.survivors.len(),
            1,
            "a real dead-code finding survives the guard"
        );
        assert!(outcome.refuted.is_empty());
    }

    #[test]
    fn guard_passes_similar_backed_reuse_miss_straight_to_the_agent() {
        // A reuse-miss finding backed ONLY by a `similar` (candidate) probe —
        // never a fact, so the guard must never refute it.
        let finding = Finding {
            file: "src/new.rs".to_string(),
            line: 5,
            validator: "deduplicate".to_string(),
            rule: Some("prefer-reuse".to_string()),
            severity: Severity::Nit,
            claim: "Reimplements an existing util — reuse `mean_squared_error`.".to_string(),
            evidence: "`similar`: 0.91 match at `util.rs:3`".to_string(),
            suggestion: Some("Call the existing util.".to_string()),
        };
        let candidate = Candidate {
            finding,
            source_slice: "fn my_mse() {}".to_string(),
            probe_results: vec![ProbeResult {
                name: "similar".to_string(),
                kind: ProbeKind::Candidate,
                target: "my_mse".to_string(),
                rows: vec![ProbeRow {
                    file_path: "src/util.rs".to_string(),
                    symbol: Some("mean_squared_error".to_string()),
                    line: Some(3),
                    similarity: Some(0.91),
                    detail: None,
                }],
            }],
        };

        let outcome = run_guard(&[candidate]);

        assert_eq!(
            outcome.survivors.len(),
            1,
            "a similar-backed reuse-miss must always pass the guard to the agent"
        );
        assert!(outcome.refuted.is_empty());
    }

    #[test]
    fn guard_auto_refutes_duplication_when_duplicates_fact_is_empty() {
        // A duplication finding whose `duplicates` fact found NO matching block.
        let finding = Finding {
            file: "src/a.rs".to_string(),
            line: 12,
            validator: "duplication".to_string(),
            rule: Some("no-copy-paste".to_string()),
            severity: Severity::Warning,
            claim: "Duplicated block also lives in b.rs.".to_string(),
            evidence: "`duplicates`: 0.94 match".to_string(),
            suggestion: None,
        };
        let candidate = Candidate {
            finding,
            source_slice: "fn a() {}".to_string(),
            probe_results: vec![ProbeResult {
                name: "duplicates".to_string(),
                kind: ProbeKind::Fact,
                target: "src/a.rs".to_string(),
                rows: vec![],
            }],
        };

        let outcome = run_guard(&[candidate]);

        assert!(
            outcome.survivors.is_empty(),
            "an empty duplicates fact refutes duplication"
        );
        assert_eq!(outcome.refuted.len(), 1);
        assert_eq!(outcome.refuted[0].refuted_by, Some(RefutingLayer::Guard));
    }

    #[test]
    fn guard_passes_duplication_with_a_real_matching_block() {
        // A duplication finding whose `duplicates` fact DID find a block — the
        // guard cannot refute it, so it survives to the agent.
        let finding = Finding {
            file: "src/a.rs".to_string(),
            line: 12,
            validator: "duplication".to_string(),
            rule: None,
            severity: Severity::Warning,
            claim: "Duplicated block also lives in b.rs.".to_string(),
            evidence: "`duplicates`: 0.94 match at b.rs".to_string(),
            suggestion: None,
        };
        let candidate = Candidate {
            finding,
            source_slice: "fn a() {}".to_string(),
            probe_results: vec![ProbeResult {
                name: "duplicates".to_string(),
                kind: ProbeKind::Fact,
                target: "src/a.rs".to_string(),
                rows: vec![ProbeRow {
                    file_path: "src/b.rs".to_string(),
                    symbol: None,
                    line: Some(88),
                    similarity: Some(0.94),
                    detail: None,
                }],
            }],
        };

        let outcome = run_guard(&[candidate]);

        assert_eq!(
            outcome.survivors.len(),
            1,
            "a real duplication survives the guard"
        );
        assert!(outcome.refuted.is_empty());
    }

    // ---- verdict parser (pure) -------------------------------------------

    #[test]
    fn parse_verdict_reads_a_fenced_confirm() {
        let v = parse_verdict(
            "Here is my verdict:\n\n```json\n{\"confirmed\": true, \"reason\": \"proven\"}\n```\n",
        );
        assert!(v.confirmed);
        assert_eq!(v.reason, "proven");
    }

    #[test]
    fn parse_verdict_reads_a_bare_object_in_prose() {
        let v = parse_verdict(
            "Verdict: {\"confirmed\": false, \"reason\": \"disproven by callers\"} done.",
        );
        assert!(!v.confirmed);
        assert_eq!(v.reason, "disproven by callers");
    }

    #[test]
    fn parse_verdict_refutes_by_default_on_malformed_input() {
        // Anything that is not a well-formed verdict object refutes.
        let v = parse_verdict("I am not sure, it might be an issue.");
        assert!(
            !v.confirmed,
            "an unparseable verdict must refute by default"
        );
    }

    #[test]
    fn parse_verdict_refutes_by_default_when_confirmed_field_absent() {
        // The required `confirmed` field is missing → serde fails → refuted.
        let v = parse_verdict("```json\n{\"reason\": \"hmm\"}\n```");
        assert!(
            !v.confirmed,
            "a verdict lacking `confirmed` must refute by default"
        );
    }

    #[test]
    fn render_verify_prompt_inverts_fanout_with_claim_evidence_and_disprove_instruction() {
        let candidate = survivor("rendered");
        let prompt = render_verify_prompt(&candidate);

        assert!(
            prompt.contains("try to DISPROVE"),
            "must instruct the adversary: {prompt}"
        );
        assert!(
            prompt.contains("CLAIM[rendered]"),
            "must carry the specific claim: {prompt}"
        );
        assert!(
            prompt.contains("fn body_rendered"),
            "must carry the source slice: {prompt}"
        );
        assert!(
            prompt.contains("probe `similar`"),
            "must carry the probe evidence: {prompt}"
        );
        assert!(
            prompt.contains("`confirmed`"),
            "must state the verdict contract: {prompt}"
        );
    }

    // ---- scripted mock-agent harness -------------------------------------
    //
    // A minimal ACP agent that maps each incoming verify prompt onto a scripted
    // verdict by substring match (the finding's claim), delivering it as a
    // streamed `agent_message_chunk` — the same shape the pool's collector reads.
    // A script entry can be set to error, proving a failing verify task resolves
    // to refuted without deadlocking the rest.

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use agent_client_protocol::schema::{
        ContentBlock as AcpContentBlock, ContentChunk, InitializeResponse, NewSessionResponse,
        PromptRequest, PromptResponse, SessionNotification, SessionUpdate, TextContent,
    };
    use agent_client_protocol::{Channel, Client, ConnectTo, ConnectionTo, Role};

    use crate::validators::{AgentPool, PoolConfig};

    struct ScriptedAgent {
        next_session: AtomicUsize,
        /// (claim-substring, Some(verdict-json) | None=error), matched in order.
        script: Vec<(String, Option<String>)>,
        seen: Mutex<Vec<String>>,
    }

    impl ScriptedAgent {
        fn new(script: Vec<(String, Option<String>)>) -> Arc<Self> {
            Arc::new(Self {
                next_session: AtomicUsize::new(0),
                script,
                seen: Mutex::new(Vec::new()),
            })
        }

        fn seen_prompts(&self) -> Vec<String> {
            self.seen.lock().unwrap().clone()
        }

        fn response_for(&self, prompt: &str) -> Option<String> {
            for (needle, response) in &self.script {
                if prompt.contains(needle) {
                    return response.clone();
                }
            }
            // No script entry → a malformed body so the parser refutes by default.
            Some("no verdict here".to_string())
        }

        fn is_error(&self, prompt: &str) -> bool {
            self.script
                .iter()
                .find(|(needle, _)| prompt.contains(needle))
                .map(|(_, response)| response.is_none())
                .unwrap_or(false)
        }
    }

    struct ScriptedAdapter(Arc<ScriptedAgent>);

    impl ConnectTo<Client> for ScriptedAdapter {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            agent_client_protocol::Agent
                .builder()
                .name("scripted-verifier")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |_n: agent_client_protocol::ClientNotification, _cx| Ok(()),
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    fn dispatch(
        mock: &Arc<ScriptedAgent>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        let cx = cx.clone();
        cx.clone().spawn(async move {
            match request {
                Req::InitializeRequest(_) => responder
                    .cast()
                    .respond_with_result(Ok(InitializeResponse::new(1.into()))),
                Req::NewSessionRequest(_req) => {
                    let n = mock.next_session.fetch_add(1, Ordering::SeqCst);
                    let id = agent_client_protocol::schema::SessionId::new(format!("sess-{n}"));
                    responder
                        .cast()
                        .respond_with_result(Ok(NewSessionResponse::new(id)))
                }
                Req::PromptRequest(req) => {
                    let prompt = prompt_text(&req);
                    mock.seen.lock().unwrap().push(prompt.clone());
                    if mock.is_error(&prompt) {
                        return responder
                            .cast::<PromptResponse>()
                            .respond_with_error(agent_client_protocol::Error::internal_error());
                    }
                    if let Some(text) = mock.response_for(&prompt) {
                        let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                            AcpContentBlock::Text(TextContent::new(text)),
                        ));
                        let notif = SessionNotification::new(req.session_id.clone(), update);
                        let _ = cx.send_notification(notif);
                    }
                    responder.cast().respond_with_result(Ok(PromptResponse::new(
                        agent_client_protocol::schema::StopReason::EndTurn,
                    )))
                }
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    fn prompt_text(req: &PromptRequest) -> String {
        req.prompt
            .iter()
            .filter_map(|block| match block {
                AcpContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn new_notifier() -> Arc<claude_agent::NotificationSender> {
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        Arc::new(notifier)
    }

    /// Run `body` against a pool backed by the scripted verifier agent.
    async fn with_pool<F, Fut, R>(agent: Arc<ScriptedAgent>, config: PoolConfig, body: F) -> R
    where
        F: FnOnce(AgentPool) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);
        let (channel_a, channel_b) = Channel::duplex();

        let agent_task = tokio::spawn(async move {
            let _ = ScriptedAdapter(agent).connect_to(channel_a).await;
        });

        let notifier_for_handler = Arc::clone(&notifier);
        let result = Client
            .builder()
            .name("verify-test-client")
            .on_receive_notification(
                async move |notif: SessionNotification, _cx| {
                    let _ = notifier_for_handler.send_update(notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(channel_b, async move |conn: ConnectionTo<_>| {
                let pool = AgentPool::new(conn, notifier_body, config);
                Ok(body(pool).await)
            })
            .await
            .expect("client connect_with failed");

        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// A verdict object as the verifier agent would emit it, fenced in prose.
    fn verdict_json(confirmed: bool, reason: &str) -> String {
        format!(
            "After trying to disprove the claim:\n\n```json\n{{\"confirmed\": {confirmed}, \
             \"reason\": \"{reason}\"}}\n```\n"
        )
    }

    /// A guard-surviving candidate whose claim carries a unique `marker` the
    /// scripted agent matches on, and probe results the guard cannot decide.
    fn survivor(marker: &str) -> Candidate {
        Candidate {
            finding: Finding {
                file: "src/x.rs".to_string(),
                line: 1,
                validator: "deduplicate".to_string(),
                rule: Some("prefer-reuse".to_string()),
                severity: Severity::Warning,
                claim: format!("CLAIM[{marker}]: reimplements an existing util."),
                evidence: "`similar`: 0.9 match".to_string(),
                suggestion: None,
            },
            source_slice: format!("fn body_{marker}() {{}}"),
            probe_results: vec![ProbeResult {
                name: "similar".to_string(),
                kind: ProbeKind::Candidate,
                target: marker.to_string(),
                rows: vec![],
            }],
        }
    }

    #[tokio::test]
    async fn verifier_confirms_refutes_and_treats_errors_as_refuted() {
        // 4 guard-surviving candidates: 2 confirm, 1 refute, 1 whose verify errors.
        let candidates = vec![
            survivor("alpha"),
            survivor("beta"),
            survivor("gamma"),
            survivor("delta"),
        ];

        let agent = ScriptedAgent::new(vec![
            (
                "CLAIM[alpha]".to_string(),
                Some(verdict_json(true, "alpha is substantiated")),
            ),
            (
                "CLAIM[beta]".to_string(),
                Some(verdict_json(true, "beta is substantiated")),
            ),
            (
                "CLAIM[gamma]".to_string(),
                Some(verdict_json(false, "gamma is disproven")),
            ),
            // delta's verify task errors → must resolve to refuted by default.
            ("CLAIM[delta]".to_string(), None),
        ]);

        let outcome = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            verify_findings(candidates, &pool).await
        })
        .await;

        // Two confirmed (alpha, beta), two refuted (gamma by agent, delta by error).
        assert_eq!(outcome.confirmed_count(), 2, "alpha and beta are confirmed");
        assert_eq!(outcome.refuted_count(), 2, "gamma and delta are refuted");

        let verdict = |marker: &str| -> &VerifiedFinding {
            outcome
                .verified
                .iter()
                .find(|v| v.finding.claim.contains(&format!("CLAIM[{marker}]")))
                .unwrap_or_else(|| panic!("verdict for {marker}"))
        };

        assert!(verdict("alpha").confirmed);
        assert!(verdict("beta").confirmed);
        assert!(!verdict("gamma").confirmed);
        assert_eq!(verdict("gamma").refuted_by, Some(RefutingLayer::Agent));
        // delta's task errored → refuted by the agent layer, default-false.
        assert!(
            !verdict("delta").confirmed,
            "an erroring verify task refutes by default"
        );
        assert_eq!(verdict("delta").refuted_by, Some(RefutingLayer::Agent));
    }

    #[tokio::test]
    async fn verify_tasks_pipeline_on_the_same_pool_as_in_flight_fanout() {
        // One shared single-worker pool. A "fan-out" task is submitted directly
        // to it; the verify stage submits to the SAME pool. With one worker, the
        // verify task can only run after the fan-out one drains — proving there
        // is no separate verify stage/pool, they share one queue.
        let candidate = survivor("pipelined");

        let agent = ScriptedAgent::new(vec![
            (
                "FANOUT_MARKER".to_string(),
                Some("fan-out done".to_string()),
            ),
            (
                "CLAIM[pipelined]".to_string(),
                Some(verdict_json(true, "pipelined and confirmed")),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
            // A fan-out task already in flight on the shared pool.
            let fanout_rx = pool.submit("FANOUT_MARKER: a still-running fan-out task");
            // Verify submits to the SAME pool; its task queues behind the fan-out
            // one and is drained by the same single worker.
            let outcome = verify_findings(vec![candidate], &pool).await;
            // The fan-out task also completed on the shared pool.
            let fanout = fanout_rx
                .await
                .expect("fan-out delivered")
                .expect("fan-out ok");
            assert_eq!(fanout.content, "fan-out done");
            outcome
        })
        .await;

        // The verify verdict came back confirmed.
        assert_eq!(
            outcome.confirmed_count(),
            1,
            "the pipelined verify confirmed"
        );

        // The one shared pool serviced BOTH the fan-out and the verify prompt —
        // proof they pipelined through a single queue, not separate stages.
        let seen = agent_probe.seen_prompts();
        assert!(
            seen.iter().any(|p| p.contains("FANOUT_MARKER")),
            "the shared pool ran the fan-out task, got: {seen:?}"
        );
        assert!(
            seen.iter().any(|p| p.contains("Adversarial verification")),
            "the SAME shared pool ran the verify task, got: {seen:?}"
        );
    }
}
