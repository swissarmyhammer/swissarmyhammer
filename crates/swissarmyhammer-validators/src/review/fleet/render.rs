//! Prompt rendering for the fan-out fleet — every byte an agent task is
//! prompted with.
//!
//! The pieces compose byte-identically into the two prompt shapes the fleet
//! sends (the shared run prime + per-validator fork suffix, and the monolithic
//! per-validator fallback), so the warm and degraded paths never drift. Only
//! PROMPT rule bodies ever render: a tool rule's body never reaches an LLM —
//! the tool runner ([`crate::review::tool_rules`]) executes tool rules
//! instead. The framing/measure helpers ([`rendered_file_block_bytes`],
//! [`prompt_framing_bytes`]) run these same renderers, so the number the batch
//! packer budgets on and the number the agent receives are the same bytes.

use std::fmt::Write as _;

use crate::review::probes::{render_probe_evidence, render_probe_evidence_within, ProbeResult};
use crate::review::scope::{FileWork, LineAnnotation, ValidatorWork, WorkList};
use crate::validators::{RuleSet, ValidatorLoader};

use super::{
    MANDATE_HEADER, MAX_SHARED_EVIDENCE_BYTES, OUTPUT_CONTRACT, PRIME_HANDOFF, VALIDATOR_HEADER,
};

/// The header that opens both fan-out prompt renderings — the monolithic
/// fallback ([`render_fleet_prompt`]) and the shared run prime
/// ([`render_run_prime`]) — so the two prompt shapes stay byte-identical on
/// this section and a wording change lands in one place.
const CHANGE_PURPOSE_HEADER: &str = "# Change purpose\n\n";

/// Render the fan-out prompt for one validator task — the monolithic fallback
/// shape (one fresh session, everything for the validator in one prompt).
///
/// Self-contained and scoped exactly as the old per-validator prompt was: the
/// change purpose, that validator's own files (path + semantic diff + bounded
/// source slice + probe evidence), and the validator's instructions (mandate +
/// every prompt rule body — excluding tool rules, which are executed
/// separately by the tool runner — + output contract). It is the cold fallback
/// when priming or this validator's fork fails — correct, just not warm.
///
/// The warm path splits the run's large shared content into the run prime
/// ([`render_run_prime`], every file, primed once) and per-validator forks
/// ([`render_validator_suffix`], one validator's prompt rules each). The fallback re-renders
/// both halves for the validator in one prompt, so a degraded task is
/// byte-for-byte the same review of the validator against its files — only the
/// session reuse differs.
///
/// `validator` is the work-list entry (its name and the file work); `ruleset` is
/// the same validator's loaded [`RuleSet`], the authoritative source of the
/// mandate (its description) and the verbatim rule bodies.
pub fn render_fleet_prompt(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
) -> String {
    let mut out = String::new();
    out.push_str(CHANGE_PURPOSE_HEADER);
    out.push_str(change_purpose.trim());
    out.push_str("\n\n");
    out.push_str(&render_file_payload(validator.files()));
    render_shared_probe_evidence(&mut out, validator.shared_probe_results());
    out.push_str(&render_validator_suffix(validator, ruleset));
    out
}

/// Render the run's shared primed prefix the prime turn decodes ONCE per review
/// run: the change purpose + every distinct file under review (path + semantic
/// diff + bounded source slice + probe evidence), ending with [`PRIME_HANDOFF`].
///
/// This is the large content shared across every validator — the diffs are primed
/// and cached once, never re-sent per validator. It carries NO validator-specific
/// text; the validator's rules arrive on each fork as [`render_validator_suffix`].
/// Files are de-duplicated by path (a file matched by several validators is
/// inlined once), so the prime stays a single rendering of the whole change.
///
/// The render is a pure function of its inputs — byte-stable across calls — so
/// every validator fork of the primed session shares the exact prefix bytes the
/// parent decoded, and the fork's first decode reuses the full saved state.
pub fn render_run_prime(work: &WorkList) -> String {
    let mut out = String::new();
    out.push_str(CHANGE_PURPOSE_HEADER);
    out.push_str(work.change_purpose().trim());
    out.push_str("\n\n");
    let distinct: Vec<FileWork> = work.distinct_files().cloned().collect();
    out.push_str(&render_file_payload(&distinct));
    render_shared_probe_evidence(&mut out, &work.shared_probe_results());
    out.push_str(PRIME_HANDOFF);
    out
}

