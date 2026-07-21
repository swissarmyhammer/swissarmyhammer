use super::*;

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_sem::model::change::{ChangeType, SemanticChange};

use crate::review::probes::{ProbeKind, ProbeResult, ProbeRow};
use crate::review::scope::WorkList;
use crate::review::test_support::{
    findings_json, with_pool, ForkMode, ScriptedAgent, ScriptedAgentConfig, ScriptedReply,
    MOCK_PREFIX_TOKENS,
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
            })
            .collect(),
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
        vec![format!("{name}-rule")],
        vec!["duplicates".to_string()],
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
    let outcome = run_fleet(work, loader, pool, None).await;
    if let Some(guard) = outcome.prime {
        unpin_prefix_session(guard).await;
    }
    FleetOutcome {
        prime: None,
        ..outcome
    }
}

// ---- config tests ----------------------------------------------------

#[test]
fn default_batch_size_is_256_kib() {
    // The default budget clears the largest single source file in a typical
    // change (~95 KB) so an ordinary commit reviews without tripping the
    // oversize-file error; only genuinely huge multi-file diffs still split.
    assert_eq!(DEFAULT_BATCH_SIZE, 256 * 1024);
    assert_eq!(DEFAULT_BATCH_SIZE, 262144);
    assert_eq!(FleetConfig::default().batch_size(), DEFAULT_BATCH_SIZE);
}

// ---- renderer tests (pure) -------------------------------------------

#[test]
fn monolithic_prompt_contains_change_purpose_mandate_rules_and_output_contract() {
    let rs = ruleset(
        "deduplicate",
        "DEDUP_MANDATE: never copy-paste logic.",
        &[(
            "no-copy-paste",
            "RULE_BODY: extract shared helpers verbatim.",
        )],
    );
    let vw = validator_work(
        "deduplicate",
        vec![file_work("src/a.rs", "alpha", "src/x.rs")],
    );

    // The monolithic fallback for one validator: change purpose + the
    // validator's files + the validator's instructions (its full ruleset),
    // all in one self-contained prompt.
    let prompt = render_fleet_prompt("PURPOSE: scaffolding the parser.", &vw, &rs);

    assert!(
        prompt.contains("PURPOSE: scaffolding the parser."),
        "{prompt}"
    );
    assert!(
        prompt.contains("DEDUP_MANDATE: never copy-paste logic."),
        "{prompt}"
    );
    assert!(
        prompt.contains("RULE_BODY: extract shared helpers verbatim."),
        "rule body must appear verbatim: {prompt}"
    );
    // The validator's file is inlined (the cold fallback is self-contained).
    assert!(prompt.contains("## File: src/a.rs"), "{prompt}");
    assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
    // Output contract: the four load-bearing finding fields.
    assert!(prompt.contains("`rule`"), "{prompt}");
    assert!(prompt.contains("`claim`"), "{prompt}");
    assert!(prompt.contains("`evidence`"), "{prompt}");
    assert!(prompt.contains("`suggestion`"), "{prompt}");
    // Binary pass/fail: the contract carries no severity field at all.
    assert!(!prompt.contains("`severity`"), "{prompt}");
}

#[test]
fn monolithic_prompt_renders_all_of_the_validators_rules() {
    // A multi-rule validator: the per-validator monolithic prompt carries
    // EVERY one of the validator's rules.
    let rs = ruleset(
        "deduplicate",
        "mandate",
        &[
            ("no-copy-paste", "FIRST_RULE_BODY"),
            ("prefer-reuse", "SECOND_RULE_BODY"),
        ],
    );
    let vw = validator_work(
        "deduplicate",
        vec![file_work("src/a.rs", "alpha", "src/dup_of_a.rs")],
    );

    let prompt = render_fleet_prompt("purpose", &vw, &rs);

    assert!(
        prompt.contains("FIRST_RULE_BODY"),
        "the validator's first rule body must appear: {prompt}"
    );
    assert!(
        prompt.contains("SECOND_RULE_BODY"),
        "the validator's second rule body must also appear: {prompt}"
    );
    // The validator's file, slice, and probe evidence are present.
    assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
    assert!(
        prompt.contains("probe `duplicates`"),
        "probe evidence must be rendered: {prompt}"
    );
    assert!(
        prompt.contains(&format!("src/dup_of_a.rs:{TEST_PROBE_LINE}")),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!("@ {TEST_SIMILARITY:.2}")),
        "{prompt}"
    );
}

/// The run prime carries the change + every diff and NOT any validator text;
/// the per-validator suffix carries that validator's full ruleset and NOT any
/// file content. Both renders are byte-stable so every fork shares the exact
/// primed prefix.
#[test]
fn run_prime_holds_change_and_diffs_only_and_validator_suffix_holds_the_full_ruleset() {
    let rs = ruleset(
        "deduplicate",
        "DEDUP_MANDATE: never copy-paste logic.",
        &[
            ("no-copy-paste", "RULE_BODY: extract shared helpers."),
            ("prefer-reuse", "OTHER_RULE_BODY: reuse first."),
        ],
    );
    let vw = validator_work(
        "deduplicate",
        vec![file_work("src/a.rs", "alpha", "src/x.rs")],
    );
    let work = WorkList::new(
        "PURPOSE: scaffolding the parser.".to_string(),
        vec![vw.clone()],
    );

    // Byte-stable: two renders of the same inputs are identical, so every
    // validator fork shares the exact prefix the prime turn decoded.
    let prime = render_run_prime(&work);
    assert_eq!(
        prime,
        render_run_prime(&work),
        "the run prime render must be byte-stable across calls"
    );
    let suffix = render_validator_suffix(&vw, &rs);
    assert_eq!(suffix, render_validator_suffix(&vw, &rs));

    // The PRIME carries the change purpose and the file diff/source, ending
    // with the handoff — and carries NO validator text or contract.
    assert!(
        prime.contains("PURPOSE: scaffolding the parser."),
        "{prime}"
    );
    assert!(prime.contains("# Files under review"), "{prime}");
    assert!(prime.contains("## File: src/a.rs"), "{prime}");
    assert!(prime.contains("// slice for src/a.rs"), "{prime}");
    assert!(prime.contains("probe `duplicates`"), "{prime}");
    assert!(
        prime.ends_with(PRIME_HANDOFF),
        "the prime must end with the prime handoff: {prime}"
    );
    assert!(
        !prime.contains("DEDUP_MANDATE")
            && !prime.contains("RULE_BODY")
            && !prime.contains("## Output contract"),
        "the prime must carry NO validator text or contract: {prime}"
    );

    // The SUFFIX carries the validator + mandate + EVERY rule + contract,
    // and NOT the file's source contents (those live in the prime).
    assert!(
        suffix.contains(&format!("{VALIDATOR_HEADER}deduplicate")),
        "{suffix}"
    );
    assert!(suffix.contains("DEDUP_MANDATE"), "{suffix}");
    assert!(
        suffix.contains("RULE_BODY") && suffix.contains("OTHER_RULE_BODY"),
        "the suffix must carry ALL of the validator's rules: {suffix}"
    );
    assert!(suffix.contains("## Output contract"), "{suffix}");
    // The suffix names the focus file (path only) but never re-sends its
    // source — the cached prime already has it.
    assert!(
        suffix.contains("`src/a.rs`"),
        "the suffix must name the focus file path: {suffix}"
    );
    assert!(
        !suffix.contains("// slice for src/a.rs"),
        "the suffix must NOT re-send the file's source: {suffix}"
    );
    // Non-empty by construction — a fork turn never degenerates to a full
    // reprocess.
    assert!(
        !suffix.is_empty(),
        "the per-validator suffix must be non-empty"
    );

    // The monolithic fallback for the validator is self-contained: change +
    // validator's files + the validator suffix (path-scoped, contract, all
    // rules).
    let monolithic = render_fleet_prompt(work.change_purpose(), &vw, &rs);
    assert!(
        monolithic.contains("PURPOSE: scaffolding the parser."),
        "{monolithic}"
    );
    assert!(monolithic.contains("## File: src/a.rs"), "{monolithic}");
    assert!(monolithic.contains("// slice for src/a.rs"), "{monolithic}");
    assert!(monolithic.contains("RULE_BODY"), "{monolithic}");
    assert!(monolithic.contains("OTHER_RULE_BODY"), "{monolithic}");
    assert!(monolithic.ends_with(&suffix), "{monolithic}");
}

