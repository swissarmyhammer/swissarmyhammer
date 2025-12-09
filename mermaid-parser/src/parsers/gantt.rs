use crate::common::ast::{
    AccessibilityInfo, GanttDiagram, GanttSection, GanttTask, TaskStatus, Weekday, WeekdaySettings,
};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum GanttToken {
    Gantt,                 // "gantt"
    Title(String),         // "title Project Name"
    DateFormat(String),    // "dateFormat YYYY-MM-DD"
    AxisFormat(String),    // "axisFormat %m/%d"
    TickInterval(String),  // "tickInterval 1week"
    Includes(String),      // "includes 2014-01-06"
    Excludes(String),      // "excludes weekends"
    TodayMarker(String),   // "todayMarker stroke-width:5px"
    InclusiveEndDates,     // "inclusiveEndDates"
    TopAxis,               // "topAxis"
    WeekdayMonday,         // "weekday monday"
    WeekdayTuesday,        // "weekday tuesday"
    WeekdayWednesday,      // "weekday wednesday"
    WeekdayThursday,       // "weekday thursday"
    WeekdayFriday,         // "weekday friday"
    WeekdaySaturday,       // "weekday saturday"
    WeekdaySunday,         // "weekday sunday"
    WeekendFriday,         // "weekend friday"
    WeekendSaturday,       // "weekend saturday"
    Date(String),          // "2014-01-01"
    Section(String),       // "section Development"
    TaskText(String),      // Task name
    TaskData(String),      // ": active, 2014-01-01, 30d"
    Colon,                 // ":"
    Click(String),         // "click taskId"
    Href(String),          // "href http://example.com"
    Call(String),          // "call function"
    CallArgs(String),      // "(arg1, arg2)"
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description
    NewLine,
    Eof,
}

pub fn parse(input: &str) -> Result<GanttDiagram> {
    let tokens = gantt_lexer()
        .parse(input)
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to tokenize gantt diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    parse_gantt_diagram(&tokens)
}