/// Render the per-validator suffix a forked session is prompted with: the
/// validator header, mandate, the files this validator must focus on, every
/// prompt rule body (excluding tool rules, which are executed separately by
/// the tool runner), and the output contract.
/// The files' contents are already in the fork's inherited prime; only their
/// paths are named here so the validator stays scoped to its matched files (not
/// every file in the prime), without re-sending any diff.
///
/// Always non-empty: it carries at least the rule bodies and the output contract,
/// so a fork turn never degenerates to a full reprocess (`lcp == new_len`).
pub fn render_validator_suffix(validator: &ValidatorWork, ruleset: &RuleSet) -> String {
    render_suffix(validator.validator_name(), ruleset, validator.files())
}

/// The suffix bytes that do NOT move with the batch's file list — everything
/// [`render_validator_suffix`] renders except the focus-file lines.
///
/// The identity `render_validator_suffix(v, rs).len() ==
/// validator_suffix_framing_bytes(v, rs) + sum of focus_file_line_bytes` holds
/// by construction: both go through [`render_suffix`], one with the file list
/// and one with none.
///
/// This is the suffix term [`prompt_framing_bytes`] reserves, and the split is
/// what makes that reserve independent of how many files the change carries.
/// The focus-file line is a PER-FILE cost — a batch's suffix lists that batch's
/// files, not the run's — so [`rendered_file_block_bytes`] charges it to the
/// file, where the packer already budgets per file. Measured as run framing it
/// was charged against the run's WHOLE file list in every batch, which both
/// over-reserved for the batch actually being sent and grew the framing without
/// bound as the change grew.
fn validator_suffix_framing_bytes(validator: &ValidatorWork, ruleset: &RuleSet) -> usize {
    render_suffix(validator.validator_name(), ruleset, &[]).len()
}

/// The shared body of [`render_validator_suffix`] and
/// [`validator_suffix_framing_bytes`]: the validator header, mandate, guidance,
/// the focus-file list over `files`, every prompt rule body, and the output
/// contract.
fn render_suffix(validator_name: &str, ruleset: &RuleSet, files: &[FileWork]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{VALIDATOR_HEADER}{validator_name}\n");
    out.push_str(MANDATE_HEADER);
    out.push_str(ruleset.description().trim());
    out.push_str("\n\n");

    render_validator_guidance(&mut out, ruleset.manifest_body());

    render_focus_files(&mut out, files);

    out.push_str("## Rules\n\n");
    // Tool rules never render: no LLM reads a tool rule's body — the tool
    // runner ([`crate::review::tool_rules`]) executes them instead.
    for rule in ruleset.rules.iter().filter(|rule| !rule.is_tool_rule()) {
        let _ = writeln!(out, "### Rule: {}\n", rule.name);
        out.push_str(rule.body.trim());
        out.push_str("\n\n");
    }

    out.push_str(OUTPUT_CONTRACT);
    out.push('\n');
    out
}

/// Append the validator's VALIDATOR.md prose body as a validator-level guidance
/// block, emitted between the [`MANDATE_HEADER`] (the description) and `## Rules`
/// so it is shared by every rule in the validator's fan-out.
///
/// This is authored validator-WIDE direction — intent, scope, and blanket
/// exclusions that apply across all of a validator's rules (e.g. "this validator
/// does not apply to test code"). An empty body emits nothing, keeping the render
/// byte-identical for validators that carry no body (the fork-prefix reuse
/// contract depends on this render being a pure function of its inputs).
fn render_validator_guidance(out: &mut String, body: &str) {
    let body = body.trim();
    if body.is_empty() {
        return;
    }
    out.push_str("## Guidance\n\n");
    out.push_str(body);
    out.push_str("\n\n");
}

