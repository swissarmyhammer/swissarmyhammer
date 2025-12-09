//! Shared parser utilities to eliminate code duplication across diagram parsers

use crate::common::ast::AccessibilityInfo;
use chumsky::prelude::*;

/// Represents supported diagram types with their header variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramType {
    Architecture,
    Sequence,
    Pie,
    Mindmap,
    Xy,
    Treemap,
    State,
    Radar,
    Quadrant,
    Packet,
    Kanban,
}

impl DiagramType {
    /// Get the valid header strings for this diagram type
    pub fn headers(&self) -> &'static [&'static str] {
        match self {
            DiagramType::Architecture => &["architecture", "architecture-beta"],
            DiagramType::Sequence => &["sequenceDiagram"],
            DiagramType::Pie => &["pie"],
            DiagramType::Mindmap => &["mindmap"],
            DiagramType::Xy => &["xychart-beta"],
            DiagramType::Treemap => &["treemap"],
            DiagramType::State => &["stateDiagram", "stateDiagram-v2"],
            DiagramType::Radar => &["radar"],
            DiagramType::Quadrant => &["quadrant"],
            DiagramType::Packet => &["packet-beta", "packet"],
            DiagramType::Kanban => &["kanban"],
        }
    }

    /// Get the diagram type name for display purposes
    pub fn name(&self) -> &'static str {
        match self {
            DiagramType::Architecture => "architecture",
            DiagramType::Sequence => "sequence",
            DiagramType::Pie => "pie",
            DiagramType::Mindmap => "mindmap",
            DiagramType::Xy => "xy chart",
            DiagramType::Treemap => "treemap",
            DiagramType::State => "state",
            DiagramType::Radar => "radar",
            DiagramType::Quadrant => "quadrant",
            DiagramType::Packet => "packet",
            DiagramType::Kanban => "kanban",
        }
    }
}

/// Common token types shared across parsers
#[derive(Debug, Clone, PartialEq)]
pub enum CommonToken {
    NewLine,
    Comment(String),
}

/// Parse comments that start with %% or //
pub fn parse_comment<'src>(
) -> impl Parser<'src, &'src str, CommonToken, extra::Err<Simple<'src, char>>> + Clone {
    choice((
        just("%%").then(none_of('\n').repeated()),
        just("//").then(none_of('\n').repeated()),
    ))
    .map(|_| CommonToken::Comment("".to_string()))
}

/// Parse whitespace (spaces and tabs)
pub fn parse_whitespace<'src>(
) -> impl Parser<'src, &'src str, (), extra::Err<Simple<'src, char>>> + Clone {
    one_of(" \t").repeated().ignored()
}

/// Parse one or more whitespace characters
pub fn parse_whitespace_required<'src>(
) -> impl Parser<'src, &'src str, (), extra::Err<Simple<'src, char>>> + Clone {
    one_of(" \t").repeated().at_least(1).ignored()
}

/// Parse newlines (both \n and \r\n)
pub fn parse_newline<'src>(
) -> impl Parser<'src, &'src str, CommonToken, extra::Err<Simple<'src, char>>> + Clone {
    choice((just("\n"), just("\r\n"))).map(|_| CommonToken::NewLine)
}

/// Macro to create standardized parse functions with consistent error handling
#[macro_export]
macro_rules! create_parser_fn {
    ($vis:vis fn $name:ident($input:ident: &str) -> Result<$output:ty> {
        lexer: $lexer:expr,
        parser: $parser:expr,
        diagram_type: $diagram_type:literal
    }) => {
        $vis fn $name($input: &str) -> $crate::error::Result<$output> {
            let tokens = $lexer()
                .parse($input)
                .into_result()
                .map_err(|e| $crate::error::ParseError::SyntaxError {
                    message: format!("Failed to tokenize {} diagram", $diagram_type),
                    expected: vec![],
                    found: format!("{:?}", e),
                    line: 0,
                    column: 0,
                })?;

            let result = $parser()
                .parse(&tokens[..])
                .into_result()
                .map_err(|e| $crate::error::ParseError::SyntaxError {
                    message: format!("Failed to parse {} diagram", $diagram_type),
                    expected: vec![],
                    found: format!("{:?}", e),
                    line: 0,
                    column: 0,
                })?;

            Ok(result)
        }
    };
}

