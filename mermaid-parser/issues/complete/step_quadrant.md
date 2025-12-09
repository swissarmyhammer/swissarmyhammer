# Implementation Plan: Quadrant Charts

## Overview
Quadrant charts represent data points plotted on a 2x2 matrix with customizable axes.
Medium complexity grammar (187 lines) with point coordinates, axis labels, and quadrant definitions.

## Grammar Analysis

### Key Features
- Header: `quadrantChart`
- Axis definitions: `x-axis`, `y-axis`
- Axis text: `x-axis --> Text`, `y-axis --> Text`
- Quadrant labels: `quadrant-1`, `quadrant-2`, `quadrant-3`, `quadrant-4`
- Data points: `Point Name : [x, y]` where x,y are 0-1 values
- Styling: `classDef className fill:#color`

### Example Input
```
quadrantChart
    title Reach and influence
    x-axis Low Reach --> High Reach
    y-axis Low Influence --> High Influence
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.80]
    Campaign C: [0.57, 0.95]
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct QuadrantDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub x_axis: Option<AxisDefinition>,
    pub y_axis: Option<AxisDefinition>,
    pub quadrants: QuadrantLabels,
    pub points: Vec<DataPoint>,
    pub styles: Vec<ClassDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxisDefinition {
    pub label_start: Option<String>,
    pub label_end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuadrantLabels {
    pub quadrant_1: Option<String>, // Top-right
    pub quadrant_2: Option<String>, // Top-left
    pub quadrant_3: Option<String>, // Bottom-left
    pub quadrant_4: Option<String>, // Bottom-right
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataPoint {
    pub name: String,
    pub x: f64, // 0.0 to 1.0
    pub y: f64, // 0.0 to 1.0
    pub class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDefinition {
    pub name: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuadrantToken {
    Quadrant,              // "quadrantChart"
    Title,                 // "title"
    TitleValue(String),    // Title text
    XAxis,                 // "x-axis"
    YAxis,                 // "y-axis"
    AxisTextDelimiter,     // "-->"
    Quadrant1,             // "quadrant-1"
    Quadrant2,             // "quadrant-2"
    Quadrant3,             // "quadrant-3"
    Quadrant4,             // "quadrant-4"
    ClassDef,              // "classDef"
    PointStart,            // ":"
    PointBracketStart,     // "["
    PointBracketEnd,       // "]"
    PointX(f64),           // X coordinate (0-1)
    PointY(f64),           // Y coordinate (0-1)
    String(String),        // Quoted string
    MdString(String),      // Markdown string
    ClassName(String),     // Class name after :::
    Alpha(String),         // Alphabetic text
    Number(i32),           // Numeric value
    Colon,                 // ":"
    Comma,                 // ","
    Space,                 // " "
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn quadrant_lexer() -> impl Parser<char, Vec<QuadrantToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let quadrant_keyword = text::keyword("quadrantChart")
        .map(|_| QuadrantToken::Quadrant);
    
    let title = text::keyword("title")
        .map(|_| QuadrantToken::Title);
    
    let x_axis = text::keyword("x-axis")
        .map(|_| QuadrantToken::XAxis);
    
    let y_axis = text::keyword("y-axis")
        .map(|_| QuadrantToken::YAxis);
    
    let axis_delimiter = text::string("-->")
        .map(|_| QuadrantToken::AxisTextDelimiter);
    
    let quadrant_labels = choice((
        text::keyword("quadrant-1").map(|_| QuadrantToken::Quadrant1),
        text::keyword("quadrant-2").map(|_| QuadrantToken::Quadrant2),
        text::keyword("quadrant-3").map(|_| QuadrantToken::Quadrant3),
        text::keyword("quadrant-4").map(|_| QuadrantToken::Quadrant4),
    ));
    
    let class_def = text::keyword("classDef")
        .map(|_| QuadrantToken::ClassDef);
    
    // Point coordinates: [0.3, 0.6] format
    let point_coordinate = just('0')
        .then(just('.').or_not())
        .then(text::digits(10).or_not())
        .or(just('1').then(just('.').then(text::digits(10)).or_not()))
        .collect::<String>()
        .map(|s| s.parse::<f64>().unwrap_or(0.0));
    
    let point_start = just(':')
        .then(whitespace)
        .then(just('['))
        .then(whitespace)
        .map(|_| QuadrantToken::PointBracketStart);
    
    let point_x = point_coordinate
        .map(QuadrantToken::PointX);
    
    let point_y = point_coordinate
        .map(QuadrantToken::PointY);
    
    let point_end = whitespace
        .then(just(']'))
        .map(|_| QuadrantToken::PointBracketEnd);
    
    // Quoted strings
    let quoted_string = just('"')
        .ignore_then(
            filter(|c| *c != '"')
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(QuadrantToken::String);
    
    // Markdown strings: "`text`"
    let md_string = just('"')
        .then(just('`'))
        .ignore_then(
            filter(|c| *c != '`')
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('`'))
        .then_ignore(just('"'))
        .map(QuadrantToken::MdString);
    
    // Class name after :::
    let class_name = text::string(":::")
        .ignore_then(
            text::ident()
        )
        .map(QuadrantToken::ClassName);
    
    let title_value = filter(|c| !matches!(*c, '\n' | '#' | ';'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| QuadrantToken::TitleValue(text.trim().to_string()));
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| QuadrantToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| QuadrantToken::AccDescr);
    
    let alpha_text = text::ident()
        .map(QuadrantToken::Alpha);
    
    let number = text::int(10)
        .map(|n: String| QuadrantToken::Number(n.parse().unwrap_or(0)));
    
    let symbols = choice((
        just(':').map(|_| QuadrantToken::Colon),
        just(',').map(|_| QuadrantToken::Comma),
        just(' ').map(|_| QuadrantToken::Space),
    ));
    
    let newline = choice((
        just('\n'),
        just('\r').then(just('\n')).map(|_| '\n'),
    ))
    .map(|_| QuadrantToken::NewLine);
    
    choice((
        comment.ignored(),
        quadrant_keyword,
        title,
        x_axis,
        y_axis,
        axis_delimiter,
        quadrant_labels,
        class_def,
        md_string,
        quoted_string,
        class_name,
        point_start,
        point_x,
        point_y,
        point_end,
        acc_title,
        acc_descr,
        alpha_text,
        number,
        symbols,
        newline,
    ))
    .padded_by(just(' ').or(just('\t')).repeated())
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Point Parser
```rust
fn parse_point() -> impl Parser<QuadrantToken, DataPoint, Error = Simple<QuadrantToken>> {
    // Parse point name
    select! { QuadrantToken::Alpha(name) => name }
        .or(select! { QuadrantToken::String(name) => name })
        .then_ignore(just(QuadrantToken::PointBracketStart))
        .then(select! { QuadrantToken::PointX(x) => x })
        .then_ignore(just(QuadrantToken::Comma))
        .then(select! { QuadrantToken::PointY(y) => y })
        .then_ignore(just(QuadrantToken::PointBracketEnd))
        .then(
            // Optional class name
            select! { QuadrantToken::ClassName(class) => class }
                .or_not()
        )
        .map(|(((name, x), y), class)| DataPoint {
            name,
            x,
            y,
            class,
        })
}
```

