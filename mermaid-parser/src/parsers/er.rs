//! Entity Relationship diagram parser implementation

use crate::common::ast::{
    AccessibilityInfo, Attribute, CardinalityValue, Entity, ErCardinality, ErDiagram,
    ErRelationship, KeyType,
};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ERToken {
    ERDiagram,          // "erDiagram"
    EntityName(String), // Entity identifier
    EntityAlias {
        // alias[display name] or alias["display name"]
        alias: String,
        name: String,
    },
    RelSymbol(String),     // ||--||, ||--o{, etc.
    Label(String),         // : "relationship label"
    LeftBrace,             // {
    RightBrace,            // }
    AttributeType(String), // string, int, float, etc.
    AttributeName(String), // Attribute identifier
    KeyType(KeyType),      // PK, FK, UK
    QuotedString(String),  // "comment"
    Comment(String),       // %% comment
    AccTitle(String),      // accTitle: content
    AccDescr(String),      // accDescr: content
    Style(String),         // style directive content
    ClassDef(String),      // classDef directive content
    ClassAssignment {
        // entity:::class
        entity: String,
        class: String,
    },
    // Natural language relationship tokens
    To,                 // "to"
    Optionally,         // "optionally"
    Zero,               // "zero"
    One,                // "one"
    Many,               // "many"
    Only,               // "only"
    Exactly,            // "exactly"
    Or,                 // "or"
    More,               // "more"
    Number(String),     // "1", "0+", "1+", etc.
    ManyWithParam(i32), // many(0), many(1), etc.
    Colon,              // :
    Comma,              // ,
    NewLine,
    Eof,
}

impl From<&ERToken> for String {
    fn from(token: &ERToken) -> Self {
        format!("{:?}", token)
    }
}

