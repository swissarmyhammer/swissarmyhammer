# Implementation Plan: Gantt Charts

## Overview
Gantt charts represent project timelines with tasks, dates, and dependencies.
Medium complexity grammar (188 lines) with date formatting, sections, and task scheduling.

## Grammar Analysis

### Key Features
- Header: `gantt`
- Date formatting: `dateFormat YYYY-MM-DD`
- Axis formatting: `axisFormat %m/%d`
- Task definitions: `Task Name : status, duration`
- Sections: `section Section Name`
- Interactivity: `click`, `href`, `call` commands
- Configuration: `inclusiveEndDates`, `topAxis`, `todayMarker`

### Example Input
```
gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
        A task           :a1, 2014-01-01, 30d
        Another task     :after a1, 20d
    section Another
        Task in sec      :2014-01-12, 12d
        another task     :24d
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GanttDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub date_format: Option<String>,
    pub axis_format: Option<String>,
    pub tick_interval: Option<String>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub today_marker: Option<String>,
    pub inclusive_end_dates: bool,
    pub top_axis: bool,
    pub weekdays: WeekdaySettings,
    pub sections: Vec<GanttSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttTask {
    pub name: String,
    pub id: Option<String>,
    pub start_date: Option<String>,
    pub duration: Option<String>,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub progress: Option<f32>,
    pub interactions: Vec<TaskInteraction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Active,
    Done,
    Critical,
    Milestone,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskInteraction {
    Click { task_id: String },
    Href { url: String },
    Call { function: String, args: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeekdaySettings {
    pub start_day: Option<Weekday>,
    pub weekend: Vec<Weekday>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Weekday {
    Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GanttToken {
    Gantt,                    // "gantt"
    Title(String),            // "title Project Name"
    DateFormat(String),       // "dateFormat YYYY-MM-DD"
    AxisFormat(String),       // "axisFormat %m/%d"
    TickInterval(String),     // "tickInterval 1week"
    Includes(String),         // "includes 2014-01-06"
    Excludes(String),         // "excludes weekends"
    TodayMarker(String),      // "todayMarker stroke-width:5px"
    InclusiveEndDates,        // "inclusiveEndDates"
    TopAxis,                  // "topAxis"
    WeekdayMonday,           // "weekday monday"
    WeekdayTuesday,          // "weekday tuesday"
    WeekdayWednesday,        // "weekday wednesday"
    WeekdayThursday,         // "weekday thursday"
    WeekdayFriday,           // "weekday friday"
    WeekdaySaturday,         // "weekday saturday"
    WeekdaySunday,           // "weekday sunday"
    WeekendFriday,           // "weekend friday"
    WeekendSaturday,         // "weekend saturday"
    Date(String),            // "2014-01-01"
    Section(String),         // "section Development"
    TaskText(String),        // Task name
    TaskData(String),        // ": active, 2014-01-01, 30d"
    Colon,                   // ":"
    Click(String),           // "click taskId"
    Href(String),            // "href http://example.com"
    Call(String),            // "call function"
    CallArgs(String),        // "(arg1, arg2)"
    AccTitle,                // "accTitle"
    AccTitleValue(String),   // Accessibility title
    AccDescr,                // "accDescr"
    AccDescrValue(String),   // Accessibility description
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn gantt_lexer() -> impl Parser<char, Vec<GanttToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let gantt_keyword = text::keyword("gantt")
        .map(|_| GanttToken::Gantt);
    
    let date_format = text::keyword("dateFormat")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, format)| GanttToken::DateFormat(format.trim().to_string()));
    
    let axis_format = text::keyword("axisFormat")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, format)| GanttToken::AxisFormat(format.trim().to_string()));
    
    let tick_interval = text::keyword("tickInterval")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, interval)| GanttToken::TickInterval(interval.trim().to_string()));
    
    let includes = text::keyword("includes")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, inc)| GanttToken::Includes(inc.trim().to_string()));
    
    let excludes = text::keyword("excludes")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, exc)| GanttToken::Excludes(exc.trim().to_string()));
    
    let today_marker = text::keyword("todayMarker")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, marker)| GanttToken::TodayMarker(marker.trim().to_string()));
    
    let flags = choice((
        text::keyword("inclusiveEndDates").map(|_| GanttToken::InclusiveEndDates),
        text::keyword("topAxis").map(|_| GanttToken::TopAxis),
    ));
    
    let weekdays = choice((
        text::keyword("weekday monday").map(|_| GanttToken::WeekdayMonday),
        text::keyword("weekday tuesday").map(|_| GanttToken::WeekdayTuesday),
        text::keyword("weekday wednesday").map(|_| GanttToken::WeekdayWednesday),
        text::keyword("weekday thursday").map(|_| GanttToken::WeekdayThursday),
        text::keyword("weekday friday").map(|_| GanttToken::WeekdayFriday),
        text::keyword("weekday saturday").map(|_| GanttToken::WeekdaySaturday),
        text::keyword("weekday sunday").map(|_| GanttToken::WeekdaySunday),
        text::keyword("weekend friday").map(|_| GanttToken::WeekendFriday),
        text::keyword("weekend saturday").map(|_| GanttToken::WeekendSaturday),
    ));
    
    let date = text::digits(4)
        .then_ignore(just('-'))
        .then(text::digits(2))
        .then_ignore(just('-'))
        .then(text::digits(2))
        .map(|((year, month), day)| {
            GanttToken::Date(format!("{}-{}-{}", year, month, day))
        });
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(_, title)| GanttToken::Title(title.trim().to_string()));
    
    let section = text::keyword("section")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(_, name)| GanttToken::Section(name.trim().to_string()));
    
    let task_data = just(':')
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, data)| GanttToken::TaskData(data.trim().to_string()));
    
    let task_text = filter(|c| !matches!(*c, '\n' | '#' | ':' | ';'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| GanttToken::TaskText(text.trim().to_string()))
        .filter(|token| {
            if let GanttToken::TaskText(text) = token {
                !text.is_empty() && !is_keyword(text)
            } else {
                true
            }
        });
    
    let click = text::keyword("click")
        .then(whitespace.at_least(1))
        .then(
            filter(|c| !c.is_whitespace() && *c != '\n')
                .repeated()
                .collect::<String>()
        )
        .map(|(_, id)| GanttToken::Click(id));
    
    let href = text::keyword("href")
        .then(whitespace.at_least(1))
        .then_ignore(just('"'))
        .then(
            take_until(just('"'))
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(|(_, url)| GanttToken::Href(url));
    
    let call = text::keyword("call")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('('), just('\n'), just(';'))))
                .collect::<String>()
        )
        .map(|(_, func)| GanttToken::Call(func.trim().to_string()));
    
    let newline = just('\n')
        .map(|_| GanttToken::NewLine);
    
    choice((
        comment.ignored(),
        gantt_keyword,
        date_format,
        axis_format,
        tick_interval,
        includes,
        excludes,
        today_marker,
        flags,
        weekdays,
        date,
        title,
        section,
        click,
        href,
        call,
        task_data,
        task_text,
        newline,
    ))
    .padded_by(whitespace)
    .repeated()
    .then_ignore(end())
}

fn is_keyword(text: &str) -> bool {
    matches!(text.to_lowercase().as_str(),
        "gantt" | "dateformat" | "axisformat" | "tickinterval" | "includes" |
        "excludes" | "todaymarker" | "inclusiveenddates" | "topaxis" |
        "title" | "section" | "click" | "href" | "call" | "weekday" | "weekend"
    )
}
```

