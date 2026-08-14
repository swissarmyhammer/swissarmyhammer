//! Batching — split a scoped work-list into budgeted, whole-file batches.
//!
//! [`batch_work_list`] packs (validator, file) pairs into batches whose
//! rendered payload fits the caller's [`BatchBudget`]; a pair whose rendered
//! block alone exceeds the budget's per-file cap becomes a [`SkippedFile`] gap,
//! never a hard error.

use std::collections::{BTreeMap, BTreeSet};

use super::{FileWork, ValidatorWork, WorkList};

/// The largest RENDERED block one (validator, file) pair may contribute to a
/// prompt, in bytes. A pair over it is a [`SkippedFile`].
///
/// Newtype over `usize` so [`BatchBudget::new`]'s two byte counts cannot be
/// transposed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileCapBytes(pub usize);

/// How many RENDERED bytes of file blocks one batch's prompt may carry.
///
/// Newtype over `usize` so [`BatchBudget::new`]'s two byte counts cannot be
/// transposed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatchBytes(pub usize);

/// The two byte limits [`batch_work_list`] packs against.
///
/// They are deliberately two numbers, because they answer two different
/// questions:
///
/// - `file_cap` decides the over-cap **verdict**: is this one file too large to
///   review at all? That answer must depend on the FILE alone. The caller
///   derives it from constants
///   ([`FleetConfig::file_block_cap`](crate::review::fleet::FleetConfig::file_block_cap)),
///   never from how many files the change carries.
/// - `batch_bytes` decides where batch **boundaries** fall: how much fits
///   alongside this run's measured prompt framing. That answer legitimately
///   moves with the run, because the framing does.
///
/// One number served both jobs before (^tsram0q) and the two answers could not
/// both be right: an over-cap finding tells the author to split the file, the
/// split grows the change, a bigger change renders more framing, the smaller
/// remainder puts MORE files over cap. Splitting made the next round worse, so
/// the loop never converged. Separating the numbers is what breaks it — a file
/// that satisfied the cap can only go over it by growing.
///
/// `batch_bytes` may be smaller than `file_cap` when the framing is large. A
/// file between the two is not over cap: the packer gives it a batch of its
/// own, which is the smallest prompt that can carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatchBudget {
    /// The constant per-file cap; the over-cap verdict is measured against it.
    file_cap: usize,
    /// The per-batch packing budget; batch boundaries are measured against it.
    batch_bytes: usize,
}

impl BatchBudget {
    /// Pair a constant per-file cap with a per-batch packing budget.
    pub fn new(file_cap: FileCapBytes, batch_bytes: BatchBytes) -> Self {
        Self {
            file_cap: file_cap.0,
            batch_bytes: batch_bytes.0,
        }
    }

    /// The largest rendered block one (validator, file) pair may contribute,
    /// in bytes. A pair over it is reported as a [`SkippedFile`].
    pub fn file_cap(&self) -> usize {
        self.file_cap
    }

    /// The rendered file-block bytes one batch may carry.
    pub fn batch_bytes(&self) -> usize {
        self.batch_bytes
    }
}

/// A (validator, file) pair [`batch_work_list`] could not pack into any batch
/// because the file's RENDERED block alone exceeds the per-file cap.
///
/// A file is atomic — it is never split across batches — so an oversized block
/// cannot be packed at all. Rather than a hard error that would block review of
/// every OTHER file in the scope, `batch_work_list` excludes the pair and
/// reports it here; [`run_review`](crate::review::run_review) carries it through
/// to the final [`ReviewReport`](crate::review::ReviewReport) as a named "not
/// reviewed, too large" gap.
///
/// The gap is a **pair**, not a path. A file's rendered block carries the probe
/// evidence selected for one validator, so the same path can cost kilobytes for
/// one validator and megabytes for another; dropping the path from the batch
/// would cost every other validator a file it could easily afford. The fields
/// are private (read through the getters) so the shape can evolve without a
/// field-level API commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    /// The oversized file's repo-relative path.
    path: String,
    /// The validator whose rendering of the file did not fit.
    validator: String,
    /// The rendered size of that validator's block for the file, in bytes.
    size: usize,
    /// The per-file cap it exceeded, in bytes.
    cap: usize,
}

impl SkippedFile {
    /// Construct a [`SkippedFile`] directly for a synthesis-layer test fixture
    /// (`crate::review::synthesize`'s tests), which asserts on rendering given a
    /// skip list rather than driving the whole packer to produce one.
    #[cfg(test)]
    pub(crate) fn for_test(path: &str, validator: &str, size: usize, cap: usize) -> Self {
        Self {
            path: path.to_string(),
            validator: validator.to_string(),
            size,
            cap,
        }
    }

    /// The oversized file's repo-relative path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The validator whose rendering of the file did not fit.
    pub fn validator(&self) -> &str {
        &self.validator
    }

    /// The rendered size of that validator's block for the file, in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The per-file cap it exceeded, in bytes.
    pub fn cap(&self) -> usize {
        self.cap
    }
}