fn er_lexer<'src>() -> impl Parser<'src, &'src str, Vec<ERToken>, extra::Err<Simple<'src, char>>> {
    let whitespace = just(' ').or(just('\t')).repeated();

    let comment = just("%%")
        .then(none_of('\n').repeated())
        .map(|_| ERToken::Comment("".to_string()));

    let er_keyword = just("erDiagram").map(|_| ERToken::ERDiagram);

    // Accessibility directives
    let acc_title = just("accTitle:")
        .ignore_then(whitespace)
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|content| ERToken::AccTitle(content.trim().to_string()));

    let acc_descr = just("accDescr:")
        .ignore_then(whitespace)
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|content| ERToken::AccDescr(content.trim().to_string()));

    // Style directive - matches "style <id> <css-properties>"
    let style_directive = just("style")
        .ignore_then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|content| ERToken::Style(content.trim().to_string()));

    // ClassDef directive - matches "classDef <name> <css-properties>"
    let class_def_directive = just("classDef")
        .ignore_then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|content| ERToken::ClassDef(content.trim().to_string()));

    // Relationship symbols (order matters for overlapping patterns - longer first)
    let rel_symbols = choice((
        just("}o--|{").to("many-to-one-or-more"), // Added missing pattern
        just("}o--o{").to("many-to-many"),
        just("}o--||").to("many-to-one"),
        just("||--o{").to("one-to-many"),
        just("||--o|").to("one-to-zero-or-one"), // Added missing pattern
        just("||--||").to("one-to-one"),
        just("||--|{").to("one-to-one-or-more"), // Moved before shorter patterns
        just("}|--|{").to("one-or-more-to-one-or-more"),
        just("}|--||").to("one-or-more-to-one"),
        just("o{--||").to("zero-or-one-to-one"), // Added missing pattern
        just("|o--|{").to("zero-or-one-to-one-or-more"), // Added missing pattern
        just("u--o{").to("unique-to-many"),      // Added missing pattern
        just("}|..o{").to("one-or-more-to-many-optional"), // Added missing pattern
        just("}|..o|").to("one-or-more-to-zero-or-one-optional"), // Added missing pattern
        just("}|..|{").to("one-or-more-to-one-or-more-optional"), // Added missing pattern
        just("|o..o{").to("zero-or-one-to-many-optional"), // Added missing pattern
        just("|o..o|").to("zero-or-one-to-zero-or-one-optional"), // Added missing pattern
        just("|o..||").to("zero-or-one-to-one-optional"), // Added missing pattern
        just("}o..o|").to("many-to-zero-or-one-optional"), // Added missing pattern
        just("}o..o{").to("many-to-many-optional"),
        just("}o..||").to("many-to-one-optional"),
        just("||..o{").to("one-to-many-optional"),
        just("||..||").to("one-to-one-optional"),
    ))
    .map(|s: &str| ERToken::RelSymbol(s.to_string()));

    // Key types
    let key_type = choice((
        just("PK").to(ERToken::KeyType(KeyType::PK)),
        just("FK").to(ERToken::KeyType(KeyType::FK)),
        just("UK").to(ERToken::KeyType(KeyType::UK)),
    ));

    // Attribute types with optional modifiers
    let base_attr_types = choice((
        just("string"),
        just("int"),
        just("integer"),
        just("float"),
        just("double"),
        just("decimal"),
        just("boolean"),
        just("bool"),
        just("date"),
        just("datetime"),
        just("timestamp"),
        just("time"),
        just("blob"),
        just("text"),
        just("varchar"),
        just("char"),
    ));

    // Attribute type with optional length (e.g., string(99)) or array notation (e.g., string[])
    let attr_types = base_attr_types
        .then(
            choice((
                // Array notation: string[]
                just("[]").to("[]".to_string()),
                // Length notation: string(99)
                just('(')
                    .ignore_then(text::int(10))
                    .then_ignore(just(')'))
                    .map(|n| format!("({})", n)),
            ))
            .or_not(),
        )
        .map(|(base, modifier)| {
            if let Some(mod_str) = modifier {
                ERToken::AttributeType(format!("{}{}", base, mod_str))
            } else {
                ERToken::AttributeType(base.to_string())
            }
        });

    // Quoted string
    let quoted_string = just('"')
        .ignore_then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(ERToken::QuotedString);

    // Entity alias: alias[name] or alias["quoted name"]
    let entity_alias = text::ident()
        .then_ignore(just('['))
        .then(choice((
            // Quoted name: "Customer Account"
            just('"')
                .ignore_then(none_of('"').repeated().collect::<String>())
                .then_ignore(just('"')),
            // Unquoted name: Person
            none_of(']').repeated().collect::<String>(),
        )))
        .then_ignore(just(']'))
        .map(|(alias, name): (&str, String)| ERToken::EntityAlias {
            alias: alias.to_string(),
            name: name.trim().to_string(),
        });

    // Class assignment: entity:::class
    let class_assignment = choice((
        // Hyphenated entity with class: LINE-ITEM:::foo
        text::ident()
            .then_ignore(just('-'))
            .then(text::ident())
            .then_ignore(just(":::"))
            .then(text::ident())
            .map(
                |((first, second), class): ((&str, &str), &str)| ERToken::ClassAssignment {
                    entity: format!("{}-{}", first, second),
                    class: class.to_string(),
                },
            ),
        // Regular entity with class: PERSON:::foo
        text::ident()
            .then_ignore(just(":::"))
            .then(text::ident())
            .map(|(entity, class): (&str, &str)| ERToken::ClassAssignment {
                entity: entity.to_string(),
                class: class.to_string(),
            }),
    ));

    // Entity/Attribute names (identifiers)
    let identifier = choice((
        // Hyphenated identifier like LINE-ITEM
        text::ident()
            .then(just('-'))
            .then(text::ident())
            .map(|((first, _), second)| format!("{}-{}", first, second)),
        // Regular identifier
        text::ident().map(|s: &str| s.to_string()),
    ))
    .map(ERToken::EntityName);

    let left_brace = just('{').to(ERToken::LeftBrace);
    let right_brace = just('}').to(ERToken::RightBrace);
    let colon = just(':').to(ERToken::Colon);
    let comma = just(',').to(ERToken::Comma);
    let newline = just('\n').to(ERToken::NewLine);

    // Helper to ensure keywords are not followed by word characters
    let keyword = |s: &'static str| just(s).then_ignore(text::ident().not().rewind());

    // Natural language relationship keywords
    let to_keyword = keyword("to").to(ERToken::To);
    let optionally_keyword = keyword("optionally").to(ERToken::Optionally);
    let zero_keyword = keyword("zero").to(ERToken::Zero);
    let one_keyword = keyword("one").to(ERToken::One);
    let many_keyword = keyword("many").to(ERToken::Many);
    let only_keyword = keyword("only").to(ERToken::Only);
    let exactly_keyword = keyword("exactly").to(ERToken::Exactly);
    let or_keyword = keyword("or").to(ERToken::Or);
    let more_keyword = keyword("more").to(ERToken::More);

    // Numbers with optional + suffix (e.g., "1", "0+", "1+")
    let number_token =
        text::int(10)
            .then(just('+').or_not())
            .map(|(n, plus): (&str, Option<char>)| {
                if plus.is_some() {
                    ERToken::Number(format!("{n}+"))
                } else {
                    ERToken::Number(n.to_string())
                }
            });

    // many(n) pattern
    let many_with_param = just("many")
        .then_ignore(just('('))
        .then(text::int(10))
        .then_ignore(just(')'))
        .map(|(_, n): (&str, &str)| ERToken::ManyWithParam(n.parse().unwrap_or(0)));

    // Natural language keywords grouped together
    let nat_lang_keywords = choice((
        many_with_param, // Must come before "many" keyword
        to_keyword,
        optionally_keyword,
        zero_keyword,
        one_keyword,
        many_keyword,
        only_keyword,
        exactly_keyword,
        or_keyword,
        more_keyword,
        number_token,
    ));

    // Directive tokens grouped together
    let directives = choice((acc_title, acc_descr, style_directive, class_def_directive));

    // Special tokens group
    let special_tokens = choice((
        comment,
        er_keyword,
        key_type,
        attr_types,
        rel_symbols,
        quoted_string,
        entity_alias,
        class_assignment,
    ));

    // Combine all tokens (order matters - longer patterns first)
    let token = choice((
        directives, // Must come before identifier
        special_tokens,
        nat_lang_keywords, // Must come before identifier
        left_brace,
        right_brace,
        colon,
        comma,
        identifier,
    ));

    // Handle whitespace and newlines
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
}