/// The validator's VALIDATOR.md prose body is folded into the per-validator
/// suffix as a validator-wide guidance block, positioned AFTER the mandate
/// (description) and BEFORE the rules so it is shared by every rule.
#[test]
fn validator_suffix_emits_the_manifest_body_after_mandate_before_rules() {
    let rs = ruleset_with_body(
        "duplication",
        "DEDUP_MANDATE: never copy-paste logic.",
        "This validator does not apply to test code.",
        &[("no-copy-paste", "RULE_BODY: extract shared helpers.")],
    );
    let vw = validator_work(
        "duplication",
        vec![file_work("src/a.rs", "alpha", "src/x.rs")],
    );

    let suffix = render_validator_suffix(&vw, &rs);

    // The body line appears verbatim in the suffix.
    assert!(
        suffix.contains("does not apply to test code"),
        "the validator body guidance must appear in the suffix: {suffix}"
    );

    // Ordering: mandate < body guidance < rules.
    let mandate_at = suffix
        .find("DEDUP_MANDATE")
        .expect("mandate must be present");
    let body_at = suffix
        .find("does not apply to test code")
        .expect("body must be present");
    let rules_at = suffix
        .find("## Rules")
        .expect("rules header must be present");
    assert!(
        mandate_at < body_at,
        "the body must come AFTER the mandate: {suffix}"
    );
    assert!(
        body_at < rules_at,
        "the body must come BEFORE the rules: {suffix}"
    );
}

/// A validator with no VALIDATOR.md body emits no guidance block — the suffix
/// is unchanged for body-less validators (the fork-prefix reuse contract
/// depends on the render being a pure function of its inputs).
#[test]
fn validator_suffix_omits_guidance_when_body_is_empty() {
    let rs = ruleset("duplication", "mandate", &[("no-copy-paste", "RULE_BODY")]);
    let vw = validator_work(
        "duplication",
        vec![file_work("src/a.rs", "alpha", "src/x.rs")],
    );

    let suffix = render_validator_suffix(&vw, &rs);
    assert!(
        !suffix.contains("## Guidance"),
        "a body-less validator must emit no guidance block: {suffix}"
    );
}

/// The monolithic fallback shares the same `render_validator_suffix`, so the
/// validator body guidance reaches the degraded path too.
#[test]
fn monolithic_prompt_contains_the_manifest_body_guidance() {
    let rs = ruleset_with_body(
        "duplication",
        "mandate",
        "This validator does not apply to test code.",
        &[("no-copy-paste", "RULE_BODY")],
    );
    let vw = validator_work(
        "duplication",
        vec![file_work("src/a.rs", "alpha", "src/x.rs")],
    );

    let prompt = render_fleet_prompt("purpose", &vw, &rs);
    assert!(
        prompt.contains("does not apply to test code"),
        "the validator body guidance must reach the monolithic fallback: {prompt}"
    );
}

/// The run prime de-duplicates files matched by several validators: a file
/// in two validators' work appears ONCE in the cached prefix.
#[test]
fn run_prime_dedups_files_shared_across_validators() {
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-a", vec![file_work("src/shared.rs", "s", "src/x.rs")]),
            validator_work("val-b", vec![file_work("src/shared.rs", "s", "src/x.rs")]),
        ],
    );

    let prime = render_run_prime(&work);
    assert_eq!(
        prime.matches("## File: src/shared.rs").count(),
        1,
        "a file matched by two validators is inlined once in the prime: {prime}"
    );
}

/// A small (fully-inlined) changed file's payload carries the file's
/// COMPLETE current contents in a clearly-labeled fenced block plus explicit
/// "you do NOT need to read this file" framing — so the model stops
/// re-reading the changed file it was already handed.
#[test]
fn full_inline_payload_carries_complete_source_and_no_reread_framing() {
    // A FileWork whose source_slice is the WHOLE file, including a marker line
    // the old bounded slice would have trimmed.
    let file = file_work_with_slice(
        "src/a.rs",
        "alpha",
        "src/x.rs",
        "use std::fmt;\n// distant_marker_kept_in_full\npub fn alpha() {}".to_string(),
    );

    let payload = render_file_payload(std::slice::from_ref(&file));

    // The complete source — including the distant marker — is present.
    assert!(
        payload.contains("// distant_marker_kept_in_full"),
        "full inline must carry every line of the file: {payload}"
    );
    // Explicit framing that the file is the complete contents and need not
    // be read.
    assert!(
        payload.to_lowercase().contains("full")
            && payload.to_lowercase().contains("do not need to read"),
        "the block must frame the source as the full file the model need not read: {payload}"
    );
    // The whole inlined file is the review boundary; the "What changed"
    // semantic diff is orientation only, NOT the review boundary — so the
    // model reviews every line, not just the changed region.
    let lower = payload.to_lowercase();
    assert!(
        lower.contains("whole file") || lower.contains("every line"),
        "the block must name the whole file as the review boundary: {payload}"
    );
    assert!(
        lower.contains("orientation only"),
        "the diff section must be framed as orientation only: {payload}"
    );
    assert!(
        lower.contains("not the review boundary"),
        "the diff section must be framed as NOT the review boundary: {payload}"
    );
}

