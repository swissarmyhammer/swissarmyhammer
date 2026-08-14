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
    let prompt = render_fleet_prompt(
        "PURPOSE: scaffolding the parser.",
        &vw,
        &rs,
        ReviewSubject::Files,
    );

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

    let prompt = render_fleet_prompt("purpose", &vw, &rs, ReviewSubject::Files);

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
    let suffix = render_validator_suffix(&vw, &rs, ReviewSubject::Files);
    assert_eq!(
        suffix,
        render_validator_suffix(&vw, &rs, ReviewSubject::Files)
    );

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
    let monolithic = render_fleet_prompt(work.change_purpose(), &vw, &rs, ReviewSubject::Files);
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

    let suffix = render_validator_suffix(&vw, &rs, ReviewSubject::Files);

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

    let suffix = render_validator_suffix(&vw, &rs, ReviewSubject::Files);
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

    let prompt = render_fleet_prompt("purpose", &vw, &rs, ReviewSubject::Files);
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

    let prompt = render_fleet_prompt("purpose", &vw, &rs, ReviewSubject::Files);

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
    let monolithic = render_fleet_prompt(
        work.change_purpose(),
        &other_validator,
        &rs,
        ReviewSubject::Files,
    );
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

    let payload = render_file_payload(std::slice::from_ref(&file), ReviewSubject::Files);

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
    // The contract a `review file` op renders: the whole of each named file
    // is the subject.
    let contract = crate::review::fleet::output_contract(ReviewSubject::Files);
    assert!(
        contract.contains("other files"),
        "the contract must scope reads to other (cross-file) files: {contract}"
    );
    // The changed files are provided in full — the contract says so.
    assert!(
        contract.to_lowercase().contains("already provided")
            || contract.to_lowercase().contains("provided in full"),
        "the contract must state the changed files are provided in full: {contract}"
    );
}

/// The contract must demand reporting EVERY occurrence of every rule that
/// fires in a single pass — one finding per `file:line`, never stopping at the
/// first match. Bail-fast (find-one → fix → re-review) is the re-review token
/// storm this contract exists to prevent.
#[test]
fn output_contract_demands_every_occurrence_with_no_bail_fast() {
    // The contract a `review file` op renders: the whole of each named file
    // is the subject.
    let contract = crate::review::fleet::output_contract(ReviewSubject::Files);
    let lower = contract.to_lowercase();
    assert!(
        lower.contains("every occurrence of every rule"),
        "the contract must demand every occurrence of every rule: {contract}"
    );
    assert!(
        lower.contains("do not stop at the first"),
        "the contract must forbid stopping at the first match: {contract}"
    );
    assert!(
        contract.contains("one finding per `file:line`"),
        "the contract must require one finding per file:line: {contract}"
    );
}

/// The contract must name the WHOLE current file as the review boundary and
/// demote the semantic diff to orientation only — so a small model does not
/// anchor on the changed region and under-report pre-existing instances
/// elsewhere in the file (the finding-dribble this card kills).
#[test]
fn output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff() {
    // The contract a `review file` op renders: the whole of each named file
    // is the subject.
    let contract = crate::review::fleet::output_contract(ReviewSubject::Files);
    let lower = contract.to_lowercase();
    assert!(
        contract.contains("## Review scope"),
        "the contract must carry an explicit review-scope section: {contract}"
    );
    assert!(
        lower.contains("whole current file"),
        "the contract must name the whole current file as the review boundary: \
         {contract}"
    );
    assert!(
        lower.contains("pre-existing instances"),
        "the contract must put pre-existing instances in scope: {contract}"
    );
    assert!(
        lower.contains("orientation only"),
        "the contract must frame the semantic diff as orientation only: {contract}"
    );
    assert!(
        lower.contains("not the review boundary"),
        "the contract must state the diff is NOT the review boundary: {contract}"
    );
}

// ---------------------------------------------------------------------------
// The same renderers under [`ReviewSubject::Diffs`] — what a `review working`
// or `review sha` op puts in a prompt. Every renderer above takes the subject,
// so each needs both variants: a renderer that answered `Files` correctly and
// `Diffs` wrongly would pass every test above.
// ---------------------------------------------------------------------------

/// The 8-character blame label every line of the diff fixtures carries. No
/// assertion reads the value, but the renderer prints the column at a fixed
/// width, so it has to be exactly 8 characters wide.
const TEST_BLAME_SHA: &str = "beefcafe";

/// How many lines the diff-view fixture's file holds — comfortably more than
/// the context band a changed region carries, so the diff render must elide
/// both above and below the change.
const DIFF_FIXTURE_LINES: usize = 200;

/// The 1-based line the diff-view fixture's change touched. It sits deep
/// enough inside [`DIFF_FIXTURE_LINES`] that untouched content stands on both
/// sides of it.
const DIFF_FIXTURE_CHANGED_LINE: usize = 120;

/// The source text of the diff fixture's line `line` — distinct per line, so
/// an assertion naming one line can never match another.
fn fixture_line_text(line: usize) -> String {
    format!("fn line_{line}() {{}}")
}

