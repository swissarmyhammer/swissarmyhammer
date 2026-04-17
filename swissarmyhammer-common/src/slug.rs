//! Canonical URL-safe slug generation shared across the workspace.
//!
//! This is the single source of truth for how a display string becomes a
//! mention/filter slug. The frontend `kanban-app/ui/src/lib/slugify.ts`
//! mirrors this exact algorithm, and the two implementations are kept in
//! lockstep via the parity corpus in `tests/slug_parity_corpus.txt` which
//! both sides run over.
//!
//! ## Algorithm
//!
//! Given an input string, the slug is produced by:
//!
//! 1. ASCII-lowercasing every character.
//! 2. Replacing every run of characters outside `[a-z0-9]` (after lowercasing)
//!    with a single `-`.
//! 3. Stripping any leading or trailing `-`.
//!
//! The transformation is idempotent: `slug(slug(s)) == slug(s)` for every
//! string `s`.
//!
//! ## Unicode
//!
//! Non-ASCII characters are treated as "not `[a-z0-9]`" and therefore
//! collapse into hyphen runs. This matches the TypeScript regex
//! `/[^a-z0-9]+/g` exactly. Transliteration is out of scope — slugs derived
//! from non-Latin names will typically reduce to the empty string, which is
//! fine because entity IDs remain the canonical identifier; the slug is a
//! display convenience, not a stable identifier.

