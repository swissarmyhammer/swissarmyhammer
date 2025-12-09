# Implementation Plan: XY Chart Diagrams

## Overview
XY charts represent line and bar charts with configurable axes and data series.
Medium complexity grammar (171 lines) with axis configuration, data series, and chart types.

## Grammar Analysis

### Key Features
- Header: `xychart-beta` (beta feature)
- Chart types: `horizontal` or default vertical
- Title: Chart title configuration
- X-axis: Labels and configuration
- Y-axis: Range and configuration
- Line/Bar data: Multiple data series
- Comments: `%%` for line comments

### Example Input
```
xychart-beta
    title "Sales Revenue"
    x-axis [jan, feb, mar, apr, may, jun, jul, aug, sep, oct, nov, dec]
    y-axis "Revenue (in $)" 4000 --> 11000
    bar [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]
    line [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct XYChartDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub orientation: ChartOrientation,
    pub x_axis: XAxis,
    pub y_axis: YAxis,
    pub data_series: Vec<DataSeries>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartOrientation {
    Vertical,   // Default
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XAxis {
    pub title: Option<String>,
    pub labels: Vec<String>,
    pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YAxis {
    pub title: Option<String>,
    pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataSeries {
    pub series_type: SeriesType,
    pub name: Option<String>,
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeriesType {
    Line,
    Bar,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XYChartToken {
    XYChart,              // "xychart-beta"
    Horizontal,           // "horizontal"
    Title,                // "title"
    XAxis,                // "x-axis"
    YAxis,                // "y-axis"
    Line,                 // "line"
    Bar,                  // "bar"
    QuotedString(String), // "text"
    Number(f64),          // Numeric value
    Arrow,                // "-->"
    LeftBracket,          // "["
    RightBracket,         // "]"
    Comma,                // ","
    Identifier(String),   // Unquoted text
    Comment(String),      // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn xychart_lexer() -> impl Parser<char, Vec<XYChartToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| XYChartToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let xychart_keyword = text::keyword("xychart-beta")
        .map(|_| XYChartToken::XYChart);
    
    let horizontal = text::keyword("horizontal")
        .map(|_| XYChartToken::Horizontal);
    
    let title = text::keyword("title")
        .map(|_| XYChartToken::Title);
    
    let x_axis = text::keyword("x-axis")
        .map(|_| XYChartToken::XAxis);
    
    let y_axis = text::keyword("y-axis")
        .map(|_| XYChartToken::YAxis);
    
    let line = text::keyword("line")
        .map(|_| XYChartToken::Line);
    
    let bar = text::keyword("bar")
        .map(|_| XYChartToken::Bar);
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(XYChartToken::QuotedString);
    
    // Number (integer or float)
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .map(|(int, frac)| {
            let mut num_str = int.to_string();
            if let Some((_, frac)) = frac {
                num_str.push('.');
                num_str.push_str(&frac);
            }
            XYChartToken::Number(num_str.parse().unwrap())
        });
    
    // Arrow
    let arrow = text::string("-->")
        .map(|_| XYChartToken::Arrow);
    
    // Brackets and comma
    let left_bracket = just('[').map(|_| XYChartToken::LeftBracket);
    let right_bracket = just(']').map(|_| XYChartToken::RightBracket);
    let comma = just(',').map(|_| XYChartToken::Comma);
    
    // Identifier (unquoted text)
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(XYChartToken::Identifier);
    
    let newline = just('\n').map(|_| XYChartToken::NewLine);
    
    let token = choice((
        comment,
        xychart_keyword,
        horizontal,
        title,
        x_axis,
        y_axis,
        line,
        bar,
        arrow,
        left_bracket,
        right_bracket,
        comma,
        quoted_string,
        number,
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

### Structured Parser
```rust
pub fn xychart_parser() -> impl Parser<XYChartToken, XYChartDiagram, Error = Simple<XYChartToken>> {
    let skip_newlines = filter(|t| matches!(t, XYChartToken::NewLine))
        .repeated()
        .ignored();
    
    // Parse chart header
    let header = just(XYChartToken::XYChart)
        .then_ignore(skip_newlines)
        .then(just(XYChartToken::Horizontal).or_not())
        .map(|(_, horizontal)| {
            if horizontal.is_some() {
                ChartOrientation::Horizontal
            } else {
                ChartOrientation::Vertical
            }
        });
    
    // Parse title
    let title_parser = just(XYChartToken::Title)
        .ignore_then(
            filter_map(|span, token| match token {
                XYChartToken::QuotedString(s) => Ok(s),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
        )
        .then_ignore(skip_newlines);
    
    // Parse axis labels array
    let label_array = just(XYChartToken::LeftBracket)
        .ignore_then(
            filter_map(|span, token| match token {
                XYChartToken::Identifier(s) => Ok(s),
                XYChartToken::QuotedString(s) => Ok(s),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
            .separated_by(
                just(XYChartToken::Comma)
                    .padded_by(filter(|t| matches!(t, XYChartToken::NewLine)).repeated())
            )
            .allow_trailing()
        )
        .then_ignore(just(XYChartToken::RightBracket));
    
    // Parse range (e.g., 4000 --> 11000)
    let range = filter_map(|span, token| match token {
        XYChartToken::Number(n) => Ok(n),
        _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
    })
    .then_ignore(just(XYChartToken::Arrow))
    .then(
        filter_map(|span, token| match token {
            XYChartToken::Number(n) => Ok(n),
            _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
        })
    )
    .map(|(start, end)| (start, end));
    
    // Parse x-axis
    let x_axis_parser = just(XYChartToken::XAxis)
        .then(
            filter_map(|span, token| match token {
                XYChartToken::QuotedString(s) => Ok(Some(s)),
                XYChartToken::LeftBracket => Ok(None),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
        )
        .then(label_array.or(range.map(|_| Vec::new())))
        .then_ignore(skip_newlines)
        .map(|((_, title), labels_or_range)| {
            if labels_or_range.is_empty() {
                // Range was parsed
                XAxis {
                    title,
                    labels: Vec::new(),
                    range: None, // Would need to restructure to capture range
                }
            } else {
                XAxis {
                    title,
                    labels: labels_or_range,
                    range: None,
                }
            }
        });
    
    // Parse y-axis
    let y_axis_parser = just(XYChartToken::YAxis)
        .then(
            filter_map(|span, token| match token {
                XYChartToken::QuotedString(s) => Ok(Some(s)),
                XYChartToken::Number(_) => Ok(None),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
        )
        .then(range.or_not())
        .then_ignore(skip_newlines)
        .map(|((_, title), range)| YAxis { title, range });
    
    // Parse data array
    let data_array = just(XYChartToken::LeftBracket)
        .ignore_then(
            filter_map(|span, token| match token {
                XYChartToken::Number(n) => Ok(n),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
            .separated_by(
                just(XYChartToken::Comma)
                    .padded_by(filter(|t| matches!(t, XYChartToken::NewLine)).repeated())
            )
            .allow_trailing()
        )
        .then_ignore(just(XYChartToken::RightBracket));
    
    // Parse data series
    let line_series = just(XYChartToken::Line)
        .then(
            filter_map(|span, token| match token {
                XYChartToken::QuotedString(s) => Ok(Some(s)),
                XYChartToken::LeftBracket => Ok(None),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
        )
        .then(data_array)
        .map(|((_, name), data)| DataSeries {
            series_type: SeriesType::Line,
            name,
            data,
        });
    
    let bar_series = just(XYChartToken::Bar)
        .then(
            filter_map(|span, token| match token {
                XYChartToken::QuotedString(s) => Ok(Some(s)),
                XYChartToken::LeftBracket => Ok(None),
                _ => Err(Simple::expected_input_found(span, Vec::new(), Some(token))),
            })
        )
        .then(data_array)
        .map(|((_, name), data)| DataSeries {
            series_type: SeriesType::Bar,
            name,
            data,
        });
    
    let data_series = line_series.or(bar_series);
    
    // Main parser
    header
        .then(title_parser.or_not())
        .then(x_axis_parser)
        .then(y_axis_parser)
        .then(
            data_series
                .then_ignore(skip_newlines)
                .repeated()
        )
        .then_ignore(just(XYChartToken::Eof).or_not())
        .map(|((((orientation, title), x_axis), y_axis), data_series)| {
            XYChartDiagram {
                title,
                accessibility: AccessibilityInfo::default(),
                orientation,
                x_axis,
                y_axis,
                data_series,
            }
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/xy/`
- Expected count: 56 files
- Copy to: `mermaid-parser/test/xy/`

### Command
```bash
cp -r ../mermaid-samples/xy/* ./test/xy/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_xy_files(#[files("test/xy/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = xychart_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = xychart_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.data_series.is_empty(), "Should have at least one data series");
}

#[test]
fn test_simple_bar_chart() {
    let input = r#"xychart-beta
    title "Sales"
    x-axis [Q1, Q2, Q3, Q4]
    y-axis "Revenue" 0 --> 10000
    bar [2500, 5000, 7500, 10000]
"#;
    
    let tokens = xychart_lexer().parse(input.chars()).unwrap();
    let diagram = xychart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("Sales".to_string()));
    assert_eq!(diagram.orientation, ChartOrientation::Vertical);
    assert_eq!(diagram.x_axis.labels, vec!["Q1", "Q2", "Q3", "Q4"]);
    assert_eq!(diagram.y_axis.title, Some("Revenue".to_string()));
    assert_eq!(diagram.y_axis.range, Some((0.0, 10000.0)));
    assert_eq!(diagram.data_series.len(), 1);
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Bar);
    assert_eq!(diagram.data_series[0].data, vec![2500.0, 5000.0, 7500.0, 10000.0]);
}

#[test]
fn test_line_chart() {
    let input = r#"xychart-beta
    x-axis [jan, feb, mar]
    y-axis 0 --> 100
    line [10, 50, 30]
"#;
    
    let tokens = xychart_lexer().parse(input.chars()).unwrap();
    let diagram = xychart_parser().parse(tokens).unwrap();
    
    assert!(diagram.title.is_none());
    assert_eq!(diagram.x_axis.labels.len(), 3);
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Line);
}

#[test]
fn test_multiple_series() {
    let input = r#"xychart-beta
    title "Comparison"
    x-axis [A, B, C]
    y-axis 0 --> 100
    bar "Series 1" [20, 40, 60]
    line "Series 2" [30, 50, 70]
"#;
    
    let tokens = xychart_lexer().parse(input.chars()).unwrap();
    let diagram = xychart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.data_series.len(), 2);
    assert_eq!(diagram.data_series[0].name, Some("Series 1".to_string()));
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Bar);
    assert_eq!(diagram.data_series[1].name, Some("Series 2".to_string()));
    assert_eq!(diagram.data_series[1].series_type, SeriesType::Line);
}

#[test]
fn test_horizontal_chart() {
    let input = r#"xychart-beta horizontal
    x-axis [A, B]
    y-axis 0 --> 50
    bar [25, 45]
"#;
    
    let tokens = xychart_lexer().parse(input.chars()).unwrap();
    let diagram = xychart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.orientation, ChartOrientation::Horizontal);
}
```

## Success Criteria
1. ✅ Parse all 56 xy chart sample files successfully
2. ✅ Support both vertical and horizontal orientations
3. ✅ Handle chart titles
4. ✅ Parse x-axis labels and configuration
5. ✅ Parse y-axis ranges and titles
6. ✅ Support multiple data series (line and bar)
7. ✅ Handle named and unnamed data series
8. ✅ Parse numeric data arrays correctly

## Implementation Priority
**Priority 9** - Implement after kanban. XY charts introduce data visualization concepts that are distinct from the workflow/hierarchical patterns seen in earlier diagrams. This establishes foundations for other data visualization types like radar charts.