/// The output contract scopes intrinsic reads to OTHER files (cross-file
/// duplication, callers, type defs), not the changed files already inlined in
/// full — while still leaving the tools advertised.
#[test]
fn output_contract_scopes_reads_to_other_files() {
    assert!(
        OUTPUT_CONTRACT.contains("other files"),
        "the contract must scope reads to other (cross-file) files: {OUTPUT_CONTRACT}"
    );
    // The changed files are provided in full — the contract says so.
    assert!(
        OUTPUT_CONTRACT.to_lowercase().contains("already provided")
            || OUTPUT_CONTRACT.to_lowercase().contains("provided in full"),
        "the contract must state the changed files are provided in full: {OUTPUT_CONTRACT}"
    );
}

/// The contract must demand reporting EVERY occurrence of every rule that
/// fires in a single pass — one finding per `file:line`, never stopping at the
/// first match. Bail-fast (find-one → fix → re-review) is the re-review token
/// storm this contract exists to prevent.
#[test]
fn output_contract_demands_every_occurrence_with_no_bail_fast() {
    let lower = OUTPUT_CONTRACT.to_lowercase();
    assert!(
        lower.contains("every occurrence of every rule"),
        "the contract must demand every occurrence of every rule: {OUTPUT_CONTRACT}"
    );
    assert!(
        lower.contains("do not stop at the first"),
        "the contract must forbid stopping at the first match: {OUTPUT_CONTRACT}"
    );
    assert!(
        OUTPUT_CONTRACT.contains("one finding per `file:line`"),
        "the contract must require one finding per file:line: {OUTPUT_CONTRACT}"
    );
}

/// The contract must name the WHOLE current file as the review boundary and
/// demote the semantic diff to orientation only — so a small model does not
/// anchor on the changed region and under-report pre-existing instances
/// elsewhere in the file (the finding-dribble this card kills).
#[test]
fn output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff() {
    let lower = OUTPUT_CONTRACT.to_lowercase();
    assert!(
        OUTPUT_CONTRACT.contains("## Review scope"),
        "the contract must carry an explicit review-scope section: {OUTPUT_CONTRACT}"
    );
    assert!(
        lower.contains("whole current file"),
        "the contract must name the whole current file as the review boundary: \
         {OUTPUT_CONTRACT}"
    );
    assert!(
        lower.contains("pre-existing instances"),
        "the contract must put pre-existing instances in scope: {OUTPUT_CONTRACT}"
    );
    assert!(
        lower.contains("orientation only"),
        "the contract must frame the semantic diff as orientation only: {OUTPUT_CONTRACT}"
    );
    assert!(
        lower.contains("not the review boundary"),
        "the contract must state the diff is NOT the review boundary: {OUTPUT_CONTRACT}"
    );
}

// ---- orchestrator tests (scripted mock agent) ------------------------

#[tokio::test]
async fn fan_out_two_validators_two_files_submits_one_prime_and_one_fork_per_validator() {
    // Two validators over the same two files. Under the new grain — fork per
    // VALIDATOR, files in the shared prime — the run primes ONCE and forks ONE
    // task per validator: 2 validators = 2 forks, regardless of how many files
    // each validator reviews or how many rules it carries.
    let rs_a = ruleset("val-a", "mandate a", &[("ra", "body a")]);
    let rs_b = ruleset("val-b", "mandate b", &[("rb", "body b")]);
    let loader = loader_with(vec![rs_a, rs_b]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work(
                "val-a",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
            validator_work(
                "val-b",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
        ],
    );

    // Script: a finding for each validator. The fork inherits the shared
    // prime (all files) and appends the validator suffix carrying the
    // validator header, so we key on that header.
    let agent = forking_agent(vec![
        // Each validator's first pass is exhaustive, so its completeness
        // re-scan finds nothing more — this test asserts the first-pass
        // fan-out shape (one prime + one fork per validator + one re-scan).
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}val-a\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "ra",
                "dup in a",
            )),
        ),
        (
            format!("{VALIDATOR_HEADER}val-b\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/b.rs",
                TEST_FINDING_LINE,
                "rb",
                "dup in b",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await.findings
    })
    .await;

    let seen = agent_probe.seen_prompts();
    // Exactly ONE shared prime for the whole run (not one per validator).
    let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
    assert_eq!(
        primes, 1,
        "the run primes the shared prefix exactly once: {seen:#?}"
    );
    // One forked validator task per validator: 2 validators = 2 forks.
    let validator_tasks = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .count();
    assert_eq!(
        validator_tasks, 2,
        "one forked task per validator: {seen:#?}"
    );
    // Two validator forks PLUS one completeness re-scan fork each (the
    // re-scan inherits the validator session) = four forks total.
    assert_eq!(
        agent_probe.fork_count(),
        4,
        "one validator fork plus one completeness re-scan fork per validator"
    );

    // Every finding is tagged with its validator (overriding the agent's
    // self-reported `ignored-by-agent`), and the rule tag survives.
    let a = findings
        .iter()
        .find(|f| f.claim == "dup in a")
        .expect("val-a finding");
    assert_eq!(a.validator, "val-a");
    assert_eq!(a.rule.as_deref(), Some("ra"));
    let b = findings
        .iter()
        .find(|f| f.claim == "dup in b")
        .expect("val-b finding");
    assert_eq!(b.validator, "val-b");
    assert_eq!(b.rule.as_deref(), Some("rb"));
    assert!(
        findings.iter().all(|f| f.validator != "ignored-by-agent"),
        "the agent's self-reported validator must be overridden"
    );
}

/// A file containing several instances of ONE rule, touched by a single
/// commit, must yield ALL of them on the FIRST review pass — the whole-file
/// sweep, not a dribble of one-instance-per-re-review. Driven end-to-end
/// through `run_fleet` with a scripted agent that reports every instance.
#[tokio::test]
async fn one_rule_with_many_instances_reports_them_all_on_the_first_pass() {
    let rs = ruleset(
        "magic-numbers",
        "no unexplained numeric literals",
        &[("no-magic", "name your constants")],
    );
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "magic-numbers",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // The agent reports several instances of the one rule across the whole
    // file in a single reply; its completeness re-scan then finds nothing
    // more. Each instance sits on its own line derived from TEST_FINDING_LINE
    // so the findings are distinct file:line instances, not a shared-line
    // collapse — the exact lines are immaterial.
    let instances = [
        ("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7"),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        ),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 2,
            "no-magic",
            "magic number 99",
        ),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 3,
            "no-magic",
            "magic number 256",
        ),
    ];
    let first_pass = findings_array_json(&instances);
    let agent = forking_agent(vec![
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}magic-numbers\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(first_pass),
        ),
    ]);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await.findings
    })
    .await;

    let magic: Vec<_> = findings
        .iter()
        .filter(|f| f.rule.as_deref() == Some("no-magic"))
        .collect();
    assert_eq!(
        magic.len(),
        instances.len(),
        "all instances of the one rule must report on the first pass, \
         not dribble one per round: {findings:#?}"
    );
    assert!(
        magic.iter().all(|f| f.validator == "magic-numbers"),
        "every instance is tagged with its validator: {findings:#?}"
    );
}

