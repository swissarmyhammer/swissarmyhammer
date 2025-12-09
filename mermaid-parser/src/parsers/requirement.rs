//! Requirement diagram parser implementation

use crate::common::ast::{
    AccessibilityInfo, Element, RelationshipType, Requirement, RequirementDiagram,
    RequirementRelationship, RequirementType, RiskLevel, VerificationMethod,
};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum RequirementToken {
    RequirementDiagram,       // "requirementDiagram"
    Requirement,              // "requirement"
    FunctionalRequirement,    // "functionalRequirement"
    PerformanceRequirement,   // "performanceRequirement"
    InterfaceRequirement,     // "interfaceRequirement"
    PhysicalRequirement,      // "physicalRequirement"
    DesignConstraint,         // "designConstraint"
    Element,                  // "element"
    Id,                       // "id:"
    Text,                     // "text:"
    Risk,                     // "risk:"
    VerifyMethod,             // "verifymethod:"
    Type,                     // "type:"
    DocRef,                   // "docRef:"
    LeftBrace,                // {
    RightBrace,               // }
    Arrow,                    // ->
    BackArrow,                // <-
    Dash,                     // -
    Identifier(String),       // Names and values
    QuotedString(String),     // Quoted text
    RelationshipType(String), // satisfies, traces, etc.
    Comment(String),          // %% comment
    AccTitle,                 // accTitle:
    AccDescr,                 // accDescr: or accDescr {
    Direction,                // direction
    Style,                    // style
    ClassDef,                 // classDef
    Class,                    // class
    NewLine,
    Eof,
}

impl From<&RequirementToken> for String {
    fn from(token: &RequirementToken) -> Self {
        format!("{:?}", token)
    }
}

pub fn requirement_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<RequirementToken>, extra::Err<Simple<'src, char>>> {
    let whitespace = just(' ').or(just('\t')).repeated();

    let comment = just("%%")
        .then(none_of('\n').repeated())
        .map(|_| RequirementToken::Comment("".to_string()));

    // Keywords
    let requirement_diagram =
        text::keyword("requirementDiagram").map(|_| RequirementToken::RequirementDiagram);

    let requirement_types = choice((
        text::keyword("functionalRequirement").map(|_| RequirementToken::FunctionalRequirement),
        text::keyword("performanceRequirement").map(|_| RequirementToken::PerformanceRequirement),
        text::keyword("interfaceRequirement").map(|_| RequirementToken::InterfaceRequirement),
        text::keyword("physicalRequirement").map(|_| RequirementToken::PhysicalRequirement),
        text::keyword("designConstraint").map(|_| RequirementToken::DesignConstraint),
        text::keyword("requirement").map(|_| RequirementToken::Requirement),
    ));

    let element_keyword = text::keyword("element").map(|_| RequirementToken::Element);

    // Property keywords
    let properties = choice((
        text::keyword("id").map(|_| RequirementToken::Id),
        text::keyword("text").map(|_| RequirementToken::Text),
        text::keyword("risk").map(|_| RequirementToken::Risk),
        text::keyword("verifymethod").map(|_| RequirementToken::VerifyMethod),
        text::keyword("type").map(|_| RequirementToken::Type),
        text::keyword("docRef").map(|_| RequirementToken::DocRef),
        text::keyword("accTitle").map(|_| RequirementToken::AccTitle),
        text::keyword("accDescr").map(|_| RequirementToken::AccDescr),
        text::keyword("direction").map(|_| RequirementToken::Direction),
        text::keyword("style").map(|_| RequirementToken::Style),
        text::keyword("classDef").map(|_| RequirementToken::ClassDef),
        text::keyword("class").map(|_| RequirementToken::Class),
    ));

    // Colon separator
    let colon = just(':').map(|_| RequirementToken::Identifier(":".to_string()));

    // Relationship types
    let relationship_types = choice((
        text::keyword("contains"),
        text::keyword("copies"),
        text::keyword("derives"),
        text::keyword("satisfies"),
        text::keyword("verifies"),
        text::keyword("refines"),
        text::keyword("traces"),
    ))
    .map(|rel: &str| RequirementToken::RelationshipType(rel.to_string()));

    // Arrows and symbols
    let arrow = just("->").map(|_| RequirementToken::Arrow);
    let back_arrow = just("<-").map(|_| RequirementToken::BackArrow);
    let dash = just('-').map(|_| RequirementToken::Dash);

    let left_brace = just('{').map(|_| RequirementToken::LeftBrace);
    let right_brace = just('}').map(|_| RequirementToken::RightBrace);

    // Quoted string (can span multiple lines in requirements)
    let quoted_string = just('"')
        .ignore_then(none_of("\"").repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(RequirementToken::QuotedString);

    // Numbers (for IDs and values)
    let number = text::int(10).map(|s: &str| RequirementToken::Identifier(s.to_string()));

    // Multi-word identifier (for text values that can contain spaces and punctuation)
    let multi_word = none_of("\n{}")
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|s| RequirementToken::Identifier(s.trim().to_string()));

    // Simple identifier (alphanumeric with underscores, dots, slashes)
    // Also allow identifiers starting with underscores
    let identifier = choice((
        text::ident().map(|s: &str| s.to_string()),
        // Match identifiers that start with underscores
        just('_')
            .repeated()
            .at_least(1)
            .collect::<String>()
            .then(text::ident().or_not())
            .map(|(underscores, rest): (String, Option<&str>)| {
                let mut result = underscores;
                if let Some(r) = rest {
                    result.push_str(r);
                }
                result
            }),
    ))
    .map(|s: String| RequirementToken::Identifier(s));

    let newline = just('\n').map(|_| RequirementToken::NewLine);

    // Order matters - check requirement_diagram before requirement_types
    let token = choice((
        comment,
        requirement_diagram,
        requirement_types,
        element_keyword,
        properties,
        relationship_types,
        arrow,
        back_arrow,
        dash,
        left_brace,
        right_brace,
        quoted_string,
        colon,
        number,
        identifier,
        multi_word,
    ));

    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .collect::<Vec<_>>()
}

