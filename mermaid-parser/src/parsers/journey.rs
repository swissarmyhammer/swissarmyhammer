use crate::common::ast::{AccessibilityInfo, JourneyDiagram, JourneySection, JourneyTask};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum JourneyToken {
    Journey,               // "journey"
    Title(String),         // "title My Title"
    Section(String),       // "section Section Name"
    TaskName(String),      // Task name
    TaskData(String),      // ": score: Actor1, Actor2"
    Colon,                 // ":"
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title value
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description value
    NewLine,
    Eof,
}

pub fn parse(input: &str) -> Result<JourneyDiagram> {
    let tokens =
        journey_lexer()
            .parse(input)
            .into_result()
            .map_err(|e| ParseError::SyntaxError {
                message: "Failed to tokenize journey diagram".to_string(),
                expected: vec![],
                found: format!("{:?}", e),
                line: 0,
                column: 0,
            })?;

    parse_journey_diagram(&tokens)
}

fn parse_journey_diagram(tokens: &[JourneyToken]) -> Result<JourneyDiagram> {
    let mut i = 0;

    // Find and skip the "journey" header
    while i < tokens.len() && !matches!(tokens[i], JourneyToken::Journey) {
        i += 1;
    }

    if i >= tokens.len() {
        return Err(ParseError::SyntaxError {
            message: "Expected 'journey' keyword".to_string(),
            expected: vec!["journey".to_string()],
            found: "end of input".to_string(),
            line: 0,
            column: 0,
        });
    }

    i += 1; // Skip "journey"

    // Skip optional newline after journey
    if i < tokens.len() && matches!(tokens[i], JourneyToken::NewLine) {
        i += 1;
    }

    let mut diagram = JourneyDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        sections: Vec::new(),
    };

    let mut current_section: Option<JourneySection> = None;
    let mut pending_task: Option<String> = None;

    while i < tokens.len() {
        match &tokens[i] {
            JourneyToken::Title(title) => {
                diagram.title = Some(title.clone());
            }
            JourneyToken::Section(name) => {
                // Save any current section
                if let Some(section) = current_section.take() {
                    diagram.sections.push(section);
                }
                current_section = Some(JourneySection {
                    name: name.clone(),
                    tasks: Vec::new(),
                });
            }
            JourneyToken::TaskName(name) => {
                pending_task = Some(name.clone());
            }
            JourneyToken::TaskData(data) => {
                // We have task data, create a task
                let task_name = if let Some(name) = pending_task.take() {
                    name
                } else {
                    "Unnamed Task".to_string()
                };

                let (score, actors) = parse_task_data(data);

                let task = JourneyTask {
                    name: task_name,
                    score,
                    actors,
                };

                if let Some(ref mut section) = current_section {
                    section.tasks.push(task);
                } else {
                    // Create a default section if none exists
                    current_section = Some(JourneySection {
                        name: "Default".to_string(),
                        tasks: vec![task],
                    });
                }
            }
            JourneyToken::AccTitleValue(title) => {
                diagram.accessibility.title = Some(title.clone());
            }
            JourneyToken::AccDescrValue(descr) => {
                diagram.accessibility.description = Some(descr.clone());
            }
            _ => {
                // Skip other tokens
            }
        }
        i += 1;
    }

    // Save any remaining section
    if let Some(section) = current_section {
        diagram.sections.push(section);
    }

    Ok(diagram)
}