/// A magic-numbers single-validator `WorkList` over one file — the shared
/// setup for the follow-up-sweep tests, which all drive the loop on one
/// validator and assert on what it surfaced and how many sweeps it took.
fn magic_numbers_work() -> (ValidatorLoader, WorkList) {
    let rs = ruleset(
        "magic-numbers",
        "no unexplained numeric literals",
        &[("no-magic", "name your constants")],
    );
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "magic-numbers",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );
    (loader, work)
}

/// The first-pass script entry: keyed on the validator header so it answers
/// the first review turn (never a follow-up sweep, which carries the sweep
/// header instead) with `findings`.
fn first_pass_entry(findings: String) -> (String, ScriptedReply) {
    (
        "# Validator: magic-numbers".to_string() + "\n\n## Mandate",
        ScriptedReply::Text(findings),
    )
}

/// The sessions each follow-up sweep turn ran on, in order — the prompts
/// carrying the sweep header, mapped to their session. The loop drives the
/// session forward, so these must be a chain of DISTINCT sessions (one fresh
/// fork per sweep), never the same session re-forked.
fn sweep_sessions(probe: &ScriptedAgent) -> Vec<String> {
    probe
        .prompted_sessions()
        .into_iter()
        .zip(probe.seen_prompts())
        .filter(|(_, prompt)| prompt.contains(RESCAN_NEEDLE))
        .map(|(session, _)| session)
        .collect()
}

/// Lever 2 (a) — the follow-up sweep keeps going while turns return findings
/// and STOPS when a turn goes dry (`[]`). The first pass under-reports one
/// instance; sweep 1 recovers one more, sweep 2 one more, sweep 3 is empty
/// and ends the loop. All four findings merge on the first review, distinct.
#[tokio::test]
async fn followup_sweep_continues_while_findings_arrive_and_stops_when_dry() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    // ONE script entry keyed on the sweep header answers EVERY sweep, with a
    // different delta each turn — findings, findings, then dry. A constant
    // prompt is re-sent each sweep, so this sequence is the only way to script
    // the model converging across the loop.
    let sweep_deltas = ScriptedReply::sequence([
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        )]),
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 2,
            "no-magic",
            "magic number 99",
        )]),
        "[]".to_string(),
    ]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), sweep_deltas),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await.findings
    })
    .await;

    // First pass (1) + sweep 1 (1) + sweep 2 (1) = 3 findings; sweep 3 is dry.
    assert_eq!(
        findings.len(),
        3,
        "every instance recovered across the sweeps must merge: {findings:#?}"
    );
    let lines: std::collections::BTreeSet<u32> = findings.iter().map(|f| f.line).collect();
    assert_eq!(
        lines.len(),
        3,
        "the merged findings are distinct file:line instances, not re-reports: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .all(|f| f.validator == "magic-numbers" && f.rule.as_deref() == Some("no-magic")),
        "merged findings keep their validator and rule tags: {findings:#?}"
    );

    // Three sweep turns fired: two that returned findings plus the dry one
    // that stopped the loop — well under the runaway cap.
    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len(),
        3,
        "the loop runs sweeps until one goes dry, then stops: {sessions:#?}"
    );
}

/// Lever 2 (c) — the loop drives the SAME accumulating session forward, not a
/// re-fork of the first pass. Each sweep forks the session that delivered the
/// PRIOR sweep's answer, so the sweeps run on a chain of distinct sessions and
/// the model's own earlier answers are in context — the structural reason it
/// converges instead of oscillating.
#[tokio::test]
async fn followup_sweep_drives_the_session_forward_not_reforking_the_first_pass() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    let sweep_deltas = ScriptedReply::sequence([
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        )]),
        "[]".to_string(),
    ]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), sweep_deltas),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await;
    })
    .await;

    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len(),
        2,
        "two sweeps fired (one with findings, one dry): {sessions:#?}"
    );
    let distinct: std::collections::BTreeSet<&String> = sessions.iter().collect();
    assert_eq!(
        distinct.len(),
        sessions.len(),
        "each sweep runs on a fresh fork of the prior sweep's session — a forward chain, \
         never the same first-pass session re-forked: {sessions:#?}"
    );

    // The load-bearing proof: the SECOND sweep ran on a session forked from
    // the FIRST sweep's session, so its accumulated context already carries
    // the first sweep's nudge — the sweep header appears TWICE. Re-forking the
    // first pass each time would leave it appearing only once, the model would
    // never see its own prior answer, and the loop could not converge.
    let last_sweep_history = probe
        .session_history(sessions.last().unwrap())
        .expect("the last sweep's session ran");
    let header_occurrences = last_sweep_history.matches(RESCAN_NEEDLE).count();
    assert_eq!(
        header_occurrences, 2,
        "the second sweep continues the first sweep's session (forward chain), so its context \
         holds the nudge twice — not a re-fork of the first pass: {last_sweep_history}"
    );
}

/// Lever 2 (b) — the runaway cap. A model that never goes dry (every sweep
/// returns the same finding) is bounded: the loop stops after exactly
/// [`MAX_FOLLOWUP_SWEEPS`] sweeps rather than looping forever. The re-reported
/// duplicates are harmless — downstream `dedup_exact` collapses them.
#[tokio::test]
async fn followup_sweep_stops_at_the_cap_when_never_dry() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    // Every sweep returns a (non-empty) finding, so the model never says
    // "none left" — only the cap can terminate the loop.
    let never_dry = findings_array_json(&[(
        "src/a.rs",
        TEST_FINDING_LINE + 1,
        "no-magic",
        "magic number 13",
    )]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), ScriptedReply::Text(never_dry)),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await;
    })
    .await;

    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len() as u32,
        MAX_FOLLOWUP_SWEEPS,
        "a never-dry model is bounded at the runaway cap, not looped forever: {sessions:#?}"
    );
}

/// Lever 2 (d) — an empty first pass spends ZERO follow-up turns. A clean
/// validator has nothing to be incomplete about, so the loop is skipped
/// entirely: one validator fork, no sweeps.
#[tokio::test]
async fn empty_first_pass_spends_no_followup_sweeps() {
    let (loader, work) = magic_numbers_work();

    // The first pass finds nothing; the sweep header still has a (would-be)
    // entry so a stray sweep would be observable — it must not fire.
    let agent = forking_agent(vec![
        (
            RESCAN_NEEDLE.to_string(),
            ScriptedReply::Text("[]".to_string()),
        ),
        first_pass_entry("[]".to_string()),
    ]);
    let probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await.findings
    })
    .await;

    assert!(
        findings.is_empty(),
        "a clean validator reports nothing: {findings:#?}"
    );
    let sessions = sweep_sessions(&probe);
    assert!(
        sessions.is_empty(),
        "an empty first pass must not spend any follow-up sweep turn: {sessions:#?}"
    );
    assert_eq!(
        probe.fork_count(),
        1,
        "exactly one validator fork and no sweep fork on a clean validator"
    );
}