fn er_parser<'src>(
) -> impl Parser<'src, &'src [ERToken], ErDiagram, extra::Err<Simple<'src, ERToken>>> {
    // Parse erDiagram header
    let header = just(ERToken::ERDiagram)
        .then_ignore(any().filter(|t| matches!(t, ERToken::NewLine)).repeated());

    // Parse entity name (can be EntityName, EntityAlias, ClassAssignment, or QuotedString)
    let entity_name = any().try_map(|t, span| {
        match t {
            ERToken::EntityName(name) => Ok(name),
            ERToken::EntityAlias { alias, name: _ } => Ok(alias), // Use alias for relationships
            ERToken::ClassAssignment { entity, class: _ } => Ok(entity), // Use entity name, ignore class for now
            ERToken::QuotedString(name) => Ok(name), // Support quoted entity names
            _ => Err(Simple::new(Some(t.into()), span)),
        }
    });

    // Parse entity alias specifically for entity definitions
    let entity_alias = any().try_map(|t, span| match t {
        ERToken::EntityAlias { alias, name } => Ok((alias, name)),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse attribute type
    let attr_type = any().try_map(|t, span| match t {
        ERToken::AttributeType(typ) => Ok(typ),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse key type
    let key_type = any().try_map(|t, span| match t {
        ERToken::KeyType(kt) => Ok(kt),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse quoted string
    let quoted_string = any().try_map(|t, span| match t {
        ERToken::QuotedString(s) => Ok(s),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse accessibility directives
    let acc_title = any().try_map(|t, span| match t {
        ERToken::AccTitle(title) => Ok(title),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    let acc_descr = any().try_map(|t, span| match t {
        ERToken::AccDescr(descr) => Ok(descr),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse style directives
    let style_directive = any().try_map(|t, span| match t {
        ERToken::Style(content) => Ok(content),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse classDef directives
    let class_def_directive = any().try_map(|t, span| match t {
        ERToken::ClassDef(content) => Ok(content),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse multiple key types separated by commas (e.g., "PK, FK")
    let key_types = key_type
        .then(just(ERToken::Comma).ignore_then(key_type).repeated())
        .map(|(first, _rest)| first); // For now, only use the first key type

    // Parse attribute: type name [key_type] ["comment"]
    let attribute = attr_type
        .then(entity_name) // Attribute names use same parser as entity names
        .then(key_types.or_not())
        .then(quoted_string.or_not())
        .map(|(((attr_type, name), key_type), comment)| Attribute {
            name,
            attr_type,
            key_type,
            comment,
        });

    // Parse entity definition: ENTITY { attributes } or alias[Entity Name] { attributes } or "Entity Name" { attributes }
    let entity_def = choice((
        // Entity with alias: alias[name] { attributes }
        entity_alias
            .then_ignore(just(ERToken::LeftBrace))
            .then(
                any()
                    .filter(|t| matches!(t, ERToken::NewLine))
                    .repeated()
                    .ignore_then(attribute.clone())
                    .then_ignore(any().filter(|t| matches!(t, ERToken::NewLine)).repeated())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(ERToken::RightBrace))
            .map(|((alias, _display_name), attributes)| Entity {
                name: alias, // Use alias as the entity identifier
                attributes,
            }),
        // Regular entity: ENTITY { attributes } or "Entity Name" { attributes }
        entity_name
            .then_ignore(just(ERToken::LeftBrace))
            .then(
                any()
                    .filter(|t| matches!(t, ERToken::NewLine))
                    .repeated()
                    .ignore_then(attribute.clone())
                    .then_ignore(any().filter(|t| matches!(t, ERToken::NewLine)).repeated())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(ERToken::RightBrace))
            .map(|(name, attributes)| Entity { name, attributes }),
    ));

    // Parse relationship symbol and convert to cardinality
    let rel_symbol = any().try_map(|t, span| match t {
        ERToken::RelSymbol(symbol) => Ok(symbol),
        _ => Err(Simple::new(Some(t.into()), span)),
    });

    // Parse natural language cardinality (e.g., "1", "zero or more", "many(0)", etc.)
    let nat_lang_cardinality = choice((
        // many(n) pattern
        any().try_map(|t, span| match t {
            ERToken::ManyWithParam(n) => Ok((
                if n == 0 {
                    CardinalityValue::Zero
                } else {
                    CardinalityValue::One
                },
                CardinalityValue::Many,
            )),
            _ => Err(Simple::new(Some(t.into()), span)),
        }),
        // "only one" or "exactly one"
        just(ERToken::Only)
            .or(just(ERToken::Exactly))
            .ignore_then(just(ERToken::One))
            .map(|_| (CardinalityValue::One, CardinalityValue::One)),
        // "zero or more", "zero or many"
        just(ERToken::Zero)
            .ignore_then(just(ERToken::Or))
            .ignore_then(just(ERToken::More).or(just(ERToken::Many)))
            .map(|_| (CardinalityValue::Zero, CardinalityValue::Many)),
        // "one or more", "one or many"
        just(ERToken::One)
            .ignore_then(just(ERToken::Or))
            .ignore_then(just(ERToken::More).or(just(ERToken::Many)))
            .map(|_| (CardinalityValue::One, CardinalityValue::Many)),
        // "one or zero"
        just(ERToken::One)
            .ignore_then(just(ERToken::Or))
            .ignore_then(just(ERToken::Zero))
            .map(|_| (CardinalityValue::Zero, CardinalityValue::One)),
        // "zero or one"
        just(ERToken::Zero)
            .ignore_then(just(ERToken::Or))
            .ignore_then(just(ERToken::One))
            .map(|_| (CardinalityValue::Zero, CardinalityValue::One)),
        // Simple "zero", "one", "many"
        just(ERToken::Zero).map(|_| (CardinalityValue::Zero, CardinalityValue::Zero)),
        just(ERToken::One).map(|_| (CardinalityValue::One, CardinalityValue::One)),
        just(ERToken::Many).map(|_| (CardinalityValue::Zero, CardinalityValue::Many)),
        // Numbers like "1", "0+", "1+"
        any().try_map(|t, span| match &t {
            ERToken::Number(n) => {
                if n == "0+" {
                    Ok((CardinalityValue::Zero, CardinalityValue::Many))
                } else if n == "1+" {
                    Ok((CardinalityValue::One, CardinalityValue::Many))
                } else if n == "1" {
                    Ok((CardinalityValue::One, CardinalityValue::One))
                } else if n == "0" {
                    Ok((CardinalityValue::Zero, CardinalityValue::Zero))
                } else {
                    Err(Simple::new(Some(t.into()), span))
                }
            }
            _ => Err(Simple::new(Some(t.into()), span)),
        }),
    ));

    // Parse natural language relationship: ENTITY cardinality [optionally] to cardinality ENTITY : label
    let nat_lang_relationship = entity_name
        .then(nat_lang_cardinality.clone())
        .then(just(ERToken::Optionally).or_not())
        .then_ignore(just(ERToken::To))
        .then(nat_lang_cardinality)
        .then(entity_name)
        .then(just(ERToken::Colon).ignore_then(entity_name).or_not())
        .map(
            |(((((left_entity, left_card), _optionally), right_card), right_entity), label)| {
                ErRelationship {
                    left_entity,
                    right_entity,
                    left_cardinality: ErCardinality {
                        min: left_card.0,
                        max: left_card.1,
                    },
                    right_cardinality: ErCardinality {
                        min: right_card.0,
                        max: right_card.1,
                    },
                    label,
                }
            },
        );

    // Parse a simple relationship: ENTITY1 ||--o{ ENTITY2 : label
    let symbolic_relationship = entity_name
        .then(rel_symbol)
        .then(entity_name)
        .then(just(ERToken::Colon).ignore_then(entity_name).or_not())
        .map(|(((left_entity, symbol), right_entity), label)| {
            let (left_card, right_card) = parse_cardinality(&symbol);
            ErRelationship {
                left_entity,
                right_entity,
                left_cardinality: left_card,
                right_cardinality: right_card,
                label,
            }
        });

    // Either symbolic or natural language relationship
    let relationship = choice((nat_lang_relationship, symbolic_relationship));

    // Skip newlines and other tokens
    let skip_token = any().filter(|t| {
        !matches!(
            t,
            ERToken::EntityName(_)
                | ERToken::RelSymbol(_)
                | ERToken::ClassAssignment { .. }
                | ERToken::EntityAlias { .. }
                | ERToken::QuotedString(_)
                | ERToken::To
                | ERToken::Optionally
                | ERToken::Zero
                | ERToken::One
                | ERToken::Many
                | ERToken::Only
                | ERToken::Exactly
                | ERToken::Or
                | ERToken::More
                | ERToken::Number(_)
                | ERToken::ManyWithParam(_)
                | ERToken::Comma
        )
    });

    // Parse standalone entity (just entity name, no braces or relationships)
    let standalone_entity = entity_name.map(|name| Entity {
        name,
        attributes: Vec::new(),
    });

    // Parse diagram content - include accessibility directives, style, and classDef
    // Order matters: try more specific patterns first
    let content = choice((
        entity_def.map(|e| (Some(e), None, None, None, None, None)),
        relationship.map(|r| (None, Some(r), None, None, None, None)),
        acc_title.map(|t| (None, None, Some(t), None, None, None)),
        acc_descr.map(|d| (None, None, None, Some(d), None, None)),
        style_directive.map(|s| (None, None, None, None, Some(s), None)),
        class_def_directive.map(|c| (None, None, None, None, None, Some(c))),
        standalone_entity.map(|e| (Some(e), None, None, None, None, None)),
        skip_token.map(|_| (None, None, None, None, None, None)),
    ))
    .repeated()
    .collect::<Vec<_>>();

    header.ignore_then(content).then_ignore(end()).map(|items| {
        let mut entities = HashMap::new();
        let mut relationships = Vec::new();
        let mut acc_title = None;
        let mut acc_descr = None;

        for (entity_opt, rel_opt, title_opt, descr_opt, _style_opt, _class_def_opt) in items {
            if let Some(entity) = entity_opt {
                entities.insert(entity.name.clone(), entity);
            }
            if let Some(rel) = rel_opt {
                relationships.push(rel);
            }
            if let Some(title) = title_opt {
                acc_title = Some(title);
            }
            if let Some(descr) = descr_opt {
                acc_descr = Some(descr);
            }
            // Style and classDef directives are parsed but not stored in the AST currently
        }

        ErDiagram {
            title: None,
            accessibility: AccessibilityInfo {
                title: acc_title,
                description: acc_descr,
            },
            entities,
            relationships,
        }
    })
}

fn parse_cardinality(symbol: &str) -> (ErCardinality, ErCardinality) {
    match symbol {
        "one-to-one" | "||--||" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
        ),
        "one-to-many" | "||--o{" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
        "many-to-one" | "}o--||" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
        ),
        "many-to-many" | "}o--o{" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
        "one-to-one-or-more" | "||--|{" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
        ),
        "one-or-more-to-zero-or-one-optional" | "}|..o|" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
        ),
        "one-or-more-to-one-or-more-optional" | "}|..|{" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
        ),
        "one-to-zero-or-one" | "||--o|" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
        ),
        "zero-or-one-to-one" | "o{--||" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
        ),
        "zero-or-one-to-one-or-more" | "|o--|{" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
        ),
        "unique-to-many" | "u--o{" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
        "many-to-zero-or-one-optional" | "}o..o|" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
        ),
        "zero-or-one-to-one-optional" | "|o..||" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
        ),
        "zero-or-one-to-zero-or-one-optional" | "|o..o|" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
        ),
        "many-to-one-or-more" | "}o--|{" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
        ),
        "one-or-more-to-many-optional" | "}|..o{" => (
            ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
        "zero-or-one-to-many-optional" | "|o..o{" => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::One,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
        _ => (
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
        ),
    }
}

pub fn parse(input: &str) -> Result<ErDiagram> {
    // Strip metadata comments before parsing
    let clean_input = crate::common::lexer::strip_metadata_comments(input);

    let tokens =
        er_lexer()
            .parse(&clean_input)
            .into_result()
            .map_err(|e| ParseError::SyntaxError {
                message: "Failed to tokenize ER diagram".to_string(),
                expected: vec![],
                found: format!("{:?}", e),
                line: 0,
                column: 0,
            })?;

    let result =
        er_parser()
            .parse(&tokens[..])
            .into_result()
            .map_err(|e| ParseError::SyntaxError {
                message: "Failed to parse ER diagram".to_string(),
                expected: vec![],
                found: format!("{:?}", e),
                line: 0,
                column: 0,
            });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_er_keyword() {
        let input = "erDiagram";
        let tokens = er_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], ERToken::ERDiagram);
    }

    #[test]
    fn test_lexer_relationship_symbols() {
        let input = "CUSTOMER ||--o{ ORDER";
        let tokens = er_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Should have: CUSTOMER, ||--o{, ORDER
        assert!(
            tokens.len() >= 3,
            "Expected at least 3 tokens, got: {:?}",
            tokens
        );
        assert_eq!(tokens[0], ERToken::EntityName("CUSTOMER".to_string()));
        assert_eq!(tokens[1], ERToken::RelSymbol("one-to-many".to_string()));
        assert_eq!(tokens[2], ERToken::EntityName("ORDER".to_string()));
    }

    #[test]
    fn test_lexer_entity_with_attributes() {
        let input = r#"CUSTOMER {
        string name PK
    }"#;
        let tokens = er_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        // Should include: CUSTOMER, {, newline, string, name, PK, newline, }
        let expected_tokens = vec![
            ERToken::EntityName("CUSTOMER".to_string()),
            ERToken::LeftBrace,
            ERToken::NewLine,
            ERToken::AttributeType("string".to_string()),
            ERToken::EntityName("name".to_string()),
            ERToken::KeyType(KeyType::PK),
            ERToken::NewLine,
            ERToken::RightBrace,
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
    fn test_simple_er_diagram() {
        let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.relationships.len(), 1);

        let rel = &diagram.relationships[0];
        assert_eq!(rel.left_entity, "CUSTOMER");
        assert_eq!(rel.right_entity, "ORDER");
        assert_eq!(rel.label, Some("places".to_string()));
    }

    #[test]
    fn test_parser_multiple_relationships() {
        let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.relationships.len(), 2);

        let rel1 = &diagram.relationships[0];
        assert_eq!(rel1.left_entity, "CUSTOMER");
        assert_eq!(rel1.right_entity, "ORDER");
        assert_eq!(rel1.label, Some("places".to_string()));

        let rel2 = &diagram.relationships[1];
        assert_eq!(rel2.left_entity, "ORDER");
        assert_eq!(rel2.right_entity, "LINE-ITEM");
        assert_eq!(rel2.label, Some("contains".to_string()));
    }

    #[test]
    fn test_parser_entity_with_attributes() {
        let input = r#"erDiagram
    CUSTOMER {
        string name PK
        int customerId
        string address
    }
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.entities.len(), 1);

        let customer = &diagram.entities["CUSTOMER"];
        assert_eq!(customer.name, "CUSTOMER");
        assert_eq!(customer.attributes.len(), 3);

        let name_attr = &customer.attributes[0];
        assert_eq!(name_attr.name, "name");
        assert_eq!(name_attr.attr_type, "string");
        assert_eq!(name_attr.key_type, Some(KeyType::PK));

        let id_attr = &customer.attributes[1];
        assert_eq!(id_attr.name, "customerId");
        assert_eq!(id_attr.attr_type, "int");
        assert_eq!(id_attr.key_type, None);
    }

    #[test]
    fn test_parser_complete_er_diagram() {
        let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name PK
        string customerId PK
        string address
        string phoneNumber
    }
    ORDER ||--|{ LINE-ITEM : contains
    ORDER {
        int orderId PK
        string customerId FK
        date orderDate
        string status
    }
    LINE-ITEM {
        string productId FK
        int quantity
        float pricePerUnit
    }
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.entities.len(), 3);
        assert_eq!(diagram.relationships.len(), 2);

        // Check CUSTOMER entity
        let customer = &diagram.entities["CUSTOMER"];
        assert_eq!(customer.attributes.len(), 4);

        // Check ORDER entity
        let order = &diagram.entities["ORDER"];
        assert_eq!(order.attributes.len(), 4);
        assert_eq!(order.attributes[1].key_type, Some(KeyType::FK));

        // Check relationships
        assert_eq!(diagram.relationships[0].left_entity, "CUSTOMER");
        assert_eq!(diagram.relationships[0].right_entity, "ORDER");
    }

    #[test]
    fn test_real_er_file() {
        let input =
            std::fs::read_to_string("test/er/entityRelationshipDiagram_md_001.mermaid").unwrap();
        // Remove metadata comments
        let input = input
            .lines()
            .filter(|line| !line.starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        let result = parse(&input);
        assert!(result.is_ok(), "Failed to parse real file: {:?}", result);

        let diagram = result.unwrap();
        // Just verify it parses successfully and has content
        assert!(!diagram.entities.is_empty() || !diagram.relationships.is_empty());
    }
}