fn parse_task_data(data: &str) -> (i32, Vec<String>) {
    let parts: Vec<&str> = data.split(':').collect();

    if parts.len() >= 2 {
        let score = parts[0].trim().parse::<i32>().unwrap_or(0);
        let actors: Vec<String> = parts[1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        (score, actors)
    } else {
        (0, vec![])
    }
}

fn journey_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<JourneyToken>, extra::Err<Simple<'src, char>>> {
    let whitespace = one_of(" \t").repeated();

    let comment = just("%")
        .then(just("%"))
        .then(none_of('\n').repeated())
        .ignored();

    let journey_keyword = text::keyword("journey").map(|_| JourneyToken::Journey);

    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| JourneyToken::Title(text.trim().to_string()));

    let section = text::keyword("section")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| JourneyToken::Section(text.trim().to_string()));

    let acc_title = text::keyword("accTitle")
        .then(choice((
            just(':').then(whitespace.at_least(1)).to(()),
            whitespace.at_least(1).to(()),
        )))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| JourneyToken::AccTitleValue(text.trim().to_string()));

    let acc_descr = text::keyword("accDescr")
        .then(choice((
            just(':').then(whitespace.at_least(1)).to(()),
            whitespace.at_least(1).to(()),
        )))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| JourneyToken::AccDescrValue(text.trim().to_string()));

    let task_data = just(':')
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|text| JourneyToken::TaskData(text.trim().to_string()));

    let task_name = none_of("\n:")
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| JourneyToken::TaskName(text.trim().to_string()))
        .filter(|token| {
            if let JourneyToken::TaskName(text) = token {
                !text.is_empty()
                    && !text.starts_with("journey")
                    && !text.starts_with("title")
                    && !text.starts_with("section")
                    && !text.starts_with("accTitle")
                    && !text.starts_with("accDescr")
            } else {
                true
            }
        });

    let colon = just(':').map(|_| JourneyToken::Colon);

    let newline = choice((just("\n"), just("\r\n"))).map(|_| JourneyToken::NewLine);

    let token = choice((
        journey_keyword,
        title,
        section,
        acc_title,
        acc_descr,
        task_data,
        task_name,
        colon,
        newline,
    ));

    // Skip any leading whitespace/newlines before the first token
    let leading_ws = choice((one_of(" \t\n\r").ignored(), comment)).repeated();

    leading_ws.ignore_then(
        choice((comment.map(|_| None), token.map(Some)))
            .padded_by(whitespace)
            .repeated()
            .collect::<Vec<_>>()
            .map(|tokens| tokens.into_iter().flatten().collect()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_journey() {
        let input = r#"journey
    title My working day
    section Go to work
        Make tea: 5: Me
        Go upstairs: 3: Me
        Do work: 1: Me, Cat
    section Go home
        Go downstairs: 5: Me
        Sit down: 3: Me
"#;

        let result = parse(input);
        assert!(
            result.is_ok(),
            "Failed to parse simple journey: {:?}",
            result
        );

        let diagram = result.unwrap();
        assert_eq!(diagram.title, Some("My working day".to_string()));
        assert_eq!(diagram.sections.len(), 2);

        // Check first section
        assert_eq!(diagram.sections[0].name, "Go to work");
        assert_eq!(diagram.sections[0].tasks.len(), 3);

        let first_task = &diagram.sections[0].tasks[0];
        assert_eq!(first_task.name, "Make tea");
        assert_eq!(first_task.score, 5);
        assert_eq!(first_task.actors, vec!["Me"]);

        let third_task = &diagram.sections[0].tasks[2];
        assert_eq!(third_task.name, "Do work");
        assert_eq!(third_task.score, 1);
        assert_eq!(third_task.actors, vec!["Me", "Cat"]);
    }

    #[test]
    fn test_task_data_parsing() {
        let (score, actors) = parse_task_data("5: Me");
        assert_eq!(score, 5);
        assert_eq!(actors, vec!["Me"]);

        let (score, actors) = parse_task_data("3: Me, Cat, Dog");
        assert_eq!(score, 3);
        assert_eq!(actors, vec!["Me", "Cat", "Dog"]);

        let (score, actors) = parse_task_data("invalid");
        assert_eq!(score, 0);
        assert_eq!(actors.len(), 0);
    }

    #[test]
    fn test_accessibility() {
        let input = r#"journey
    accTitle My Journey Accessibility Title
    accDescr This journey shows user satisfaction
    title My Day
"#;

        let result = parse(input);
        assert!(
            result.is_ok(),
            "Failed to parse accessibility: {:?}",
            result
        );

        let diagram = result.unwrap();
        assert_eq!(
            diagram.accessibility.title,
            Some("My Journey Accessibility Title".to_string())
        );
        assert_eq!(
            diagram.accessibility.description,
            Some("This journey shows user satisfaction".to_string())
        );
    }

    #[test]
    fn test_minimal_journey() {
        let input = r#"journey"#;

        let result = parse(input);
        assert!(
            result.is_ok(),
            "Failed to parse minimal journey: {:?}",
            result
        );

        let diagram = result.unwrap();
        assert_eq!(diagram.sections.len(), 0);
    }
}
