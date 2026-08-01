//! Line-anchored splitting of frontmatter + markdown body text.
//!
//! Both on-disk readers in this crate read the same file format, so they share
//! one splitter and the delimiter rule cannot drift between them:
//! [`crate::io::read_entity`] (via `parse_frontmatter_body`) and
//! [`crate::store::EntityTypeStore`] (via its `deserialize`).

/// Strip a line's terminator, returning the line's content.
///
/// Handles `\n` and `\r\n`. A line with no terminator -- the last line of text
/// that does not end in a newline -- comes back unchanged.
fn line_content(raw: &str) -> &str {
    let without_lf = raw.strip_suffix('\n').unwrap_or(raw);
    without_lf.strip_suffix('\r').unwrap_or(without_lf)
}

/// Whether `raw` is a frontmatter delimiter line: exactly three hyphens, with
/// nothing on the line but its terminator.
fn is_delimiter_line(raw: &str) -> bool {
    line_content(raw) == "---"
}

/// Split frontmatter + body text on line-anchored `---` delimiters.
///
/// The opening delimiter is the first line of `content` and must be exactly
/// three hyphens. The frontmatter runs to the next delimiter line. Returns
/// `(frontmatter, body)`, where `frontmatter` is the bytes between the two
/// delimiter lines and `body` is every byte after the closing delimiter line's
/// terminator. The body is a borrowed slice of `content`, so it keeps its
/// bytes exactly: no newline is added or dropped, and CRLF stays CRLF.
///
/// Only a line that is exactly three hyphens delimits. Three hyphens indented,
/// or with any other text on the line, is ordinary content and stays in the
/// frontmatter. That is what keeps a three-hyphen run inside a YAML scalar --
/// a comment's prose, a title -- from ending the frontmatter early: an emitter
/// writes such a run either indented inside a block scalar or escaped inside a
/// quoted one, never alone at column 0.
///
/// Returns `None` when the first line is not a delimiter line, or when no
/// closing delimiter line follows it. Callers turn that into their own
/// "invalid frontmatter" error.
pub(crate) fn split_frontmatter_body(content: &str) -> Option<(&str, &str)> {
    let opening = content.split_inclusive('\n').next()?;
    if !is_delimiter_line(opening) {
        return None;
    }
    let after_opening = &content[opening.len()..];

    let mut offset = 0;
    for raw in after_opening.split_inclusive('\n') {
        if is_delimiter_line(raw) {
            let frontmatter = &after_opening[..offset];
            let body = &after_opening[offset + raw.len()..];
            return Some((frontmatter, body));
        }
        offset += raw.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::split_frontmatter_body;

    #[test]
    fn splits_on_the_first_delimiter_line() {
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbody\n"),
            Some(("title: x\n", "body\n"))
        );
    }

    #[test]
    fn keeps_the_body_bytes_exactly() {
        // No trailing newline.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbody"),
            Some(("title: x\n", "body"))
        );
        // Empty body, closing delimiter terminated.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\n"),
            Some(("title: x\n", ""))
        );
        // Empty body, closing delimiter unterminated.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---"),
            Some(("title: x\n", ""))
        );
        // A body that is only newlines keeps every one of them.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\n\n\n"),
            Some(("title: x\n", "\n\n"))
        );
    }

    #[test]
    fn crlf_delimiter_lines_leave_a_crlf_body_intact() {
        assert_eq!(
            split_frontmatter_body("---\r\ntitle: x\r\n---\r\nbody\r\n"),
            Some(("title: x\r\n", "body\r\n"))
        );
    }

    #[test]
    fn three_hyphens_inside_the_frontmatter_do_not_delimit() {
        // Indented, as a YAML block scalar writes them.
        assert_eq!(
            split_frontmatter_body("---\ntext: |-\n  before\n  ---\n  after\n---\nbody"),
            Some(("text: |-\n  before\n  ---\n  after\n", "body"))
        );
        // Embedded in a longer line, as a quoted scalar writes them.
        assert_eq!(
            split_frontmatter_body("---\ntitle: a --- b\n---\nbody"),
            Some(("title: a --- b\n", "body"))
        );
        // A longer run of hyphens is not the delimiter either.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n----\n---\nbody"),
            Some(("title: x\n----\n", "body"))
        );
    }

    #[test]
    fn three_hyphens_in_the_body_stay_in_the_body() {
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbefore\n---\nafter"),
            Some(("title: x\n", "before\n---\nafter"))
        );
    }

    #[test]
    fn empty_frontmatter_splits_to_an_empty_slice() {
        assert_eq!(split_frontmatter_body("---\n---\nbody"), Some(("", "body")));
    }

    #[test]
    fn rejects_text_that_does_not_open_with_a_delimiter_line() {
        assert_eq!(split_frontmatter_body(""), None);
        assert_eq!(split_frontmatter_body("just prose"), None);
        // Content before the opening delimiter is not frontmatter.
        assert_eq!(split_frontmatter_body("prose\n---\ntitle: x\n---\n"), None);
        // The opening line must be only the delimiter.
        assert_eq!(split_frontmatter_body("--- \ntitle: x\n---\n"), None);
    }

    #[test]
    fn rejects_text_with_no_closing_delimiter_line() {
        assert_eq!(split_frontmatter_body("---\ntitle: x\n"), None);
        assert_eq!(split_frontmatter_body("---\n"), None);
        assert_eq!(split_frontmatter_body("---"), None);
    }
}