/// Macro to create standardized parse functions for line-based parsers
#[macro_export]
macro_rules! create_line_parser_fn {
    ($vis:vis fn $name:ident($input:ident: &str) -> Result<$output:ty> {
        parser_fn: $parser_fn:expr,
        diagram_type: $diagram_type:literal
    }) => {
        $vis fn $name($input: &str) -> $crate::error::Result<$output> {
            let tokens = $parser_fn()
                .parse($input)
                .into_result()
                .map_err(|e| $crate::error::ParseError::SyntaxError {
                    message: format!("Failed to tokenize {} diagram", $diagram_type),
                    expected: vec![],
                    found: format!("{:?}", e),
                    line: 0,
                    column: 0,
                })?;

            $parser_fn(&tokens)
        }
    };
}

/// Parses common diagram directives (title, accTitle, accDescr)
/// Returns true if the line was handled, false if not recognized
pub fn parse_common_directives(
    line: &str,
    title: &mut Option<String>,
    accessibility: &mut AccessibilityInfo,
) -> bool {
    let trimmed = line.trim();

    // Handle lines that start with \t by stripping it first
    let effective_trimmed = if trimmed.starts_with("\\t") {
        trimmed.strip_prefix("\\t").unwrap_or(trimmed).trim()
    } else {
        trimmed
    };

    // Parse title directive
    if let Some(title_text) = effective_trimmed.strip_prefix("title ") {
        *title = Some(title_text.trim().to_string());
        return true;
    }

    // Parse accessibility title (handle both "accTitle:" and "accTitle ")
    if let Some(acc_title) = effective_trimmed.strip_prefix("accTitle:") {
        accessibility.title = Some(acc_title.trim().to_string());
        return true;
    }
    if let Some(acc_title) = effective_trimmed.strip_prefix("accTitle ") {
        accessibility.title = Some(acc_title.trim().to_string());
        return true;
    }

    // Parse accessibility description (handle both "accDescr:" and "accDescr ")
    if let Some(acc_desc) = effective_trimmed.strip_prefix("accDescr:") {
        accessibility.description = Some(acc_desc.trim().to_string());
        return true;
    }
    if let Some(acc_desc) = effective_trimmed.strip_prefix("accDescr ") {
        accessibility.description = Some(acc_desc.trim().to_string());
        return true;
    }

    false
}

/// Enhanced directive parser that handles multi-line accessibility descriptions
pub struct CommonDirectiveParser {
    in_multiline_desc: bool,
    multiline_content: Vec<String>,
}

impl CommonDirectiveParser {
    pub fn new() -> Self {
        Self {
            in_multiline_desc: false,
            multiline_content: Vec::new(),
        }
    }

    /// Parse common directives with multi-line support
    /// Returns true if line was handled
    pub fn parse_line(
        &mut self,
        line: &str,
        title: &mut Option<String>,
        accessibility: &mut AccessibilityInfo,
    ) -> bool {
        let trimmed = line.trim();

        // Handle lines that start with \t by stripping it first
        let effective_trimmed = if trimmed.starts_with("\\t") {
            trimmed.strip_prefix("\\t").unwrap_or(trimmed).trim()
        } else {
            trimmed
        };

        // Handle multi-line accessibility description end
        if self.in_multiline_desc && effective_trimmed == "}" {
            self.in_multiline_desc = false;
            if !self.multiline_content.is_empty() {
                accessibility.description = Some(self.multiline_content.join(" "));
                self.multiline_content.clear();
            }
            return true;
        }

        // Handle multi-line accessibility description content
        if self.in_multiline_desc {
            if !effective_trimmed.is_empty()
                && !effective_trimmed.starts_with("//")
                && !effective_trimmed.starts_with("%%")
            {
                self.multiline_content.push(effective_trimmed.to_string());
            }
            return true;
        }

        // Handle multi-line accessibility description start
        if effective_trimmed.starts_with("accDescr {") {
            self.in_multiline_desc = true;
            return true;
        }

        // Handle single-line directives
        parse_common_directives(line, title, accessibility)
    }
}

