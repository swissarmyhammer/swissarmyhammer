//! Basic tests to improve pretty_print coverage for core functionality

use mermaid_parser::common::ast::*;
use mermaid_parser::*;
use std::collections::HashMap;

// Test PrintOptions default implementation
#[test]
fn test_print_options_default() {
    let options = PrintOptions::default();
    assert_eq!(options.indent_width, 4);
    assert_eq!(options.max_line_length, 80);
    assert!(!options.align_arrows);
    assert!(!options.sort_nodes);
    assert!(!options.compact_mode);
}

// Test edge cases in PrintOptions
#[test]
fn test_print_options_edge_cases() {
    let input = "flowchart TD\nA --> B";
    let diagram = parse_diagram(input).expect("Failed to parse");

    // Test with zero indent width
    let zero_indent = PrintOptions {
        indent_width: 0,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };
    let output = diagram.to_mermaid_pretty(&zero_indent);
    assert!(output.contains("flowchart TD"));

    // Test with very large indent
    let large_indent = PrintOptions {
        indent_width: 100,
        max_line_length: 1000,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };
    let large_output = diagram.to_mermaid_pretty(&large_indent);
    assert!(large_output.contains("flowchart TD"));

    // Test with small max line length
    let small_line = PrintOptions {
        indent_width: 4,
        max_line_length: 5,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };
    let small_output = diagram.to_mermaid_pretty(&small_line);
    assert!(small_output.contains("flowchart TD"));
}

// Test compact mode behavior
#[test]
fn test_compact_mode_comprehensive() {
    let input = "flowchart TD\nA[Start] --> B{Decision}\nB -->|Yes| C[End]\nB -->|No| D[Stop]";
    let diagram = parse_diagram(input).expect("Failed to parse");

    let compact_options = PrintOptions {
        indent_width: 4,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: true,
    };

    let compact_output = diagram.to_mermaid_pretty(&compact_options);

    // Verify no lines are indented in compact mode
    for line in compact_output.lines() {
        if !line.trim().is_empty() {
            assert!(
                !line.starts_with("    "),
                "Line should not be indented in compact mode: '{}'",
                line
            );
        }
    }

    // Test compact mode with align_arrows
    let compact_align = PrintOptions {
        compact_mode: true,
        align_arrows: true,
        ..Default::default()
    };
    let compact_align_output = diagram.to_mermaid_pretty(&compact_align);
    assert!(compact_align_output.contains("flowchart TD"));

    // Test compact mode with sort_nodes
    let compact_sort = PrintOptions {
        compact_mode: true,
        sort_nodes: true,
        ..Default::default()
    };
    let compact_sort_output = diagram.to_mermaid_pretty(&compact_sort);
    assert!(compact_sort_output.contains("flowchart TD"));
}

// Test flowchart with all flow directions
#[test]
fn test_flowchart_all_directions() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Node A".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let directions = [
        FlowDirection::TD,
        FlowDirection::TB,
        FlowDirection::BT,
        FlowDirection::RL,
        FlowDirection::LR,
    ];

    let expected_strings = ["TD", "TB", "BT", "RL", "LR"];

    for (direction, expected) in directions.iter().zip(expected_strings.iter()) {
        let diagram = DiagramType::Flowchart(FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: direction.clone(),
            nodes: nodes.clone(),
            edges: vec![],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        let output = diagram.to_mermaid();
        assert!(output.contains(&format!("flowchart {}", expected)));
    }
}

