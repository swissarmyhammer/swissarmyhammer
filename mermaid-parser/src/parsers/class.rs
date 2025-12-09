//! Class diagram parser implementation

use crate::common::ast::{AccessibilityInfo, Class, ClassDiagram};
use crate::common::parser_utils::{parse_comment, parse_whitespace};
use chumsky::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassToken {
    ClassDiagram,           // "classDiagram"
    Class,                  // "class"
    ClassName(String),      // Class identifier
    LeftBrace,              // {
    RightBrace,             // }
    LeftAngle,              // <
    RightAngle,             // >
    Pipe,                   // |
    Star,                   // *
    Circle,                 // o
    Dash,                   // -
    DashDash,               // --
    DotDot,                 // ..
    LeftParen,              // (
    RightParen,             // )
    Colon,                  // :
    Comma,                  // ,
    Plus,                   // +
    Minus,                  // -
    Hash,                   // #
    Tilde,                  // ~
    Dollar,                 // $
    QuotedString(String),   // "text"
    StereotypeStart,        // <<
    StereotypeEnd,          // >>
    StereotypeName(String), // interface, abstract, etc.
    TypeName(String),       // int, String, etc.
    Identifier(String),     // General identifier
    Cardinality(String),    // 1, *, 0..1, etc.
    Inheritance,            // <|--
    Composition,            // *--
    Aggregation,            // o--
    Association,            // <--
    Dependency,             // <..
    Realization,            // <|..
    Comment(String),        // %% comment
    NewLine,
    Eof,
}

fn class_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<ClassToken>, extra::Err<Simple<'src, char>>> {
    let comment = parse_comment().map(|_| ClassToken::Comment("".to_string()));

    let class_diagram = just("classDiagram").map(|_| ClassToken::ClassDiagram);
    let class_keyword = just("class").map(|_| ClassToken::Class);

    // Relationship symbols (order matters for overlapping patterns - longer first)
    let relationships = choice((
        just("<|--").to(ClassToken::Inheritance),
        just("<|..").to(ClassToken::Realization),
        just("*--").to(ClassToken::Composition),
        just("o--").to(ClassToken::Aggregation),
        just("<--").to(ClassToken::Association),
        just("<..").to(ClassToken::Dependency),
        just("--").to(ClassToken::DashDash),
        just("..").to(ClassToken::DotDot),
    ));

    // Stereotypes (must come before individual < and > tokens)
    let stereotype = just("<<")
        .ignore_then(none_of('>').repeated().collect::<String>())
        .then_ignore(just(">>"))
        .map(|name: String| ClassToken::StereotypeName(name.trim().to_string()));

    // Simple identifier (must come after keywords)
    let identifier = text::ident().map(|s: &str| ClassToken::Identifier(s.to_string()));

    let newline = just('\n').map(|_| ClassToken::NewLine);

    let token = choice((
        comment,
        class_diagram,
        class_keyword,
        relationships,
        stereotype,
        just('(').to(ClassToken::LeftParen),
        just(')').to(ClassToken::RightParen),
        just('{').to(ClassToken::LeftBrace),
        just('}').to(ClassToken::RightBrace),
        just(',').to(ClassToken::Comma),
        just('+').to(ClassToken::Plus),
        just('-').to(ClassToken::Minus),
        just('#').to(ClassToken::Hash),
        just('~').to(ClassToken::Tilde),
        just(':').to(ClassToken::Colon),
        just('|').to(ClassToken::Pipe),
        just('*').to(ClassToken::Star),
        just('o').to(ClassToken::Circle),
        just('<').to(ClassToken::LeftAngle),
        just('>').to(ClassToken::RightAngle),
        just('"')
            .ignore_then(none_of('"').repeated().collect::<String>())
            .then_ignore(just('"'))
            .map(ClassToken::QuotedString),
        identifier,
    ));

    // Handle whitespace separately from tokens
    parse_whitespace()
        .ignore_then(token)
        .or(newline)
        .repeated()
        .collect::<Vec<_>>()
}

fn class_parser<'src>(
) -> impl Parser<'src, &'src [ClassToken], ClassDiagram, extra::Err<Simple<'src, ClassToken>>> {
    // Parse classDiagram header
    let header = just(ClassToken::ClassDiagram).then_ignore(
        any()
            .filter(|t| matches!(t, ClassToken::NewLine))
            .repeated(),
    );

    // Parse a simple class definition: "class ClassName"
    let simple_class = just(ClassToken::Class)
        .ignore_then(any().try_map(|t, span| match t {
            ClassToken::Identifier(name) => Ok(name),
            _ => Err(Simple::new(Some(t.into()), span)),
        }))
        .map(|name: String| Class {
            name: name.clone(),
            stereotype: None,
            members: Vec::new(),
            annotations: Vec::new(),
            css_class: None,
        });

    // Skip newlines and other tokens for now
    let skip_token = any().filter(|t| !matches!(t, ClassToken::Class));

    // Parse diagram content
    let content = choice((simple_class.map(Some), skip_token.map(|_| None)))
        .repeated()
        .collect::<Vec<_>>();

    header.ignore_then(content).map(|classes_opt| {
        let mut classes = HashMap::new();

        for class in classes_opt.into_iter().flatten() {
            classes.insert(class.name.clone(), class);
        }

        ClassDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            classes,
            relationships: Vec::new(),
            notes: Vec::new(),
        }
    })
}

