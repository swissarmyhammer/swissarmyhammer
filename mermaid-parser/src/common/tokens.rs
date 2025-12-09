//! Common token types used across multiple parsers

/// Common tokens that appear in multiple Mermaid diagram types
#[derive(Debug, Clone, PartialEq)]
pub enum CommonToken {
    /// Newline character(s)
    NewLine,

    /// End of file
    Eof,

    /// Title declaration
    Title(String),

    /// Accessibility title
    AccTitle,

    /// Accessibility title value
    AccTitleValue(String),

    /// Accessibility description
    AccDescr,

    /// Accessibility description value
    AccDescrValue(String),

    /// Multiline accessibility description
    AccDescrMultiline(String),

    /// Section declaration
    Section(String),

    /// Comment (ignored during parsing)
    Comment(String),

    /// Whitespace (usually ignored)
    Whitespace,

    /// Invalid token
    Invalid(char),
}

/// Span information for error reporting
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    pub fn single(position: usize, line: usize, column: usize) -> Self {
        Self {
            start: position,
            end: position + 1,
            line,
            column,
        }
    }
}

/// Token with location information
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken<T> {
    pub token: T,
    pub span: Span,
}

impl<T> SpannedToken<T> {
    pub fn new(token: T, span: Span) -> Self {
        Self { token, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_token_equality() {
        assert_eq!(CommonToken::NewLine, CommonToken::NewLine);
        assert_eq!(CommonToken::Eof, CommonToken::Eof);
        assert_eq!(CommonToken::AccTitle, CommonToken::AccTitle);
        assert_eq!(CommonToken::AccDescr, CommonToken::AccDescr);
        assert_eq!(CommonToken::Whitespace, CommonToken::Whitespace);
    }

    #[test]
    fn test_common_token_with_values() {
        let title1 = CommonToken::Title("Test Title".to_string());
        let title2 = CommonToken::Title("Test Title".to_string());
        let title3 = CommonToken::Title("Different Title".to_string());

        assert_eq!(title1, title2);
        assert_ne!(title1, title3);
    }

    #[test]
    fn test_accessibility_tokens() {
        let acc_title_val = CommonToken::AccTitleValue("Accessible Title".to_string());
        let acc_descr_val = CommonToken::AccDescrValue("Accessible Description".to_string());
        let acc_descr_multi = CommonToken::AccDescrMultiline("Line 1\nLine 2".to_string());

        assert_ne!(acc_title_val, acc_descr_val);
        assert_ne!(acc_descr_val, acc_descr_multi);
    }

    #[test]
    fn test_section_token() {
        let section = CommonToken::Section("Section Name".to_string());
        assert!(matches!(section, CommonToken::Section(_)));

        if let CommonToken::Section(name) = section {
            assert_eq!(name, "Section Name");
        }
    }

    #[test]
    fn test_comment_token() {
        let comment = CommonToken::Comment("This is a comment".to_string());
        assert!(matches!(comment, CommonToken::Comment(_)));
    }

    #[test]
    fn test_invalid_token() {
        let invalid = CommonToken::Invalid('?');
        assert!(matches!(invalid, CommonToken::Invalid(_)));

        if let CommonToken::Invalid(ch) = invalid {
            assert_eq!(ch, '?');
        }
    }

    #[test]
    fn test_span_creation() {
        let span = Span::new(0, 5, 1, 0);
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 5);
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 0);
    }

    #[test]
    fn test_span_single() {
        let span = Span::single(10, 2, 5);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 11);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 5);
    }

    #[test]
    fn test_span_equality() {
        let span1 = Span::new(0, 5, 1, 0);
        let span2 = Span::new(0, 5, 1, 0);
        let span3 = Span::new(1, 6, 1, 1);

        assert_eq!(span1, span2);
        assert_ne!(span1, span3);
    }

    #[test]
    fn test_spanned_token_creation() {
        let span = Span::new(0, 5, 1, 0);
        let token = CommonToken::NewLine;
        let spanned = SpannedToken::new(token.clone(), span);

        assert_eq!(spanned.token, token);
        assert_eq!(spanned.span, span);
    }

    #[test]
    fn test_spanned_token_with_generic_type() {
        let span = Span::new(5, 10, 1, 5);
        let token_str = "identifier";
        let spanned = SpannedToken::new(token_str, span);

        assert_eq!(spanned.token, "identifier");
        assert_eq!(spanned.span.start, 5);
        assert_eq!(spanned.span.end, 10);
    }

    #[test]
    fn test_spanned_token_equality() {
        let span = Span::new(0, 5, 1, 0);
        let token = CommonToken::Title("Test".to_string());

        let spanned1 = SpannedToken::new(token.clone(), span);
        let spanned2 = SpannedToken::new(token.clone(), span);
        let spanned3 = SpannedToken::new(CommonToken::NewLine, span);

        assert_eq!(spanned1, spanned2);
        assert_ne!(spanned1, spanned3);
    }

    #[test]
    fn test_debug_formatting() {
        let token = CommonToken::Title("Debug Test".to_string());
        let debug_str = format!("{:?}", token);
        assert!(debug_str.contains("Title"));
        assert!(debug_str.contains("Debug Test"));
    }

    #[test]
    fn test_clone_trait() {
        let original_token = CommonToken::Section("Original".to_string());
        let cloned_token = original_token.clone();
        assert_eq!(original_token, cloned_token);

        let span = Span::new(0, 5, 1, 0);
        let cloned_span = span; // Span implements Copy, so no need to clone
        assert_eq!(span, cloned_span);
    }
}
