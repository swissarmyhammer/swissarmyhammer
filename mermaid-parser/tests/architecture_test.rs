use mermaid_parser::{common::ast::ArchEdgeType, parse_diagram};
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_architecture_files(#[files("test/architecture/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty files or files with only test identifiers
    if content.is_empty()
        || content.lines().any(|line| {
            line.trim().starts_with("architecture-")
                && !line.trim().starts_with("architecture-beta")
        })
    {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Architecture(_diagram) => {
            // Just verify it parsed successfully - some test files might be empty
        }
        _ => panic!("Expected Architecture diagram from {:?}", path),
    }
}

#[test]
fn test_simple_architecture_diagram() {
    let input = r#"architecture-beta
group api(cloud)[API]

service web(internet)[Web Server] in api
service app(server)[App Server] in api
service db(database)[Database] in api

web --> app : REST API
app --> db : SQL
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Architecture(diagram) => {
            assert_eq!(diagram.groups.len(), 1);
            assert_eq!(diagram.services.len(), 3);
            assert_eq!(diagram.edges.len(), 2);

            // Check group
            let api_group = diagram.groups.get("api").unwrap();
            assert_eq!(api_group.id, "api");
            assert_eq!(api_group.title, "API");
            assert_eq!(api_group.icon, Some("cloud".to_string()));

            // Check services
            let web_service = diagram.services.get("web").unwrap();
            assert_eq!(web_service.id, "web");
            assert_eq!(web_service.title, "Web Server");
            assert_eq!(web_service.icon, Some("internet".to_string()));
            assert_eq!(web_service.in_group, Some("api".to_string()));

            // Check edges
            assert_eq!(diagram.edges.len(), 2);
            let first_edge = &diagram.edges[0];
            assert_eq!(first_edge.from.id, "web");
            assert_eq!(first_edge.to.id, "app");
            assert_eq!(first_edge.label, Some("REST API".to_string()));
        }
        _ => panic!("Expected Architecture diagram"),
    }
}

#[test]
fn test_architecture_with_edges() {
    let input = r#"architecture
    service client(person)[Client]
    service server(server)[Server]
    
    client <--> server : HTTP
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Architecture(diagram) => {
            assert_eq!(diagram.services.len(), 2);
            assert_eq!(diagram.edges.len(), 1);

            let edge = &diagram.edges[0];
            assert_eq!(edge.from.id, "client");
            assert_eq!(edge.to.id, "server");
            assert!(matches!(edge.edge_type, ArchEdgeType::BiArrow));
        }
        _ => panic!("Expected Architecture diagram"),
    }
}

#[test]
fn test_nested_groups() {
    let input = r#"architecture
    group outer[Outer Group]
    group inner[Inner Group] in outer
    
    service svc1[Service 1] in inner
    service svc2[Service 2] in outer
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Architecture(diagram) => {
            assert_eq!(diagram.groups.len(), 2);
            assert_eq!(diagram.services.len(), 2);

            // Check nested group
            let inner_group = diagram.groups.get("inner").unwrap();
            assert_eq!(inner_group.in_group, Some("outer".to_string()));
        }
        _ => panic!("Expected Architecture diagram"),
    }
}