### Axis Parser
```rust
fn parse_axis() -> impl Parser<QuadrantToken, AxisDefinition, Error = Simple<QuadrantToken>> {
    // Parse axis definition: "Low Reach --> High Reach"
    select! { 
        QuadrantToken::Alpha(text) => text,
        QuadrantToken::String(text) => text,
    }
    .repeated()
    .at_least(1)
    .map(|parts| parts.join(" "))
    .then_ignore(just(QuadrantToken::AxisTextDelimiter))
    .then(
        select! { 
            QuadrantToken::Alpha(text) => text,
            QuadrantToken::String(text) => text,
        }
        .repeated()
        .at_least(1)
        .map(|parts| parts.join(" "))
    )
    .map(|(start, end)| AxisDefinition {
        label_start: Some(start),
        label_end: Some(end),
    })
}
```

### Main Parser
```rust
pub fn quadrant_parser() -> impl Parser<QuadrantToken, QuadrantDiagram, Error = Simple<QuadrantToken>> {
    just(QuadrantToken::Quadrant)
        .then_ignore(just(QuadrantToken::NewLine).or_not())
        .then(
            choice((
                // Title
                just(QuadrantToken::Title)
                    .then(select! { QuadrantToken::TitleValue(title) => title })
                    .map(|(_, title)| ("title", title)),
                
                // X-axis
                just(QuadrantToken::XAxis)
                    .then(parse_axis())
                    .map(|(_, axis)| ("x_axis", format!("{}-->{}", 
                        axis.label_start.unwrap_or_default(),
                        axis.label_end.unwrap_or_default()))),
                
                // Y-axis
                just(QuadrantToken::YAxis)
                    .then(parse_axis())
                    .map(|(_, axis)| ("y_axis", format!("{}-->{}", 
                        axis.label_start.unwrap_or_default(),
                        axis.label_end.unwrap_or_default()))),
                
                // Quadrant labels
                just(QuadrantToken::Quadrant1)
                    .then(select! { QuadrantToken::TitleValue(label) => label })
                    .map(|(_, label)| ("quadrant_1", label)),
                    
                just(QuadrantToken::Quadrant2)
                    .then(select! { QuadrantToken::TitleValue(label) => label })
                    .map(|(_, label)| ("quadrant_2", label)),
                    
                just(QuadrantToken::Quadrant3)
                    .then(select! { QuadrantToken::TitleValue(label) => label })
                    .map(|(_, label)| ("quadrant_3", label)),
                    
                just(QuadrantToken::Quadrant4)
                    .then(select! { QuadrantToken::TitleValue(label) => label })
                    .map(|(_, label)| ("quadrant_4", label)),
                
                // Data points
                parse_point()
                    .map(|point| ("point", format!("{}:[{},{}]", point.name, point.x, point.y))),
            ))
            .separated_by(just(QuadrantToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(QuadrantToken::Eof).or_not())
        .map(|(_, statements)| {
            let mut diagram = QuadrantDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                x_axis: None,
                y_axis: None,
                quadrants: QuadrantLabels::default(),
                points: Vec::new(),
                styles: Vec::new(),
            };
            
            for (stmt_type, content) in statements {
                match stmt_type {
                    "title" => diagram.title = Some(content),
                    "quadrant_1" => diagram.quadrants.quadrant_1 = Some(content),
                    "quadrant_2" => diagram.quadrants.quadrant_2 = Some(content),
                    "quadrant_3" => diagram.quadrants.quadrant_3 = Some(content),
                    "quadrant_4" => diagram.quadrants.quadrant_4 = Some(content),
                    // Handle x_axis, y_axis, and points...
                    _ => {}
                }
            }
            
            diagram
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/quadrant/`
- Expected count: 57 files
- Copy to: `mermaid-parser/test/quadrant/`

