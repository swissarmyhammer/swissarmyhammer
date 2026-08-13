//! Tests for the [fan-out fleet](super).
//!
//! The tests are split by subject, one module for each. The review engine
//! renders a whole file into one agent prompt, and a file over the per-file
//! prompt cap is not reviewed at all, so a test tree this size has to be
//! several files rather than one.
//!
//! - [`attribution`] — pinning the rule name an agent cited onto the roster.
//! - [`budget`] — the byte budget: the config constants, the over-cap verdict,
//!   the rendered measure the packer costs a file by, and the framing's share.
//! - [`fanout`] — the orchestrator against a scripted agent: what one run
//!   submits, and the follow-up sweeps it drives.
//! - [`forking`] — the primed prefix, the forks that hang off it, and the
//!   degraded fork modes that fall back to a monolithic prompt.
//! - [`progress`] — the progress stream a run emits, and the tally it reports
//!   when tasks fail.
//! - [`reask`] — the one re-ask a forked task gets when its reply cannot be
//!   read.
//! - [`renderer`] — what the prime, the validator suffix and the monolithic
//!   fallback put in a prompt. No agent runs.
//! - [`reuse`] — how a fork's attachment and cache usage classify as warm or
//!   cold.
//!
//! This module carries what those eight share: the imports, the ruleset and
//! work-list fixtures, and the scripted-agent harness.

mod attribution;
mod budget;
mod fanout;
mod forking;
mod progress;
mod reask;
mod renderer;
mod reuse;

use super::*;

use crate::validators::ForkAttachment;

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_sem::model::change::{ChangeType, SemanticChange};

use crate::review::probes::{ProbeKind, ProbeResult, ProbeRow};
use crate::review::scope::{ProbeNames, RuleNames, WorkList};
use crate::review::test_support::{
    findings_json, malformed_findings_json, uniform_budget, with_pool, ForkMode, ScriptedAgent,
    ScriptedAgentConfig, ScriptedReply, MOCK_PREFIX_TOKENS,
};
use crate::validators::types::{Rule, RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch};
use crate::validators::{PoolConfig, ValidatorLoader, ValidatorSource};
use claude_agent::protocol_translator::CacheUsage;

// ---- fixtures --------------------------------------------------------

/// The 1-based source line every scripted finding fixture points at. The
/// exact value is immaterial to these tests (none assert on the line); naming
/// it keeps the fixtures from sprinkling an unexplained literal.
const TEST_FINDING_LINE: u32 = 42;

/// The 1-based line the shared `file_work` fixture's `duplicates` probe row
/// cites. Like [`TEST_FINDING_LINE`] the exact value is immaterial; naming it
/// keeps the hidden fixture constant out of the probe row and its assertions.
const TEST_PROBE_LINE: u32 = 88;

/// The similarity score the shared `file_work` fixture's `duplicates` probe
/// row reports. Like [`TEST_PROBE_LINE`] the exact value is immaterial; naming
/// it keeps the score out of the fixture, the agent-output helper, and the
/// rendered-prompt assertion so all three stay locked to one number. Rendered
/// with `{:.2}` (matching the production probe formatting) wherever it appears
/// as text.
const TEST_SIMILARITY: f32 = 0.94;

/// A RuleSet whose mandate (description) and rule bodies are distinctive so
/// the rendered prompt can be asserted against them verbatim. Carries no
/// VALIDATOR.md body — use [`ruleset_with_body`] when the body matters.
fn ruleset(name: &str, mandate: &str, rules: &[(&str, &str)]) -> RuleSet {
    ruleset_with_body(name, mandate, "", rules)
}

/// Like [`ruleset`] but with a distinctive VALIDATOR.md prose `body` so the
/// rendered prompt can be asserted against the validator-wide guidance block.
fn ruleset_with_body(name: &str, mandate: &str, body: &str, rules: &[(&str, &str)]) -> RuleSet {
    RuleSet {
        manifest: RuleSetManifest {
            name: name.to_string(),
            description: mandate.to_string(),
            metadata: RuleSetMetadata {
                version: "1.0.0".to_string(),
            },
            match_criteria: Some(ValidatorMatch {
                tools: vec![],
                files: vec!["*.rs".to_string()],
                project_types: vec![],
            }),
            trigger_matcher: None,
            tags: vec![],
            probes: vec![],
            timeout: 30,
            once: false,
        },
        rules: rules
            .iter()
            .map(|(rname, body)| Rule {
                name: rname.to_string(),
                description: format!("{rname} description"),
                body: body.to_string(),
                timeout: None,
                ..Rule::default()
            })
            .collect(),
        rule_failures: vec![],
        manifest_body: body.to_string(),
        source: ValidatorSource::Builtin,
        base_path: PathBuf::from("/test"),
    }
}