/// Append the "files in scope for this validator" list: the paths of the
/// validator's matched files. The contents are in the inherited prime; this just
/// scopes the validator to those files so it does not flag files another
/// validator matched.
fn render_focus_files(out: &mut String, files: &[FileWork]) {
    out.push_str(
        "## Files in scope\n\nApply the rules below to the WHOLE current contents of each \
         file listed here — their complete current source is already provided above. Review \
         every line of these files, not only the lines the change touched: a rule that fires \
         anywhere in one of these files is in scope and must be reported now.\n\n",
    );
    for file in files {
        out.push_str(&focus_file_line(file));
    }
    out.push('\n');
}

/// The one line [`render_focus_files`] spends naming `file`.
///
/// Rendered through one helper so [`focus_file_line_bytes`] measures the bytes
/// the suffix actually writes rather than a second guess at the format.
fn focus_file_line(file: &FileWork) -> String {
    format!("- `{}`\n", file.path())
}

/// The bytes `file` adds to the validator suffix's focus-file list.
///
/// Charged to the file by [`rendered_file_block_bytes`], not reserved as run
/// framing — see [`validator_suffix_framing_bytes`] for why this cost belongs
/// to the file.
///
/// `pub(super)` for the fleet test that adds the per-file lines back onto the
/// reserved suffix framing and checks the sum against the whole rendered
/// suffix.
pub(super) fn focus_file_line_bytes(file: &FileWork) -> usize {
    focus_file_line(file).len()
}

/// The header that opens every file payload, in the prime and in the
/// monolithic fallback alike. Shared by [`render_file_payload`] and
/// [`prompt_framing_bytes`] so the framing reserve counts the same bytes the
/// payload writes.
const FILE_PAYLOAD_HEADER: &str = "# Files under review\n\n";

/// The rendered bytes one file contributes to a prompt.
///
/// Measured by running the real [`render_file_block`] and taking the length, so
/// the number the packer budgets on and the number the agent receives are the
/// same bytes. This is the cost function
/// [`batch_work_list`](crate::review::scope::batch_work_list) is handed; see
/// [`DEFAULT_BATCH_SIZE`](super::DEFAULT_BATCH_SIZE) for why nothing derived from
/// [`FileWork::source_slice`](crate::review::scope::FileWork::source_slice) can
/// stand in for it.
///
/// The cost is per **(validator, file) pair**, not per path: a file's block
/// carries the probe evidence selected for THAT validator, so the same path can
/// cost kilobytes for one validator and megabytes for another.
///
/// The file's [`focus_file_line_bytes`] are charged here too. A monolithic
/// prompt names each of its files twice — once as a block and once in the
/// suffix's focus-file list — and both lines arrive with the file, so the packer
/// charges the pair for both. What this buys is the exact accounting the batch
/// guarantee rests on: a rendered prompt is never more than
/// [`prompt_framing_bytes`] plus the sum of this cost over the batch's files.
pub fn rendered_file_block_bytes(file: &FileWork) -> usize {
    let mut out = String::new();
    render_file_block(&mut out, file);
    out.len() + focus_file_line_bytes(file)
}

/// The header introducing the batch-scoped shared probe evidence section —
/// currently just the `<changed-set>` `duplicates` comparison. Shared by
/// [`render_shared_probe_evidence`] and [`prompt_framing_bytes`] so the
/// framing reserve counts the same bytes the section writes.
const SHARED_EVIDENCE_HEADER: &str = "# Shared evidence\n\n";

