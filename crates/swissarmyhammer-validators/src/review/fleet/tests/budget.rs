//! The byte budget a run packs against.
//!
//! Four subjects stand here because they share their fixtures: the config
//! constants, the over-cap verdict that must not move with the change around
//! it, the rendered measure the packer costs a file by, and the share of the
//! cap the framing keeps.

use super::*;

#[test]
fn the_batch_budget_and_the_agent_prompt_cap_are_one_constant() {
    // The defect this pins: the batch budget and the agent's prompt cap were
    // three independent numbers in three crates, so the batcher packed ~4x
    // what the agent would accept and every fat batch came back as a bare
    // `invalid_params`. The budget is now the cap, read from the one place
    // that declares it — they cannot drift apart.
    assert_eq!(
        AGENT_PROMPT_CAP,
        claude_agent::constants::sizes::messages::MAX_PROMPT_LENGTH,
        "the fleet reads the agent's cap; it never re-declares one"
    );
    assert_eq!(
        DEFAULT_BATCH_SIZE, AGENT_PROMPT_CAP,
        "the default batch budget IS the cap; the framing reserve is subtracted per run"
    );
    assert_eq!(FleetConfig::default().batch_size(), DEFAULT_BATCH_SIZE);
}

#[test]
fn a_caller_supplied_batch_size_is_clamped_to_the_agent_prompt_cap() {
    // `batch_size` is a user-facing `review` modifier, so a caller can ask for
    // a budget the agent would reject outright. The config clamps rather than
    // trusting it.
    assert_eq!(
        FleetConfig::new(AGENT_PROMPT_CAP * 4).batch_size(),
        AGENT_PROMPT_CAP,
        "no caller can raise the budget above the cap"
    );
    let under = AGENT_PROMPT_CAP / 4;
    assert_eq!(
        FleetConfig::new(under).batch_size(),
        under,
        "a stricter caller budget is honored as-is"
    );
}

#[test]
fn the_file_payload_budget_leaves_room_for_the_prompt_framing() {
    // The cap applies to the WHOLE prompt, and a batch prompt carries the
    // change purpose, the payload header, and the validator's full ruleset on
    // top of its file blocks. The budget the packer gets is what is left.
    const FRAMING: usize = 40_000;
    assert_eq!(
        FleetConfig::default().file_payload_budget(FRAMING),
        AGENT_PROMPT_CAP - FRAMING
    );
    assert_eq!(
        FleetConfig::default().file_payload_budget(AGENT_PROMPT_CAP * 2),
        0,
        "framing alone over the cap leaves no room, and never underflows"
    );
}

// ---- the over-cap verdict is a constant ------------------------------

/// A caller-supplied `batch_size`, in bytes, well under
/// [`MAX_FILE_BLOCK_BYTES`] — the case where the caller's budget, not the
/// constant, is the stricter of the two.
const CALLER_BATCH_SIZE: usize = 6_000;

/// The repo-relative path of the file whose over-cap verdict is under test.
const SUBJECT_PATH: &str = "src/subject.rs";

/// One source line of the subject file: 27 raw bytes, rendering to about 49
/// with the number/sha/mark columns. Repeating one line makes the fixture's
/// size a function of the line COUNT alone.
const SUBJECT_SOURCE_LINE: &str = "fn filler() { let x = 1; }\n";

/// How many [`SUBJECT_SOURCE_LINE`]s the subject file carries: enough to render
/// past what a heavily framed run leaves for file blocks, and still inside the
/// per-file cap. The test asserts both premises rather than trusting the
/// arithmetic.
const SUBJECT_SOURCE_LINES: usize = 4_000;

/// How many `<changed-set>` duplicate rows the bigger change carries. Each row
/// renders to a few dozen bytes, so this is several hundred kilobytes of run
/// framing — the term that shrank the packer's budget between two review
/// rounds in production.
const GROWN_CHANGE_DUPLICATE_ROWS: usize = 12_000;

/// How many other files the bigger change touches beside the subject.
const GROWN_CHANGE_OTHER_FILES: usize = 20;

/// How many short lines the deliberately over-cap file carries — enough that
/// its rendered block clears [`MAX_FILE_BLOCK_BYTES`] on its own.
const OVER_CAP_SOURCE_LINES: usize = 14_000;