/// A loader carrying the given rulesets, matched by name in `run_fleet`.
fn loader_with(rulesets: Vec<RuleSet>) -> ValidatorLoader {
    let mut loader = ValidatorLoader::new();
    for rs in rulesets {
        loader.add_builtin_ruleset(rs);
    }
    loader
}

/// A `FileWork` carrying a distinctive added entity, a source slice tagged
/// with the path, and one `duplicates` probe row.
fn file_work(path: &str, symbol: &str, dup_at: &str) -> FileWork {
    file_work_with_slice(
        path,
        symbol,
        dup_at,
        format!("// slice for {path}\nfn {symbol}() {{}}"),
    )
}

/// [`file_work`] with a caller-chosen source slice, for tests that assert on
/// how a specific slice renders.
fn file_work_with_slice(path: &str, symbol: &str, dup_at: &str, source_slice: String) -> FileWork {
    FileWork::new(
        path.to_string(),
        vec![SemanticChange {
            id: format!("{path}:{symbol}"),
            entity_id: symbol.to_string(),
            change_type: ChangeType::Added,
            entity_type: "function".to_string(),
            entity_name: symbol.to_string(),
            file_path: path.to_string(),
            old_file_path: None,
            before_content: None,
            after_content: Some(format!("fn {symbol}() {{}}")),
            commit_sha: None,
            author: None,
            timestamp: None,
            structural_change: None,
        }],
        vec![symbol.to_string()],
        source_slice,
        vec![ProbeResult {
            name: "duplicates".to_string(),
            kind: ProbeKind::Fact,
            target: path.to_string(),
            rows: vec![ProbeRow {
                file_path: dup_at.to_string(),
                symbol: Some(symbol.to_string()),
                line: Some(TEST_PROBE_LINE),
                similarity: Some(TEST_SIMILARITY),
                detail: None,
            }],
        }],
    )
}

fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
    ValidatorWork::new(
        name.to_string(),
        RuleNames::new([format!("{name}-rule")]),
        ProbeNames::new(["duplicates".to_string()]),
        files,
    )
}

// ---- scripted mock agent (shared harness) ------------------------------
//
// The scripted ACP agent lives in `crate::review::test_support` — one
// implementation shared with verify.rs, drive.rs, and the pool tests.
// Fleet tests run it with the fork extension `Supported` unless a test
// selects a degraded `ForkMode` explicitly.

/// A fork-capable scripted agent — the default fleet backend under test.
/// The [`ForkMode::Supported`] special case of [`agent_with_fork_mode`].
fn forking_agent(script: Vec<(String, ScriptedReply)>) -> Arc<ScriptedAgent> {
    agent_with_fork_mode(script, ForkMode::Supported)
}

/// A scripted agent in the given [`ForkMode`] (the default fleet config
/// otherwise).
fn agent_with_fork_mode(
    script: Vec<(String, ScriptedReply)>,
    fork_mode: ForkMode,
) -> Arc<ScriptedAgent> {
    ScriptedAgent::with_config(
        script,
        ScriptedAgentConfig {
            fork_mode,
            ..ScriptedAgentConfig::default()
        },
    )
}

/// The stable header [`FOLLOWUP_PROMPT`] carries — only a follow-up sweep
/// turn sends it, so a script entry keyed on it matches a sweep fork's
/// context and never the first-pass prompt.
const RESCAN_NEEDLE: &str = "## Completeness re-scan";

/// Broadcast-channel capacity for a rebind's notification stream. A small
/// buffer is plenty here: these single-prompt rebinds emit one reply each,
/// well under capacity, so the subscriber never lags chunks away.
const BROADCAST_BUFFER_SIZE: usize = 8;

/// A scripted follow-up reply that finds nothing further, going dry on the
/// first sweep. Every warm fork now drives at least one follow-up sweep after
/// its first pass; a test asserting unchanged first-pass behavior scripts the
/// first sweep to add nothing so the loop terminates immediately. Keyed on
/// [`RESCAN_NEEDLE`] and ordered FIRST so it wins on the sweep fork's context
/// (which also inherits the first-pass needles).
fn rescan_finds_nothing() -> (String, ScriptedReply) {
    (
        RESCAN_NEEDLE.to_string(),
        ScriptedReply::Text("[]".to_string()),
    )
}