/// Convert a display string to its canonical filter/mention slug.
///
/// # Parameters
///
/// - `s` — the input display string (e.g. a project or actor's human-readable name)
///
/// # Returns
///
/// The lowercase, hyphen-separated slug with leading/trailing hyphens stripped.
/// The empty string is returned for any input that contains no ASCII
/// alphanumeric characters.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_common::slug;
///
/// assert_eq!(slug("Hello World"), "hello-world");
/// assert_eq!(slug("Task card & field polish"), "task-card-field-polish");
/// assert_eq!(slug("---mixed---"), "mixed");
/// assert_eq!(slug("slug(slug(x))"), slug(&slug("slug(slug(x))")));
/// ```
pub fn slug(s: &str) -> String {
    // Pre-allocate for the common case where the slug isn't much longer
    // than the input. The output can't exceed the input's UTF-8 length
    // once non-alphanumerics collapse, so this is an upper bound.
    let mut out = String::with_capacity(s.len());
    let mut last_was_hyphen = true; // synthetic leading boundary — suppresses the leading hyphen naturally

    for ch in s.chars() {
        let lower = ch.to_ascii_lowercase();
        let is_alnum = lower.is_ascii_lowercase() || lower.is_ascii_digit();
        if is_alnum {
            out.push(lower);
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }

    // Strip trailing hyphen if present. We emitted at most one trailing
    // hyphen because we collapse runs via `last_was_hyphen`.
    if out.ends_with('-') {
        out.pop();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn empty_input_is_empty_slug() {
        assert_eq!(slug(""), "");
    }

    #[test]
    fn only_punctuation_is_empty_slug() {
        assert_eq!(slug("!!!"), "");
        assert_eq!(slug("   "), "");
        assert_eq!(slug("---"), "");
        assert_eq!(slug("&*()"), "");
    }

    #[test]
    fn simple_ascii_passthrough() {
        assert_eq!(slug("hello"), "hello");
        assert_eq!(slug("hello-world"), "hello-world");
        assert_eq!(slug("abc123"), "abc123");
    }

    #[test]
    fn lowercases_uppercase_input() {
        assert_eq!(slug("HELLO"), "hello");
        assert_eq!(slug("MixedCase"), "mixedcase");
        assert_eq!(slug("CamelCase Name"), "camelcase-name");
    }

    #[test]
    fn collapses_non_alphanumeric_runs_to_single_hyphen() {
        assert_eq!(slug("hello     world"), "hello-world");
        assert_eq!(slug("hello___world"), "hello-world");
        assert_eq!(slug("a!@#b"), "a-b");
        assert_eq!(slug("one & two, three."), "one-two-three");
    }

    #[test]
    fn strips_leading_non_alphanumeric() {
        assert_eq!(slug("---hello"), "hello");
        assert_eq!(slug("   hello"), "hello");
        assert_eq!(slug("!!!hello"), "hello");
    }

    #[test]
    fn strips_trailing_non_alphanumeric() {
        assert_eq!(slug("hello---"), "hello");
        assert_eq!(slug("hello   "), "hello");
        assert_eq!(slug("hello!!!"), "hello");
    }

    #[test]
    fn strips_leading_and_trailing_together() {
        assert_eq!(slug("  ---hello world---  "), "hello-world");
    }

    #[test]
    fn idempotent() {
        let inputs = [
            "",
            "hello",
            "Hello World",
            "Task card & field polish",
            "---mixed---",
            "MULTI   spaces",
            "unicodé name",
            "emoji 🚀 rocket",
            "CJK テスト text",
        ];
        for input in inputs {
            let once = slug(input);
            let twice = slug(&once);
            assert_eq!(
                once, twice,
                "slug not idempotent on {input:?}: first={once:?} second={twice:?}"
            );
        }
    }

    #[test]
    fn unicode_punctuation_collapses_like_ascii() {
        // Smart quotes, em dash, non-breaking space — all non-ASCII, all
        // must collapse into the single-hyphen-run rule.
        assert_eq!(slug("one\u{2013}two"), "one-two"); // en dash
        assert_eq!(slug("one\u{2014}two"), "one-two"); // em dash
        assert_eq!(slug("one\u{00A0}two"), "one-two"); // non-breaking space
        assert_eq!(slug("\u{201C}quoted\u{201D}"), "quoted"); // curly quotes
    }

    #[test]
    fn reproducer_from_task() {
        // The concrete reproducer from the task description:
        //   Project name "Task card & field polish" must slugify to
        //   "task-card-field-polish" so the autocomplete picker's output
        //   matches the frontend's display string.
        assert_eq!(slug("Task card & field polish"), "task-card-field-polish");
    }

    #[test]
    fn real_world_display_names() {
        assert_eq!(slug("Frontend: UI polish"), "frontend-ui-polish");
        assert_eq!(slug("Claude-Code (v2)"), "claude-code-v2");
        assert_eq!(slug("bug #42"), "bug-42");
        assert_eq!(slug("swissarmyhammer / kanban"), "swissarmyhammer-kanban");
    }

    // ─────────────────────────────────────────────────────────────────
    // Parity corpus
    //
    // This file is loaded by both the Rust and TypeScript parity tests.
    // Every line is an input; the two implementations must produce
    // byte-identical output for every line. Keep this list diverse and
    // keep `swissarmyhammer-common/tests/slug_parity_corpus.txt` the
    // single source of truth — the TS test reads the same file.
    // ─────────────────────────────────────────────────────────────────

    /// Load every parity corpus line and assert the Rust slug is stable.
    /// The TS side of the parity test is in
    /// `kanban-app/ui/src/lib/slugify.parity.test.ts` and loads the same
    /// file; byte equality with this module's output is the contract.
    #[test]
    fn parity_corpus_rust_side_is_stable() {
        let corpus = include_str!("../tests/slug_parity_corpus.txt");
        let mut lines = 0usize;
        for raw_line in corpus.lines() {
            // Skip comment and blank lines so the corpus file can be
            // self-documenting.
            if raw_line.starts_with('#') || raw_line.is_empty() {
                continue;
            }
            lines += 1;
            let result = slug(raw_line);
            // Double-slug must equal single-slug.
            assert_eq!(
                slug(&result),
                result,
                "corpus entry {raw_line:?} is not idempotent (first={result:?})"
            );
        }
        assert!(
            lines >= 100,
            "parity corpus must have at least 100 entries, found {lines}"
        );
    }
}