/// The mark column of a line the change added or modified.
const CHANGED_MARK: char = '+';

/// The mark column of an unchanged context line.
const CONTEXT_MARK: char = ' ';

/// The `{line:>6} | {sha:8} {mark} | {text}` row line `line` renders to.
///
/// One function for both marks — [`CHANGED_MARK`] and [`CONTEXT_MARK`] — so the
/// two rows cannot drift into disagreeing about the column layout they share.
fn format_row(line: usize, mark: char) -> String {
    format!(
        "{line:>6} | {TEST_BLAME_SHA} {mark} | {}",
        fixture_line_text(line)
    )
}

/// A [`FileWork`] spanning [`DIFF_FIXTURE_LINES`] lines, with the 1-based
/// lines in `touched` marked as the ones this change added or modified — the
/// shape the scope stage hands the renderer for a `review working` or
/// `review sha` op.
fn file_work_with_changed_lines(path: &str, touched: &[usize]) -> FileWork {
    let source: String = (1..=DIFF_FIXTURE_LINES)
        .map(|line| format!("{}\n", fixture_line_text(line)))
        .collect();
    file_work_with_slice(path, "alpha", "src/x.rs", source).with_line_annotations(
        (1..=DIFF_FIXTURE_LINES).map(|line| {
            crate::review::scope::LineAnnotation::new(TEST_BLAME_SHA, touched.contains(&line))
        }),
    )
}

/// Under [`ReviewSubject::Diffs`] a file block prints the lines the change
/// touched plus their context band and elides the rest, so a one-line edit to
/// a long file costs a handful of lines rather than the file.
#[test]
fn diff_payload_prints_the_changed_region_and_elides_the_rest() {
    let file = file_work_with_changed_lines("src/a.rs", &[DIFF_FIXTURE_CHANGED_LINE]);

    let payload = render_file_payload(std::slice::from_ref(&file), ReviewSubject::Diffs);

    assert!(
        payload.contains(&format_row(DIFF_FIXTURE_CHANGED_LINE, CHANGED_MARK)),
        "the changed line must print with its TRUE number and a `+` mark: {payload}"
    );
    assert!(
        payload.contains(&format_row(DIFF_FIXTURE_CHANGED_LINE - 1, CONTEXT_MARK)),
        "the line above the change must print as unmarked context: {payload}"
    );
    assert!(
        !payload.contains(&fixture_line_text(1)),
        "a line far above the change must not print: {payload}"
    );
    assert!(
        !payload.contains(&fixture_line_text(DIFF_FIXTURE_LINES)),
        "a line far below the change must not print: {payload}"
    );
    assert!(
        payload.contains("unchanged line(s) not shown"),
        "the elided stretches must be named, never silently dropped: {payload}"
    );

    // The block names the marked lines as the boundary, never the whole file.
    let lower = payload.to_lowercase();
    assert!(
        lower.contains("added or modified"),
        "the block must name the added/modified lines as the subject: {payload}"
    );
    assert!(
        !lower.contains("complete current source"),
        "a diff block must never claim to be the complete current source: {payload}"
    );
}

/// The other half of the same fixture: [`ReviewSubject::Files`] prints every
/// line the diff view elides, so neither subject can quietly become the other.
#[test]
fn whole_file_payload_prints_every_line_the_diff_payload_elides() {
    let file = file_work_with_changed_lines("src/a.rs", &[DIFF_FIXTURE_CHANGED_LINE]);

    let payload = render_file_payload(std::slice::from_ref(&file), ReviewSubject::Files);

    assert!(
        payload.contains(&fixture_line_text(1)),
        "the whole-file view must print the first line: {payload}"
    );
    assert!(
        payload.contains(&fixture_line_text(DIFF_FIXTURE_LINES)),
        "the whole-file view must print the last line: {payload}"
    );
    assert!(
        !payload.contains("unchanged line(s) not shown"),
        "the whole-file view elides nothing: {payload}"
    );
}

/// A file the change left alone renders a stated "nothing to review here"
/// rather than an empty block, which a model reads as an empty file and
/// invents a finding about.
#[test]
fn diff_payload_says_so_when_the_change_touched_no_line_of_the_file() {
    let file = file_work_with_changed_lines("src/a.rs", &[]);

    let payload = render_file_payload(std::slice::from_ref(&file), ReviewSubject::Diffs);

    assert!(
        payload.contains("nothing here to REVIEW"),
        "an untouched file must say it has nothing to review: {payload}"
    );
    assert!(
        !payload.contains(&fixture_line_text(DIFF_FIXTURE_CHANGED_LINE)),
        "an untouched file's source must not print at all: {payload}"
    );
}

