use mermaid_parser::{parse_diagram, MermaidPrinter, PrintOptions};

#[test]
fn test_pretty_printer_exists() {
    // Just test that the pretty printer trait is available
    let input = "pie\n\"A\": 10\n\"B\": 20";
    let diagram = parse_diagram(input).expect("Failed to parse");
    let output = diagram.to_mermaid();
    assert!(output.contains("pie"));
}

#[test]
fn test_pretty_printer_with_options() {
    let input = "pie\n\"A\": 10\n\"B\": 20";
    let diagram = parse_diagram(input).expect("Failed to parse");

    let options = PrintOptions {
        indent_width: 2,
        max_line_length: 100,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: true,
    };

    let output = diagram.to_mermaid_pretty(&options);
    assert!(output.contains("pie"));
}

#[test]
fn test_mindmap_pretty_print() {
    let input = "mindmap\nroot((mindmap))\n  Origins\n    Long history";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    // Verify structure is preserved
    assert!(output.contains("mindmap"));
    assert!(output.contains("root((mindmap))"));
    assert!(output.contains("Origins"));
    assert!(output.contains("Long history"));
}

#[test]
fn test_flowchart_basic_structure() {
    let input = "flowchart TD\nA[Start]\nB{Decision}";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("flowchart TD"));
    assert!(output.contains("A[Start]"));
    assert!(output.contains("B{Decision}"));
}

#[test]
fn test_sequence_basic_structure() {
    let input = "sequenceDiagram\nAlice->>Bob: Hello";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("sequenceDiagram"));
    assert!(output.contains("participant Alice"));
    assert!(output.contains("participant Bob"));
    assert!(output.contains("Alice ->> Bob: Hello"));
}

#[test]
fn test_class_basic_structure() {
    let input = "classDiagram\nclass Animal\nclass Dog";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("classDiagram"));
    assert!(output.contains("class Animal"));
    assert!(output.contains("class Dog"));
}

#[test]
fn test_pie_chart_structure() {
    let input = "pie\n\"Dogs\": 386\n\"Cats\": 85";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("pie"));
    assert!(output.contains("\"Dogs\" : 386"));
    assert!(output.contains("\"Cats\" : 85"));
}

#[test]
fn test_state_diagram_structure() {
    let input = "stateDiagram-v2\n[*] --> Still\nStill --> Moving";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("stateDiagram-v2"));
    assert!(output.contains("[*] --> Still"));
    assert!(output.contains("Still --> Moving"));
}

#[test]
fn test_gantt_basic_structure() {
    let input = "gantt\ntitle My Gantt\nsection Planning\nTask 1: task1, 2023-01-01, 10d";

    let diagram = parse_diagram(input).expect("Failed to parse diagram");
    let output = diagram.to_mermaid();

    assert!(output.contains("gantt"));
    assert!(output.contains("title My Gantt"));
    assert!(output.contains("section Planning"));
}
