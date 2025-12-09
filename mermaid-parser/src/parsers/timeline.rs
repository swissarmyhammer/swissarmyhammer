//! Timeline diagram parser implementation

use crate::common::ast::{AccessibilityInfo, TimelineDiagram, TimelineItem, TimelineSection};
use crate::common::parser_utils::{parse_comment, parse_whitespace, parse_whitespace_required};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum TimelineToken {
    Timeline,              // "timeline"
    Title(String),         // "title My Title"
    Section(String),       // "section Section Name"
    Period(String),        // Period text
    Event(String),         // ": Event text"
    AccTitleValue(String), // Accessibility title value
    AccDescrValue(String), // Accessibility description value
    NewLine,
}

crate::create_parser_fn! {
    pub fn parse(input: &str) -> Result<TimelineDiagram> {
        lexer: timeline_lexer,
        parser: timeline_parser,
        diagram_type: "timeline"
    }
}

fn timeline_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<TimelineToken>, extra::Err<Simple<'src, char>>> {
    // Comment lines starting with %% or # and extending to end of line
    let comment = choice((
        parse_comment().ignored(),
        just("#")
            .then(none_of('\n').repeated())
            .then(just('\n').or_not())
            .ignored(),
    ));

    let timeline_keyword = just("timeline").map(|_| TimelineToken::Timeline);

    let title = just("title")
        .padded_by(parse_whitespace())
        .ignore_then(none_of('\n').repeated().at_least(1).collect::<String>())
        .map(|text| TimelineToken::Title(text.trim().to_string()));

    let section = just("section")
        .padded_by(parse_whitespace())
        .ignore_then(none_of('\n').repeated().at_least(1).collect::<String>())
        .map(|text| TimelineToken::Section(text.trim().to_string()));

    let acc_title_line = just("accTitle")
        .then(parse_whitespace())
        .then(just(':'))
        .then(parse_whitespace())
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| TimelineToken::AccTitleValue(text.trim().to_string()));

    let acc_descr_line = just("accDescr")
        .then(parse_whitespace())
        .then(just(':'))
        .then(parse_whitespace())
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| TimelineToken::AccDescrValue(text.trim().to_string()));

    let event = just(':')
        .then(parse_whitespace_required())
        .ignore_then(none_of('\n').repeated().at_least(1).collect::<String>())
        .map(|text| TimelineToken::Event(text.trim().to_string()));

    // Period: any line that doesn't start with keywords or special characters
    let period = none_of('\n')
        .repeated()
        .at_least(1)
        .collect::<String>()
        .try_map(|text: String, span| {
            let trimmed = text.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("timeline")
                || trimmed.starts_with("title")
                || trimmed.starts_with("section")
                || trimmed.starts_with("accTitle")
                || trimmed.starts_with("accDescr")
                || trimmed.starts_with(':')
                || trimmed.starts_with("%%")
                || trimmed.starts_with('#')
            {
                Err(Simple::new(None, span))
            } else {
                Ok(TimelineToken::Period(trimmed.to_string()))
            }
        });

    let newline = choice((just("\n"), just("\r\n"))).map(|_| TimelineToken::NewLine);

    let token = choice((
        timeline_keyword,
        title,
        section,
        acc_title_line,
        acc_descr_line,
        event,
        period,
        newline,
    ));

    // Skip any leading whitespace/newlines before the first token
    let leading_ws = choice((one_of(" \t\n\r").ignored(), comment.clone())).repeated();

    leading_ws.ignore_then(
        choice((comment.map(|_| None), token.map(Some)))
            .padded_by(parse_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .map(|tokens| tokens.into_iter().flatten().collect()),
    )
}

fn timeline_parser<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    &'tokens [TimelineToken],
    TimelineDiagram,
    extra::Err<Simple<'tokens, TimelineToken>>,