#[tokio::test]
async fn multi_rule_validator_forks_one_task_carrying_all_rules_against_one_prime() {
    // One validator with three rules over ten files. The files all live in
    // the single shared prime; the fan-out is per VALIDATOR, so this mints
    // exactly one prime + ONE validator fork carrying ALL THREE rules — never
    // per-rule, per-file, or per-batch.
    let rs = ruleset(
        "val",
        "mandate",
        &[
            ("r1", "RULE1_MARKER body 1"),
            ("r2", "RULE2_MARKER body 2"),
            ("r3", "RULE3_MARKER body 3"),
        ],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = (0..10)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await
    })
    .await;

    let seen = agent_probe.seen_prompts();
    let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
    assert_eq!(primes, 1, "one shared prime for the whole run: {seen:#?}");
    let validator_tasks = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .count();
    assert_eq!(
        validator_tasks, 1,
        "one validator → one forked validator task (not three rule tasks, not ten file tasks): {seen:#?}"
    );
    assert_eq!(outcome.attempted(), 1, "one validator task attempted");

    // The single prime carries ALL ten files' diffs; the validator fork
    // carries every rule of the validator (no file content re-sent).
    let prime = seen
        .iter()
        .find(|p| p.contains(PRIME_HANDOFF))
        .expect("the run prime");
    assert_eq!(
        prime.matches("## File: ").count(),
        10,
        "the shared prime inlines every file once: {prime}"
    );
    let validator_suffix = seen
        .iter()
        .find(|p| p.starts_with("# Validator:"))
        .expect("a validator fork");
    assert!(
        validator_suffix.contains("RULE1_MARKER")
            && validator_suffix.contains("RULE2_MARKER")
            && validator_suffix.contains("RULE3_MARKER"),
        "the validator fork must carry ALL of its rules: {validator_suffix}"
    );
    assert!(
        !validator_suffix.contains("## File: "),
        "a validator fork must NOT re-send file content (it is in the prime): {validator_suffix}"
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn fan_out_logs_the_rule_names_being_applied_per_validator() {
    // A validator with two distinctively-named rules; the fan-out log must
    // name the rules being applied (sourced from the loader's RuleSet) so the
    // logs show exactly which validator×rules ran.
    let rs = ruleset(
        "deduplicate",
        "mandate",
        &[("no-copy-paste", "body a"), ("prefer-reuse", "body b")],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = vec![file_work("src/a.rs", "alpha", "src/x.rs")];
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work("deduplicate", files)],
    );

    let agent = forking_agent(vec![]);
    let _findings = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await
    })
    .await;

    // The batching log carries the rule names from the loader's RuleSet as a
    // structured field (the exact bracketed list only this log emits — the
    // rendered prompt spells rules as `### Rule: ...` prose, not this shape).
    assert!(logs_contain("rules=[\"no-copy-paste\", \"prefer-reuse\"]"));
}

// ---- primed-prefix + fork orchestration tests -------------------------

#[tokio::test]
#[tracing_test::traced_test]
async fn prefix_is_primed_once_per_run_and_validators_fork_suffix_only() {
    // One validator, two rules, over four files. The new grain: the change +
    // every file diff is primed ONCE for the whole run, and each VALIDATOR
    // forks it sending only its validator suffix (its full ruleset). So: 1
    // prime + 1 validator fork carrying BOTH rules, never one fork per rule
    // and never one fork per file/batch.
    let rs = ruleset(
        "val",
        "MANDATE_MARKER mandate",
        &[("r1", "RULE1_MARKER body"), ("r2", "RULE2_MARKER body")],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = (0..4)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    // The validator's fork emits a finding. The fork inherits the shared
    // prime (all files) and appends the validator suffix (which carries the
    // mandate marker), so we key on that marker.
    let agent = forking_agent(vec![
        // The first pass is exhaustive; its completeness re-scan finds
        // nothing more, so this test asserts the unchanged one-fork-per-
        // validator prime shape (plus the bounded re-scan fork).
        rescan_finds_nothing(),
        (
            "MANDATE_MARKER".to_string(),
            ScriptedReply::Text(findings_json(
                "src/f0.rs",
                TEST_FINDING_LINE,
                "r1",
                "warm finding",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    // Drive the prime lifecycle the way `run_review` does: run the fleet,
    // then release the returned shared-prime guard once the run drains.
    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let outcome = run_fleet(&work, &loader, &pool, None).await;
        if let Some(guard) = outcome.prime {
            unpin_prefix_session(guard).await;
        }
        FleetOutcome {
            prime: None,
            ..outcome
        }
    })
    .await;

    let seen = agent_probe.seen_prompts();
    let primes: Vec<&String> = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).collect();
    assert_eq!(
        primes.len(),
        1,
        "the shared prefix is primed exactly once per RUN: {seen:#?}"
    );
    // The prime carries the change + every file diff, and NO validator text.
    assert!(
        primes[0].contains("# Files under review") && primes[0].contains("## File: src/f0.rs"),
        "the prime carries the diffs: {}",
        primes[0]
    );
    assert!(
        !primes[0].contains("MANDATE_MARKER")
            && !primes[0].contains("RULE1_MARKER")
            && !primes[0].contains("RULE2_MARKER"),
        "the prime must NOT carry any validator text: {}",
        primes[0]
    );

    // One forked task per validator, carrying ONLY its validator suffix (the
    // validator/mandate/full-ruleset/contract) and never re-sending file
    // content.
    let validator_tasks: Vec<&String> = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .collect();
    assert_eq!(
        validator_tasks.len(),
        1,
        "the validator forks the primed session and sends ONLY its validator suffix: {seen:#?}"
    );
    assert!(
        validator_tasks.iter().all(|p| !p.contains("## File: ")),
        "validator forks must not re-send the file diffs: {validator_tasks:#?}"
    );
    // The single validator fork carries BOTH of the validator's rules.
    assert!(validator_tasks[0].contains("RULE1_MARKER"));
    assert!(validator_tasks[0].contains("RULE2_MARKER"));
    // One validator fork plus its one bounded completeness re-scan fork.
    assert_eq!(
        agent_probe.fork_count(),
        2,
        "one validator fork plus one completeness re-scan fork"
    );

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1, "{:#?}", outcome.findings);
    assert_eq!(outcome.findings[0].claim, "warm finding");
    assert_eq!(outcome.findings[0].validator, "val");

    // The shared prime was pinned for the run and unpinned when it drained.
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
        "pin the shared prime for the run, unpin when it drains"
    );

    // Observability: each fork task logs the warm reuse and token count,
    // classified as a warm KV fork (the native llama/qwen path).
    assert!(logs_contain("fleet task prefix reuse"));
    assert!(logs_contain("reuse=\"warm KV fork\""));
    assert!(logs_contain(&format!(
        "reused_tokens=Some({MOCK_PREFIX_TOKENS})"
    )));
    assert!(logs_contain("primed shared run prefix session"));
}

/// The shared run prime is born pinned through the PRODUCTION prime path:
/// `prime_run_prefix` → `submit_primed` → the prompt's `_meta` pin-on-save
/// intent → the agent saving its prefix pinned atomically at turn completion
/// — BEFORE any separate `session/pin` confirm runs. This is the end-to-end
/// (scripted agent, no real model) assertion for the structural close of the
/// prime→pin eviction race: the prefix is never an unpinned eviction
/// candidate, independent of any post-turn pin.
#[tokio::test]
async fn primed_prefix_is_born_pinned_through_the_production_path() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let files: Vec<FileWork> = (0..2)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    // The shared prime session (`sess-0`) was born pinned by the prime turn's
    // `_meta` intent — recorded at turn completion, before the post-turn
    // `session/pin` confirm. Forked validator sessions are NOT born pinned
    // (they save their own cold state unpinned).
    assert_eq!(
        agent_probe.born_pinned_sessions(),
        vec!["sess-0".to_string()],
        "the run prime must be born pinned through the production prime path, \
         and only the prime (not the forked validator sessions)"
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn fork_failure_falls_back_to_monolithic_without_losing_tasks() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![
                file_work("src/a.rs", "alpha", "src/x.rs"),
                file_work("src/b.rs", "beta", "src/y.rs"),
            ],
        )],
    );

    // Every `session/fork` is rejected; the validator task must fall back to
    // a fresh-session monolithic prompt and still deliver its findings.
    let agent = agent_with_fork_mode(
        vec![(
            "## File: src/a.rs".to_string(),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "r",
                "found despite fork failure",
            )),
        )],
        ForkMode::RejectFork,
    );
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1, "one validator task");
    assert_eq!(outcome.failed(), 0, "a failed fork is never a lost task");
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "found despite fork failure");

    // The fallback prompt is the full monolithic shape (rules + files).
    let seen = agent_probe.seen_prompts();
    let monolithic = seen
        .iter()
        .filter(|p| p.contains(MANDATE_HEADER) && p.contains("# Files under review"))
        .count();
    assert_eq!(
        monolithic, 1,
        "the validator fell back to a monolithic prompt: {seen:#?}"
    );
    assert!(logs_contain("falling back to a monolithic"));

    // The prime succeeded, so it was pinned and is unpinned when the run drains.
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn unsupported_fork_extension_degrades_to_monolithic_prompts() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![
                file_work("src/a.rs", "alpha", "src/x.rs"),
                file_work("src/b.rs", "beta", "src/y.rs"),
            ],
        )],
    );

    // The backend implements NO extension methods: the prime turn runs but
    // its state can never be confirmed, so the whole run degrades to
    // monolithic per-validator prompts — never a lost task.
    let agent = agent_with_fork_mode(
        vec![(
            "## File: src/b.rs".to_string(),
            ScriptedReply::Text(findings_json(
                "src/b.rs",
                TEST_FINDING_LINE,
                "r",
                "found without forks",
            )),
        )],
        ForkMode::Unsupported,
    );
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1, "one validator task");
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "found without forks");

    let seen = agent_probe.seen_prompts();
    let monolithic = seen
        .iter()
        .filter(|p| p.contains("## Mandate") && p.contains("# Files under review"))
        .count();
    assert_eq!(monolithic, 1, "{seen:#?}");
    assert_eq!(
        agent_probe.fork_count(),
        0,
        "no forks on an unsupported backend"
    );
    assert!(
        agent_probe.pin_calls().is_empty(),
        "nothing is pinned when state confirmation fails"
    );
    assert!(logs_contain("falling back to monolithic prompts"));
}