impl Default for CommonDirectiveParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod directive_tests {
    use super::*;

    #[test]
    fn test_parse_title_directive() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "title My Diagram",
            &mut title,
            &mut acc
        ));
        assert_eq!(title, Some("My Diagram".to_string()));
    }

    #[test]
    fn test_parse_title_with_whitespace() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "title   My Diagram   ",
            &mut title,
            &mut acc
        ));
        assert_eq!(title, Some("My Diagram".to_string()));
    }

    #[test]
    fn test_parse_accessibility_title_colon() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "accTitle: Accessible Title",
            &mut title,
            &mut acc
        ));
        assert_eq!(acc.title, Some("Accessible Title".to_string()));
    }

    #[test]
    fn test_parse_accessibility_title_space() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "accTitle Accessible Title",
            &mut title,
            &mut acc
        ));
        assert_eq!(acc.title, Some("Accessible Title".to_string()));
    }

    #[test]
    fn test_parse_accessibility_description_colon() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "accDescr: Description text",
            &mut title,
            &mut acc
        ));
        assert_eq!(acc.description, Some("Description text".to_string()));
    }

    #[test]
    fn test_parse_accessibility_description_space() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parse_common_directives(
            "accDescr Description text",
            &mut title,
            &mut acc
        ));
        assert_eq!(acc.description, Some("Description text".to_string()));
    }

    #[test]
    fn test_multiline_accessibility_description() {
        let mut parser = CommonDirectiveParser::new();
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        // Start multi-line
        assert!(parser.parse_line("accDescr {", &mut title, &mut acc));

        // Content lines
        assert!(parser.parse_line("This is a long", &mut title, &mut acc));
        assert!(parser.parse_line("accessibility description", &mut title, &mut acc));

        // End multi-line
        assert!(parser.parse_line("}", &mut title, &mut acc));

        assert_eq!(
            acc.description,
            Some("This is a long accessibility description".to_string())
        );
    }

    #[test]
    fn test_multiline_description_ignores_comments() {
        let mut parser = CommonDirectiveParser::new();
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        // Start multi-line
        assert!(parser.parse_line("accDescr {", &mut title, &mut acc));

        // Content with comments and empty lines
        assert!(parser.parse_line("This is content", &mut title, &mut acc));
        assert!(parser.parse_line("// This is a comment", &mut title, &mut acc));
        assert!(parser.parse_line("%% Another comment", &mut title, &mut acc));
        assert!(parser.parse_line("", &mut title, &mut acc));
        assert!(parser.parse_line("More content", &mut title, &mut acc));

        // End multi-line
        assert!(parser.parse_line("}", &mut title, &mut acc));

        assert_eq!(
            acc.description,
            Some("This is content More content".to_string())
        );
    }

    #[test]
    fn test_unrecognized_directive() {
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(!parse_common_directives(
            "unknown directive",
            &mut title,
            &mut acc
        ));
        assert_eq!(title, None);
        assert_eq!(acc.title, None);
        assert_eq!(acc.description, None);
    }

    #[test]
    fn test_parser_handles_single_line_directives() {
        let mut parser = CommonDirectiveParser::new();
        let mut title = None;
        let mut acc = AccessibilityInfo::default();

        assert!(parser.parse_line("title Test Title", &mut title, &mut acc));
        assert_eq!(title, Some("Test Title".to_string()));

        assert!(parser.parse_line("accTitle: Test Acc Title", &mut title, &mut acc));
        assert_eq!(acc.title, Some("Test Acc Title".to_string()));

        assert!(parser.parse_line("accDescr: Test Description", &mut title, &mut acc));
        assert_eq!(acc.description, Some("Test Description".to_string()));
    }

    #[test]
    fn test_parser_not_in_multiline_mode_initially() {
        let parser = CommonDirectiveParser::new();
        assert!(!parser.in_multiline_desc);
        assert!(parser.multiline_content.is_empty());
    }
}