fn requirement_parser<'src>() -> impl Parser<
    'src,
    &'src [RequirementToken],
    RequirementDiagram,
    extra::Err<Simple<'src, RequirementToken>>,
> {
    // Parse requirementDiagram header (skip leading newlines)
    // Accept both "requirementDiagram" and "requirement" as headers
    let header = any()
        .filter(|t| matches!(t, RequirementToken::NewLine))
        .repeated()
        .ignore_then(choice((
            just(RequirementToken::RequirementDiagram),
            just(RequirementToken::Requirement).filter(|_| true), // Accept "requirement" as standalone header
        )))
        .then_ignore(
            any()
                .filter(|t| matches!(t, RequirementToken::NewLine))
                .repeated(),
        );

    // Parse requirement type tokens
    let requirement_type_token = any().try_map(|t, span| match t {
        RequirementToken::Requirement => Ok(RequirementType::Requirement),
        RequirementToken::FunctionalRequirement => Ok(RequirementType::FunctionalRequirement),
        RequirementToken::PerformanceRequirement => Ok(RequirementType::PerformanceRequirement),
        RequirementToken::InterfaceRequirement => Ok(RequirementType::InterfaceRequirement),
        RequirementToken::PhysicalRequirement => Ok(RequirementType::PhysicalRequirement),
        RequirementToken::DesignConstraint => Ok(RequirementType::DesignConstraint),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse identifier (can be regular identifier or quoted string)
    let identifier = any().try_map(|t, span| match t {
        RequirementToken::Identifier(name) => Ok(name),
        RequirementToken::QuotedString(name) => Ok(name),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse quoted string
    let _quoted_string = any::<&[RequirementToken], extra::Err<Simple<RequirementToken>>>()
        .try_map(|t, span| match t {
            RequirementToken::QuotedString(s) => Ok(s),
            _ => Err(Simple::new(Some(t.into()), span)),
        });

    // Parse requirement definition
    let requirement_def = requirement_type_token
        .then(identifier)
        // Optional class specification (:::classname)
        .then_ignore(
            just(RequirementToken::Identifier(":".to_string()))
                .repeated()
                .at_most(3)
                .then(identifier.or_not())
                .or_not(),
        )
        .then_ignore(just(RequirementToken::LeftBrace))
        .then(
            // Parse requirement properties
            any()
                .filter(|t| !matches!(t, RequirementToken::RightBrace))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(RequirementToken::RightBrace))
        .map(|((req_type, name), tokens)| {
            let mut id = String::new();
            let mut text = String::new();
            let mut risk = None;
            let mut verify_method = None;

            let mut i = 0;
            while i < tokens.len() {
                match &tokens[i] {
                    RequirementToken::Id => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        if i < tokens.len() {
                            if let RequirementToken::Identifier(val) = &tokens[i] {
                                id = val.clone();
                            }
                        }
                        i += 1;
                    }
                    RequirementToken::Text => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        // Collect multiple tokens until we hit a newline or property keyword
                        let mut text_parts = Vec::new();
                        while i < tokens.len() {
                            match &tokens[i] {
                                RequirementToken::Identifier(val) => {
                                    text_parts.push(val.clone());
                                    i += 1;
                                }
                                RequirementToken::QuotedString(val) => {
                                    text_parts.push(val.clone());
                                    i += 1;
                                    break; // Quoted string is complete
                                }
                                RequirementToken::NewLine => {
                                    break; // End of line, stop collecting
                                }
                                // Treat these tokens as text content, not keywords in this context
                                RequirementToken::Text => {
                                    text_parts.push("text".to_string());
                                    i += 1;
                                }
                                RequirementToken::Risk => {
                                    text_parts.push("risk".to_string());
                                    i += 1;
                                }
                                RequirementToken::Type => {
                                    text_parts.push("type".to_string());
                                    i += 1;
                                }
                                // Stop at property keywords that are likely to be actual properties
                                RequirementToken::Id
                                | RequirementToken::VerifyMethod
                                | RequirementToken::DocRef => {
                                    break;
                                }
                                _ => {
                                    // Hit other special tokens, stop collecting
                                    break;
                                }
                            }
                        }
                        // Join parts intelligently - don't add spaces before punctuation
                        text = text_parts.into_iter().fold(String::new(), |mut acc, part| {
                            if acc.is_empty() {
                                acc.push_str(&part);
                            } else if part.starts_with('.')
                                || part.starts_with(',')
                                || part.starts_with('!')
                                || part.starts_with('?')
                            {
                                acc.push_str(&part); // No space before punctuation
                            } else {
                                acc.push(' ');
                                acc.push_str(&part);
                            }
                            acc
                        });
                    }
                    RequirementToken::Risk => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        if i < tokens.len() {
                            if let RequirementToken::Identifier(val) = &tokens[i] {
                                risk = match val.to_lowercase().as_str() {
                                    "low" => Some(RiskLevel::Low),
                                    "medium" => Some(RiskLevel::Medium),
                                    "high" => Some(RiskLevel::High),
                                    _ => None,
                                };
                            }
                        }
                        i += 1;
                    }
                    RequirementToken::VerifyMethod => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        if i < tokens.len() {
                            if let RequirementToken::Identifier(val) = &tokens[i] {
                                verify_method = match val.to_lowercase().as_str() {
                                    "analysis" => Some(VerificationMethod::Analysis),
                                    "inspection" => Some(VerificationMethod::Inspection),
                                    "test" => Some(VerificationMethod::Test),
                                    "demonstration" => Some(VerificationMethod::Demonstration),
                                    _ => None,
                                };
                            }
                        }
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }

            Requirement {
                name,
                req_type,
                id,
                text,
                risk,
                verify_method,
            }
        });

    // Parse element definition
    let element_def = just(RequirementToken::Element)
        .ignore_then(identifier)
        .then_ignore(just(RequirementToken::LeftBrace))
        .then(
            any()
                .filter(|t| !matches!(t, RequirementToken::RightBrace))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(RequirementToken::RightBrace))
        .map(|(name, tokens)| {
            let mut element_type = String::new();
            let mut doc_ref = None;

            let mut i = 0;
            while i < tokens.len() {
                match &tokens[i] {
                    RequirementToken::Type => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        // Collect multiple tokens for element type
                        let mut type_parts = Vec::new();
                        while i < tokens.len() {
                            match &tokens[i] {
                                RequirementToken::Identifier(val) => {
                                    type_parts.push(val.clone());
                                    i += 1;
                                }
                                RequirementToken::QuotedString(val) => {
                                    type_parts.push(val.clone());
                                    i += 1;
                                    break; // Quoted string is complete
                                }
                                RequirementToken::NewLine => {
                                    break; // End of line, stop collecting
                                }
                                // Stop at property keywords
                                RequirementToken::DocRef => {
                                    break;
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                        element_type =
                            type_parts.into_iter().fold(String::new(), |mut acc, part| {
                                if acc.is_empty() {
                                    acc.push_str(&part);
                                } else if part.starts_with('.')
                                    || part.starts_with(',')
                                    || part.starts_with('!')
                                    || part.starts_with('?')
                                {
                                    acc.push_str(&part); // No space before punctuation
                                } else {
                                    acc.push(' ');
                                    acc.push_str(&part);
                                }
                                acc
                            });
                    }
                    RequirementToken::DocRef => {
                        i += 1;
                        // Skip colon if present
                        if i < tokens.len()
                            && matches!(&tokens[i], RequirementToken::Identifier(s) if s == ":")
                        {
                            i += 1;
                        }
                        // Collect multiple tokens for doc ref
                        let mut ref_parts = Vec::new();
                        while i < tokens.len() {
                            match &tokens[i] {
                                RequirementToken::Identifier(val) => {
                                    ref_parts.push(val.clone());
                                    i += 1;
                                }
                                RequirementToken::QuotedString(val) => {
                                    ref_parts.push(val.clone());
                                    i += 1;
                                    break; // Quoted string is complete
                                }
                                RequirementToken::NewLine => {
                                    break; // End of line, stop collecting
                                }
                                // Stop at property keywords
                                RequirementToken::Type => {
                                    break;
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                        if !ref_parts.is_empty() {
                            doc_ref = Some(ref_parts.join(""));
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }

            Element {
                name,
                element_type,
                doc_ref,
            }
        });

    // Parse relationship
    let relationship_def = identifier
        .then(choice((
            just(RequirementToken::Dash)
                .ignore_then(any().try_map(|t, span| match t {
                    RequirementToken::RelationshipType(rel) => Ok((rel, false)),
                    _ => Err(Simple::new(Some(t.into()), span)),
                }))
                .then_ignore(just(RequirementToken::Arrow)),
            just(RequirementToken::BackArrow)
                .ignore_then(any().try_map(|t, span| match t {
                    RequirementToken::RelationshipType(rel) => Ok((rel, true)),
                    _ => Err(Simple::new(Some(t.into()), span)),
                }))
                .then_ignore(just(RequirementToken::Dash)),
        )))
        .then(identifier)
        .map(|((source, (rel_str, is_reverse)), target)| {
            let rel_type = match rel_str.as_str() {
                "contains" => RelationshipType::Contains,
                "copies" => RelationshipType::Copies,
                "derives" => RelationshipType::Derives,
                "satisfies" => RelationshipType::Satisfies,
                "verifies" => RelationshipType::Verifies,
                "refines" => RelationshipType::Refines,
                "traces" => RelationshipType::Traces,
                _ => RelationshipType::Traces, // Default
            };

            if is_reverse {
                RequirementRelationship {
                    source: target,
                    target: source,
                    relationship_type: rel_type,
                }
            } else {
                RequirementRelationship {
                    source,
                    target,
                    relationship_type: rel_type,
                }
            }
        });

    // Parse accessibility directives
    let acc_title_def = just(RequirementToken::AccTitle)
        .ignore_then(just(RequirementToken::Identifier(":".to_string())).or_not())
        .ignore_then(
            any()
                .filter(|t| !matches!(t, RequirementToken::NewLine))
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(|tokens| {
            let title = tokens
                .into_iter()
                .filter_map(|t| match t {
                    RequirementToken::Identifier(s) | RequirementToken::QuotedString(s) => Some(s),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            ("acc_title".to_string(), title)
        });

    let acc_descr_def =
        just(RequirementToken::AccDescr)
            .ignore_then(just(RequirementToken::Identifier(":".to_string())).or_not())
            .then(choice((
                // Single line description
                any()
                    .filter(|t| !matches!(t, RequirementToken::NewLine))
                    .repeated()
                    .at_least(1)
                    .collect::<Vec<_>>()
                    .map(|tokens| {
                        tokens
                            .into_iter()
                            .filter_map(|t| match t {
                                RequirementToken::Identifier(s)
                                | RequirementToken::QuotedString(s) => Some(s),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    }),
                // Multi-line description with braces
                just(RequirementToken::LeftBrace)
                    .ignore_then(
                        any()
                            .filter(|t| !matches!(t, RequirementToken::RightBrace))
                            .repeated()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(just(RequirementToken::RightBrace))
                    .map(|tokens| {
                        tokens
                            .into_iter()
                            .filter_map(|t| match t {
                                RequirementToken::Identifier(s)
                                | RequirementToken::QuotedString(s) => Some(s),
                                RequirementToken::NewLine => Some("\n".to_string()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string()
                    }),
            )))
            .map(|(_, descr)| ("acc_descr".to_string(), descr));

    // Parse direction directive (ignored, just for compatibility)
    let direction_def = just(RequirementToken::Direction).then(
        any()
            .filter(|t| !matches!(t, RequirementToken::NewLine))
            .repeated(),
    );

    // Parse style directive (ignored, just for compatibility)
    let style_def = just(RequirementToken::Style).then(
        any()
            .filter(|t| !matches!(t, RequirementToken::NewLine))
            .repeated(),
    );

    // Parse classDef directive (ignored, just for compatibility)
    let class_def_directive = just(RequirementToken::ClassDef).then(
        any()
            .filter(|t| !matches!(t, RequirementToken::NewLine))
            .repeated(),
    );

    // Parse class directive (ignored, just for compatibility)
    let class_directive = just(RequirementToken::Class).then(
        any()
            .filter(|t| !matches!(t, RequirementToken::NewLine))
            .repeated(),
    );

    // Main parser
    header
        .then(
            choice((
                requirement_def.map(|req| (req.name.clone(), Some(req), None, None, None)),
                element_def.map(|elem| (elem.name.clone(), None, Some(elem), None, None)),
                relationship_def.map(|rel| ("".to_string(), None, None, Some(rel), None)),
                acc_title_def
                    .map(|(key, val)| ("".to_string(), None, None, None, Some((key, val)))),
                acc_descr_def
                    .map(|(key, val)| ("".to_string(), None, None, None, Some((key, val)))),
                direction_def.map(|_| ("".to_string(), None, None, None, None)),
                style_def.map(|_| ("".to_string(), None, None, None, None)),
                class_def_directive.map(|_| ("".to_string(), None, None, None, None)),
                class_directive.map(|_| ("".to_string(), None, None, None, None)),
                any()
                    .filter(|t| {
                        matches!(t, RequirementToken::NewLine | RequirementToken::Comment(_))
                    })
                    .map(|_| ("".to_string(), None, None, None, None)),
            ))
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(_, items)| {
            let mut requirements = HashMap::new();
            let mut elements = HashMap::new();
            let mut relationships = Vec::new();
            let mut accessibility = AccessibilityInfo::default();

            for (name, req, elem, rel, acc) in items {
                if let Some(r) = req {
                    requirements.insert(name, r);
                } else if let Some(e) = elem {
                    elements.insert(name, e);
                } else if let Some(r) = rel {
                    relationships.push(r);
                } else if let Some((key, val)) = acc {
                    match key.as_str() {
                        "acc_title" => accessibility.title = Some(val),
                        "acc_descr" => accessibility.description = Some(val),
                        _ => {}
                    }
                }
            }

            RequirementDiagram {
                title: None,
                accessibility,
                requirements,
                elements,
                relationships,
            }
        })
}

/// Parse requirement diagram from input string
pub fn parse(input: &str) -> Result<RequirementDiagram> {
    // Strip metadata comments before parsing
    let clean_input = crate::common::lexer::strip_metadata_comments(input);

    let tokens = requirement_lexer()
        .parse(&clean_input)
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to tokenize requirement diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    let parser = requirement_parser();
    let result = parser
        .parse(&tokens[..])
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to parse requirement diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        });

    result
}