/// The `<changed-set>` `duplicates` evidence a change carrying `rows`
/// duplicate pairs produces — the batch-scoped block every batch's prompt
/// repeats, and the framing term that grows with the change.
fn changed_set_duplicates(rows: usize) -> ProbeResult {
    ProbeResult {
        name: "duplicates".to_string(),
        kind: ProbeKind::Fact,
        target: "<changed-set>".to_string(),
        rows: (0..rows)
            .map(|index| ProbeRow {
                file_path: format!("src/dup{index}.rs"),
                symbol: Some(format!("sym{index}")),
                line: Some(TEST_PROBE_LINE),
                similarity: Some(TEST_SIMILARITY),
                detail: None,
            })
            .collect(),
    }
}

/// A minimal loader for the budget fixtures: one validator, one short rule, so
/// the ruleset contributes almost nothing to the framing and the shared probe
/// evidence is the only term that grows.
fn budget_fixture_loader() -> ValidatorLoader {
    loader_with(vec![ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", "RULE BODY.")],
    )])
}

#[test]
fn the_per_file_cap_holds_still_while_the_batch_budget_follows_the_framing() {
    // The two numbers answer two questions, so only one of them may read the
    // run's framing. Batch boundaries move with the framing; the over-cap
    // verdict does not.
    const SMALL_FRAMING: usize = 1_000;
    const HEAVY_FRAMING: usize = 400_000;
    let config = FleetConfig::default();

    assert_eq!(
        config.batch_budget(SMALL_FRAMING).file_cap(),
        config.batch_budget(HEAVY_FRAMING).file_cap(),
        "the over-cap verdict never reads the framing"
    );
    assert!(
        config.batch_budget(SMALL_FRAMING).batch_bytes()
            > config.batch_budget(HEAVY_FRAMING).batch_bytes(),
        "batch boundaries still move with the framing"
    );
    assert_eq!(
        config.file_block_cap(),
        MAX_FILE_BLOCK_BYTES,
        "the default cap is the constant"
    );
    assert_eq!(
        FleetConfig::new(CALLER_BATCH_SIZE).file_block_cap(),
        CALLER_BATCH_SIZE,
        "a stricter caller budget lowers the cap with it"
    );
}

#[test]
fn a_file_inside_the_cap_stays_inside_it_when_the_change_around_it_grows() {
    // The defect this pins: an over-cap finding tells the author to split the
    // file, the split grows the change, the bigger change renders more shared
    // evidence, and the smaller remainder used to put MORE files over cap.
    // Splitting made the next round worse, so the loop never converged. A file
    // that did not grow must keep its verdict.
    let loader = budget_fixture_loader();
    let config = FleetConfig::default();

    let subject = bare_file_work(
        SUBJECT_PATH,
        SUBJECT_SOURCE_LINE.repeat(SUBJECT_SOURCE_LINES),
    );
    let rendered = rendered_file_block_bytes(&subject, ReviewSubject::Files);
    assert!(
        rendered < config.file_block_cap(),
        "the subject must be inside the per-file cap: {rendered} rendered bytes vs {}",
        config.file_block_cap()
    );

    let small_change = WorkList::new(
        "PURPOSE: one file.".to_string(),
        vec![validator_work("bulk", vec![subject.clone()])],
    );

    let mut grown_files = vec![subject];
    grown_files.extend((0..GROWN_CHANGE_OTHER_FILES).map(|index| {
        bare_file_work(
            &format!("src/other{index}.rs"),
            SUBJECT_SOURCE_LINE.to_string(),
        )
    }));
    let grown_change = WorkList::new(
        "PURPOSE: the same file, in a bigger change.".to_string(),
        vec![validator_work("bulk", grown_files)
            .with_shared_probe_results([changed_set_duplicates(GROWN_CHANGE_DUPLICATE_ROWS)])],
    );

    let small_framing = prompt_framing_bytes(&small_change, &loader);
    let grown_framing = prompt_framing_bytes(&grown_change, &loader);
    assert!(
        grown_framing > small_framing,
        "premise: the bigger change must still frame more than the small one \
         ({grown_framing} vs {small_framing})"
    );
    assert!(
        grown_framing <= MAX_FRAMING_BYTES,
        "however much evidence the change carries, the framing stays inside its share, so it \
         can never crowd out a file that satisfies the per-file cap ({grown_framing} framing, \
         {rendered} rendered, {MAX_FRAMING_BYTES}-byte framing share)"
    );

    let (_, small_skips) = crate::review::scope::batch_work_list(
        &small_change,
        config.batch_budget(small_framing),
        |file| rendered_file_block_bytes(file, ReviewSubject::Files),
    );
    assert!(
        small_skips.is_empty(),
        "the subject is inside the cap, so the small change reviews it: {small_skips:?}"
    );

    let (_, grown_skips) = crate::review::scope::batch_work_list(
        &grown_change,
        config.batch_budget(grown_framing),
        |file| rendered_file_block_bytes(file, ReviewSubject::Files),
    );
    assert!(
        !grown_skips.iter().any(|skip| skip.path() == SUBJECT_PATH),
        "the subject did not grow, so a bigger change around it cannot put it over cap: \
         {grown_skips:?}"
    );
}

