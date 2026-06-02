//! Streaming splitter that routes the model's raw output into two channels:
//! *visible* message text and *thought* reasoning text.
//!
//! The model's raw output interleaves natural-language text with ChatML-style
//! control spans — `<think>…</think>` reasoning and `<tool_call>…</tool_call>`
//! blocks (the Qwen convention). The full raw text still flows to
//! `ChatTemplateEngine::extract_tool_calls` (which turns `<tool_call>` blocks
//! into structured ACP `ToolCall`s) and to the response meta. But here we
//! separate the streams so each lands on the right ACP notification:
//!
//! - `<think>` content goes to the **thought** stream (rendered as
//!   `SessionUpdate::AgentThoughtChunk` by the caller).
//! - `<tool_call>` content is dropped from the visible stream entirely — the
//!   structured `ToolCall` is the only representation a client should see.
//! - Everything else is **visible** text (rendered as
//!   `SessionUpdate::AgentMessageChunk`).
//!
//! Generation streams token-by-token, and a control tag can be split across
//! chunks (`<tool` then `_call>`), so this is a small state machine that buffers
//! just enough to recognize a tag straddling a chunk boundary.
//!
//! Output ordering: `push` returns a list of [`FilterSegment`]s in the order
//! the source text produced them, so a single chunk like
//! `"reply <think>r</think> more"` emits `Visible("reply ")`,
//! `Thought("r")`, `Visible(" more")` — and the caller broadcasts them in
//! that order. Aggregating visible/thought into two flat strings per push
//! would lose this ordering and surface reasoning AFTER its surrounding
//! text in the UI, which is what the user observed before this fix.
//!
//! Unterminated `<think>` spans (the model exhausts its budget mid-`<think>`)
//! ARE surfaced as thought content on `finish()` — the user wants to see what
//! the model was reasoning about even if it didn't get to write the final
//! answer. Unterminated `<tool_call>` spans are dropped: a partial tool-call
//! body has no safe representation (we can't execute or display it).

/// The (open, close) tag pairs whose spans are *routed*. Routing depends on
/// `SuppressedKind`: `Think` content streams to the thought channel; `ToolCall`
/// content is dropped.
const SUPPRESSED_SPANS: &[(&str, &str, SuppressedKind)] = &[
    ("<think>", "</think>", SuppressedKind::Think),
    ("<tool_call>", "</tool_call>", SuppressedKind::ToolCall),
];

/// What to do with content inside a recognized suppression span.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SuppressedKind {
    /// Reasoning — route to the thought stream (visible to the user as a
    /// thought chunk).
    Think,
    /// Tool call — drop entirely (the structured tool call is surfaced via
    /// a separate notification path).
    ToolCall,
}

/// One ordered slice of output from `push`/`finish`. Segments appear in the
/// order the underlying source text produced them, so the caller can
/// broadcast each one in turn and preserve the original chronology of
/// visible text vs. reasoning vs. (dropped) tool-call markup.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum FilterSegment {
    /// Plain assistant message text — broadcast as `AgentMessageChunk`.
    Visible(String),
    /// Reasoning text from a `<think>` span — broadcast as `AgentThoughtChunk`.
    Thought(String),
}

impl FilterSegment {
    fn is_empty(&self) -> bool {
        match self {
            FilterSegment::Visible(s) | FilterSegment::Thought(s) => s.is_empty(),
        }
    }
}

