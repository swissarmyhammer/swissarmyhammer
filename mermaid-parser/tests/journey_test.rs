use mermaid_parser::parsers::journey;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_journey_files(#[files("test/journey/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read file {:?}: {}", path, e));

    // Remove metadata comments that might interfere with parsing
    let content = content
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Skip empty files
    if content.trim().is_empty() {
        return;
    }

    let result = journey::parse(&content);

    match result {
        Ok(diagram) => {
            // Basic validation - journey should have either sections or be minimal
            println!("✅ Successfully parsed {:?}", path.file_name().unwrap());
            println!("   Title: {:?}", diagram.title);
            println!("   Sections: {}", diagram.sections.len());
            println!("   Accessibility: {:?}", diagram.accessibility);
        }
        Err(e) => {
            panic!(
                "Failed to parse journey file {:?}:\nContent:\n{}\nError: {:?}",
                path.file_name().unwrap(),
                content,
                e
            );
        }
    }
}

#[test]
fn test_journey_with_complex_tasks() {
    let input = r#"journey
    title Complex User Journey
    section Planning Phase
        Research options: 3: User, Advisor
        Compare solutions: 2: User
        Make decision: 4: User, Manager
    section Implementation
        Setup environment: 1: Developer, DevOps
        Write code: 5: Developer
        Review code: 4: Developer, Senior Dev
        Deploy: 3: DevOps
"#;

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse complex journey: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Complex User Journey".to_string()));
    assert_eq!(diagram.sections.len(), 2);

    // Check Planning Phase
    let planning = &diagram.sections[0];
    assert_eq!(planning.name, "Planning Phase");
    assert_eq!(planning.tasks.len(), 3);

    let research_task = &planning.tasks[0];
    assert_eq!(research_task.name, "Research options");
    assert_eq!(research_task.score, 3);
    assert_eq!(research_task.actors, vec!["User", "Advisor"]);

    // Check Implementation Phase
    let implementation = &diagram.sections[1];
    assert_eq!(implementation.name, "Implementation");
    assert_eq!(implementation.tasks.len(), 4);

    let deploy_task = &implementation.tasks[3];
    assert_eq!(deploy_task.name, "Deploy");
    assert_eq!(deploy_task.score, 3);
    assert_eq!(deploy_task.actors, vec!["DevOps"]);
}

#[test]
fn test_journey_with_accessibility() {
    let input = r#"journey
    accTitle My Journey Accessibility Title
    accDescr This journey shows user satisfaction levels
    title Shopping Experience
    section Browse
        Look at products: 4: Customer
"#;

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse accessibility journey: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.title,
        Some("My Journey Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("This journey shows user satisfaction levels".to_string())
    );
    assert_eq!(diagram.title, Some("Shopping Experience".to_string()));
}

#[test]
fn test_minimal_journey() {
    let input = "journey";

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse minimal journey: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.title, None);
    assert_eq!(diagram.sections.len(), 0);
}

#[test]
fn test_journey_without_sections() {
    let input = r#"journey
    title My Simple Day
"#;

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse journey without sections: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("My Simple Day".to_string()));
    assert_eq!(diagram.sections.len(), 0);
}

#[test]
fn test_task_with_multiple_actors() {
    let input = r#"journey
    section Team Work
        Brainstorm ideas: 5: Alice, Bob, Carol, Dave
"#;

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse multi-actor task: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 1);

    let task = &diagram.sections[0].tasks[0];
    assert_eq!(task.name, "Brainstorm ideas");
    assert_eq!(task.score, 5);
    assert_eq!(task.actors, vec!["Alice", "Bob", "Carol", "Dave"]);
}

#[test]
fn test_task_score_edge_cases() {
    let input = r#"journey
    section Edge Cases
        Zero score: 0: User
        Negative score: -1: User
        High score: 10: User
"#;

    let result = journey::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse edge case scores: {:?}",
        result
    );

    let diagram = result.unwrap();
    let tasks = &diagram.sections[0].tasks;

    assert_eq!(tasks[0].score, 0);
    assert_eq!(tasks[1].score, -1);
    assert_eq!(tasks[2].score, 10);
}