#[test]
fn two_runs_over_the_same_change_report_the_same_over_cap_files() {
    // The other half of the convergence contract: re-running review over
    // unchanged content re-reports exactly the same gaps, so an author can tell
    // a fix from the engine moving under them.
    let loader = budget_fixture_loader();
    let config = FleetConfig::default();

    let over_cap = bare_file_work(SUBJECT_PATH, short_line_source(OVER_CAP_SOURCE_LINES));
    let rendered = rendered_file_block_bytes(&over_cap, ReviewSubject::Files);
    assert!(
        rendered > config.file_block_cap(),
        "the subject must be over the per-file cap: {rendered} rendered bytes vs {}",
        config.file_block_cap()
    );

    let change = WorkList::new(
        "PURPOSE: one oversized file beside a small one.".to_string(),
        vec![validator_work(
            "bulk",
            vec![
                over_cap,
                bare_file_work("src/small.rs", "fn ok() {}\n".to_string()),
            ],
        )],
    );

    let review_once = || {
        let framing = prompt_framing_bytes(&change, &loader);
        let (_, skipped) =
            crate::review::scope::batch_work_list(&change, config.batch_budget(framing), |file| {
                rendered_file_block_bytes(file, ReviewSubject::Files)
            });
        skipped
    };

    let first = review_once();
    let second = review_once();
    assert_eq!(
        first.iter().map(|skip| skip.path()).collect::<Vec<_>>(),
        vec![SUBJECT_PATH],
        "only the oversized file is a gap"
    );
    assert_eq!(
        first, second,
        "the same content reports the same over-cap files, byte counts and all"
    );
}

// ---- rendered-budget tests -------------------------------------------

/// A source of `lines` short lines — the content shape a fixed expansion
/// multiplier gets most wrong.
///
/// Every rendered line gains a fixed ~16 bytes for the `{line:>6} | {sha:8}
/// {mark} | ` columns, so expansion is a function of LINE COUNT, not of byte
/// count. A file of 4-byte lines renders at ~5x its raw size; a file of
/// 200-byte lines at ~1.08x. No single multiplier on raw bytes can be right
/// for both.
fn short_line_source(lines: usize) -> String {
    (0..lines).map(|_| "a;\n").collect()
}

/// A `FileWork` with no semantic diff and no probe evidence, so a test that
/// measures rendering is measuring the source render and the fixed framing
/// only.
fn bare_file_work(path: &str, source: String) -> FileWork {
    FileWork::new(path.to_string(), vec![], vec![], source, vec![])
}