/// Validates diagram header and handles first line processing
///
/// # Arguments
/// * `line` - The input line to validate
/// * `line_num` - Line number for error reporting (0-based)
/// * `expected_headers` - Array of valid diagram headers to match against
/// * `first_line_processed` - Modified to true when a valid header is found
///
/// # Returns
/// * `Ok((true, trimmed_line))` if line was handled (skip to next line)
/// * `Ok((false, trimmed_line))` if line should be processed by parser  
/// * `Err()` if invalid header found
pub fn validate_diagram_header<'a>(
    line: &'a str,
    line_num: usize,
    expected_headers: &[&str],
    first_line_processed: &mut bool,
) -> crate::error::Result<(bool, &'a str)> {
    use crate::error::ParseError;

    assert!(
        !expected_headers.is_empty(),
        "Expected headers cannot be empty"
    );

    let trimmed = line.trim();

    if *first_line_processed {
        return Ok((false, trimmed));
    }

    if should_skip_line(trimmed) {
        return Ok((true, trimmed)); // Skip this line
    }

    if !expected_headers.iter().any(|h| trimmed.starts_with(h)) {
        return Err(ParseError::SyntaxError {
            message: format!("Expected {} header", expected_headers.join(" or ")),
            expected: expected_headers.iter().map(|s| s.to_string()).collect(),
            found: trimmed.to_string(),
            line: line_num + 1,
            column: 1,
        });
    }

    *first_line_processed = true;
    Ok((true, trimmed))
}

/// Validates diagram header using DiagramType enum for better type safety
///
/// # Arguments
/// * `line` - The input line to validate
/// * `line_num` - Line number for error reporting (0-based)
/// * `diagram_type` - The expected diagram type
/// * `first_line_processed` - Modified to true when a valid header is found
///
/// # Returns
/// * `Ok((true, trimmed_line))` if line was handled (skip to next line)
/// * `Ok((false, trimmed_line))` if line should be processed by parser  
/// * `Err()` if invalid header found
pub fn validate_diagram_header_typed<'a>(
    line: &'a str,
    line_num: usize,
    diagram_type: DiagramType,
    first_line_processed: &mut bool,
) -> crate::error::Result<(bool, &'a str)> {
    validate_diagram_header(line, line_num, diagram_type.headers(), first_line_processed)
}

/// Checks if a line should be skipped (empty, comments)
pub fn should_skip_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%")
}

#[cfg(test)]
mod header_validation_tests {
    use super::*;
    use crate::error::ParseError;

    /// Test helper to validate successful header processing
    fn assert_header_success(line: &str, expected_headers: &[&str], expected_trimmed: &str) {
        let mut first_line = false;
        let result = validate_diagram_header(line, 0, expected_headers, &mut first_line);
        assert!(result.is_ok(), "Expected success for line: {}", line);
        let (should_skip, trimmed) = result.unwrap();
        assert!(should_skip, "Expected to skip line: {}", line);
        assert_eq!(
            trimmed, expected_trimmed,
            "Trimmed content mismatch for line: {}",
            line
        );
        assert!(first_line, "Expected first_line to be marked as processed");
    }

    /// Test helper to validate header skipping (for comments, empty lines)
    fn assert_header_skip_without_processing(
        line: &str,
        expected_headers: &[&str],
        expected_trimmed: &str,
    ) {
        let mut first_line = false;
        let result = validate_diagram_header(line, 0, expected_headers, &mut first_line);
        assert!(result.is_ok(), "Expected success for line: {}", line);
        let (should_skip, trimmed) = result.unwrap();
        assert!(should_skip, "Expected to skip line: {}", line);
        assert_eq!(
            trimmed, expected_trimmed,
            "Trimmed content mismatch for line: {}",
            line
        );
        assert!(!first_line, "Expected first_line to remain unprocessed");
    }