crate::create_parser_fn! {
    pub fn parse(input: &str) -> Result<ClassDiagram> {
        lexer: class_lexer,
        parser: class_parser,
        diagram_type: "class"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_class_diagram_keyword() {
        let input = "classDiagram";
        let tokens = class_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], ClassToken::ClassDiagram);
    }

    #[test]
    fn test_lexer_class_keyword() {
        let input = "classDiagram\nclass Animal";
        let tokens = class_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Should have: classDiagram, newline, class, identifier
        assert!(
            tokens.len() >= 4,
            "Expected at least 4 tokens, got: {:?}",
            tokens
        );
        assert_eq!(tokens[0], ClassToken::ClassDiagram);
        assert_eq!(tokens[1], ClassToken::NewLine);
        assert_eq!(tokens[2], ClassToken::Class);
        assert_eq!(tokens[3], ClassToken::Identifier("Animal".to_string()));
    }

    #[test]
    fn test_lexer_visibility_and_braces() {
        let input = "class Animal {\n+int age\n-String name\n}";
        let tokens = class_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Should have: class, Animal, {, newline, +, int, age, newline, -, String, name, newline, }
        let expected_tokens = vec![
            ClassToken::Class,
            ClassToken::Identifier("Animal".to_string()),
            ClassToken::LeftBrace,
            ClassToken::NewLine,
            ClassToken::Plus,
            ClassToken::Identifier("int".to_string()),
            ClassToken::Identifier("age".to_string()),
            ClassToken::NewLine,
            ClassToken::Minus,
            ClassToken::Identifier("String".to_string()),
            ClassToken::Identifier("name".to_string()),
            ClassToken::NewLine,
            ClassToken::RightBrace,
        ];

        assert_eq!(
            tokens.len(),
            expected_tokens.len(),
            "Token count mismatch. Got: {:?}",
            tokens
        );
        for (i, (expected, actual)) in expected_tokens.iter().zip(tokens.iter()).enumerate() {
            assert_eq!(expected, actual, "Token mismatch at index {}", i);
        }
    }

    #[test]
    fn test_lexer_stereotypes() {
        let input = "class Animal {\n<<interface>>\n}";
        let tokens = class_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Find the stereotype token
        let stereotype_token = tokens
            .iter()
            .find(|token| matches!(token, ClassToken::StereotypeName(_)));
        assert!(
            stereotype_token.is_some(),
            "Should find stereotype token in: {:?}",
            tokens
        );

        if let Some(ClassToken::StereotypeName(name)) = stereotype_token {
            assert_eq!(name, "interface");
        }
    }

    #[test]
    fn test_lexer_relationships() {
        let input = "Animal <|-- Dog";
        let tokens = class_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Should have: Animal, <|--, Dog
        let expected_tokens = [
            ClassToken::Identifier("Animal".to_string()),
            ClassToken::Inheritance,
            ClassToken::Identifier("Dog".to_string()),
        ];

        assert_eq!(
            tokens.len(),
            expected_tokens.len(),
            "Token count mismatch. Got: {:?}",
            tokens
        );
        for (i, (expected, actual)) in expected_tokens.iter().zip(tokens.iter()).enumerate() {
            assert_eq!(expected, actual, "Token mismatch at index {}", i);
        }
    }

    #[test]
    fn test_parser_basic_class() {
        let input = "classDiagram\nclass Animal";

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert!(
            diagram.classes.contains_key("Animal"),
            "Should contain Animal class"
        );

        let animal = &diagram.classes["Animal"];
        assert_eq!(animal.name, "Animal");
        assert!(animal.members.is_empty());
        assert!(animal.stereotype.is_none());
    }

    #[test]
    fn test_simple_class_diagram() {
        let input = r#"classDiagram
    class Animal {
        +int age
        +String gender
    }
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let _diagram = result.unwrap();
        // For now, just verify it doesn't crash - will expand as we implement features
    }

    #[test]
    fn test_real_class_file() {
        let input = r#"classDiagram
    class Animal
    Vehicle <|-- Car
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse real file: {:?}", result);

        let diagram = result.unwrap();
        assert!(
            diagram.classes.contains_key("Animal"),
            "Should contain Animal class"
        );
        // Note: relationships not implemented yet, so just check classes
    }
}