#[test]
fn a_short_line_file_the_raw_byte_budget_admits_is_measured_by_its_rendered_size() {
    // The case a fixed multiplier misses. `tiny.rs` is 3000 bytes of raw
    // source — comfortably inside a 6000-byte budget — but renders to well
    // over it, because 1000 lines each gain the number/sha/mark columns.
    const BUDGET: usize = 6_000;
    const LINES: usize = 1_000;

    let file = bare_file_work("tiny.rs", short_line_source(LINES));
    assert!(
        file.source_slice().len() <= BUDGET,
        "the raw source is inside the budget, which is the whole point: {} vs {BUDGET}",
        file.source_slice().len()
    );

    let rendered = rendered_file_block_bytes(&file, ReviewSubject::Files);
    assert!(
        rendered > BUDGET,
        "the RENDERED block is over the budget: {rendered} vs {BUDGET}"
    );

    let work = WorkList::new("purpose".to_string(), vec![validator_work("v", vec![file])]);
    let (batches, skipped) =
        crate::review::scope::batch_work_list(&work, uniform_budget(BUDGET), |file| {
            rendered_file_block_bytes(file, ReviewSubject::Files)
        });

    assert!(
        batches.is_empty(),
        "the file cannot be packed, so no batch carries it: {batches:?}"
    );
    assert_eq!(skipped.len(), 1, "it is reported, never silently dropped");
    assert_eq!(skipped[0].path(), "tiny.rs");
    assert_eq!(
        skipped[0].validator(),
        "v",
        "the gap names which validator could not carry the file"
    );
    assert_eq!(
        skipped[0].size(),
        rendered,
        "the reported size is the rendered size, not the raw source size"
    );
    assert_eq!(skipped[0].cap(), BUDGET);
}

#[test]
fn one_validators_oversized_file_does_not_cost_the_other_validators_that_file() {
    // The fan-out grain is the (validator, file) pair, so the gap is that
    // pair, not the file. `heavy` carries evidence that blows the budget for
    // `fat.rs`; `light` reviews the same file with none, and must keep it.
    const BUDGET: usize = 6_000;

    let bloated = bare_file_work("fat.rs", short_line_source(1_000));
    let lean = bare_file_work("fat.rs", "fn ok() {}\n".to_string());

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("heavy", vec![bloated]),
            validator_work("light", vec![lean]),
        ],
    );
    let (batches, skipped) =
        crate::review::scope::batch_work_list(&work, uniform_budget(BUDGET), |file| {
            rendered_file_block_bytes(file, ReviewSubject::Files)
        });

    assert_eq!(batches.len(), 1, "the affordable pair still reviews");
    assert_eq!(
        batches[0]
            .validators()
            .iter()
            .map(|v| v.validator_name())
            .collect::<Vec<_>>(),
        vec!["light"],
        "only the validator that could not afford the file is dropped"
    );
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].validator(), "heavy");
}

#[test]
fn every_prompt_a_packed_batch_sends_fits_inside_the_agent_prompt_cap() {
    // The acceptance test: pack a full run through the REAL batching and the
    // REAL renderers, then measure the actual prompts. Both shapes are
    // measured — the shared prime and, because the claude backend never saves
    // restorable prime state, the monolithic per-validator fallback that is
    // its production path.
    let ruleset = ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", &"RULE BODY sentence. ".repeat(500))],
    );
    let loader = loader_with(vec![ruleset.clone()]);

    // Twelve files of ~64 KB of raw source each: ~768 KB raw, well past the
    // 512 KiB cap, so the run MUST split into several batches.
    let files: Vec<FileWork> = (0..12)
        .map(|i| {
            file_work_with_slice(
                &format!("src/f{i}.rs"),
                &format!("sym{i}"),
                "src/other.rs",
                "fn filler() { let x = 1; }\n".repeat(2_400),
            )
        })
        .collect();
    let work = WorkList::new(
        "PURPOSE: a large multi-file change.".to_string(),
        vec![validator_work("bulk", files)],
    );

    let framing = prompt_framing_bytes(&work, &loader);
    let budget = FleetConfig::default().batch_budget(framing);
    let (batches, skipped) = crate::review::scope::batch_work_list(&work, budget, |file| {
        rendered_file_block_bytes(file, ReviewSubject::Files)
    });

    assert!(
        skipped.is_empty(),
        "no file here is individually oversized: {skipped:?}"
    );
    assert!(
        batches.len() > 1,
        "a run this large must split into several batches, not one over-cap prompt"
    );

    for batch in &batches {
        let prime = render_run_prime(batch);
        assert!(
            prime.len() <= AGENT_PROMPT_CAP,
            "a batch's shared prime is {} bytes, over the {AGENT_PROMPT_CAP}-byte cap",
            prime.len()
        );
        for validator in batch.validators() {
            let monolithic = render_fleet_prompt(
                batch.change_purpose(),
                validator,
                &ruleset,
                ReviewSubject::Files,
            );
            assert!(
                monolithic.len() <= AGENT_PROMPT_CAP,
                "{}'s monolithic prompt is {} bytes, over the {AGENT_PROMPT_CAP}-byte cap",
                validator.validator_name(),
                monolithic.len()
            );
        }
    }
}