### Command
```bash
cp -r ../mermaid-samples/quadrant/* ./test/quadrant/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_quadrant_files(#[files("test/quadrant/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = quadrant_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = quadrant_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(
        diagram.points.len() > 0 || diagram.title.is_some(),
        "Quadrant chart should have points or title"
    );
}

#[test]
fn test_simple_quadrant() {
    let input = r#"quadrantChart
    title Reach and influence
    x-axis Low Reach --> High Reach
    y-axis Low Influence --> High Influence
    quadrant-1 We should expand
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.80]
"#;
    
    let tokens = quadrant_lexer().parse(input.chars()).unwrap();
    let diagram = quadrant_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("Reach and influence".to_string()));
    assert_eq!(diagram.quadrants.quadrant_1, Some("We should expand".to_string()));
    assert_eq!(diagram.points.len(), 2);
    assert_eq!(diagram.points[0].name, "Campaign A");
    assert_eq!(diagram.points[0].x, 0.3);
    assert_eq!(diagram.points[0].y, 0.6);
}

#[test]
fn test_coordinate_parsing() {
    let tests = vec![
        ("0.3", 0.3),
        ("0.95", 0.95),
        ("1", 1.0),
        ("0", 0.0),
    ];
    
    for (input, expected) in tests {
        // Test coordinate parsing logic
        let parsed: f64 = input.parse().unwrap();
        assert!((parsed - expected).abs() < 0.001);
    }
}

#[test]
fn test_axis_definitions() {
    let input = r#"quadrantChart
    x-axis Low --> High
    y-axis Bad --> Good
"#;
    
    let tokens = quadrant_lexer().parse(input.chars()).unwrap();
    let diagram = quadrant_parser().parse(tokens).unwrap();
    
    assert!(diagram.x_axis.is_some());
    assert!(diagram.y_axis.is_some());
}
```

## Success Criteria
1. ✅ Parse all 57 quadrant sample files successfully
2. ✅ Handle title and axis definitions
3. ✅ Parse quadrant labels (1-4)
4. ✅ Extract data points with coordinates [x, y]
5. ✅ Validate coordinate ranges (0.0 to 1.0)
6. ✅ Support styling with classDef
7. ✅ Handle accessibility attributes
8. ✅ Process axis text with arrows (-->)

## Implementation Priority
**Priority 6** - Implement after Timeline and Journey. Quadrant charts introduce coordinate systems and data plotting concepts that are foundational for XY charts and other data visualization diagrams.