/// Render the batch-scoped shared probe evidence — currently just the
/// `<changed-set>` `duplicates` comparison — ONCE per prompt, after the
/// per-file blocks. Emits nothing when `results` is empty, so a prompt with
/// no shared evidence is byte-identical to one rendered without this section
/// at all.
///
/// This evidence spans the WHOLE change under review, not any single file, so
/// rendering it inside every [`render_file_block`] used to repeat the
/// identical, potentially multi-megabyte block once per file in the batch —
/// zero additional information at N× the bytes (^t7f5fqf). Rendering it once
/// here, shared by [`render_run_prime`] and [`render_fleet_prompt`] alike,
/// shows the model the exact same rows at a fraction of the cost.
///
/// # The section is capped
///
/// The rows are bounded by [`MAX_SHARED_EVIDENCE_BYTES`], and the rows past the
/// cap are replaced by a notice naming how many were dropped, so the model never
/// reads a truncated list as an exhaustive one. On a real 15-file change of this
/// repo the uncapped section rendered about 452 KB — 96% of the whole framing —
/// because the `<changed-set>` rows are PAIRWISE and so grow with the square of
/// the changed entity count (^x8z9hgf).
///
/// A fixed cap rather than a per-batch filter of the rows, on purpose. A row
/// names two files that may land in different batches, so filtering by batch
/// drops real evidence; the framing is measured once before batching, so a
/// batch-dependent section would make the measure a fixed point of the packing
/// it feeds; and filtering bounds nothing — one batch's file can still appear in
/// thousands of rows. A constant keeps this section byte-identical across both
/// prompt shapes and across runs over the same change, which the fork-prefix
/// reuse and the ^tsram0q convergence contract both depend on.
pub fn render_shared_probe_evidence(out: &mut String, results: &[ProbeResult]) {
    if results.is_empty() {
        return;
    }
    let start = out.len();
    out.push_str(SHARED_EVIDENCE_HEADER);
    out.push_str(
        "The evidence below spans the WHOLE change under review, not any single \
         file, so it is shown ONCE here rather than repeated per file above. It \
         still applies to any file a row below names.\n\n",
    );
    // Reserve the notice out of the cap before any row renders, so the notice
    // itself can never push the section past it.
    let row_budget =
        MAX_SHARED_EVIDENCE_BYTES.saturating_sub(out.len() - start + MAX_OMITTED_ROWS_NOTICE_BYTES);
    let omitted = render_probe_evidence_within(out, results, false, row_budget);
    if omitted > 0 {
        out.push_str(&omitted_rows_notice(omitted));
    }
}

/// The largest an [`omitted_rows_notice`] can render to, in bytes — the fixed
/// sentence plus the widest row count a `usize` can hold.
///
/// [`render_shared_probe_evidence`] reserves it out of
/// [`MAX_SHARED_EVIDENCE_BYTES`] before it renders any row, so a section that
/// truncates still fits the cap. The reserve is only sound if it really does
/// cover the widest notice, so it is `pub(super)` for the fleet test that pins
/// it against the real rendering.
pub(super) const MAX_OMITTED_ROWS_NOTICE_BYTES: usize = 320;

/// The notice that replaces the rows [`render_shared_probe_evidence`] had to
/// drop, so a truncated block never reads as an exhaustive one.
pub(super) fn omitted_rows_notice(omitted: usize) -> String {
    format!(
        "_{omitted} further evidence rows are NOT shown. This shared block is capped at \
         {MAX_SHARED_EVIDENCE_BYTES} bytes so every batch's prompt fits the agent's prompt \
         cap. Treat the rows above as a sample of the duplicate evidence, not the whole \
         list._\n\n"
    )
}