#[test]
fn the_prompt_framing_bytes_cover_the_purpose_the_payload_header_and_the_ruleset() {
    // The framing reserve must bound everything a prompt carries that is not a
    // file block, or the packer hands back a budget that overflows the cap the
    // moment the rules are appended.
    let ruleset = ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", &"RULE BODY sentence. ".repeat(500))],
    );
    let loader = loader_with(vec![ruleset.clone()]);
    let validator = validator_work("bulk", vec![file_work("src/a.rs", "alpha", "src/x.rs")]);
    let work = WorkList::new("PURPOSE: framing.".to_string(), vec![validator]);

    let framing = prompt_framing_bytes(&work, &loader);
    let monolithic = render_fleet_prompt(
        work.change_purpose(),
        &work.validators()[0],
        loader.get_ruleset("bulk").expect("the ruleset is loaded"),
        ReviewSubject::Files,
    );
    let blocks: usize = work
        .distinct_files()
        .map(|file| rendered_file_block_bytes(file, ReviewSubject::Files))
        .sum();

    assert!(
        framing >= monolithic.len() - blocks,
        "framing ({framing}) must cover the whole prompt minus its file blocks ({})",
        monolithic.len() - blocks
    );
}

// ---- the framing stays inside its share of the cap --------------------

/// How many `<changed-set>` duplicate rows the framing-bound fixtures carry —
/// far more than [`MAX_SHARED_EVIDENCE_BYTES`] admits, so every one of them
/// exercises the truncation rather than the rows simply fitting.
const OVER_CAP_EVIDENCE_ROWS: usize = 20_000;

/// How many files the file-count fixture spreads across, to show the framing no
/// longer moves with the size of the change's file list.
const FRAMING_FIXTURE_FILES: usize = 40;

/// The line count the per-file cap fixture probes with before it scales up to
/// the cap. Big enough that the block's fixed header amortizes away, small
/// enough to render quickly.
const CAP_PROBE_LINES: usize = 1_000;

/// A `FileWork` whose rendered cost is the largest a whole number of
/// [`SUBJECT_SOURCE_LINE`]s can reach without passing `cap` — a file that FILLS
/// the per-file cap.
///
/// Sized by probing the real renderer rather than by a hand-computed line
/// count, so it keeps filling the cap when the block format changes.
fn file_filling_the_cap(path: &str, cap: usize) -> FileWork {
    let probe = bare_file_work(path, SUBJECT_SOURCE_LINE.repeat(CAP_PROBE_LINES));
    let per_line = rendered_file_block_bytes(&probe, ReviewSubject::Files) / CAP_PROBE_LINES;
    let mut lines = cap / per_line;
    while rendered_file_block_bytes(
        &bare_file_work(path, SUBJECT_SOURCE_LINE.repeat(lines)),
        ReviewSubject::Files,
    ) > cap
    {
        lines -= 1;
    }
    bare_file_work(path, SUBJECT_SOURCE_LINE.repeat(lines))
}

