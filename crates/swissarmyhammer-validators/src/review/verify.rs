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
//! Each [`VerifiedFinding`] records which layer reached the verdict
//! ([`VerifiedFinding::decided_by`]) — the deciding layer regardless of the
//! verdict, not a "was refuted" flag — so synthesis can report confirmed/refuted
//! counts and reasons.
//!
//! # Verify reuses the run's shared prime
//!
//! The fleet stage primes the run's large shared content — the change purpose
//! and every file's diff/source/probe evidence — into ONE session and forks it
//! per validator (see [`fleet`](crate::review::fleet)). That same change+diffs
//! context is also the bulk of what a verify prompt needs, so verify forks the
//! SAME primed prefix: each verify task runs on a `session/fork` of the run
//! prime and sends only its per-candidate tail (the adversary header, the claim
//! under test, the candidate's `source_slice`, the probe evidence, the verdict
//! contract). The cached change/diff prefix is reused across fan-out AND verify,
//! maximizing fork reuse on qwen and the hosted prefix cache on Claude.
//!
//! When the run has no prime (priming failed, or there is none to fork), or when
//! an individual fork fails, the verify task falls back to a fresh-session
//! prompt — correct, just cold. The prime's pin is held by
//! [`run_review`](crate::review::run_review) across both stages and released
//! once verify has drained.

use std::fmt::Write as _;

use agent_client_protocol::schema::SessionId;
use serde::Deserialize;