/// The framing of a batch's prompt — every byte the prompt carries that is NOT
/// a file block — broken into the three terms it is made of.
///
/// A batch sends two prompt shapes, and both are framing plus file blocks:
///
/// - the shared prime — change purpose + payload header + blocks +
///   [`render_shared_probe_evidence`] + [`PRIME_HANDOFF`];
/// - the monolithic per-validator fallback — change purpose + payload header +
///   that validator's blocks + [`render_shared_probe_evidence`] +
///   [`render_validator_suffix`].
///
/// [`total`](Self::total) is the upper bound over both shapes and over every
/// validator in the run. The terms are reported separately because they behave
/// differently and only one of them was ever the problem: on a real 15-file
/// change of this repo the shared evidence measured about 452 KB against about
/// 17 KB of validator suffix and under 1 KB of change purpose (^x8z9hgf).
/// [`run_review`](crate::review::run_review) logs the split, so the next run
/// that gets tight says which term did it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptFraming {
    /// The change-purpose section plus the file-payload header — the fixed
    /// preamble every prompt shape opens with.
    purpose: usize,
    /// The capped [`render_shared_probe_evidence`] section, measured against
    /// the WHOLE work-list's dedup'd shared results
    /// ([`WorkList::shared_probe_results`]) rather than one batch: that
    /// evidence is batch-scoped, not file-scoped, so it costs the same bytes in
    /// every batch's prompt regardless of which files that batch carries.
    shared_evidence: usize,
    /// The largest [`validator_suffix_framing_bytes`] in the run, floored at
    /// [`PRIME_HANDOFF`] — the larger of the two prompt shapes' tails. Excludes
    /// the focus-file lines, which [`rendered_file_block_bytes`] charges to
    /// their file instead.
    validator_suffix: usize,
    /// The validator whose suffix set [`validator_suffix`](Self::validator_suffix),
    /// or `None` when no validator in the work-list has a ruleset in the loader.
    largest_validator: Option<String>,
}

impl PromptFraming {
    /// The change-purpose section plus the file-payload header.
    pub fn purpose(&self) -> usize {
        self.purpose
    }

    /// The capped shared probe evidence section.
    pub fn shared_evidence(&self) -> usize {
        self.shared_evidence
    }

    /// The largest validator suffix, focus-file lines excluded.
    pub fn validator_suffix(&self) -> usize {
        self.validator_suffix
    }

    /// The validator whose suffix set [`validator_suffix`](Self::validator_suffix).
    pub fn largest_validator(&self) -> Option<&str> {
        self.largest_validator.as_deref()
    }

    /// The whole framing — the sum of the three terms.
    pub fn total(&self) -> usize {
        self.purpose + self.shared_evidence + self.validator_suffix
    }
}

/// Measure a batch prompt's framing, term by term.
///
/// Every term runs the real renderer, so the number the packer budgets on and
/// the number the agent receives are the same bytes.
///
/// A validator the `loader` does not know is skipped, exactly as
/// `plan_fan_out` skips it — it never renders a prompt, so it cannot frame
/// one.
pub fn prompt_framing(work: &WorkList, loader: &ValidatorLoader) -> PromptFraming {
    let purpose = CHANGE_PURPOSE_HEADER.len()
        + work.change_purpose().trim().len()
        + "\n\n".len()
        + FILE_PAYLOAD_HEADER.len();

    let mut shared_evidence = String::new();
    render_shared_probe_evidence(&mut shared_evidence, &work.shared_probe_results());

    let largest = work
        .validators()
        .iter()
        .filter_map(|validator| {
            let ruleset = loader.get_ruleset(validator.validator_name())?;
            Some((
                validator_suffix_framing_bytes(validator, ruleset),
                validator.validator_name(),
            ))
        })
        .max_by_key(|(bytes, _)| *bytes);

    PromptFraming {
        purpose,
        shared_evidence: shared_evidence.len(),
        // Floored at the prime's own tail: the prime carries the handoff where
        // a fork carries a suffix, so the reserve must cover whichever is
        // larger.
        validator_suffix: largest
            .map_or(0, |(bytes, _)| bytes)
            .max(PRIME_HANDOFF.len()),
        largest_validator: largest.map(|(_, name)| name.to_string()),
    }
}