#[test]
fn a_batch_prompt_fits_the_cap_when_one_file_fills_the_per_file_cap() {
    // The acceptance test for ^x8z9hgf, driven through the REAL renderers and
    // the REAL packer. Two files each filling the per-file cap cannot share a
    // batch, so one batch carries a single full-cap block; the change also
    // carries far more duplicate evidence than a prompt can hold. Both prompt
    // shapes must still fit — the shared prime and, because the claude backend
    // never saves restorable prime state, the monolithic fallback that is its
    // production path.
    let ruleset = ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", &"RULE BODY sentence. ".repeat(500))],
    );
    let loader = loader_with(vec![ruleset.clone()]);
    let config = FleetConfig::default();

    let first = file_filling_the_cap(SUBJECT_PATH, config.file_block_cap());
    let second = file_filling_the_cap("src/second.rs", config.file_block_cap());
    let full_cap_cost = rendered_file_block_bytes(&first, ReviewSubject::Files);

    let work = WorkList::new(
        "PURPOSE: two files at the per-file cap, in a change carrying far more duplicate \
         evidence than one prompt can hold."
            .to_string(),
        vec![validator_work("bulk", vec![first, second])
            .with_shared_probe_results([changed_set_duplicates(OVER_CAP_EVIDENCE_ROWS)])],
    );

    let framing = prompt_framing(&work, &loader);
    assert!(
        framing.shared_evidence() <= MAX_SHARED_EVIDENCE_BYTES,
        "premise: the evidence must have been truncated, not merely fit: {} vs \
         {MAX_SHARED_EVIDENCE_BYTES}",
        framing.shared_evidence()
    );
    assert!(
        framing.total() <= MAX_FRAMING_BYTES,
        "the framing must stay inside its share of the cap: {} vs {MAX_FRAMING_BYTES}",
        framing.total()
    );
    assert!(
        framing.total() + config.file_block_cap() <= AGENT_PROMPT_CAP,
        "a full-cap file plus this run's framing must fit the prompt cap: {} + {} vs \
         {AGENT_PROMPT_CAP}",
        framing.total(),
        config.file_block_cap()
    );

    let budget = config.batch_budget(framing.total());
    let (batches, skipped) = crate::review::scope::batch_work_list(&work, budget, |file| {
        rendered_file_block_bytes(file, ReviewSubject::Files)
    });
    assert!(
        skipped.is_empty(),
        "neither file is over the per-file cap, so both are reviewed: {skipped:?}"
    );
    assert!(
        batches
            .iter()
            .any(|batch| batch.distinct_files().count() == 1),
        "premise: a full-cap file must take a batch of its own, which is the prompt this \
         test exists to size"
    );

    for batch in &batches {
        let prime = render_run_prime(batch);
        assert!(
            prime.len() <= AGENT_PROMPT_CAP,
            "a batch's shared prime is {} bytes, over the {AGENT_PROMPT_CAP}-byte cap \
             (framing {}, full-cap block {full_cap_cost})",
            prime.len(),
            framing.total()
        );
        for validator in batch.validators() {
            let monolithic = render_fleet_prompt(
                batch.change_purpose(),
                validator,
                &ruleset,
                ReviewSubject::Files,
            );
            assert!(
                monolithic.len() <= AGENT_PROMPT_CAP,
                "{}'s monolithic prompt is {} bytes, over the {AGENT_PROMPT_CAP}-byte cap \
                 (framing {}, full-cap block {full_cap_cost})",
                validator.validator_name(),
                monolithic.len(),
                framing.total()
            );
        }
    }
}

#[test]
fn the_shared_evidence_block_is_capped_and_names_the_rows_it_dropped() {
    // Truncating evidence is a real loss, so the block must never read as an
    // exhaustive list once it is partial.
    let mut out = String::new();
    render_shared_probe_evidence(&mut out, &[changed_set_duplicates(OVER_CAP_EVIDENCE_ROWS)]);

    assert!(
        out.len() <= MAX_SHARED_EVIDENCE_BYTES,
        "the section is capped: {} vs {MAX_SHARED_EVIDENCE_BYTES}",
        out.len()
    );
    assert!(
        out.contains("further evidence rows are NOT shown"),
        "the block must say it is partial, or the model reads it as the whole list"
    );

    let rows_shown = out.matches("src/dup").count();
    assert!(
        rows_shown > 0,
        "the rows that fit are still rendered, so the evidence is sampled, not dropped"
    );
    assert!(
        out.contains(&format!("{} further", OVER_CAP_EVIDENCE_ROWS - rows_shown)),
        "the notice must name exactly how many rows were left out; it shows {rows_shown} of \
         {OVER_CAP_EVIDENCE_ROWS}: {}",
        &out[out.len().saturating_sub(400)..]
    );
}

#[test]
fn a_shared_evidence_block_that_fits_renders_every_row_and_no_notice() {
    // The cap must be invisible below it: a change whose evidence fits is
    // rendered exactly as it was before the cap existed.
    const FITTING_ROWS: usize = 10;
    let mut out = String::new();
    render_shared_probe_evidence(&mut out, &[changed_set_duplicates(FITTING_ROWS)]);

    assert_eq!(
        out.matches("src/dup").count(),
        FITTING_ROWS,
        "every row renders when they all fit"
    );
    assert!(
        !out.contains("further evidence rows are NOT shown"),
        "no notice when nothing was dropped: {out}"
    );
}

