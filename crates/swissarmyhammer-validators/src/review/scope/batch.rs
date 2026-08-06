//! Batching — split a scoped work-list into budgeted, whole-file batches.
//!
//! [`batch_work_list`] packs (validator, file) pairs into batches whose
//! rendered payload fits the caller's byte budget; a pair whose rendered block
//! alone exceeds the budget becomes a [`SkippedFile`] gap, never a hard error.

use std::collections::{BTreeMap, BTreeSet};

use super::{FileWork, ValidatorWork, WorkList};

/// A (validator, file) pair [`batch_work_list`] could not pack into any batch
/// because the file's RENDERED block alone exceeds the batch budget.
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
    /// The per-batch rendered budget it exceeded, in bytes.
    budget: usize,
}

impl SkippedFile {
    /// Construct a [`SkippedFile`] directly for a synthesis-layer test fixture
    /// (`crate::review::synthesize`'s tests), which asserts on rendering given a
    /// skip list rather than driving the whole packer to produce one.
    #[cfg(test)]
    pub(crate) fn for_test(path: &str, validator: &str, size: usize, budget: usize) -> Self {
        Self {
            path: path.to_string(),
            validator: validator.to_string(),
            size,
            budget,
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

    /// The per-batch rendered budget it exceeded, in bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }
}

/// Split a [`WorkList`] into budgeted batches at **whole-file** granularity, so
/// every prompt a batch sends stays inside `budget` bytes of file content.
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
/// 1. Any pair whose own cost exceeds `budget` is dropped and reported as a
///    [`SkippedFile`]. It could not be packed without either splitting the file
///    (forbidden) or blowing the budget, and dropping the whole PATH would cost
///    every other validator a file it could easily afford.
/// 2. The surviving distinct files are packed greedily in
///    [`WorkList::distinct_files`] order (the order the prime renders them),
///    each charged the LARGEST surviving cost any validator has for it — the
///    bound that covers both the shared prime and any one validator's
///    monolithic fallback.
///
/// Each returned [`WorkList`] carries every validator that has at least one file
/// in that batch, with the validator's files filtered to the batch (validators
/// left with no files in a batch are dropped). The change purpose is carried
/// verbatim so every batch's prime frames the same overall change. A work-list
/// with no files (no validator matched) yields no batches.
///
/// This never errors: a caller that wants a hard stop on an oversized file
/// checks the returned skip list itself.
pub fn batch_work_list(
    work: &WorkList,
    budget: usize,
    cost: &dyn Fn(&FileWork) -> usize,
) -> (Vec<WorkList>, Vec<SkippedFile>) {
    // Step 1: cost every (validator, file) pair once, dropping the pairs no
    // batch could ever carry and keeping the largest surviving cost per path.
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let mut affordable: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut path_cost: BTreeMap<&str, usize> = BTreeMap::new();
    for validator in &work.validators {
        for file in &validator.files {
            let size = cost(file);
            if size > budget {
                skipped.push(SkippedFile {
                    path: file.path.clone(),
                    validator: validator.validator_name.clone(),
                    size,
                    budget,
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
        if !current.is_empty() && current_bytes + size > budget {
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
    }
}