## Step 3: Parser Implementation

### Task Data Parser
```rust
fn parse_task_data(data: &str) -> (Option<String>, Option<String>, Option<String>, TaskStatus, Option<f32>) {
    let parts: Vec<&str> = data.split(',').map(|s| s.trim()).collect();
    
    let mut id = None;
    let mut start_date = None;
    let mut duration = None;
    let mut status = TaskStatus::None;
    let mut progress = None;
    
    for part in parts {
        if part.starts_with("after ") {
            // Dependency
            continue;
        } else if part.contains("-") && part.len() == 10 {
            // Date format YYYY-MM-DD
            start_date = Some(part.to_string());
        } else if part.ends_with("d") || part.ends_with("h") || part.ends_with("w") {
            // Duration
            duration = Some(part.to_string());
        } else if part == "active" {
            status = TaskStatus::Active;
        } else if part == "done" {
            status = TaskStatus::Done;
        } else if part == "crit" {
            status = TaskStatus::Critical;
        } else if part == "milestone" {
            status = TaskStatus::Milestone;
        } else if part.ends_with("%") {
            // Progress percentage
            if let Ok(pct) = part.trim_end_matches('%').parse::<f32>() {
                progress = Some(pct);
            }
        } else if !part.is_empty() {
            // Task ID
            id = Some(part.to_string());
        }
    }
    
    (id, start_date, duration, status, progress)
}
```