#[test]
fn the_omitted_rows_notice_fits_the_bytes_reserved_for_it() {
    // The reserve is what keeps the notice from pushing the section past its
    // cap, so it must cover the widest count the notice can ever render.
    let widest = crate::review::fleet::render::omitted_rows_notice(usize::MAX);
    assert!(
        widest.len() <= crate::review::fleet::render::MAX_OMITTED_ROWS_NOTICE_BYTES,
        "the widest notice is {} bytes against a {}-byte reserve: {widest}",
        widest.len(),
        crate::review::fleet::render::MAX_OMITTED_ROWS_NOTICE_BYTES
    );
}

#[test]
fn a_validators_suffix_splits_into_run_framing_plus_one_line_per_file() {
    // The focus-file lines are charged to their file, not reserved as run
    // framing. That is only sound if the two halves add back up to the suffix
    // the agent actually receives.
    let rs = ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", "RULE BODY.")],
    );
    let loader = loader_with(vec![rs.clone()]);

    let files: Vec<FileWork> = (0..FRAMING_FIXTURE_FILES)
        .map(|index| bare_file_work(&format!("src/f{index}.rs"), "fn ok() {}\n".to_string()))
        .collect();
    let work = WorkList::new(
        "PURPOSE: many files.".to_string(),
        vec![validator_work("bulk", files.clone())],
    );

    let suffix = render_validator_suffix(&work.validators()[0], &rs, ReviewSubject::Files).len();
    let framing = prompt_framing(&work, &loader);
    let focus_lines: usize = files
        .iter()
        .map(crate::review::fleet::render::focus_file_line_bytes)
        .sum();

    assert_eq!(
        suffix,
        framing.validator_suffix() + focus_lines,
        "the whole suffix must be the reserved framing plus the per-file lines"
    );
}

#[test]
fn the_framing_stops_growing_with_the_number_of_files_in_the_change() {
    // The framing is a per-BATCH reserve, so a term that grows with the run's
    // file count shrinks every batch's budget for files that batch does not
    // even carry.
    let rs = ruleset(
        "bulk",
        "MANDATE: review everything.",
        &[("one-rule", "RULE BODY.")],
    );
    let loader = loader_with(vec![rs]);

    let one = WorkList::new(
        "PURPOSE: same purpose.".to_string(),
        vec![validator_work(
            "bulk",
            vec![bare_file_work("src/f0.rs", "fn ok() {}\n".to_string())],
        )],
    );
    let many = WorkList::new(
        "PURPOSE: same purpose.".to_string(),
        vec![validator_work(
            "bulk",
            (0..FRAMING_FIXTURE_FILES)
                .map(|index| {
                    bare_file_work(&format!("src/f{index}.rs"), "fn ok() {}\n".to_string())
                })
                .collect(),
        )],
    );

    assert_eq!(
        prompt_framing_bytes(&one, &loader),
        prompt_framing_bytes(&many, &loader),
        "the same purpose and the same ruleset frame the same bytes, however many files the \
         change touches"
    );
}

#[test]
fn every_builtin_validators_suffix_fits_the_framings_authored_share() {
    // The framing bound holds only if the authored half — the change purpose
    // and the largest validator's rule bodies — stays inside what the shared
    // evidence cap leaves it. That half is authored Markdown, so the shipped
    // validators are what has to satisfy it.
    let authored_share = MAX_FRAMING_BYTES - MAX_SHARED_EVIDENCE_BYTES;
    let mut loader = ValidatorLoader::new();
    crate::builtin::load_builtins(&mut loader);
    let rulesets = loader.list_rulesets();
    assert!(
        !rulesets.is_empty(),
        "the builtin validators must load, or this proves nothing"
    );

    for rs in rulesets {
        let validator = ValidatorWork::new(
            rs.manifest.name.clone(),
            RuleNames::new(rs.rules.iter().map(|rule| rule.name.clone())),
            ProbeNames::new(rs.manifest.probes.iter().cloned()),
            Vec::new(),
        );
        let suffix = render_validator_suffix(&validator, rs, ReviewSubject::Files).len();
        assert!(
            suffix < authored_share,
            "`{}` frames {suffix} bytes against the {authored_share}-byte authored share",
            rs.manifest.name
        );
    }
}
