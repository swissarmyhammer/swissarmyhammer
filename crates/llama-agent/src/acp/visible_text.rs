//! Streaming filter that removes reasoning/tool-call markup from the agent's
//! *visible* message text.
//!
//! The model's raw output interleaves natural-language text with ChatML-style
//! control spans — `<think>…</think>` reasoning and `<tool_call>…</tool_call>`
//! blocks (the Qwen convention). The full raw text still flows to
//! `ChatTemplateEngine::extract_tool_calls` (which turns `<tool_call>` blocks
//! into structured ACP `ToolCall`s) and to the response meta. But the text the
//! client *sees* as the assistant message must not contain that markup — a
//! structured tool call rendered in the UI plus its raw `<tool_call>` JSON
//! duplicated in the message body is exactly the bug this fixes.
//!
//! Generation streams token-by-token, and a control tag can be split across
//! chunks (`<tool` then `_call>`), so this is a small state machine that buffers
//! just enough to recognize a tag straddling a chunk boundary.
//!
//! Unterminated spans (an open tag with no matching close before the stream
//! ends — e.g. the model exhausts its budget mid-`<think>`) are suppressed in
//! full: there is no complete answer to show, and leaking a half-tag is worse
//! than showing nothing.

/// The (open, close) tag pairs whose spans are stripped from visible text.
/// These are the ChatML/Qwen conventions the chat templates emit.
const SUPPRESSED_SPANS: &[(&str, &str)] =
    &[("<think>", "</think>"), ("<tool_call>", "</tool_call>")];

/// Stateful, streaming stripper of [`SUPPRESSED_SPANS`]. Feed each generated
/// chunk to [`push`](Self::push) and broadcast whatever it returns; call
/// [`finish`](Self::finish) once the stream ends to flush any trailing text.
#[derive(Default)]
pub(crate) struct VisibleTextFilter {
    /// When `Some(close)`, we are inside a suppressed span and are discarding
    /// input until `close` is seen.
    inside_close: Option<&'static str>,
    /// Text held back because it might be the start of a tag straddling the
    /// next chunk boundary (a proper prefix of an open tag when outside, or of
    /// the close tag when inside).
    buf: String,
}

impl VisibleTextFilter {
    /// Feed the next chunk of raw generated text; returns the visible text to
    /// emit (markup spans removed). May return an empty string.
    pub(crate) fn push(&mut self, text: &str) -> String {
        self.buf.push_str(text);
        let mut out = String::new();

        loop {
            match self.inside_close {
                None => {
                    if let Some((idx, open, close)) = earliest_open(&self.buf) {
                        // Everything before the open tag is visible; the open
                        // tag itself is consumed and we enter the span.
                        out.push_str(&self.buf[..idx]);
                        self.buf.drain(..idx + open.len());
                        self.inside_close = Some(close);
                        continue;
                    }
                    // No complete open tag. Emit everything except a trailing
                    // suffix that could be the start of one.
                    let keep = longest_open_prefix_suffix(&self.buf);
                    let emit_to = self.buf.len() - keep;
                    out.push_str(&self.buf[..emit_to]);
                    self.buf.drain(..emit_to);
                    break;
                }
                Some(close) => {
                    if let Some(idx) = self.buf.find(close) {
                        // Discard the suppressed span up to and including close.
                        self.buf.drain(..idx + close.len());
                        self.inside_close = None;
                        continue;
                    }
                    // Still inside: discard all but a trailing suffix that could
                    // be the start of the close tag.
                    let keep = longest_prefix_suffix(&self.buf, close);
                    let drop_to = self.buf.len() - keep;
                    self.buf.drain(..drop_to);
                    break;
                }
            }
        }

        out
    }

    /// Flush at end of stream. Outside a span, any leftover buffered text is
    /// real (no tag ever completed), so emit it. Inside a span, the span was
    /// never closed — suppress the remainder.
    pub(crate) fn finish(&mut self) -> String {
        if self.inside_close.is_none() {
            std::mem::take(&mut self.buf)
        } else {
            self.buf.clear();
            String::new()
        }
    }
}

/// Find the earliest complete open tag in `buf`, returning its byte index and
/// the matching (open, close) pair.
fn earliest_open(buf: &str) -> Option<(usize, &'static str, &'static str)> {
    SUPPRESSED_SPANS
        .iter()
        .filter_map(|(open, close)| buf.find(open).map(|idx| (idx, *open, *close)))
        .min_by_key(|(idx, _, _)| *idx)
}

/// Length (bytes) of the longest suffix of `buf` that is a proper prefix of any
/// open tag — i.e. text we must hold back because the rest of the tag may
/// arrive in the next chunk. All tags are ASCII so byte lengths land on char
/// boundaries.
fn longest_open_prefix_suffix(buf: &str) -> usize {
    SUPPRESSED_SPANS
        .iter()
        .map(|(open, _)| longest_prefix_suffix(buf, open))
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

    /// Run a sequence of chunks through the filter and return the full visible
    /// output (push results concatenated, then finish()).
    fn run(chunks: &[&str]) -> String {
        let mut f = VisibleTextFilter::default();
        let mut out = String::new();
        for c in chunks {
            out.push_str(&f.push(c));
        }
        out.push_str(&f.finish());
        out
    }

    #[test]
    fn plain_text_passes_through_unchanged() {
        assert_eq!(run(&["hello world"]), "hello world");
        assert_eq!(run(&["hel", "lo ", "wor", "ld"]), "hello world");
    }

    #[test]
    fn strips_a_complete_think_span() {
        assert_eq!(run(&["<think>reasoning here</think>answer"]), "answer");
    }

    #[test]
    fn strips_a_complete_tool_call_span() {
        assert_eq!(run(&["<tool_call>{\"name\":\"kanban\"}</tool_call>"]), "");
    }

    #[test]
    fn keeps_text_around_a_span() {
        assert_eq!(run(&["before <think>x</think> after"]), "before  after");
    }

    #[test]
    fn reproduces_the_reported_leak_cleanly() {
        // The exact shape the user saw: empty think + a tool_call block. The
        // visible output must contain neither tag nor the JSON payload.
        let raw = "<think>\n\n</think>\n\n<tool_call>\n{\"name\": \"kanban\", \"arguments\": {\"op\": \"get board\"}}\n</tool_call>";
        let visible = run(&[raw]);
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
            run(&["keep<to", "ol_call>junk</to", "ol_call>tail"]),
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
        assert_eq!(run(&refs), "ac");
    }

    #[test]
    fn a_lone_less_than_is_not_a_tag() {
        assert_eq!(run(&["a < b"]), "a < b");
        assert_eq!(run(&["1 ", "< ", "2"]), "1 < 2");
    }

    #[test]
    fn unterminated_span_is_suppressed_entirely() {
        // Model ran out of budget mid-think: nothing complete to show.
        assert_eq!(
            run(&["text before <think>partial reasoning"]),
            "text before "
        );
    }

    #[test]
    fn multiple_spans_in_one_stream() {
        assert_eq!(
            run(&["<think>a</think>X<tool_call>b</tool_call>Y<think>c</think>Z"]),
            "XYZ"
        );
    }

    #[test]
    fn text_then_tool_call_keeps_only_the_text() {
        assert_eq!(
            run(&["Here you go: <tool_call>{\"name\":\"x\"}</tool_call>"]),
            "Here you go: "
        );
    }
}