### Main Parser
```rust
pub fn gantt_parser() -> impl Parser<GanttToken, GanttDiagram, Error = Simple<GanttToken>> {
    // Implementation would handle all the gantt-specific tokens and build the AST
    // This is a complex parser requiring careful handling of:
    // 1. Configuration directives
    // 2. Section and task parsing
    // 3. Date and duration parsing
    // 4. Interaction commands
    // 5. Accessibility attributes
    
    just(GanttToken::Gantt)
        .then_ignore(just(GanttToken::NewLine).or_not())
        .then(
            // Parse all statements
            choice((
                // Configuration
                select! { GanttToken::DateFormat(f) => ("date_format", f) },
                select! { GanttToken::AxisFormat(f) => ("axis_format", f) },
                select! { GanttToken::Title(t) => ("title", t) },
                select! { GanttToken::Section(s) => ("section", s) },
                // Tasks and other elements...
            ))
            .separated_by(just(GanttToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(GanttToken::Eof).or_not())
        .map(|(_, statements)| {
            // Build GanttDiagram from parsed statements
            GanttDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                date_format: None,
                axis_format: None,
                tick_interval: None,
                includes: Vec::new(),
                excludes: Vec::new(),
                today_marker: None,
                inclusive_end_dates: false,
                top_axis: false,
                weekdays: WeekdaySettings::default(),
                sections: Vec::new(),
            }
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/gantt/`
- Expected count: 45 files
- Copy to: `mermaid-parser/test/gantt/`

### Command
```bash
cp -r ../mermaid-samples/gantt/* ./test/gantt/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_gantt_files(#[files("test/gantt/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = gantt_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = gantt_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(diagram.sections.len() > 0 || diagram.title.is_some(),
            "Gantt should have sections or title");
}

#[test]
fn test_simple_gantt() {
    let input = r#"gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
        A task           :a1, 2014-01-01, 30d
        Another task     :after a1, 20d
"#;
    
    let tokens = gantt_lexer().parse(input.chars()).unwrap();
    let diagram = gantt_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("A Gantt Diagram".to_string()));
    assert_eq!(diagram.date_format, Some("YYYY-MM-DD".to_string()));
    assert_eq!(diagram.sections.len(), 1);
    assert_eq!(diagram.sections[0].tasks.len(), 2);
}

#[test]
fn test_task_data_parsing() {
    let (id, start, duration, status, progress) = parse_task_data("a1, 2014-01-01, 30d");
    assert_eq!(id, Some("a1".to_string()));
    assert_eq!(start, Some("2014-01-01".to_string()));
    assert_eq!(duration, Some("30d".to_string()));
    
    let (_, _, _, status, _) = parse_task_data("active, done, crit");
    assert_eq!(status, TaskStatus::Active); // Takes first status found
}

#[test]
fn test_date_formats() {
    let input = "gantt\ndateFormat YYYY-MM-DD\n2014-01-01";
    let tokens = gantt_lexer().parse(input.chars()).unwrap();
    
    // Should contain date token
    assert!(tokens.iter().any(|t| matches!(t, GanttToken::Date(_))));
}
```

## Success Criteria
1. ✅ Parse all 45 gantt sample files successfully
2. ✅ Handle date formatting and axis configuration
3. ✅ Support sections and task definitions
4. ✅ Parse task data (IDs, dates, durations, status)
5. ✅ Handle dependencies and milestones
6. ✅ Support interactive elements (click, href, call)
7. ✅ Process weekday and weekend configurations
8. ✅ Handle accessibility attributes

## Implementation Priority
**Priority 5** - Implement after simpler grammars. Gantt charts introduce date/time parsing and project management concepts that are useful for timeline-based diagrams.