# Implementation Plan: Radar Charts

## Overview
Radar charts (spider/star charts) display multivariate data on axes starting from the same point.
TypeScript-based parser for multi-dimensional data visualization with configurable axes.

## TypeScript Parser Analysis

### Key Features (from radar parser.ts)
- Multiple data series
- Configurable axes with labels
- Data points on a 0-100 scale (or custom ranges)
- Legend support
- Optional axis configuration
- Comments: `%%` for line comments

### Example Input
```
%%{init: {'theme': 'base', 'themeVariables': {'radarBackgroundColor': '#f4f4f4', 'radarGridColor': '#333'}}}%%
radar
    title Skills Assessment
    ds Ideal
    "Communication" : 90
    "Technical" : 85
    "Leadership" : 80
    "Problem Solving" : 95
    "Creativity" : 75
    ds Current
    "Communication" : 70
    "Technical" : 90
    "Leadership" : 60
    "Problem Solving" : 85
    "Creativity" : 65
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RadarDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub config: RadarConfig,
    pub axes: Vec<String>,
    pub datasets: Vec<Dataset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarConfig {
    pub background_color: Option<String>,
    pub grid_color: Option<String>,
    pub scale_max: f64,
    pub scale_min: f64,
}

impl Default for RadarConfig {
    fn default() -> Self {
        RadarConfig {
            background_color: None,
            grid_color: None,
            scale_max: 100.0,
            scale_min: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadarToken {
    Radar,                  // "radar"
    Title,                  // "title"
    Ds,                     // "ds" (dataset)
    QuotedString(String),   // "axis label"
    Identifier(String),     // Dataset name or other identifier
    Number(f64),            // Numeric value
    Colon,                  // :
    ConfigStart,            // %%{init:
    ConfigEnd,              // }%%
    ConfigContent(String),  // Configuration JSON
    Comment(String),        // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn radar_lexer() -> impl Parser<char, Vec<RadarToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    // Configuration block %%{init: ... }%%
    let config = text::string("%%{init:")
        .then(
            take_until(text::string("}%%"))
                .collect::<String>()
        )
        .then(text::string("}%%"))
        .map(|((_, content), _)| RadarToken::ConfigContent(content));
    
    // Regular comment
    let comment = just('%')
        .then(just('%'))
        .then(
            none_of("{")
                .then(take_until(just('\n')))
        )
        .map(|(_, (_, text))| RadarToken::Comment(format!("{}{}", text.0, text.1.into_iter().collect::<String>())));
    
    // Keywords
    let keywords = choice((
        text::keyword("radar").map(|_| RadarToken::Radar),
        text::keyword("title").map(|_| RadarToken::Title),
        text::keyword("ds").map(|_| RadarToken::Ds),
    ));
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(RadarToken::QuotedString);
    
    // Number (integer or float)
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .map(|(int, frac)| {
            let mut num_str = int.to_string();
            if let Some((_, frac)) = frac {
                num_str.push('.');
                num_str.push_str(&frac);
            }
            RadarToken::Number(num_str.parse().unwrap())
        });
    
    // Identifier
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == ' ' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(|s| RadarToken::Identifier(s.trim().to_string()));
    
    let colon = just(':').map(|_| RadarToken::Colon);
    
    let newline = just('\n').map(|_| RadarToken::NewLine);
    
    let token = choice((
        config,
        comment,
        keywords,
        quoted_string,
        number,
        colon,
        identifier,
    ));
    
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Dataset-Oriented Parser
```rust
pub fn radar_parser() -> impl Parser<RadarToken, RadarDiagram, Error = Simple<RadarToken>> {
    enum ParseState {
        Initial,
        InRadar,
        InDataset(String),
    }
    
    any()
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(just(RadarToken::Eof).or_not())
        .map(|tokens| {
            let mut config = RadarConfig::default();
            let mut title = None;
            let mut axes = Vec::new();
            let mut datasets = Vec::new();
            let mut current_dataset: Option<(String, Vec<f64>)> = None;
            let mut state = ParseState::Initial;
            let mut i = 0;
            
            while i < tokens.len() {
                match (&state, &tokens[i]) {
                    (_, RadarToken::ConfigContent(content)) => {
                        parse_config(content, &mut config);
                        i += 1;
                    }
                    (_, RadarToken::Comment(_)) => {
                        i += 1;
                    }
                    (_, RadarToken::NewLine) => {
                        i += 1;
                    }
                    (_, RadarToken::Radar) => {
                        state = ParseState::InRadar;
                        i += 1;
                    }
                    (ParseState::InRadar, RadarToken::Title) => {
                        if let Some((t, consumed)) = parse_title(&tokens[i..]) {
                            title = Some(t);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    (ParseState::InRadar | ParseState::InDataset(_), RadarToken::Ds) => {
                        // Save previous dataset if exists
                        if let Some((name, values)) = current_dataset.take() {
                            datasets.push(Dataset { name, values });
                        }
                        
                        // Start new dataset
                        if let Some((name, consumed)) = parse_dataset_name(&tokens[i..]) {
                            state = ParseState::InDataset(name.clone());
                            current_dataset = Some((name, Vec::new()));
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    (ParseState::InDataset(_), RadarToken::QuotedString(axis)) => {
                        // Parse axis-value pair
                        if let Some((value, consumed)) = parse_axis_value(&tokens[i..]) {
                            // Add axis if not already present
                            if !axes.contains(axis) {
                                axes.push(axis.clone());
                            }
                            
                            // Add value to current dataset
                            if let Some((_, ref mut values)) = current_dataset {
                                // Ensure values vector has correct length
                                let axis_index = axes.iter().position(|a| a == axis).unwrap();
                                while values.len() <= axis_index {
                                    values.push(0.0);
                                }
                                values[axis_index] = value;
                            }
                            
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            
            // Save final dataset
            if let Some((name, values)) = current_dataset {
                datasets.push(Dataset { name, values });
            }
            
            // Ensure all datasets have values for all axes
            for dataset in &mut datasets {
                while dataset.values.len() < axes.len() {
                    dataset.values.push(0.0);
                }
            }
            
            RadarDiagram {
                title,
                accessibility: AccessibilityInfo::default(),
                config,
                axes,
                datasets,
            }
        })
}

fn parse_config(content: &str, config: &mut RadarConfig) {
    // Simple JSON-like parsing for theme variables
    if content.contains("radarBackgroundColor") {
        if let Some(color) = extract_value(content, "radarBackgroundColor") {
            config.background_color = Some(color);
        }
    }
    if content.contains("radarGridColor") {
        if let Some(color) = extract_value(content, "radarGridColor") {
            config.grid_color = Some(color);
        }
    }
}

fn extract_value(content: &str, key: &str) -> Option<String> {
    content.find(key)
        .and_then(|pos| {
            let after_key = &content[pos + key.len()..];
            after_key.find("'")
                .and_then(|start| {
                    let value_start = start + 1;
                    after_key[value_start..].find("'")
                        .map(|end| after_key[value_start..value_start + end].to_string())
                })
        })
}

fn parse_title(tokens: &[RadarToken]) -> Option<(String, usize)> {
    if tokens.len() < 2 {
        return None;
    }
    
    let mut i = 1; // Skip "title"
    
    match &tokens[i] {
        RadarToken::Identifier(t) | RadarToken::QuotedString(t) => {
            Some((t.clone(), 2))
        }
        _ => None,
    }
}

fn parse_dataset_name(tokens: &[RadarToken]) -> Option<(String, usize)> {
    if tokens.len() < 2 {
        return None;
    }
    
    let mut i = 1; // Skip "ds"
    
    match &tokens[i] {
        RadarToken::Identifier(name) => {
            Some((name.clone(), 2))
        }
        _ => None,
    }
}

fn parse_axis_value(tokens: &[RadarToken]) -> Option<(f64, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip axis name
    
    if !matches!(&tokens[i], RadarToken::Colon) {
        return None;
    }
    i += 1;
    
    match &tokens[i] {
        RadarToken::Number(value) => {
            Some((*value, 3))
        }
        _ => None,
    }
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/radar/`
- Expected count: 62 files
- Copy to: `mermaid-parser/test/radar/`

### Command
```bash
cp -r ../mermaid-samples/radar/* ./test/radar/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_radar_files(#[files("test/radar/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = radar_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = radar_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.axes.is_empty(), "Should have at least one axis");
    assert!(!diagram.datasets.is_empty(), "Should have at least one dataset");
}

#[test]
fn test_simple_radar() {
    let input = r#"radar
    title Skills
    ds Developer
    "Frontend" : 80
    "Backend" : 90
    "Database" : 70
    "DevOps" : 60
"#;
    
    let tokens = radar_lexer().parse(input.chars()).unwrap();
    let diagram = radar_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("Skills".to_string()));
    assert_eq!(diagram.axes.len(), 4);
    assert_eq!(diagram.datasets.len(), 1);
    
    let dataset = &diagram.datasets[0];
    assert_eq!(dataset.name, "Developer");
    assert_eq!(dataset.values, vec![80.0, 90.0, 70.0, 60.0]);
}

#[test]
fn test_multiple_datasets() {
    let input = r#"radar
    ds Current
    "A" : 50
    "B" : 60
    "C" : 70
    ds Target
    "A" : 80
    "B" : 85
    "C" : 90
"#;
    
    let tokens = radar_lexer().parse(input.chars()).unwrap();
    let diagram = radar_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.datasets.len(), 2);
    assert_eq!(diagram.datasets[0].name, "Current");
    assert_eq!(diagram.datasets[1].name, "Target");
    
    // All datasets should have same number of values as axes
    for dataset in &diagram.datasets {
        assert_eq!(dataset.values.len(), diagram.axes.len());
    }
}

#[test]
fn test_config_parsing() {
    let input = r#"%%{init: {'theme': 'base', 'themeVariables': {'radarBackgroundColor': '#f4f4f4', 'radarGridColor': '#333'}}}%%
radar
    ds Data
    "X" : 50
"#;
    
    let tokens = radar_lexer().parse(input.chars()).unwrap();
    let diagram = radar_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.config.background_color, Some("#f4f4f4".to_string()));
    assert_eq!(diagram.config.grid_color, Some("#333".to_string()));
}

#[test]
fn test_axes_consistency() {
    let input = r#"radar
    ds First
    "Speed" : 75
    "Power" : 80
    ds Second
    "Power" : 90
    "Speed" : 85
    "Agility" : 70
"#;
    
    let tokens = radar_lexer().parse(input.chars()).unwrap();
    let diagram = radar_parser().parse(tokens).unwrap();
    
    // Should have all unique axes
    assert_eq!(diagram.axes.len(), 3);
    assert!(diagram.axes.contains(&"Speed".to_string()));
    assert!(diagram.axes.contains(&"Power".to_string()));
    assert!(diagram.axes.contains(&"Agility".to_string()));
    
    // All datasets should have values for all axes
    assert_eq!(diagram.datasets[0].values.len(), 3);
    assert_eq!(diagram.datasets[1].values.len(), 3);
}

#[test]
fn test_decimal_values() {
    let input = r#"radar
    ds Scores
    "Task 1" : 85.5
    "Task 2" : 92.75
    "Task 3" : 78.25
"#;
    
    let tokens = radar_lexer().parse(input.chars()).unwrap();
    let diagram = radar_parser().parse(tokens).unwrap();
    
    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], 85.5);
    assert_eq!(values[1], 92.75);
    assert_eq!(values[2], 78.25);
}
```

## Success Criteria
1. ✅ Parse all 62 radar chart sample files successfully
2. ✅ Handle multiple datasets
3. ✅ Parse axis labels correctly
4. ✅ Support numeric values (integer and decimal)
5. ✅ Handle configuration blocks
6. ✅ Maintain axis consistency across datasets
7. ✅ Parse title and metadata
8. ✅ Support dataset names with spaces

## Implementation Priority
**Priority 10** - Implement in Phase 2 alongside other data visualization types. Radar charts are moderately complex TypeScript parsers that introduce multi-dimensional data representation. The axis-value mapping pattern is useful for other chart types.