    /// Test helper to validate header error cases
    fn assert_header_error(line: &str, expected_headers: &[&str], expected_message: &str) {
        let mut first_line = false;
        let result = validate_diagram_header(line, 0, expected_headers, &mut first_line);
        assert!(result.is_err(), "Expected error for line: {}", line);
        if let Err(ParseError::SyntaxError { message, .. }) = result {
            assert_eq!(
                message, expected_message,
                "Error message mismatch for line: {}",
                line
            );
        } else {
            panic!("Expected SyntaxError for line: {}", line);
        }
        assert!(
            !first_line,
            "Expected first_line to remain unprocessed on error"
        );
    }

    /// Test helper to validate processed line handling
    fn assert_already_processed(line: &str, expected_headers: &[&str], expected_trimmed: &str) {
        let mut first_line = true; // Already processed
        let result = validate_diagram_header(line, 0, expected_headers, &mut first_line);
        assert!(
            result.is_ok(),
            "Expected success for processed line: {}",
            line
        );
        let (should_skip, trimmed) = result.unwrap();
        assert!(
            !should_skip,
            "Expected to process line when already processed: {}",
            line
        );
        assert_eq!(
            trimmed, expected_trimmed,
            "Trimmed content mismatch for line: {}",
            line
        );
    }

    #[test]
    fn test_validate_diagram_header_success() {
        assert_header_success(
            "architecture",
            &["architecture", "architecture-beta"],
            "architecture",
        );
    }

    #[test]
    fn test_validate_diagram_header_success_beta() {
        assert_header_success(
            "architecture-beta",
            &["architecture", "architecture-beta"],
            "architecture-beta",
        );
    }

    #[test]
    fn test_validate_diagram_header_invalid() {
        assert_header_error("invalid", &["architecture"], "Expected architecture header");
    }

    #[test]
    fn test_validate_diagram_header_skips_processed() {
        assert_already_processed("anything", &["architecture"], "anything");
    }

    #[test]
    fn test_validate_diagram_header_skips_comments() {
        // Test // comment
        assert_header_skip_without_processing(
            "// this is a comment",
            &["architecture"],
            "// this is a comment",
        );

        // Test %% comment
        assert_header_skip_without_processing(
            "%% this is a comment",
            &["architecture"],
            "%% this is a comment",
        );
    }

    #[test]
    fn test_validate_diagram_header_skips_empty() {
        // Test empty line
        assert_header_skip_without_processing("", &["architecture"], "");

        // Test whitespace only
        assert_header_skip_without_processing("   \t   ", &["architecture"], "");
    }

    #[test]
    fn test_should_skip_line() {
        assert!(should_skip_line(""));
        assert!(should_skip_line("   "));
        assert!(should_skip_line("\t\t"));
        assert!(should_skip_line("// comment"));
        assert!(should_skip_line("%% comment"));
        assert!(should_skip_line("  // spaced comment"));
        assert!(should_skip_line("  %% spaced comment"));
        assert!(!should_skip_line("actual content"));
        assert!(!should_skip_line("pie"));
        assert!(!should_skip_line("architecture"));
    }

    #[test]
    fn test_validate_multiple_headers() {
        // Test state diagram v1
        assert_header_success(
            "stateDiagram",
            &["stateDiagram", "stateDiagram-v2"],
            "stateDiagram",
        );

        // Test v2
        assert_header_success(
            "stateDiagram-v2",
            &["stateDiagram", "stateDiagram-v2"],
            "stateDiagram-v2",
        );
    }

    #[test]
    fn test_validate_pie_header() {
        assert_header_success("pie", &["pie"], "pie");
    }

    #[test]
    fn test_validate_header_with_content() {
        assert_header_success("pie title Chart Name", &["pie"], "pie title Chart Name");
    }

    #[test]
    #[should_panic(expected = "Expected headers cannot be empty")]
    fn test_validate_empty_expected_headers() {
        let mut first_line = false;
        let _ = validate_diagram_header(
            "pie",
            0,
            &[], // Empty array should trigger assertion
            &mut first_line,
        );
    }