/// The `Diffs` mirror of [`output_contract_scopes_reads_to_other_files`]: the
/// block is deliberately not the whole file, so reading further is invited
/// rather than discouraged.
#[test]
fn output_contract_under_diffs_invites_reading_beyond_the_inlined_band() {
    let contract = crate::review::fleet::output_contract(ReviewSubject::Diffs);
    let lower = contract.to_lowercase();
    assert!(
        lower.contains("more of a file"),
        "the contract must invite reading more of a file than its block shows: {contract}"
    );
    assert!(
        lower.contains("another file entirely"),
        "the contract must still scope cross-file reads: {contract}"
    );
    assert!(
        !lower.contains("provided in full"),
        "a diff contract must never claim the files are provided in full: {contract}"
    );
}

/// The `Diffs` mirror of
/// [`output_contract_demands_every_occurrence_with_no_bail_fast`]: every match
/// across the marked lines, in one pass.
#[test]
fn output_contract_under_diffs_demands_every_marked_line_with_no_bail_fast() {
    let contract = crate::review::fleet::output_contract(ReviewSubject::Diffs);
    let lower = contract.to_lowercase();
    assert!(
        lower.contains("every occurrence of every rule"),
        "the contract must demand every occurrence of every rule: {contract}"
    );
    assert!(
        lower.contains("do not stop at the first"),
        "the contract must forbid stopping at the first match: {contract}"
    );
    assert!(
        contract.contains("one finding per `file:line`"),
        "the contract must require one finding per file:line: {contract}"
    );
    assert!(
        lower.contains("marked line"),
        "the completeness demand must be scoped to the marked lines: {contract}"
    );
}

/// The `Diffs` mirror of
/// [`output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff`]:
/// the marked lines are the boundary, a pre-existing defect is out of scope,
/// and the contract says what happens to a finding that lands elsewhere.
#[test]
fn output_contract_names_the_marked_lines_as_the_review_boundary_not_the_file() {
    let contract = crate::review::fleet::output_contract(ReviewSubject::Diffs);
    let lower = contract.to_lowercase();
    assert!(
        contract.contains("## Review scope"),
        "the contract must carry an explicit review-scope section: {contract}"
    );
    assert!(
        lower.contains("added or modified"),
        "the contract must name the added/modified lines as the boundary: {contract}"
    );
    assert!(
        lower.contains("refuted"),
        "the contract must state that an off-change finding is refuted: {contract}"
    );
    assert!(
        lower.contains("out of scope"),
        "the contract must put a pre-existing defect out of scope: {contract}"
    );
    assert!(
        !lower.contains("whole current file"),
        "a diff contract must never name the whole current file as the boundary: {contract}"
    );
}

/// The follow-up sweep is subject-specific too: it sweeps the marked lines
/// under `Diffs` and the whole file under `Files`. Sweeping "outside the
/// changed region" on a diff review spends four turns collecting findings the
/// verify guard then refutes.
#[test]
fn the_followup_sweep_stays_on_the_marked_lines_under_diffs_only() {
    let diffs = crate::review::fleet::followup_prompt(ReviewSubject::Diffs);
    let files = crate::review::fleet::followup_prompt(ReviewSubject::Files);

    assert!(
        diffs.contains(RESCAN_NEEDLE) && files.contains(RESCAN_NEEDLE),
        "both sweeps must carry the stable re-scan header"
    );
    assert!(
        diffs.to_lowercase().contains("stay on the marked lines"),
        "the diff sweep must hold the model to the marked lines: {diffs}"
    );
    assert!(
        !diffs.to_lowercase().contains("outside the changed region"),
        "the diff sweep must not send the model outside the change: {diffs}"
    );
    assert!(
        files.to_lowercase().contains("outside the changed region"),
        "the whole-file sweep must still reach outside the changed region: {files}"
    );
}

/// The subject reaches the end of the chain: a monolithic per-validator prompt
/// rendered under `Diffs` carries the diff focus-file statement and the diff
/// contract, and none of the whole-file wording.
#[test]
fn monolithic_prompt_under_diffs_carries_the_diff_review_boundary() {
    let rs = ruleset("deduplicate", "mandate", &[("no-copy-paste", "RULE_BODY")]);
    let vw = validator_work(
        "deduplicate",
        vec![file_work_with_changed_lines(
            "src/a.rs",
            &[DIFF_FIXTURE_CHANGED_LINE],
        )],
    );

    let prompt = render_fleet_prompt("purpose", &vw, &rs, ReviewSubject::Diffs);
    let lower = prompt.to_lowercase();

    assert!(
        prompt.contains("## Files in scope"),
        "the suffix must still list the validator's files: {prompt}"
    );
    assert!(
        lower.contains("every finding must land on one of them"),
        "the focus-file list must state the diff boundary: {prompt}"
    );
    assert!(
        !lower.contains("whole current contents"),
        "a diff prompt must not ask for the whole current contents: {prompt}"
    );
    assert!(
        !lower.contains("whole current file"),
        "a diff prompt's contract must not name the whole current file: {prompt}"
    );
    assert!(
        prompt.contains(&format_row(DIFF_FIXTURE_CHANGED_LINE, CHANGED_MARK)),
        "the inlined block must carry the marked line: {prompt}"
    );
}
