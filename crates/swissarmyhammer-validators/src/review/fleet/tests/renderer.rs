//! What the renderers put in a prompt.
//!
//! Pure rendering: the run prime, the validator suffix, the monolithic
//! fallback and the output contract, with no agent in the loop.

use super::*;

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

/// ^t7f5fqf: the batch-scoped `<changed-set>` `duplicates` comparison must
/// render ONCE in the run prime, never once per file. A production batch of
/// 10 files each carrying a ~1.43 MB shared changed-set result sent ~14.3 MB
/// for zero additional information; this pins the fix at the assembled-prompt
/// level, packing several files that all need the `duplicates` probe.
#[test]
fn run_prime_renders_the_shared_changed_set_evidence_once_not_once_per_file() {
    let shared = vec![ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![ProbeRow {
            file_path: "src/b.rs".to_string(),
            symbol: Some("beta".to_string()),
            line: None,
            similarity: Some(TEST_SIMILARITY),
            detail: Some("SHARED_CHANGED_SET_MARKER".to_string()),
        }],
    }];

    let files = vec![
        file_work("src/a.rs", "alpha", "src/b.rs"),
        file_work("src/b.rs", "beta", "src/a.rs"),
        file_work("src/c.rs", "gamma", "src/a.rs"),
    ];
    let validator = validator_work("dedup", files).with_shared_probe_results(shared);
    let work = WorkList::new("purpose".to_string(), vec![validator]);

    let prime = render_run_prime(&work);

    assert_eq!(
        prime.matches("SHARED_CHANGED_SET_MARKER").count(),
        1,
        "the shared changed-set evidence must render exactly once in the prime \
         packing 3 files, not once per file: {prime}"
    );
    // The per-file evidence each file_work fixture carries is untouched by
    // this change — it still renders once per file, as before.
    assert_eq!(
        prime.matches("probe `duplicates`").count(),
        4,
        "3 per-file `duplicates` results plus the 1 shared `<changed-set>` \
         result, never duplicated further: {prime}"
    );
}

/// The monolithic fallback ([`render_fleet_prompt`]) renders the same shared
/// evidence exactly once too — the degraded path must not silently drop the
/// `<changed-set>` evidence, nor multiply it across the validator's files.
#[test]
fn monolithic_fallback_renders_the_shared_changed_set_evidence_once_not_once_per_file() {
    let shared = vec![ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![ProbeRow {
            file_path: "src/b.rs".to_string(),
            symbol: Some("beta".to_string()),
            line: None,
            similarity: Some(TEST_SIMILARITY),
            detail: Some("SHARED_CHANGED_SET_MARKER".to_string()),
        }],
    }];
    let files = vec![
        file_work("src/a.rs", "alpha", "src/b.rs"),
        file_work("src/b.rs", "beta", "src/a.rs"),
    ];
    let vw = validator_work("dedup", files).with_shared_probe_results(shared);
    let rs = ruleset(
        "dedup",
        "DEDUP_MANDATE.",
        &[("no-copy-paste", "RULE_BODY.")],
    );

    let prompt = render_fleet_prompt("purpose", &vw, &rs);

    assert_eq!(
        prompt.matches("SHARED_CHANGED_SET_MARKER").count(),
        1,
        "the monolithic fallback must render the shared evidence exactly \
         once, never dropped and never once per file: {prompt}"
    );
}

/// [`render_run_prime`] and [`render_fleet_prompt`] deliberately read shared
/// probe evidence from two DIFFERENT scopes, mirroring how they already treat
/// file content:
///
/// - the prime is the ONE shared context every validator's fork inherits, so
///   — exactly like [`WorkList::distinct_files`] unions every validator's
///   files — it renders [`WorkList::shared_probe_results`], the union of
///   shared evidence any validator in the batch declared;
/// - the monolithic fallback is a single validator's SELF-CONTAINED prompt
///   (its own files ONLY, via [`ValidatorWork::files`], never another
///   validator's), so it renders only [`ValidatorWork::shared_probe_results`]
///   — that validator's OWN declared shared evidence.
///
/// Unifying the two onto the work-list-wide union would leak a
/// `duplicates`-declaring validator's shared evidence into an unrelated
/// validator's self-contained monolithic prompt: new information that
/// validator never had before, which could change what it flags. This test
/// pins the split as intentional, not an oversight.
#[test]
fn monolithic_fallback_never_leaks_another_validators_shared_probe_evidence() {
    let dup_shared = vec![ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: vec![ProbeRow {
            file_path: "src/b.rs".to_string(),
            symbol: Some("beta".to_string()),
            line: None,
            similarity: Some(TEST_SIMILARITY),
            detail: Some("SHARED_CHANGED_SET_MARKER".to_string()),
        }],
    }];

    let dup_validator = validator_work("dedup", vec![file_work("src/a.rs", "alpha", "src/b.rs")])
        .with_shared_probe_results(dup_shared);
    let other_validator = ValidatorWork::new(
        "style".to_string(),
        RuleNames::new(["style-rule".to_string()]),
        ProbeNames::new(["similar".to_string()]),
        vec![file_work("src/c.rs", "gamma", "src/a.rs")],
    );
    let work = WorkList::new(
        "purpose".to_string(),
        vec![dup_validator, other_validator.clone()],
    );

    // The PRIME is shared context every fork inherits: it shows the union of
    // shared evidence any validator in the batch declared, exactly once.
    let prime = render_run_prime(&work);
    assert_eq!(
        prime.matches("SHARED_CHANGED_SET_MARKER").count(),
        1,
        "the prime carries the union of the batch's shared evidence, once: {prime}"
    );

    // The MONOLITHIC fallback for "style" (which never declared `duplicates`)
    // must NOT show `dedup`'s shared evidence — its self-contained prompt
    // otherwise carries only its OWN files, never another validator's.
    let rs = ruleset("style", "STYLE_MANDATE.", &[("style-rule", "STYLE_BODY.")]);
    let monolithic = render_fleet_prompt(work.change_purpose(), &other_validator, &rs);
    assert!(
        !monolithic.contains("SHARED_CHANGED_SET_MARKER"),
        "a validator's monolithic fallback must show only its OWN declared \
         shared evidence, never another validator's: {monolithic}"
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