#[tokio::test]
#[tracing_test::traced_test]
async fn degraded_fork_runs_cold_but_still_parses_findings() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // Forks succeed but attach no parent state — the task proceeds on the
    // forked session (history is intact, just cold) and is logged.
    let agent = agent_with_fork_mode(
        vec![
            rescan_finds_nothing(),
            (
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "cold but correct",
                )),
            ),
        ],
        ForkMode::DegradedAttach,
    );

    let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "cold but correct");
    assert!(logs_contain("fleet task fork was degraded"));
}

/// The claude backend shape: a fork that attaches no native KV state
/// (`fork.prefix_tokens == None`) but whose turn reports Anthropic
/// prompt-cache reads. The forked task must resolve through the real
/// `collect_forked_task` path without error AND log the warm-cache reuse
/// (`classify_reuse` → `WarmCache`), so warm/cold is observable on claude
/// even though the native KV reuse log is blind.
#[tokio::test]
#[tracing_test::traced_test]
async fn forked_task_with_claude_cache_usage_logs_warm_cache() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // Forks succeed but attach no native parent state (claude shape:
    // `prefix_tokens == None`); the turn's `_meta` reports a warm cache read,
    // which is what makes the reuse observable on claude.
    let agent = ScriptedAgent::with_config(
        vec![
            rescan_finds_nothing(),
            (
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "warm on claude",
                )),
            ),
        ],
        ScriptedAgentConfig {
            fork_mode: ForkMode::DegradedAttach,
            cache_usage: Some(CacheUsage {
                cache_read_input_tokens: Some(2048),
                cache_creation_input_tokens: Some(16),
                input_tokens: Some(2064),
                output_tokens: Some(40),
            }),
            ..ScriptedAgentConfig::default()
        },
    );

    let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(
        outcome.failed(),
        0,
        "the forked task resolved through collect_forked_task without error"
    );
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "warm on claude");
    assert!(
        logs_contain("warm prompt cache"),
        "the warm-cache reuse must be logged so claude reuse is observable"
    );
}

#[tokio::test]
async fn prefix_session_is_unpinned_even_when_a_validator_task_errors() {
    // Two validators; the second's fork errors. The shared-prime pin must
    // still be released once the run drains, regardless of a failed validator
    // task.
    let rs_ok = ruleset("val-ok", "mandate ok", &[("ok-rule", "OK_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_ok, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-ok", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
        ],
    );

    // The `val-bad` fork carries the `bad-rule` body and errors; the `val-ok`
    // one is empty. One forked validator task errors → the unpin must still
    // happen.
    let agent = forking_agent(vec![("BAD_BODY".to_string(), ScriptedReply::Error)]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 2, "two validator tasks");
    assert_eq!(
        outcome.failed(),
        1,
        "the erroring validator task is a failed task"
    );
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
        "the prefix pin is released even when a validator task errors"
    );
}

// ---- progress events ---------------------------------------------------

