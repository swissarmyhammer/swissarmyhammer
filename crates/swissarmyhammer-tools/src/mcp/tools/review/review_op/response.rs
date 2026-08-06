//! The wire types a `review file/working/sha` op returns: the rendered
//! markdown plus the per-verdict counts, mapped from the engine's
//! [`ReviewReport`].

use serde::Serialize;

use swissarmyhammer_validators::review::ReviewReport;

/// The JSON shape returned for a `review file/working/sha` op: the rendered
/// markdown plus the per-verdict counts.
///
/// The fields are private (read through the getters); serde serializes them by
/// their field names, so the wire shape is unchanged by the encapsulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    markdown: String,
    /// The per-verdict tallies.
    counts: ReviewCountsView,
}

impl ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The per-verdict tallies.
    pub fn counts(&self) -> &ReviewCountsView {
        &self.counts
    }
}

/// The serializable view of the engine's review counts.
///
/// Review is binary pass/fail — there is no graded severity — so the rendered
/// failures are a single `findings` count, not a per-tier breakdown. The fields
/// are private (read through the getters); serde serializes them by their field
/// names, so the wire shape is unchanged by the encapsulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewCountsView {
    /// Confirmed findings rendered into the checklist (post-dedup).
    findings: usize,
    /// Findings the verifier confirmed.
    confirmed: usize,
    /// Findings the verifier refuted.
    refuted: usize,
    /// How many fan-out review tasks were attempted.
    attempted: usize,
    /// How many fan-out review tasks failed and degraded to zero findings. A
    /// non-zero value means the rendered findings are INCOMPLETE.
    failed: usize,
    /// How many changed files were excluded from review because their inlined
    /// rendered block alone exceeded the batch budget. A non-zero value means
    /// the review cannot be clean: each skipped file also becomes a CONFIRMED
    /// finding, and the markdown names each one as a "not reviewed, too large"
    /// gap.
    skipped: usize,
    /// The skipped file paths — distinct, sorted. The structured twin of
    /// `skipped`: orchestrators gate on this list without parsing markdown.
    skipped_files: Vec<String>,
}

impl ReviewCountsView {
    /// Confirmed findings rendered into the checklist (post-dedup).
    pub fn findings(&self) -> usize {
        self.findings
    }

    /// Findings the verifier confirmed.
    pub fn confirmed(&self) -> usize {
        self.confirmed
    }

    /// Findings the verifier refuted.
    pub fn refuted(&self) -> usize {
        self.refuted
    }

    /// How many fan-out review tasks were attempted.
    pub fn attempted(&self) -> usize {
        self.attempted
    }

    /// How many fan-out review tasks failed and degraded to zero findings. A
    /// non-zero value means the rendered findings are INCOMPLETE.
    pub fn failed(&self) -> usize {
        self.failed
    }

    /// How many changed files were excluded from review because their inlined
    /// rendered block alone exceeded the batch budget. A non-zero value means
    /// the review cannot be clean: each skipped file also becomes a CONFIRMED
    /// finding, and the markdown names each one as a "not reviewed, too large"
    /// gap.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// The skipped file paths — distinct, sorted. The structured twin of
    /// [`ReviewCountsView::skipped`]: orchestrators gate on this list without
    /// parsing markdown.
    pub fn skipped_files(&self) -> &[String] {
        &self.skipped_files
    }
}

/// Maps the engine's internal [`ReviewReport`] onto the tool-boundary wire
/// type [`ReviewResponse`]: the report's rendered markdown is taken as-is, and
/// its counts are re-shaped into the serializable [`ReviewCountsView`].
impl From<ReviewReport> for ReviewResponse {
    fn from(report: ReviewReport) -> Self {
        let counts = report.counts().clone();
        ReviewResponse {
            markdown: report.into_markdown(),
            counts: ReviewCountsView {
                findings: counts.findings(),
                confirmed: counts.confirmed(),
                refuted: counts.refuted(),
                attempted: counts.tasks_attempted(),
                failed: counts.tasks_failed(),
                skipped: counts.skipped(),
                skipped_files: counts.skipped_files().to_vec(),
            },
        }
    }
}