/// Two independent rebinds of one base agent must NOT share a
/// [`ScriptedReply::Sequence`] queue — each rebind is a "fresh agent", so
/// consuming the sequence on one must leave the other's untouched.
///
/// `rebind_broadcast` deep-clones the script, so each rebind gets its own
/// queue and a prompt matching the sequence needle yields the SAME first delta
/// on both. With a shallow `Arc` share (the pre-fix bug), the first rebind's
/// prompt would pop the queue and the second would see the drained tail — a
/// silent cross-rebind test-isolation leak.
#[tokio::test]
async fn rebinds_do_not_share_sequence_state() {
    const NEEDLE: &str = "consume the sequence";
    let base = forking_agent(vec![(
        NEEDLE.to_string(),
        ScriptedReply::sequence(["first-delta".to_string(), "second-delta".to_string()]),
    )]);

    // Each rebind submits one prompt matching the sequence needle and reads
    // back which delta it served.
    async fn first_served(base: &Arc<ScriptedAgent>) -> String {
        let (tx, _) = tokio::sync::broadcast::channel(BROADCAST_BUFFER_SIZE);
        // Bridge onto the live connection too, so the pool's connection-side
        // collector (the stream `with_pool` wires up) sees the reply.
        let rebind = ScriptedAgent::rebind_broadcast(base, tx, true);
        with_pool(rebind, PoolConfig::remote(1), |pool| async move {
            let result = pool
                .submit(format!("please {NEEDLE} now"))
                .await
                .expect("result")
                .expect("ok");
            result.content
        })
        .await
    }

    let one = first_served(&base).await;
    let two = first_served(&base).await;
    assert_eq!(
        one, two,
        "each rebind has its own sequence queue, so both serve the first delta; \
         a shared queue would drain across rebinds and they would diverge"
    );
    assert!(
        one.contains("first-delta"),
        "a fresh rebind serves the sequence's first delta, got: {one}"
    );
}

/// A findings array of N objects as an agent emits it, fenced in prose — the
/// multi-instance shape `findings_json` (a single finding) does not cover.
/// Each tuple is `(file, line, rule, claim)`.
fn findings_array_json(items: &[(&str, u32, &str, &str)]) -> String {
    // Built through `serde_json` so any `"`/`\` in a field is escaped
    // correctly — a raw `format!` template would corrupt the JSON.
    let objects: Vec<serde_json::Value> = items
        .iter()
        .map(|(file, line, rule, claim)| {
            json!({
                "file": file,
                "line": line,
                "validator": "ignored-by-agent",
                "rule": rule,
                "claim": claim,
                "evidence": format!("per `duplicates`: {TEST_SIMILARITY:.2}"),
                "suggestion": "extract a helper",
            })
        })
        .collect();
    let array = json!(objects);
    format!("Here are my findings:\n\n```json\n{array}\n```\n")
}

#[test]
fn findings_array_json_escapes_embedded_quotes() {
    // A claim carrying a double quote must round-trip through valid JSON,
    // proving the helper escapes rather than concatenates raw text.
    let claim = r#"the literal "7" is a magic number"#;
    let fenced = findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", claim)]);
    let body = fenced
        .split("```json")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .expect("fenced JSON block")
        .trim();
    let parsed: serde_json::Value =
        serde_json::from_str(body).expect("findings_array_json is valid JSON");
    assert_eq!(parsed[0]["claim"], json!(claim));
    assert_eq!(parsed[0]["file"], json!("src/a.rs"));
}

/// Run the fleet and then release its shared-prime pin, exactly as
/// `run_review` drives the prime lifecycle (fan-out primes once, the caller
/// unpins when the run drains). The returned outcome has its `prime` cleared
/// so the orchestrator tests can assert the full pin→unpin cycle while the
/// pool/connection is still live.
async fn run_fleet_and_unpin(
    work: &WorkList,
    loader: &ValidatorLoader,
    pool: &AgentPool,
) -> FleetOutcome {
    let outcome = run_fleet(work, loader, pool, &ToolSuppression::default(), None).await;
    if let Some(guard) = outcome.prime {
        unpin_prefix_session(guard).await;
    }
    FleetOutcome {
        prime: None,
        ..outcome
    }
}