/// Split a [`WorkList`] into budgeted batches at **whole-file** granularity, so
/// every prompt a batch sends stays inside the [`BatchBudget`].
///
/// Cramming every changed file into one shared prime overflows the review
/// model's context on a large diff — every fan-out validator then fails
/// uniformly. So the run is split into batches and each batch fans out
/// independently. A file is **atomic**: it is never split across batches.
///
/// # The cost function is the budget's unit
///
/// `cost` measures what one [`FileWork`] contributes to a prompt, and the
/// budget is denominated in whatever it returns. The fleet passes
/// [`rendered_file_block_bytes`](crate::review::fleet::rendered_file_block_bytes),
/// which renders the block through the very renderer the prompt uses, so the
/// measured bytes and the sent bytes are the same bytes. Taking it as a
/// parameter is what keeps this stage (stage 1, deterministic) from having to
/// know how the fleet stage (stage 2) formats a block, while still budgeting
/// the real thing rather than a proxy for it.
///
/// # Pairs, then paths
///
/// The cost is per **(validator, file) pair** — a block carries the probe
/// evidence selected for that validator, so the same path can cost kilobytes
/// for one validator and megabytes for another. So:
///
/// 1. Any pair whose own cost exceeds [`BatchBudget::file_cap`] is dropped and
///    reported as a [`SkippedFile`]. It could not be packed without splitting
///    the file (forbidden), and dropping the whole PATH would cost every other
///    validator a file it could easily afford.
/// 2. The surviving distinct files are packed greedily in
///    [`WorkList::distinct_files`] order (the order the prime renders them)
///    against [`BatchBudget::batch_bytes`], each charged the LARGEST surviving
///    cost any validator has for it — the bound that covers both the shared
///    prime and any one validator's monolithic fallback. A file larger than
///    that budget still satisfied the cap, so it takes a batch of its own
///    rather than being dropped.
///
/// Each returned [`WorkList`] carries every validator that has at least one file
/// in that batch, with the validator's files filtered to the batch (validators
/// left with no files in a batch are dropped). The change purpose is carried
/// verbatim so every batch's prime frames the same overall change. A work-list
/// with no files (no validator matched) yields no batches.
///
/// This never errors: a caller that wants a hard stop on an oversized file
/// checks the returned skip list itself.
pub fn batch_work_list<F: Fn(&FileWork) -> usize>(
    work: &WorkList,
    budget: BatchBudget,
    cost: F,
) -> (Vec<WorkList>, Vec<SkippedFile>) {
    // Step 1: cost every (validator, file) pair once, dropping the pairs over
    // the per-file cap and keeping the largest surviving cost per path.
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let mut affordable: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut path_cost: BTreeMap<&str, usize> = BTreeMap::new();
    for validator in &work.validators {
        for file in &validator.files {
            let size = cost(file);
            if size > budget.file_cap() {
                skipped.push(SkippedFile {
                    path: file.path.clone(),
                    validator: validator.validator_name.clone(),
                    size,
                    cap: budget.file_cap(),
                });
                continue;
            }
            affordable.insert((validator.validator_name.as_str(), file.path.as_str()));
            let entry = path_cost.entry(file.path.as_str()).or_insert(0);
            *entry = (*entry).max(size);
        }
    }

    // Step 2: pack the surviving distinct files (first-seen order, matching the
    // prime's file set); a file is never split across a batch boundary.
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_bytes = 0usize;
    for file in work.distinct_files() {
        let Some(&size) = path_cost.get(file.path.as_str()) else {
            continue;
        };
        if !current.is_empty() && current_bytes + size > budget.batch_bytes() {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(file.path.clone());
        current_bytes += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }

    let batches = batches
        .into_iter()
        .map(|paths| project_onto_files(work, &paths, &affordable))
        .collect();
    (batches, skipped)
}

/// Project a [`WorkList`] onto a subset of file paths: keep every validator that
/// has at least one file in `paths`, with its files filtered to `paths` (order
/// preserved) AND to the `affordable` (validator, path) pairs. Validators left
/// with no files are dropped. The change purpose is carried verbatim so the
/// batch's prime still frames the whole change.
///
/// The pair filter is what keeps a dropped pair out of the batch entirely —
/// including out of [`WorkList::distinct_files`], which the prime renders from
/// and which would otherwise pick the very [`FileWork`] whose rendering did not
/// fit.
fn project_onto_files(
    work: &WorkList,
    paths: &[String],
    affordable: &BTreeSet<(&str, &str)>,
) -> WorkList {
    let keep: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    let validators = work
        .validators
        .iter()
        .filter_map(|validator| {
            let files: Vec<FileWork> = validator
                .files
                .iter()
                .filter(|file| keep.contains(file.path.as_str()))
                .filter(|file| {
                    affordable.contains(&(validator.validator_name.as_str(), file.path.as_str()))
                })
                .cloned()
                .collect();
            if files.is_empty() {
                return None;
            }
            Some(ValidatorWork {
                validator_name: validator.validator_name.clone(),
                rules: validator.rules.clone(),
                probes: validator.probes.clone(),
                files,
                // Carried verbatim, never re-filtered by `paths`: this
                // evidence is batch-scoped (spans the WHOLE change), not
                // file-scoped, so it does not shrink when a batch subsets the
                // work-list's files.
                shared_probe_results: validator.shared_probe_results.clone(),
            })
        })
        .collect();
    WorkList {
        change_purpose: work.change_purpose.clone(),
        validators,
        // Empty by design: the scope stage's exclusions are a RUN-level fact
        // that rides on the work-list these batches are projected from, and
        // `run_review` reads them from there. Copying them onto every batch
        // would report the same excluded file once for each batch.
        excluded: Vec::new(),
        // Carried verbatim: a batch is a subset of the run's files, never a
        // different question, so every batch REVIEWS what the run's op named.
        subject: work.subject,
    }
}