    #[test]
    fn test_validate_header_with_mixed_case() {
        let mut first_line = false;
        let result = validate_diagram_header("PIE title Chart", 0, &["pie"], &mut first_line);
        // Should fail because header matching is case-sensitive
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError { message, .. }) = result {
            assert_eq!(message, "Expected pie header");
        }
    }

    #[test]
    fn test_validate_header_case_sensitive_success() {
        assert_header_success("pie title Chart", &["pie"], "pie title Chart");
    }

    #[test]
    fn test_validate_header_with_unicode() {
        assert_header_success(
            "pie title 📊 Unicode Chart",
            &["pie"],
            "pie title 📊 Unicode Chart",
        );
    }

    #[test]
    fn test_validate_header_with_special_characters() {
        assert_header_success(
            "pie title Chart: 50% Complete & Ready!",
            &["pie"],
            "pie title Chart: 50% Complete & Ready!",
        );
    }

    #[test]
    fn test_validate_unicode_header() {
        assert_header_success("🥧", &["🥧"], "🥧");
    }

    #[test]
    fn test_validate_header_with_tabs_and_spaces() {
        assert_header_success("\t  pie   title  Chart  \t", &["pie"], "pie   title  Chart");
    }

    #[test]
    fn test_validate_multiple_headers_error_message() {
        assert_header_error(
            "invalid",
            &["packet-beta", "packet"],
            "Expected packet-beta or packet header",
        );
    }

    #[test]
    fn test_validate_header_prefix_matching() {
        // Test that "pie-chart" matches "pie" header
        assert_header_success("pie-chart", &["pie"], "pie-chart");
    }

    #[test]
    fn test_validate_header_no_partial_match() {
        // Test that "pi" does not match "pie" header
        assert_header_error("pi", &["pie"], "Expected pie header");
    }

    #[test]
    fn test_diagram_type_headers() {
        assert_eq!(
            DiagramType::Architecture.headers(),
            &["architecture", "architecture-beta"]
        );
        assert_eq!(DiagramType::Pie.headers(), &["pie"]);
        assert_eq!(
            DiagramType::State.headers(),
            &["stateDiagram", "stateDiagram-v2"]
        );
        assert_eq!(DiagramType::Packet.headers(), &["packet-beta", "packet"]);
    }

    #[test]
    fn test_diagram_type_name() {
        assert_eq!(DiagramType::Architecture.name(), "architecture");
        assert_eq!(DiagramType::Pie.name(), "pie");
        assert_eq!(DiagramType::State.name(), "state");
        assert_eq!(DiagramType::Packet.name(), "packet");
    }

    #[test]
    fn test_validate_diagram_header_typed() {
        let mut first_line = false;
        let result =
            validate_diagram_header_typed("pie title Chart", 0, DiagramType::Pie, &mut first_line);
        assert!(result.is_ok());
        let (should_skip, trimmed) = result.unwrap();
        assert!(should_skip);
        assert_eq!(trimmed, "pie title Chart");
        assert!(first_line);
    }

    #[test]
    fn test_validate_diagram_header_typed_error() {
        let mut first_line = false;
        let result = validate_diagram_header_typed("invalid", 0, DiagramType::Pie, &mut first_line);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_diagram_header_typed_multiple_headers() {
        let mut first_line = false;

        // Test first variant
        let result =
            validate_diagram_header_typed("stateDiagram", 0, DiagramType::State, &mut first_line);
        assert!(result.is_ok());
        let (should_skip, trimmed) = result.unwrap();
        assert!(should_skip);
        assert_eq!(trimmed, "stateDiagram");
        assert!(first_line);

        // Test second variant
        first_line = false;
        let result = validate_diagram_header_typed(
            "stateDiagram-v2",
            0,
            DiagramType::State,
            &mut first_line,
        );
        assert!(result.is_ok());
        let (should_skip, trimmed) = result.unwrap();
        assert!(should_skip);
        assert_eq!(trimmed, "stateDiagram-v2");
        assert!(first_line);
    }
}