/// Stateful, streaming splitter of `<think>` and `<tool_call>` spans. Feed each
/// generated chunk to [`push`](Self::push) and broadcast every segment it
/// returns in order on the matching ACP stream; call [`finish`](Self::finish)
/// once the stream ends to flush any trailing text.
#[derive(Default)]
pub(crate) struct VisibleTextFilter {
    /// When `Some((close, kind))`, we are inside a suppression span: `close`
    /// is the tag we're searching for and `kind` decides what happens to the
    /// inner content (route to thought / drop).
    inside: Option<(&'static str, SuppressedKind)>,
    /// Text held back because it might be the start of a tag straddling the
    /// next chunk boundary (a proper prefix of an open tag when outside, or of
    /// the close tag when inside).
    buf: String,
}

impl VisibleTextFilter {
    /// Feed the next chunk of raw generated text; returns the segments to
    /// broadcast, *in source order*. May return an empty vector when the
    /// filter is only holding back a possible-tag-start suffix.
    pub(crate) fn push(&mut self, text: &str) -> Vec<FilterSegment> {
        self.buf.push_str(text);
        let mut out: Vec<FilterSegment> = Vec::new();

        loop {
            match self.inside {
                None => {
                    if let Some((idx, open, close, kind)) = earliest_open(&self.buf) {
                        // Everything before the open tag is visible; the open
                        // tag itself is consumed and we enter the span.
                        if idx > 0 {
                            push_visible(&mut out, &self.buf[..idx]);
                        }
                        self.buf.drain(..idx + open.len());
                        self.inside = Some((close, kind));
                        continue;
                    }
                    // No complete open tag. Emit everything except a trailing
                    // suffix that could be the start of one.
                    let keep = longest_open_prefix_suffix(&self.buf);
                    let emit_to = self.buf.len() - keep;
                    if emit_to > 0 {
                        push_visible(&mut out, &self.buf[..emit_to]);
                    }
                    self.buf.drain(..emit_to);
                    break;
                }
                Some((close, kind)) => {
                    if let Some(idx) = self.buf.find(close) {
                        // Inner content [0..idx] is routed by kind; the close
                        // tag itself is consumed.
                        if idx > 0 {
                            route_inner(&mut out, kind, &self.buf[..idx]);
                        }
                        self.buf.drain(..idx + close.len());
                        self.inside = None;
                        continue;
                    }
                    // Still inside: route all but a trailing suffix that could
                    // be the start of the close tag.
                    let keep = longest_prefix_suffix(&self.buf, close);
                    let emit_to = self.buf.len() - keep;
                    if emit_to > 0 {
                        route_inner(&mut out, kind, &self.buf[..emit_to]);
                    }
                    self.buf.drain(..emit_to);
                    break;
                }
            }
        }

        out.retain(|s| !s.is_empty());
        out
    }