/// Drain every buffered progress event off the receiver, synchronously.
fn drain_progress(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<ReviewProgressEvent>,
) -> Vec<ReviewProgressEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// The fan-out emits one `Planned` carrying the (validator, file) pair total,
/// then `PairStarted`/`PairDone` for EVERY pair — including a validator task
/// that errors — so a consumer counting `PairDone` events always reaches the
/// planned total.
#[tokio::test]
async fn fleet_emits_progress_events_per_validator_file_pair_including_failed_tasks() {
    let rs_ok = ruleset("val-ok", "mandate ok", &[("ok-rule", "OK_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_ok, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work(
                "val-ok",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
            validator_work("val-bad", vec![file_work("src/c.rs", "gamma", "src/z.rs")]),
        ],
    );

    // The `val-bad` fork errors; `val-ok` resolves with the default (empty)
    // findings reply. Both must still emit a PairDone for every file.
    let agent = forking_agent(vec![("BAD_BODY".to_string(), ScriptedReply::Error)]);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let tally = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let mut outcome = run_fleet(&work, &loader, &pool, Some(&progress_tx)).await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
        (outcome.attempted(), outcome.failed())
    })
    .await;
    assert_eq!(
        tally,
        (2, 1),
        "two tasks attempted, the erroring one failed"
    );

    let events = drain_progress(&mut progress_rx);
    assert_eq!(
        events.first(),
        Some(&ReviewProgressEvent::Planned { total_pairs: 3 }),
        "the first event is the plan with the pair total: {events:#?}"
    );

    let pairs = [
        ("val-ok", "src/a.rs"),
        ("val-ok", "src/b.rs"),
        ("val-bad", "src/c.rs"),
    ];
    for (validator, file) in pairs {
        assert!(
            events.iter().any(|e| matches!(
                e,
                ReviewProgressEvent::PairStarted { validator: v, file: f }
                    if v == validator && f == file
            )),
            "missing PairStarted for ({validator}, {file}): {events:#?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                ReviewProgressEvent::PairDone { validator: v, file: f }
                    if v == validator && f == file
            )),
            "missing PairDone for ({validator}, {file}) — a failed task must still \
             emit PairDone so progress reaches the total: {events:#?}"
        );
    }

    let done = events
        .iter()
        .filter(|e| matches!(e, ReviewProgressEvent::PairDone { .. }))
        .count();
    assert_eq!(
        done, 3,
        "exactly one PairDone per planned pair, so progress closes at the \
         planned total: {events:#?}"
    );
}

/// Each COMPLETED validator task emits one `Findings` event carrying that
/// task's parsed, validator-tagged findings — a validator that came back clean
/// emits an EMPTY vec (clean, not silence), while a FAILED task emits no
/// `Findings` event at all (its `PairDone` accounting still fires).
#[tokio::test]
async fn fleet_emits_findings_events_per_completed_validator_task() {
    let rs_hit = ruleset("val-hit", "mandate hit", &[("hit-rule", "HIT_BODY")]);
    let rs_clean = ruleset(
        "val-clean",
        "mandate clean",
        &[("clean-rule", "CLEAN_BODY")],
    );
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_hit, rs_clean, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-hit", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-clean", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
            validator_work("val-bad", vec![file_work("src/c.rs", "gamma", "src/z.rs")]),
        ],
    );

    // val-hit returns one finding; val-clean returns an empty array (clean);
    // val-bad's fork errors — a failed task that must emit NO Findings event.
    let agent = forking_agent(vec![
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}val-hit\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "hit-rule",
                "real issue in a",
            )),
        ),
        (
            format!("{VALIDATOR_HEADER}val-clean\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text("[]".to_string()),
        ),
        ("BAD_BODY".to_string(), ScriptedReply::Error),
    ]);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    with_pool(agent, PoolConfig::remote(3), move |pool| async move {
        let mut outcome = run_fleet(&work, &loader, &pool, Some(&progress_tx)).await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
    })
    .await;

    let events = drain_progress(&mut progress_rx);
    let findings_events: Vec<(&str, &Vec<Finding>)> = events
        .iter()
        .filter_map(|e| match e {
            ReviewProgressEvent::Findings {
                validator,
                findings,
            } => Some((validator.as_str(), findings)),
            _ => None,
        })
        .collect();

    // val-hit emitted its one validator-tagged finding as its task resolved.
    let hit = findings_events
        .iter()
        .find(|(v, _)| *v == "val-hit")
        .expect("a Findings event for val-hit");
    assert_eq!(hit.1.len(), 1, "val-hit streamed its single finding");
    assert_eq!(
        hit.1[0].validator, "val-hit",
        "the streamed finding is validator-tagged, not the agent's self-report"
    );
    assert_eq!(hit.1[0].claim, "real issue in a");

    // val-clean emitted an EMPTY Findings event — clean, not silent.
    let clean = findings_events
        .iter()
        .find(|(v, _)| *v == "val-clean")
        .expect("a Findings event for the clean validator");
    assert!(
        clean.1.is_empty(),
        "a clean validator emits an empty Findings vec: {clean:#?}"
    );

    // val-bad FAILED → no Findings event (PairDone still fires for it).
    assert!(
        !findings_events.iter().any(|(v, _)| *v == "val-bad"),
        "a failed validator task must emit no Findings event: {findings_events:#?}"
    );
}