> + Clone {
    let content_line = choice((
        select! { TimelineToken::Title(text) => Some(("title", text)) },
        select! { TimelineToken::AccTitleValue(text) => Some(("acc_title", text)) },
        select! { TimelineToken::AccDescrValue(text) => Some(("acc_descr", text)) },
        select! { TimelineToken::Section(text) => Some(("section", text)) },
        select! { TimelineToken::Period(text) => Some(("period", text)) },
        select! { TimelineToken::Event(text) => Some(("event", text)) },
        just(&TimelineToken::NewLine).to(None),
    ));

    just(&TimelineToken::Timeline)
        .then_ignore(just(&TimelineToken::NewLine).or_not())
        .ignore_then(content_line.repeated().collect::<Vec<_>>())
        .map(|lines| {
            let mut diagram = TimelineDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                sections: Vec::new(),
            };

            let mut current_section: Option<TimelineSection> = None;

            for line in lines.into_iter().flatten() {
                match line {
                    ("title", text) => diagram.title = Some(text),
                    ("acc_title", text) => diagram.accessibility.title = Some(text),
                    ("acc_descr", text) => diagram.accessibility.description = Some(text),
                    ("section", text) => {
                        // Push previous section if exists
                        if let Some(section) = current_section.take() {
                            diagram.sections.push(section);
                        }
                        current_section = Some(TimelineSection {
                            name: text,
                            items: Vec::new(),
                        });
                    }
                    ("period", text) => {
                        if let Some(ref mut section) = current_section {
                            section.items.push(TimelineItem::Period(text));
                        }
                    }
                    ("event", text) => {
                        if let Some(ref mut section) = current_section {
                            section.items.push(TimelineItem::Event(text));
                        }
                    }
                    _ => {}
                }
            }

            // Push final section if exists
            if let Some(section) = current_section {
                diagram.sections.push(section);
            }

            diagram
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_simple_timeline() {
        let input = r#"timeline
    title My Day
    section Morning
        Wake up
        : Brush teeth
    section Evening
        Dinner
        : Sleep
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse with error: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.title, Some("My Day".to_string()));
        assert_eq!(diagram.sections.len(), 2);
        assert_eq!(diagram.sections[0].name, "Morning");
        assert_eq!(diagram.sections[0].items.len(), 2);

        match &diagram.sections[0].items[0] {
            TimelineItem::Period(text) => assert_eq!(text, "Wake up"),
            _ => panic!("Expected period"),
        }

        match &diagram.sections[0].items[1] {
            TimelineItem::Event(text) => assert_eq!(text, "Brush teeth"),
            _ => panic!("Expected event"),
        }
    }

    #[test]
    fn test_accessibility() {
        let input = r#"timeline
    accTitle: Timeline Accessibility Title
    accDescr: This timeline shows my daily routine
    title My Day
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse with error: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(
            diagram.accessibility.title,
            Some("Timeline Accessibility Title".to_string())
        );
        assert_eq!(
            diagram.accessibility.description,
            Some("This timeline shows my daily routine".to_string())
        );
    }

    #[test]
    fn test_timeline_only_header() {
        let input = "timeline\n";

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse with error: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.title, None);
        assert_eq!(diagram.sections.len(), 0);
    }

    #[test]
    fn test_real_timeline_files() {
        let test_dir = Path::new("test/timeline");
        if test_dir.exists() {
            for entry in fs::read_dir(test_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mermaid") {
                    let content = fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

                    // Remove metadata comments
                    let content = content
                        .lines()
                        .filter(|line| !line.trim().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let result = parse(&content);
                    if let Err(e) = &result {
                        eprintln!("Error parsing file {:?}: {:?}", path, e);
                        eprintln!("Content after removing comments:\n{}", content);
                    }
                    assert!(result.is_ok(), "Failed to parse file: {:?}", path);

                    let _diagram = result.unwrap();
                    // Basic validation - empty timelines are valid
                    // Just ensure it parsed into a valid structure
                }
            }
        }
    }
}