// Test flowchart with all edge types
#[test]
fn test_flowchart_all_edge_types() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Source".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "B".to_string(),
        FlowNode {
            id: "B".to_string(),
            text: Some("Target".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let edge_types = [
        EdgeType::Arrow,
        EdgeType::DottedArrow,
        EdgeType::ThickArrow,
        EdgeType::OpenLink,
        EdgeType::DottedLink,
        EdgeType::ThickLink,
        EdgeType::Invisible,
        EdgeType::CircleEdge,
        EdgeType::CrossEdge,
        EdgeType::MultiDirectional,
    ];

    let expected_arrows = [
        "-->", "-.->", "==>", "---", "-.-", "===", "~~~", "--o", "--x", "<-->",
    ];

    for (edge_type, expected_arrow) in edge_types.iter().zip(expected_arrows.iter()) {
        let diagram = DiagramType::Flowchart(FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes: nodes.clone(),
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: edge_type.clone(),
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        let output = diagram.to_mermaid();
        assert!(
            output.contains(expected_arrow),
            "Expected arrow '{}' not found in output for edge type {:?}",
            expected_arrow,
            edge_type
        );
    }
}

// Test flowchart with labeled edges
#[test]
fn test_flowchart_labeled_edges() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Start".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "B".to_string(),
        FlowNode {
            id: "B".to_string(),
            text: Some("End".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let diagram = DiagramType::Flowchart(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges: vec![FlowEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            edge_type: EdgeType::Arrow,
            label: Some("proceed".to_string()),
            min_length: None,
        }],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("-->|proceed|"));

    // Test with alignment
    let align_options = PrintOptions {
        align_arrows: true,
        ..Default::default()
    };
    let aligned_output = diagram.to_mermaid_pretty(&align_options);
    assert!(aligned_output.contains("-->|proceed|"));
}

// Test sequence diagram with basic elements
#[test]
fn test_sequence_diagram_basic_elements() {
    let diagram = DiagramType::Sequence(SequenceDiagram {
        title: Some("Basic Sequence".to_string()),
        accessibility: AccessibilityInfo::default(),
        participants: vec![
            Participant {
                actor: "Alice".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
            Participant {
                actor: "Bob".to_string(),
                participant_type: ParticipantType::Actor,
                alias: None,
            },
        ],
        statements: vec![SequenceStatement::Message(Message {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            text: "Hello".to_string(),
            arrow_type: ArrowType::SolidOpen,
        })],
        autonumber: None,
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("sequenceDiagram"));
    assert!(output.contains("participant Alice"));
    assert!(output.contains("actor Bob"));
    assert!(output.contains("Alice -> Bob: Hello"));
}

// Test timeline diagram basic functionality
#[test]
fn test_timeline_diagram_basic() {
    let diagram = DiagramType::Timeline(TimelineDiagram {
        title: Some("Project Timeline".to_string()),
        accessibility: AccessibilityInfo::default(),
        sections: vec![TimelineSection {
            name: "Phase 1".to_string(),
            items: vec![
                TimelineItem::Event("Start".to_string()),
                TimelineItem::Period("Q1 2023".to_string()),
            ],
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("timeline"));
    assert!(output.contains("Phase 1"));
    assert!(output.contains("Start"));
    assert!(output.contains("Q1 2023"));
}

// Test journey diagram basic functionality
#[test]
fn test_journey_diagram_basic() {
    let diagram = DiagramType::Journey(JourneyDiagram {
        title: Some("User Journey".to_string()),
        accessibility: AccessibilityInfo::default(),
        sections: vec![JourneySection {
            name: "Discovery".to_string(),
            tasks: vec![JourneyTask {
                name: "Research".to_string(),
                score: 5,
                actors: vec!["User".to_string()],
            }],
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("journey"));
    assert!(output.contains("Discovery"));
    assert!(output.contains("Research: 5: User"));
}

// Test pie diagram basic functionality
#[test]
fn test_pie_diagram_basic() {
    let diagram = DiagramType::Pie(PieDiagram {
        title: Some("Market Share".to_string()),
        accessibility: AccessibilityInfo::default(),
        show_data: true,
        data: vec![
            PieSlice {
                label: "Company A".to_string(),
                value: 45.0,
            },
            PieSlice {
                label: "Company B".to_string(),
                value: 35.0,
            },
        ],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("pie"));
    assert!(output.contains("Company A"));
    assert!(output.contains("45"));
    assert!(output.contains("Company B"));
    assert!(output.contains("35"));
}

// Test sankey diagram basic functionality
#[test]
fn test_sankey_diagram_basic() {
    let diagram = DiagramType::Sankey(SankeyDiagram {
        nodes: vec![
            SankeyNode {
                id: "A".to_string(),
                name: "Source".to_string(),
            },
            SankeyNode {
                id: "B".to_string(),
                name: "Target".to_string(),
            },
        ],
        links: vec![SankeyLink {
            source: "A".to_string(),
            target: "B".to_string(),
            value: 100.0,
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("sankey-beta"));
    assert!(output.contains("A,B,100"));
}

// Test misc diagram basic functionality
#[test]
fn test_misc_diagram_basic() {
    let diagram = DiagramType::Misc(MiscDiagram {
        diagram_type: "custom".to_string(),
        content: MiscContent::Raw(RawDiagram {
            lines: vec![
                "custom content line 1".to_string(),
                "custom content line 2".to_string(),
            ],
        }),
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("custom content line 1"));
    assert!(output.contains("custom content line 2"));
}

// Test empty diagram handling
#[test]
fn test_empty_diagrams() {
    // Test empty flowchart
    let empty_flowchart = DiagramType::Flowchart(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes: HashMap::new(),
        edges: vec![],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let output = empty_flowchart.to_mermaid();
    assert!(output.contains("flowchart TD"));

    // Test empty sequence
    let empty_sequence = DiagramType::Sequence(SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![],
        statements: vec![],
        autonumber: None,
    });

    let seq_output = empty_sequence.to_mermaid();
    assert!(seq_output.contains("sequenceDiagram"));
}

// Test alignment functionality
#[test]
fn test_alignment_functionality() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Node A".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "B".to_string(),
        FlowNode {
            id: "B".to_string(),
            text: Some("Node B".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let diagram = DiagramType::Flowchart(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges: vec![FlowEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        }],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    // Test without alignment
    let normal_output = diagram.to_mermaid();
    assert!(normal_output.contains("A[Node A] --> B[Node B]"));

    // Test with alignment
    let align_options = PrintOptions {
        align_arrows: true,
        ..Default::default()
    };
    let aligned_output = diagram.to_mermaid_pretty(&align_options);
    assert!(aligned_output.contains("A[Node A] --> B[Node B]"));
}