fn parse_gantt_diagram(tokens: &[GanttToken]) -> Result<GanttDiagram> {
    let mut i = 0;

    // Find and skip the "gantt" header
    while i < tokens.len() && !matches!(tokens[i], GanttToken::Gantt) {
        i += 1;
    }

    if i >= tokens.len() {
        return Err(ParseError::SyntaxError {
            message: "Expected 'gantt' keyword".to_string(),
            expected: vec!["gantt".to_string()],
            found: "end of input".to_string(),
            line: 0,
            column: 0,
        });
    }

    i += 1; // Skip "gantt"

    // Skip optional newline after gantt
    if i < tokens.len() && matches!(tokens[i], GanttToken::NewLine) {
        i += 1;
    }

    let mut diagram = GanttDiagram {
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
    };

    let mut current_section: Option<GanttSection> = None;
    let mut pending_task: Option<(String, Option<String>)> = None;

    while i < tokens.len() {
        match &tokens[i] {
            GanttToken::Title(title) => {
                diagram.title = Some(title.clone());
            }
            GanttToken::DateFormat(format) => {
                diagram.date_format = Some(format.clone());
            }
            GanttToken::AxisFormat(format) => {
                diagram.axis_format = Some(format.clone());
            }
            GanttToken::TickInterval(interval) => {
                diagram.tick_interval = Some(interval.clone());
            }
            GanttToken::Includes(inc) => {
                diagram.includes.push(inc.clone());
            }
            GanttToken::Excludes(exc) => {
                diagram.excludes.push(exc.clone());
            }
            GanttToken::TodayMarker(marker) => {
                diagram.today_marker = Some(marker.clone());
            }
            GanttToken::InclusiveEndDates => {
                diagram.inclusive_end_dates = true;
            }
            GanttToken::TopAxis => {
                diagram.top_axis = true;
            }
            GanttToken::WeekdayMonday => {
                diagram.weekdays.start_day = Some(Weekday::Monday);
            }
            GanttToken::WeekdayTuesday => {
                diagram.weekdays.start_day = Some(Weekday::Tuesday);
            }
            GanttToken::WeekdayWednesday => {
                diagram.weekdays.start_day = Some(Weekday::Wednesday);
            }
            GanttToken::WeekdayThursday => {
                diagram.weekdays.start_day = Some(Weekday::Thursday);
            }
            GanttToken::WeekdayFriday => {
                diagram.weekdays.start_day = Some(Weekday::Friday);
            }
            GanttToken::WeekdaySaturday => {
                diagram.weekdays.start_day = Some(Weekday::Saturday);
            }
            GanttToken::WeekdaySunday => {
                diagram.weekdays.start_day = Some(Weekday::Sunday);
            }
            GanttToken::WeekendFriday => {
                if !diagram.weekdays.weekend.contains(&Weekday::Friday) {
                    diagram.weekdays.weekend.push(Weekday::Friday);
                }
            }
            GanttToken::WeekendSaturday => {
                if !diagram.weekdays.weekend.contains(&Weekday::Saturday) {
                    diagram.weekdays.weekend.push(Weekday::Saturday);
                }
            }
            GanttToken::Section(name) => {
                // Save any current section
                if let Some(section) = current_section.take() {
                    diagram.sections.push(section);
                }
                current_section = Some(GanttSection {
                    name: name.clone(),
                    tasks: Vec::new(),
                });
            }
            GanttToken::TaskText(text) => {
                // Start of a new task
                pending_task = Some((text.clone(), None));
            }
            GanttToken::TaskData(data) => {
                // We have task data, create a task
                let task_name = if let Some((name, _)) = pending_task.take() {
                    name
                } else {
                    "Unnamed Task".to_string()
                };

                let (id, start_date, duration, status, progress) = parse_task_data(data);
                let mut dependencies = Vec::new();

                // Extract dependencies from data
                for part in data.split(',') {
                    let part = part.trim();
                    if let Some(dep) = part.strip_prefix("after ") {
                        dependencies.push(dep.trim().to_string());
                    }
                }

                let task = GanttTask {
                    name: task_name,
                    id,
                    start_date,
                    duration,
                    dependencies,
                    status,
                    progress,
                    interactions: Vec::new(),
                };

                if let Some(ref mut section) = current_section {
                    section.tasks.push(task);
                } else {
                    // Create a default section if none exists
                    current_section = Some(GanttSection {
                        name: "Default".to_string(),
                        tasks: vec![task],
                    });
                }
            }
            GanttToken::AccTitle => {
                // Next token should be the title value
                if i + 1 < tokens.len() {
                    if let GanttToken::AccTitleValue(title) = &tokens[i + 1] {
                        diagram.accessibility.title = Some(title.clone());
                        i += 1; // Skip the value token
                    }
                }
            }
            GanttToken::AccDescr => {
                // Next token should be the description value
                if i + 1 < tokens.len() {
                    if let GanttToken::AccDescrValue(descr) = &tokens[i + 1] {
                        diagram.accessibility.description = Some(descr.clone());
                        i += 1; // Skip the value token
                    }
                }
            }
            _ => {
                // Skip other tokens for now
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

fn gantt_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<GanttToken>, extra::Err<Simple<'src, char>>> {
    let whitespace = one_of(" \t").repeated();

    let comment = just("%%").then(none_of('\n').repeated()).ignored();

    let gantt_keyword = text::keyword("gantt").map(|_| GanttToken::Gantt);

    let date_format = text::keyword("dateFormat")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n#;").repeated().collect::<String>())
        .map(|format| GanttToken::DateFormat(format.trim().to_string()));

    let axis_format = text::keyword("axisFormat")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n#;").repeated().collect::<String>())
        .map(|format| GanttToken::AxisFormat(format.trim().to_string()));

    let tick_interval = text::keyword("tickInterval")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n#;").repeated().collect::<String>())
        .map(|interval| GanttToken::TickInterval(interval.trim().to_string()));

    let includes = text::keyword("includes")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n#;").repeated().collect::<String>())
        .map(|inc| GanttToken::Includes(inc.trim().to_string()));

    let excludes = text::keyword("excludes")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n#;").repeated().collect::<String>())
        .map(|exc| GanttToken::Excludes(exc.trim().to_string()));

    let today_marker = text::keyword("todayMarker")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("\n;").repeated().collect::<String>())
        .map(|marker| GanttToken::TodayMarker(marker.trim().to_string()));

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

    // Date parser - YYYY-MM-DD format
    let date = text::int(10)
        .then_ignore(just('-'))
        .then(text::int(10))
        .then_ignore(just('-'))
        .then(text::int(10))
        .map(|((year, month), day): ((&str, &str), &str)| {
            GanttToken::Date(format!("{}-{}-{}", year, month, day))
        });

    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|title| GanttToken::Title(title.trim().to_string()));

    let section = text::keyword("section")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|name| GanttToken::Section(name.trim().to_string()));

    let task_data = just(':')
        .then(none_of("\n#;").repeated().collect::<String>())
        .map(|(_, data)| GanttToken::TaskData(data.trim().to_string()));

    let task_text = none_of("\n#:;")
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
        .ignore_then(none_of(" \t\n").repeated().collect::<String>())
        .map(GanttToken::Click);

    let href = text::keyword("href")
        .then(whitespace.at_least(1))
        .then_ignore(just('"'))
        .then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(|(_, url)| GanttToken::Href(url));

    let call = text::keyword("call")
        .then(whitespace.at_least(1))
        .ignore_then(none_of("(\n;").repeated().collect::<String>())
        .map(|func| GanttToken::Call(func.trim().to_string()));

    let newline = choice((just("\n"), just("\r\n"))).map(|_| GanttToken::NewLine);

    let token = choice((
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

fn is_keyword(text: &str) -> bool {
    matches!(
        text.to_lowercase().as_str(),
        "gantt"
            | "dateformat"
            | "axisformat"
            | "tickinterval"
            | "includes"
            | "excludes"
            | "todaymarker"
            | "inclusiveenddates"
            | "topaxis"
            | "title"
            | "section"
            | "click"
            | "href"
            | "call"
            | "weekday"
            | "weekend"
    )
}

fn parse_task_data(
    data: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    TaskStatus,
    Option<f32>,
) {
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
        }
        if part.contains("-") && part.len() == 10 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_gantt() {
        let input = r#"gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
        A task           :a1, 2014-01-01, 30d
        Another task     :after a1, 20d
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.title, Some("A Gantt Diagram".to_string()));
        assert_eq!(diagram.date_format, Some("YYYY-MM-DD".to_string()));
        assert_eq!(diagram.sections.len(), 1);
        assert_eq!(diagram.sections[0].name, "Section");
        assert_eq!(diagram.sections[0].tasks.len(), 2);

        let task1 = &diagram.sections[0].tasks[0];
        assert_eq!(task1.name, "A task");
        assert_eq!(task1.id, Some("a1".to_string()));
        assert_eq!(task1.start_date, Some("2014-01-01".to_string()));
        assert_eq!(task1.duration, Some("30d".to_string()));

        let task2 = &diagram.sections[0].tasks[1];
        assert_eq!(task2.name, "Another task");
        assert_eq!(task2.dependencies, vec!["a1"]);
        assert_eq!(task2.duration, Some("20d".to_string()));
    }

    #[test]
    fn test_task_data_parsing() {
        let (id, start, duration, _status, _progress) = parse_task_data("a1, 2014-01-01, 30d");
        assert_eq!(id, Some("a1".to_string()));
        assert_eq!(start, Some("2014-01-01".to_string()));
        assert_eq!(duration, Some("30d".to_string()));

        let (_, _, _, status, _) = parse_task_data("active, done, crit");
        assert_eq!(status, TaskStatus::Critical); // Takes last status found
    }
}