use crate::review::fleet::{classify_reuse, PrefixReuse};
use crate::review::probes::{render_probe_evidence, ProbeKind, ProbeResult};
use crate::review::types::{extract_json_value, Finding, RefutingLayer, VerifiedFinding};
use crate::validators::{AgentPool, PoolError};

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
                    decided_by: Some(RefutingLayer::Guard),
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
///
/// `prime` is the run's shared primed-prefix session (the change + all diffs),
/// when fan-out primed one. Each verify task forks it and sends only its
/// per-candidate tail, reusing the cached change/diff prefix; a verify task with
/// no prime — or one whose fork fails — falls back to a fresh-session prompt.
pub async fn verify_findings(
    candidates: Vec<Candidate>,
    pool: &AgentPool,
    prime: Option<&SessionId>,
) -> VerifyOutcome {
    let GuardOutcome { survivors, refuted } = run_guard(&candidates);

    // Submit every guard survivor to the shared pool up front, so the verify
    // tasks pipeline (they queue alongside any fan-out tasks still in flight).
    // With a run prime, each task forks it (warm reuse of the cached change +
    // diffs); without one, each is a fresh-session prompt.
    struct Pending {
        candidate: Candidate,
        rx: Submitted,
    }
    let pending: Vec<Pending> = survivors
        .into_iter()
        .map(|candidate| {
            let prompt = render_verify_prompt(&candidate);
            let rx = match prime {
                Some(parent) => Submitted::Forked(pool.submit_forked(parent, prompt)),
                None => Submitted::Fresh(pool.submit(prompt)),
            };
            Pending { candidate, rx }
        })
        .collect();

    let guard_refuted = refuted.len();

    // The guard-refuted findings carry straight through.
    let mut verified = refuted;

    // Collect each verify task, refuting by default on any failure.
    for task in pending {
        let verdict = collect_verify(task.rx, &task.candidate, pool).await;
        let finding = task.candidate.finding;
        tracing::debug!(
            file = %finding.file,
            validator = %finding.validator,
            rule = ?finding.rule,
            confirmed = verdict.confirmed,
            "verify: agent verdict"
        );
        verified.push(VerifiedFinding {
            finding,
            confirmed: verdict.confirmed,
            reason: verdict.reason,
            decided_by: Some(RefutingLayer::Agent),
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

/// How one verify task was submitted: a fork of the run's shared prime (the warm
/// path, reusing the cached change + diffs), or a fresh-session prompt (when
/// there is no prime to fork).
enum Submitted {
    Forked(tokio::sync::oneshot::Receiver<crate::validators::SessionTurnResult>),
    Fresh(tokio::sync::oneshot::Receiver<crate::validators::PromptResult>),
}

/// Resolve one verify task into a [`Verdict`], whichever way it was submitted.
///
/// A forked task logs whether the fork was warm (parent state attached, with the
/// reused token count) or degraded (cold), then parses the verdict like the
/// fresh path. A fork that FAILED outright re-submits the same verify prompt on a
/// fresh session — correct, just cold — never a lost verdict. Every other failure
/// refutes by default, exactly as the fresh path does.
async fn collect_verify(rx: Submitted, candidate: &Candidate, pool: &AgentPool) -> Verdict {
    match rx {
        Submitted::Fresh(rx) => collect_verdict(rx.await),
        Submitted::Forked(rx) => match rx.await {
            Ok(Ok(turn)) => {
                let reuse = classify_reuse(turn.fork, turn.cache_usage);
                tracing::info!(
                    file = %candidate.finding.file,
                    session = %turn.session_id,
                    reuse = reuse.label(),
                    reused_tokens = ?reuse.reused_tokens(),
                    cache_read_input_tokens = ?reuse.cache_read(),
                    cache_creation_input_tokens = ?reuse.cache_created(),
                    "verify task prefix reuse"
                );
                if matches!(reuse, PrefixReuse::Cold) {
                    tracing::debug!(
                        file = %candidate.finding.file,
                        session = %turn.session_id,
                        "verify task fork was degraded (no warm prefix reuse); proceeding cold"
                    );
                }
                parse_verdict(&turn.content)
            }
            Ok(Err(PoolError::ForkFailed {
                parent_session_id,
                message,
            })) => {
                tracing::warn!(
                    file = %candidate.finding.file,
                    parent = %parent_session_id,
                    error = %message,
                    "verify task fork failed; falling back to a fresh-session verify prompt"
                );
                collect_verdict(pool.submit(render_verify_prompt(candidate)).await)
            }
            Ok(Err(err)) => {
                tracing::warn!(error = %err, "verify task failed; refuting by default");
                Verdict {
                    confirmed: false,
                    reason: format!("verify task failed; refuted by default ({err})"),
                }
            }
            Err(_) => {
                tracing::warn!("verify task result was dropped; refuting by default");
                Verdict {
                    confirmed: false,
                    reason: "verify task result was dropped; refuted by default".to_string(),
                }
            }
        },
    }
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
/// Reuses the findings parser's shared [`extract_json_value`], retargeted to a
/// JSON *object*. An uncertain or malformed response — anything that is not
/// an explicit `{"confirmed": true, ...}` — resolves to refuted, so the agent
/// must positively substantiate a finding for it to survive.
fn parse_verdict(agent_text: &str) -> Verdict {
    let json = extract_json_value(agent_text, '{', '}');
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
    let _ = writeln!(out, "- Claim: {}", finding.claim);
    let _ = writeln!(out, "- Cited evidence: {}", finding.evidence);
    out.push('\n');

    out.push_str("# Source slice\n\n```\n");
    out.push_str(candidate.source_slice.trim_end());
    out.push_str("\n```\n\n");

    out.push_str("# Probe evidence (ground truth)\n\n");
    render_probe_evidence(&mut out, &candidate.probe_results, true);

    out.push_str(VERIFY_OUTPUT_CONTRACT);

    out
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

    /// A `dead-code` finding about `symbol` in `file`.
    fn dead_code_finding(file: &str, symbol: &str) -> Finding {
        Finding {
            file: file.to_string(),
            line: 10,
            validator: "dead-code".to_string(),
            rule: Some("no-unused".to_string()),
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
        assert_eq!(outcome.refuted[0].decided_by, Some(RefutingLayer::Guard));
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
        assert_eq!(outcome.refuted[0].decided_by, Some(RefutingLayer::Guard));
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

    // ---- scripted mock-agent harness (shared) ------------------------------
    //
    // The scripted ACP agent lives in `crate::review::test_support`. The
    // verifier flavor differs only in its default reply: a malformed body so
    // the verdict parser refutes by default when no script entry matches.

    use std::sync::Arc;

    use crate::review::test_support::{
        verdict_json, with_pool, ForkMode, ScriptedAgent, ScriptedAgentConfig, ScriptedReply,
        MOCK_PREFIX_TOKENS,
    };
    use crate::validators::PoolConfig;

    /// A scripted verifier agent: unmatched prompts get a malformed body so
    /// the parser refutes by default.
    fn verifier_agent(script: Vec<(String, ScriptedReply)>) -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            script,
            ScriptedAgentConfig {
                default_response: "no verdict here".to_string(),
                ..ScriptedAgentConfig::default()
            },
        )
    }

    /// A fork-capable scripted verifier agent — used to prove the verify stage
    /// forks the run's shared prime rather than minting fresh sessions.
    fn forking_verifier_agent(script: Vec<(String, ScriptedReply)>) -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            script,
            ScriptedAgentConfig {
                default_response: "no verdict here".to_string(),
                fork_mode: ForkMode::Supported,
                ..ScriptedAgentConfig::default()
            },
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

        let agent = verifier_agent(vec![
            (
                "CLAIM[alpha]".to_string(),
                ScriptedReply::Text(verdict_json(true, "alpha is substantiated")),
            ),
            (
                "CLAIM[beta]".to_string(),
                ScriptedReply::Text(verdict_json(true, "beta is substantiated")),
            ),
            (
                "CLAIM[gamma]".to_string(),
                ScriptedReply::Text(verdict_json(false, "gamma is disproven")),
            ),
            // delta's verify task errors → must resolve to refuted by default.
            ("CLAIM[delta]".to_string(), ScriptedReply::Error),
        ]);

        let outcome = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            verify_findings(candidates, &pool, None).await
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
        assert_eq!(verdict("gamma").decided_by, Some(RefutingLayer::Agent));
        // delta's task errored → refuted by the agent layer, default-false.
        assert!(
            !verdict("delta").confirmed,
            "an erroring verify task refutes by default"
        );
        assert_eq!(verdict("delta").decided_by, Some(RefutingLayer::Agent));
    }

    #[tokio::test]
    async fn verify_tasks_pipeline_on_the_same_pool_as_in_flight_fanout() {
        // One shared single-worker pool. A "fan-out" task is submitted directly
        // to it; the verify stage submits to the SAME pool. With one worker, the
        // verify task can only run after the fan-out one drains — proving there
        // is no separate verify stage/pool, they share one queue.
        let candidate = survivor("pipelined");

        let agent = verifier_agent(vec![
            (
                "FANOUT_MARKER".to_string(),
                ScriptedReply::Text("fan-out done".to_string()),
            ),
            (
                "CLAIM[pipelined]".to_string(),
                ScriptedReply::Text(verdict_json(true, "pipelined and confirmed")),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
            // A fan-out task already in flight on the shared pool.
            let fanout_rx = pool.submit("FANOUT_MARKER: a still-running fan-out task");
            // Verify submits to the SAME pool; its task queues behind the fan-out
            // one and is drained by the same single worker.
            let outcome = verify_findings(vec![candidate], &pool, None).await;
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

    /// Verify-stage sessions FORK the run's shared prime (the change + all diffs)
    /// rather than minting fresh sessions, so the cached change/diff prefix is
    /// reused across both fan-out and verify. This primes a session the way the
    /// fleet stage does, then drives `verify_findings` with that prime and proves
    /// the verify task ran on a `session/fork` of it (warm, parent state
    /// attached) — not a fresh session.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn verify_tasks_fork_the_run_prime() {
        use crate::review::fleet::PRIME_HANDOFF;
        use agent_client_protocol::schema::SessionId;

        let candidate = survivor("forked");

        // The prime turn replies OK (the scripted agent recognizes the handoff);
        // the verify fork's reply is keyed on the candidate's claim.
        let agent = forking_verifier_agent(vec![(
            "CLAIM[forked]".to_string(),
            ScriptedReply::Text(verdict_json(true, "confirmed on a warm fork")),
        )]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            // Prime a session the way fan-out does: a born-pinned prefix turn
            // ending in the handoff, which the scripted agent saves as forkable
            // state.
            let prime = pool
                .submit_primed(format!(
                    "# Change purpose\n\nshared diffs here\n\n{PRIME_HANDOFF}"
                ))
                .await
                .expect("prime delivered")
                .expect("prime ok");
            let prime_session: SessionId = prime.session_id;

            verify_findings(vec![candidate], &pool, Some(&prime_session)).await
        })
        .await;

        // The verify verdict came back confirmed — through the forked session.
        assert_eq!(outcome.confirmed_count(), 1, "the forked verify confirmed");

        // Exactly one fork was taken: the verify task forked the run prime.
        assert_eq!(
            agent_probe.fork_count(),
            1,
            "the verify task must fork the run prime, not mint a fresh session"
        );
        // The fork was warm — the run prime's saved state attached, classified
        // as a warm KV fork with the reused token count logged.
        assert!(logs_contain("verify task prefix reuse"));
        assert!(logs_contain("reuse=\"warm KV fork\""));
        assert!(logs_contain(&format!(
            "reused_tokens=Some({MOCK_PREFIX_TOKENS})"
        )));

        // The verify prompt ran on a CHILD session of the prime, never the prime
        // itself — proof it forked rather than re-priming or running fresh. The
        // scripted agent pairs each prompt with the session it ran on.
        let prime_session = session_of(&agent_probe, PRIME_HANDOFF);
        let verify_session = session_of(&agent_probe, "Adversarial verification");
        assert_ne!(
            verify_session, prime_session,
            "the verify task ran on a forked child session, not the prime session"
        );
    }

    /// The session id the scripted agent ran the first prompt containing `needle`
    /// on — pairs each seen prompt with its session.
    fn session_of(agent: &ScriptedAgent, needle: &str) -> String {
        agent
            .prompted_sessions()
            .into_iter()
            .zip(agent.seen_prompts())
            .find(|(_, prompt)| prompt.contains(needle))
            .map(|(session, _)| session)
            .unwrap_or_else(|| panic!("no prompt contained {needle:?}"))
    }
}