    /// Flush at end of stream.
    ///
    /// - Outside a span: any leftover buffered text is real (no tag ever
    ///   completed) → emit as visible.
    /// - Inside a `<think>` span: the span never closed (the model ran out of
    ///   budget mid-reasoning). Surface the buffered reasoning as a thought
    ///   chunk so the user can see what the model was working on — silently
    ///   dropping it is the bug card 01KSXAVM5Y2B0PMXQ4BR656NDR fixed.
    /// - Inside a `<tool_call>` span: drop. A truncated tool-call body has no
    ///   safe representation — we can neither execute it nor display the raw
    ///   JSON without confusing the UI.
    pub(crate) fn finish(&mut self) -> Vec<FilterSegment> {
        let mut out: Vec<FilterSegment> = Vec::new();
        match self.inside {
            None => {
                let buf = std::mem::take(&mut self.buf);
                if !buf.is_empty() {
                    out.push(FilterSegment::Visible(buf));
                }
            }
            Some((_close, SuppressedKind::Think)) => {
                let buf = std::mem::take(&mut self.buf);
                if !buf.is_empty() {
                    out.push(FilterSegment::Thought(buf));
                }
            }
            Some((_close, SuppressedKind::ToolCall)) => {
                self.buf.clear();
            }
        }
        self.inside = None;
        out
    }
}

/// Append a visible run to `out`. Merges with the previous segment if it was
/// also visible — keeps the segment list compact when many tiny token chunks
/// arrive outside any span.
fn push_visible(out: &mut Vec<FilterSegment>, text: &str) {
    if let Some(FilterSegment::Visible(existing)) = out.last_mut() {
        existing.push_str(text);
    } else {
        out.push(FilterSegment::Visible(text.to_string()));
    }
}

/// Append `text` to the appropriate side of `out` based on the span kind,
/// merging consecutive segments of the same kind so adjacent runs become a
/// single broadcast.
fn route_inner(out: &mut Vec<FilterSegment>, kind: SuppressedKind, text: &str) {
    match kind {
        SuppressedKind::Think => {
            if let Some(FilterSegment::Thought(existing)) = out.last_mut() {
                existing.push_str(text);
            } else {
                out.push(FilterSegment::Thought(text.to_string()));
            }
        }
        SuppressedKind::ToolCall => {}
    }
}

/// Find the earliest complete open tag in `buf`, returning its byte index,
/// the matching open/close strings, and the kind of span it opens.
fn earliest_open(buf: &str) -> Option<(usize, &'static str, &'static str, SuppressedKind)> {
    SUPPRESSED_SPANS
        .iter()
        .filter_map(|(open, close, kind)| buf.find(open).map(|idx| (idx, *open, *close, *kind)))
        .min_by_key(|(idx, _, _, _)| *idx)
}

/// Length (bytes) of the longest suffix of `buf` that is a proper prefix of any
/// open tag — i.e. text we must hold back because the rest of the tag may
/// arrive in the next chunk. All tags are ASCII so byte lengths land on char
/// boundaries.
fn longest_open_prefix_suffix(buf: &str) -> usize {
    SUPPRESSED_SPANS
        .iter()
        .map(|(open, _, _)| longest_prefix_suffix(buf, open))
        .max()
        .unwrap_or(0)
}

/// Length (bytes) of the longest suffix of `buf` that is a *proper* prefix of
/// `tag` (length 1..tag.len()). Returns 0 if none.
fn longest_prefix_suffix(buf: &str, tag: &str) -> usize {
    let max_k = buf.len().min(tag.len() - 1);
    (1..=max_k)
        .rev()
        .find(|&k| buf.as_bytes().ends_with(&tag.as_bytes()[..k]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a sequence of chunks through the filter and return the full
    /// ordered segment list (pushes concatenated, then finish()).
    fn run_segments(chunks: &[&str]) -> Vec<FilterSegment> {
        let mut f = VisibleTextFilter::default();
        let mut out: Vec<FilterSegment> = Vec::new();
        for c in chunks {
            out.extend(f.push(c));
        }
        out.extend(f.finish());
        out
    }

    /// Collapse a segment list back to (visible, thought) — preserves chronology
    /// of the kinds against each other but flattens within-kind. Useful for
    /// assertions that care about totals, not ordering.
    fn run(chunks: &[&str]) -> (String, String) {
        let mut visible = String::new();
        let mut thought = String::new();
        for seg in run_segments(chunks) {
            match seg {
                FilterSegment::Visible(s) => visible.push_str(&s),
                FilterSegment::Thought(s) => thought.push_str(&s),
            }
        }
        (visible, thought)
    }

    /// Convenience for tests that only care about visible output.
    fn run_visible(chunks: &[&str]) -> String {
        run(chunks).0
    }

    #[test]
    fn plain_text_passes_through_unchanged() {
        assert_eq!(run_visible(&["hello world"]), "hello world");
        assert_eq!(run_visible(&["hel", "lo ", "wor", "ld"]), "hello world");
    }

    #[test]
    fn complete_think_routes_to_thought_not_visible() {
        let (visible, thought) = run(&["<think>reasoning here</think>answer"]);
        assert_eq!(visible, "answer");
        assert_eq!(thought, "reasoning here");
    }

    #[test]
    fn strips_a_complete_tool_call_span() {
        let (visible, thought) = run(&["<tool_call>{\"name\":\"kanban\"}</tool_call>"]);
        assert_eq!(visible, "");
        assert_eq!(thought, "");
    }

    #[test]
    fn keeps_text_around_a_span() {
        let (visible, _) = run(&["before <think>x</think> after"]);
        assert_eq!(visible, "before  after");
    }

    #[test]
    fn reproduces_the_reported_leak_cleanly() {
        // The exact shape the user saw: empty think + a tool_call block. The
        // visible output must contain neither tag nor the JSON payload.
        let raw = "<think>\n\n</think>\n\n<tool_call>\n{\"name\": \"kanban\", \"arguments\": {\"op\": \"get board\"}}\n</tool_call>";
        let (visible, _) = run(&[raw]);
        assert!(
            !visible.contains("<think>"),
            "think tag leaked: {visible:?}"
        );
        assert!(
            !visible.contains("<tool_call>"),
            "tool_call tag leaked: {visible:?}"
        );
        assert!(
            !visible.contains("get board"),
            "tool JSON leaked: {visible:?}"
        );
        // Only inter-span whitespace remains; nothing of substance.
        assert!(
            visible.trim().is_empty(),
            "unexpected visible text: {visible:?}"
        );
    }

    #[test]
    fn handles_a_tag_split_across_chunk_boundaries() {
        // `<tool_call>` and `</tool_call>` each split mid-tag between chunks.
        assert_eq!(
            run_visible(&["keep<to", "ol_call>junk</to", "ol_call>tail"]),
            "keeptail"
        );
    }

    #[test]
    fn handles_open_tag_split_at_every_byte() {
        let chunks: Vec<String> = "a<think>b</think>c"
            .chars()
            .map(|c| c.to_string())
            .collect();
        let refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
        let (visible, thought) = run(&refs);
        assert_eq!(visible, "ac");
        assert_eq!(thought, "b");
    }

    #[test]
    fn a_lone_less_than_is_not_a_tag() {
        assert_eq!(run_visible(&["a < b"]), "a < b");
        assert_eq!(run_visible(&["1 ", "< ", "2"]), "1 < 2");
    }

    /// The regression card 01KSXAVM5Y2B0PMXQ4BR656NDR fixed: when the model
    /// runs out of budget mid-`<think>`, the buffered reasoning was being
    /// SILENTLY DROPPED. It must instead surface as a thought chunk so the
    /// user sees what the model was working on.
    #[test]
    fn unterminated_think_is_surfaced_as_thought_on_finish() {
        let (visible, thought) = run(&["text before <think>partial reasoning"]);
        assert_eq!(visible, "text before ");
        assert_eq!(
            thought, "partial reasoning",
            "buffered reasoning must reach the thought stream when the span is truncated"
        );
    }

    /// Unterminated `<tool_call>` stays dropped — a partial tool-call body
    /// can't be safely executed or displayed.
    #[test]
    fn unterminated_tool_call_is_dropped() {
        let (visible, thought) = run(&["text before <tool_call>{\"name\":\"k"]);
        assert_eq!(visible, "text before ");
        assert_eq!(
            thought, "",
            "a partial tool-call body must not leak into the thought stream"
        );
    }

    #[test]
    fn multiple_spans_in_one_stream() {
        let (visible, thought) =
            run(&["<think>a</think>X<tool_call>b</tool_call>Y<think>c</think>Z"]);
        assert_eq!(visible, "XYZ");
        assert_eq!(thought, "ac");
    }

    #[test]
    fn text_then_tool_call_keeps_only_the_text() {
        let (visible, _) = run(&["Here you go: <tool_call>{\"name\":\"x\"}</tool_call>"]);
        assert_eq!(visible, "Here you go: ");
    }

    /// Reasoning streams chunk-by-chunk to the thought channel, not all at
    /// once on close — the UI should be able to render the model "thinking"
    /// in real time, just like the visible text.
    #[test]
    fn think_content_streams_incrementally_to_thought() {
        let mut f = VisibleTextFilter::default();

        let segs = f.push("<think>step one");
        assert_eq!(segs, vec![FilterSegment::Thought("step one".into())]);

        let segs = f.push(", step two");
        assert_eq!(segs, vec![FilterSegment::Thought(", step two".into())]);

        let segs = f.push("</think>answer");
        assert_eq!(segs, vec![FilterSegment::Visible("answer".into())]);
    }

    /// The user-reported ordering bug: a chunk like
    /// `"hi <think>r</think> ok"` would, when aggregated into two flat
    /// strings, get broadcast as visible("hi  ok") then thought("r") — making
    /// the thinking land AFTER the visible text it preceded. The segmented
    /// API must preserve the source order so the UI can render thinking next
    /// to (and BEFORE) the text that follows it.
    #[test]
    fn segments_preserve_source_order_within_a_chunk() {
        let segs = run_segments(&["hi <think>r</think> ok"]);
        assert_eq!(
            segs,
            vec![
                FilterSegment::Visible("hi ".into()),
                FilterSegment::Thought("r".into()),
                FilterSegment::Visible(" ok".into()),
            ]
        );
    }

    /// A `<think>` followed by a `<tool_call>` in the same chunk: the thought
    /// must appear in the segment list *before* the tool-call markup (which
    /// is dropped) so the UI renders reasoning ahead of the tool call.
    #[test]
    fn think_before_tool_call_segments_in_order() {
        let segs = run_segments(&[
            "<think>about to call</think>let me check<tool_call>{\"name\":\"x\"}</tool_call>",
        ]);
        assert_eq!(
            segs,
            vec![
                FilterSegment::Thought("about to call".into()),
                FilterSegment::Visible("let me check".into()),
            ]
        );
    }
}