/// The bytes of a batch's prompt that are NOT file blocks — an upper bound over
/// every validator in the run, and the whole of [`prompt_framing`].
///
/// This is the number [`FleetConfig::file_payload_budget`](super::FleetConfig::file_payload_budget)
/// subtracts from the agent's prompt cap to size a batch. It is bounded by
/// [`MAX_FRAMING_BYTES`](super::MAX_FRAMING_BYTES), so a single-file batch
/// carrying a file at the per-file cap still fits the prompt cap.
pub fn prompt_framing_bytes(work: &WorkList, loader: &ValidatorLoader) -> usize {
    prompt_framing(work, loader).total()
}

/// Render the file payload — one self-contained block per file (path + semantic
/// diff + bounded source slice + probe evidence). Used by the run prime (every
/// distinct file) and the monolithic fallback (one validator's files).
pub fn render_file_payload(files: &[FileWork]) -> String {
    let mut out = String::new();
    out.push_str(FILE_PAYLOAD_HEADER);
    for file in files {
        render_file_block(&mut out, file);
    }
    out
}

/// Append one file's review block: path, the full current source, the semantic
/// diff of what changed, and the probe results rendered as evidence.
///
/// The changed file is always handed to the model **in full** — framed explicitly
/// as the complete current contents the model does NOT need to re-read, because
/// the read-round-trips that dominated review wall-clock came from the model
/// re-reading a file it was only given a partial slice of. A file whose rendered
/// block would exceed the per-file cap never reaches here as a partial view:
/// [`batch_work_list`](crate::review::scope::batch_work_list) excludes it and
/// reports it as a [`SkippedFile`](crate::review::scope::SkippedFile) gap instead
/// of trimming it to a slice.
fn render_file_block(out: &mut String, file: &FileWork) {
    let _ = writeln!(out, "## File: {}\n", file.path());

    out.push_str(
        "### Full current contents\n\n\
         This is the COMPLETE current source of the file. You do not need to read this \
         file — it is provided here in full. Review it directly. This whole file is the \
         review boundary: report every place a rule fires anywhere in it, including \
         pre-existing instances that sit outside the change described below.\n\n",
    );
    render_numbered_source(out, file);

    out.push_str(
        "### What changed (semantic diff — orientation only, NOT the review boundary)\n\n",
    );
    out.push_str(
        "The entities below are what this change touched, to orient you. They are context, \
         not the review scope: do NOT limit findings to these lines. Review the whole file \
         above and report every instance of every rule, changed or pre-existing.\n\n",
    );
    render_semantic_diff(out, file);

    out.push_str("### Probe evidence\n\n");
    render_probe_evidence(out, file.probe_results(), false);
}

/// The legend printed above every numbered source block, explaining the
/// `{line:>6} | {sha:8} {mark} | {text}` layout [`render_numbered_source`]
/// writes. Explicit about reading the printed number rather than counting
/// lines — the failure mode this whole format exists to close (an LLM
/// estimating a line number by counting drifts further wrong the deeper into
/// the file the cited line sits).
const LINE_FORMAT_LEGEND: &str = "\
Each line below is numbered and shows the commit that last changed it, in this \
exact layout: `{line:>6} | {sha:8} {mark} | {text}`.

- `line` — the 1-based line number. READ this number for any `Finding.line` \
you report — do NOT count lines yourself; counting is exactly how a cited \
line number drifts wrong, worse the deeper into the file it sits.
- `sha` — the first 8 characters of the commit that last changed the line, or \
`worktree` (an uncommitted line — including every line of a brand-new file), \
`untrackd` (git does not track this file), or `????????` (blame could not be \
determined).
- `mark` — `+` when THIS review's change touched the line, a space when it \
did not. A rule that excludes pre-existing code reads this mark, never the \
sha (the sha attributes a line to a commit; only the mark says whether this \
change touched it).
- Everything after the second `|` is the unmodified source line, exactly as \
it appears in the file.