/// Poll `condition` every [`POLL_INTERVAL`] until it holds, panicking after
/// [`POLL_TIMEOUT`]. The retry count is derived from the two so the wait
/// budget is expressed once, not as a product of two coupled literals.
async fn wait_for(what: &str, condition: impl Fn() -> bool) {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
    const POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
    let attempts = POLL_TIMEOUT.as_millis() / POLL_INTERVAL.as_millis();
    for _ in 0..attempts {
        if condition() {
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    panic!("timed out waiting for {what}");
}

/// Cancellation-safety regression: a run future dropped mid-collect
/// (review cancelled, caller timeout) must STILL release the prefix pin —
/// a pinned session is exempt from cache eviction, so a leaked pin
/// outlives the review until process restart.
#[tokio::test]
async fn prefix_pin_is_released_when_the_fanout_future_is_dropped_mid_collect() {
    let rs = ruleset("val", "mandate", &[("r", "WEDGE_BODY")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // The validator fork turn wedges forever (its suffix carries the rule
    // body), holding the fan-out mid-collect AFTER the prime has been pinned.
    let agent = forking_agent(vec![("WEDGE_BODY".to_string(), ScriptedReply::Stall)]);
    let agent_probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let fanout = tokio::spawn(async move { run_fleet(&work, &loader, &pool, None).await });

        // Wait until the prefix is pinned and the wedged validator fork is in
        // flight — the run is now mid-collect.
        wait_for("the prefix pin and the wedged validator fork", || {
            agent_probe
                .pin_calls()
                .contains(&("sess-0".to_string(), true))
                && agent_probe
                    .seen_prompts()
                    .iter()
                    .any(|p| p.starts_with("# Validator:"))
        })
        .await;

        // Cancel the review: drop the fan-out future mid-collect.
        fanout.abort();
        let _ = fanout.await;

        // The pin must still be released — the cancelled fan-out cannot
        // leak the pinned prefix session.
        wait_for("the cancelled fan-out to release the prefix pin", || {
            agent_probe
                .pin_calls()
                .contains(&("sess-0".to_string(), false))
        })
        .await;
    })
    .await;
}

#[tokio::test]
async fn one_failing_task_yields_zero_findings_without_aborting_the_rest() {
    // Two validators: the `val-bad` fork errors, the `val-good` fork finds an
    // issue. One bad validator task never aborts the rest.
    let rs_good = ruleset("val-good", "mandate good", &[("good-rule", "GOOD_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_good, rs_bad]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-good", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
        ],
    );

    // The fork carrying `BAD_BODY` errors; the `GOOD_BODY` one returns a
    // finding. Both keys appear only in their own validator's suffix.
    let agent = forking_agent(vec![
        // The good validator's first pass is exhaustive; its completeness
        // re-scan finds nothing more, so the surviving count is unchanged.
        rescan_finds_nothing(),
        ("BAD_BODY".to_string(), ScriptedReply::Error),
        (
            "GOOD_BODY".to_string(),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "good-rule",
                "real issue",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let mut outcome = run_fleet(&work, &loader, &pool, Some(&progress_tx)).await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
        outcome
    })
    .await;

    // The erroring task contributed nothing; the good one still returned.
    assert_eq!(
        outcome.findings.len(),
        1,
        "the failing task degrades to zero findings"
    );
    assert_eq!(outcome.findings[0].claim, "real issue");
    assert_eq!(outcome.findings[0].validator, "val-good");
    // The tally records both tasks attempted and exactly the one that failed.
    assert_eq!(outcome.attempted(), 2, "two validator tasks attempted");
    assert_eq!(
        outcome.failed(),
        1,
        "the erroring task is counted as failed"
    );

    // No-retry regression guard: the failing validator's prompt reaches the
    // agent exactly once — no fallback resubmission, no re-queue of the same
    // (validator, file) unit after it errors. This is the guard against ever
    // reintroducing the per-unit retry the fleet decided against.
    let bad_submissions = agent_probe
        .seen_prompts()
        .iter()
        .filter(|p| p.contains("BAD_BODY"))
        .count();
    assert_eq!(
        bad_submissions,
        1,
        "a failing fan-out task must be submitted to the agent exactly once, \
         never retried: {:?}",
        agent_probe.seen_prompts()
    );

    // Single-attempt semantics show up in the progress stream too: exactly one
    // `PairStarted`/`PairDone` per (validator, file) — including the failing
    // one — and the failure never triggers a re-plan (a second `Planned` event
    // growing the total, which would signal a re-queue).
    let events = drain_progress(&mut progress_rx);
    let planned: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ReviewProgressEvent::Planned { total_pairs } => Some(*total_pairs),
            _ => None,
        })
        .collect();
    assert_eq!(
        planned,
        vec![2],
        "the failure must not grow/replan the announced total: {events:#?}"
    );

    for (validator, file) in [("val-good", "src/a.rs"), ("val-bad", "src/b.rs")] {
        let started = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ReviewProgressEvent::PairStarted { validator: v, file: f }
                        if v == validator && f == file
                )
            })
            .count();
        assert_eq!(
            started, 1,
            "exactly one PairStarted for ({validator}, {file}), no retry: {events:#?}"
        );
        let done = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ReviewProgressEvent::PairDone { validator: v, file: f }
                        if v == validator && f == file
                )
            })
            .count();
        assert_eq!(
            done, 1,
            "exactly one PairDone for ({validator}, {file}), no retry: {events:#?}"
        );
    }
}

#[tokio::test]
async fn all_tasks_failing_yields_zero_findings_and_a_full_failure_tally() {
    // Three validators; every validator fork errors.
    let loader = loader_with(vec![
        ruleset("val-a", "mandate a", &[("r1", "body 1")]),
        ruleset("val-b", "mandate b", &[("r2", "body 2")]),
        ruleset("val-c", "mandate c", &[("r3", "body 3")]),
    ]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-a", vec![file_work("src/a.rs", "a", "src/x.rs")]),
            validator_work("val-b", vec![file_work("src/b.rs", "b", "src/y.rs")]),
            validator_work("val-c", vec![file_work("src/c.rs", "c", "src/z.rs")]),
        ],
    );

    // Every validator fork errors (every validator suffix carries the
    // validator header).
    let agent = forking_agent(vec![("# Validator:".to_string(), ScriptedReply::Error)]);

    let outcome = with_pool(agent, PoolConfig::remote(3), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert!(
        outcome.findings.is_empty(),
        "every task failed, so there are no findings"
    );
    assert_eq!(outcome.attempted(), 3, "three validator tasks attempted");
    assert_eq!(outcome.failed(), 3, "all three failed");
}

#[tokio::test]
async fn validator_missing_from_loader_is_skipped_not_panicked() {
    // The work-list names a validator the loader does not know.
    let loader = loader_with(vec![ruleset("known", "mandate", &[("r", "body")])]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "unknown",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet(&work, &loader, &pool, None).await
    })
    .await;

    assert!(
        outcome.findings.is_empty(),
        "an unknown validator yields no findings"
    );
    assert_eq!(
        outcome.attempted(),
        0,
        "no task is attempted for a validator missing from the loader"
    );
    assert_eq!(outcome.failed(), 0);
    assert!(
        agent_probe.seen_prompts().is_empty(),
        "no task is submitted for a validator missing from the loader"
    );
}

// ---- classify_reuse --------------------------------------------------

/// A native KV fork that attached its parent's saved state with a token
/// count classifies as `WarmKv` carrying that count — the llama/qwen path.
#[test]
fn test_classify_reuse_kv_fork_is_warm_kv() {
    let fork = Some(ForkAttachment {
        state_attached: true,
        prefix_tokens: Some(MOCK_PREFIX_TOKENS),
    });
    assert_eq!(
        classify_reuse(fork, None),
        PrefixReuse::WarmKv {
            reused_tokens: MOCK_PREFIX_TOKENS
        }
    );
}

/// A claude turn with `cache_read_input_tokens > 0` classifies as
/// `WarmCache` carrying the read/created split — even though the fork
/// attached no native KV token count (the production blind spot this task
/// closes).
#[test]
fn test_classify_reuse_claude_cache_read_is_warm_cache() {
    let usage = Some(CacheUsage {
        cache_read_input_tokens: Some(900),
        cache_creation_input_tokens: Some(100),
        input_tokens: Some(1000),
        output_tokens: Some(20),
    });
    assert_eq!(
        classify_reuse(None, usage),
        PrefixReuse::WarmCache {
            read: 900,
            created: 100
        }
    );
}

/// A claude turn that only wrote the cache (`cache_creation_input_tokens >
/// 0`, no reads) is a cold prefill — `Cold` (no warm reuse to report).
#[test]
fn test_classify_reuse_claude_cold_write_is_cold() {
    let usage = Some(CacheUsage {
        cache_read_input_tokens: Some(0),
        cache_creation_input_tokens: Some(1000),
        input_tokens: Some(1000),
        output_tokens: Some(20),
    });
    assert_eq!(classify_reuse(None, usage), PrefixReuse::Cold);
}

/// No fork and no usage is unknown/cold.
#[test]
fn test_classify_reuse_empty_is_cold() {
    assert_eq!(classify_reuse(None, None), PrefixReuse::Cold);
}
