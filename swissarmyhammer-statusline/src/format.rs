//! Parse format strings like "$directory $model" into ordered segments.

/// A segment in a parsed format string.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatSegment {
    /// A literal string to output as-is.
    Literal(String),
    /// A module variable reference like "directory" from "$directory".
    Variable(String),
}

/// Parse a format string into segments.
///
/// Variables are prefixed with `$`. Literal `$` can be escaped as `$$`.
/// Square brackets `[...]` are treated as literal grouping (for display).
pub fn parse_format(format: &str) -> Vec<FormatSegment> {
    let mut segments = Vec::new();
    let mut chars = format.chars().peekable();
    let mut literal = String::new();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some(&next) = chars.peek() {
                if next == '$' {
                    // $$ -> literal $, second $ is not consumed so it
                    // can introduce a variable on the next iteration.
                    literal.push('$');
                } else if next.is_alphanumeric() || next == '_' {
                    // Start of variable
                    if !literal.is_empty() {
                        segments.push(FormatSegment::Literal(std::mem::take(&mut literal)));
                    }
                    let mut var_name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            var_name.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    segments.push(FormatSegment::Variable(var_name));
                } else {
                    literal.push('$');
                }
            } else {
                literal.push('$');
            }
        } else {
            literal.push(ch);
        }
    }

    if !literal.is_empty() {
        segments.push(FormatSegment::Literal(literal));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variables() {
        let segs = parse_format("$directory $model");
        assert_eq!(
            segs,
            vec![
                FormatSegment::Variable("directory".into()),
                FormatSegment::Literal(" ".into()),
                FormatSegment::Variable("model".into()),
            ]
        );
    }

    #[test]
    fn test_adjacent_variables() {
        let segs = parse_format("$git_branch$git_status");
        assert_eq!(
            segs,
            vec![
                FormatSegment::Variable("git_branch".into()),
                FormatSegment::Variable("git_status".into()),
            ]
        );
    }

    #[test]
    fn test_escaped_dollar() {
        let segs = parse_format("$$amount");
        assert_eq!(
            segs,
            vec![
                FormatSegment::Literal("$".into()),
                FormatSegment::Variable("amount".into()),
            ]
        );
    }

    #[test]
    fn test_brackets() {
        let segs = parse_format("[$bar] $percentage%");
        assert_eq!(
            segs,
            vec![
                FormatSegment::Literal("[".into()),
                FormatSegment::Variable("bar".into()),
                FormatSegment::Literal("] ".into()),
                FormatSegment::Variable("percentage".into()),
                FormatSegment::Literal("%".into()),
            ]
        );
    }

    #[test]
    fn test_literal_only() {
        let segs = parse_format("hello world");
        assert_eq!(segs, vec![FormatSegment::Literal("hello world".into())]);
    }

    #[test]
    fn test_empty() {
        let segs = parse_format("");
        assert!(segs.is_empty());
    }

    #[test]
    fn test_trailing_dollar() {
        let segs = parse_format("cost$");
        assert_eq!(segs, vec![FormatSegment::Literal("cost$".into())]);
    }

    #[test]
    fn test_dollar_non_alphanumeric() {
        let segs = parse_format("$!bang");
        assert_eq!(segs, vec![FormatSegment::Literal("$!bang".into())]);
    }
}