";

/// Append `file`'s source as a numbered, blame-annotated block — one line of
/// [`LINE_FORMAT_LEGEND`]'s `{line:>6} | {sha:8} {mark} | {text}` layout per
/// source line, inside a fenced code block.
///
/// `file.source_slice().trim_end()` is the SAME content
/// [`crate::review::scope::compute_line_annotations`] (the scope stage) used
/// to compute [`FileWork::line_annotations`] — trimmed identically in both
/// places — so line `i` of this render is guaranteed to be annotation `i`,
/// never off-by-one. Delegates to [`render_numbered_lines`], the shared
/// renderer the verify stage also uses (see that function's docs for why: a
/// verifier that cannot read the SAME numbered source the fan-out agent saw
/// has no way to check a finding's cited line against what is actually there).
fn render_numbered_source(out: &mut String, file: &FileWork) {
    render_numbered_lines(out, file.source_slice(), file.line_annotations());
}

/// Append `source` as a numbered, blame-annotated block — one line of
/// [`LINE_FORMAT_LEGEND`]'s `{line:>6} | {sha:8} {mark} | {text}` layout per
/// source line, inside a fenced code block. An empty (trimmed) `source` renders
/// the bare empty fence: no legend, no numbering, nothing to number.
///
/// Shared by two call sites that both need the model to read a `Finding.line`
/// off a real printed number rather than guess one:
///
/// - [`render_numbered_source`] — the fan-out prime, over a [`FileWork`]'s
///   `source_slice` + `line_annotations`.
/// - [`crate::review::verify::render_verify_prompt`] — the adversarial verify
///   prompt, over a `Candidate`'s own `source_slice` + `line_annotations` (the
///   SAME pair the work-list attached to that file, carried through
///   `build_candidates`). Before this was shared, verify rendered the source as
///   a bare, unnumbered fence, so the adversary had no printed line number to
///   check a finding's cited `line` against — it could only judge whether the
///   CLAIM was plausible somewhere in the file, never whether the citation
///   itself pointed at the right place. Giving verify the identical numbered
///   view closes that gap: the adversary can now read line `N` off the same
///   block the fan-out agent read it from and refute a finding whose citation
///   does not match.
///
/// `annotations[i]` is assumed to correspond to `source`'s line `i` (both
/// derived from the same trimmed content upstream); a missing entry — should
/// not happen once every producer always attaches one per line — falls back to
/// the same `????????` sentinel a blame failure uses, never a panic on an
/// out-of-bounds index.
pub(crate) fn render_numbered_lines(
    out: &mut String,
    source: &str,
    annotations: &[LineAnnotation],
) {
    let source = source.trim_end();
    if source.is_empty() {
        out.push_str("```\n\n```\n\n");
        return;
    }

    out.push_str(LINE_FORMAT_LEGEND);
    out.push_str("```\n");
    for (i, text) in source.lines().enumerate() {
        let line = i + 1;
        let (sha, mark) = match annotations.get(i) {
            Some(annotation) => (
                annotation.sha().to_string(),
                if annotation.touched() { '+' } else { ' ' },
            ),
            None => ("????????".to_string(), ' '),
        };
        let _ = writeln!(out, "{line:>6} | {sha:8} {mark} | {text}");
    }
    out.push_str("```\n\n");
}

/// Append the structured semantic diff for a file as a list of changed entities.
fn render_semantic_diff(out: &mut String, file: &FileWork) {
    if file.semantic_diff().is_empty() {
        out.push_str("_No structured entity changes._\n\n");
        return;
    }
    for change in file.semantic_diff() {
        let _ = writeln!(
            out,
            "- {} {} `{}`",
            change.change_type, change.entity_type, change.entity_name
        );
    }
    out.push('\n');
}
