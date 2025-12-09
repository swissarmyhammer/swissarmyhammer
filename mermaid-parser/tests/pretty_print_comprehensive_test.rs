//! Comprehensive tests for pretty_print module to improve coverage

use mermaid_parser::common::ast::*;
use mermaid_parser::*;
use std::collections::HashMap;

// Test all PrintOptions combinations
#[test]
fn test_print_options_all_combinations() {
    let input = "flowchart TD\nC[Node C]\nA[Node A]\nB[Node B]\nA --> B\nB --> C";
    let diagram = parse_diagram(input).expect("Failed to parse");

    // Test compact mode
    let compact_options = PrintOptions {
        indent_width: 4,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: true,
    };
    let compact_output = diagram.to_mermaid_pretty(&compact_options);
    for line in compact_output.lines() {
        assert!(!line.starts_with("    "), "Compact mode should not indent");
    }

    // Test sort nodes
    let sort_options = PrintOptions {
        indent_width: 4,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: true,
        compact_mode: false,
    };
    let sorted_output = diagram.to_mermaid_pretty(&sort_options);
    assert!(sorted_output.contains("A[Node A]"));
    assert!(sorted_output.contains("B[Node B]"));
    assert!(sorted_output.contains("C[Node C]"));

    // Test different indent widths
    let indent_2_options = PrintOptions {
        indent_width: 2,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };
    let indent_2_output = diagram.to_mermaid_pretty(&indent_2_options);
    assert!(indent_2_output.lines().nth(1).unwrap().starts_with("  "));

    let indent_8_options = PrintOptions {
        indent_width: 8,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };
    let indent_8_output = diagram.to_mermaid_pretty(&indent_8_options);
    assert!(indent_8_output
        .lines()
        .nth(1)
        .unwrap()
        .starts_with("        "));
}

// Test all flowchart node shapes
#[test]
fn test_flowchart_all_node_shapes() {
    let mut nodes = HashMap::new();

    // Create nodes with all different shapes
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Rectangle".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "B".to_string(),
        FlowNode {
            id: "B".to_string(),
            text: Some("RoundedRectangle".to_string()),
            shape: NodeShape::RoundedRectangle,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "C".to_string(),
        FlowNode {
            id: "C".to_string(),
            text: Some("Stadium".to_string()),
            shape: NodeShape::Stadium,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "D".to_string(),
        FlowNode {
            id: "D".to_string(),
            text: Some("Subroutine".to_string()),
            shape: NodeShape::Subroutine,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "E".to_string(),
        FlowNode {
            id: "E".to_string(),
            text: Some("Cylinder".to_string()),
            shape: NodeShape::Cylinder,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "F".to_string(),
        FlowNode {
            id: "F".to_string(),
            text: Some("Circle".to_string()),
            shape: NodeShape::Circle,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "G".to_string(),
        FlowNode {
            id: "G".to_string(),
            text: Some("Asymmetric".to_string()),
            shape: NodeShape::Asymmetric,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "H".to_string(),
        FlowNode {
            id: "H".to_string(),
            text: Some("Rhombus".to_string()),
            shape: NodeShape::Rhombus,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "I".to_string(),
        FlowNode {
            id: "I".to_string(),
            text: Some("Hexagon".to_string()),
            shape: NodeShape::Hexagon,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "J".to_string(),
        FlowNode {
            id: "J".to_string(),
            text: Some("Parallelogram".to_string()),
            shape: NodeShape::Parallelogram,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "K".to_string(),
        FlowNode {
            id: "K".to_string(),
            text: Some("ParallelogramAlt".to_string()),
            shape: NodeShape::ParallelogramAlt,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "L".to_string(),
        FlowNode {
            id: "L".to_string(),
            text: Some("Trapezoid".to_string()),
            shape: NodeShape::Trapezoid,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "M".to_string(),
        FlowNode {
            id: "M".to_string(),
            text: Some("TrapezoidAlt".to_string()),
            shape: NodeShape::TrapezoidAlt,
            classes: vec![],
            icon: None,
        },
    );

    nodes.insert(
        "N".to_string(),
        FlowNode {
            id: "N".to_string(),
            text: Some("DoubleCircle".to_string()),
            shape: NodeShape::DoubleCircle,
            classes: vec![],
            icon: None,
        },
    );

    let diagram = DiagramType::Flowchart(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges: vec![],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let output = diagram.to_mermaid();

    // Verify each shape is properly formatted
    assert!(output.contains("A[Rectangle]"));
    assert!(output.contains("B(RoundedRectangle)"));
    assert!(output.contains("C([Stadium])"));
    assert!(output.contains("D[[Subroutine]]"));
    assert!(output.contains("E[(Cylinder)]"));
    assert!(output.contains("F((Circle)))"));
    assert!(output.contains("G>Asymmetric]"));
    assert!(output.contains("H{Rhombus}"));
    assert!(output.contains("I{{Hexagon}}"));
    assert!(output.contains("J[/Parallelogram\\]"));
    assert!(output.contains("K[\\ParallelogramAlt/]"));
    assert!(output.contains("L[/Trapezoid/]"));
    assert!(output.contains("M[\\TrapezoidAlt\\]"));
    assert!(output.contains("N(((DoubleCircle)))"));
}

// Test all flowchart edge types
#[test]
fn test_flowchart_all_edge_types() {
    let mut nodes = HashMap::new();
    for i in 0..=10 {
        nodes.insert(
            format!("N{}", i),
            FlowNode {
                id: format!("N{}", i),
                text: Some(format!("Node {}", i)),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );
    }

    let edges = vec![
        FlowEdge {
            from: "N0".to_string(),
            to: "N1".to_string(),
            edge_type: EdgeType::Arrow,
            label: Some("Arrow".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N1".to_string(),
            to: "N2".to_string(),
            edge_type: EdgeType::DottedArrow,
            label: Some("DottedArrow".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N2".to_string(),
            to: "N3".to_string(),
            edge_type: EdgeType::ThickArrow,
            label: Some("ThickArrow".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N3".to_string(),
            to: "N4".to_string(),
            edge_type: EdgeType::OpenLink,
            label: Some("OpenLink".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N4".to_string(),
            to: "N5".to_string(),
            edge_type: EdgeType::DottedLink,
            label: Some("DottedLink".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N5".to_string(),
            to: "N6".to_string(),
            edge_type: EdgeType::ThickLink,
            label: Some("ThickLink".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N6".to_string(),
            to: "N7".to_string(),
            edge_type: EdgeType::Invisible,
            label: Some("Invisible".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N7".to_string(),
            to: "N8".to_string(),
            edge_type: EdgeType::CircleEdge,
            label: Some("CircleEdge".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N8".to_string(),
            to: "N9".to_string(),
            edge_type: EdgeType::CrossEdge,
            label: Some("CrossEdge".to_string()),
            min_length: None,
        },
        FlowEdge {
            from: "N9".to_string(),
            to: "N10".to_string(),
            edge_type: EdgeType::MultiDirectional,
            label: Some("MultiDirectional".to_string()),
            min_length: None,
        },
    ];

    let diagram = DiagramType::Flowchart(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges,
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let output = diagram.to_mermaid();

    // Verify each edge type is properly formatted
    assert!(output.contains("N0[Node 0] -->|Arrow| N1[Node 1]"));
    assert!(output.contains("N1 -.->|DottedArrow| N2[Node 2]"));
    assert!(output.contains("N2 ==>|ThickArrow| N3[Node 3]"));
    assert!(output.contains("N3 ---|OpenLink| N4[Node 4]"));
    assert!(output.contains("N4 -.-|DottedLink| N5[Node 5]"));
    assert!(output.contains("N5 ===|ThickLink| N6[Node 6]"));
    assert!(output.contains("N6 ~~~|Invisible| N7[Node 7]"));
    assert!(output.contains("N7 --o|CircleEdge| N8[Node 8]"));
    assert!(output.contains("N8 --x|CrossEdge| N9[Node 9]"));
    assert!(output.contains("N9 <-->|MultiDirectional| N10[Node 10]"));
}

// Test sequence diagram with different message types and notes
#[test]
fn test_sequence_diagram_all_statement_types() {
    let statements = vec![
        SequenceStatement::Message(Message {
            from: "A".to_string(),
            to: "B".to_string(),
            text: "Hello".to_string(),
            arrow_type: ArrowType::SolidOpen,
        }),
        SequenceStatement::Loop(Loop {
            condition: "while active".to_string(),
            statements: vec![SequenceStatement::Message(Message {
                from: "B".to_string(),
                to: "C".to_string(),
                text: "Process".to_string(),
                arrow_type: ArrowType::SolidClosed,
            })],
        }),
        SequenceStatement::Note(Note {
            position: NotePosition::LeftOf,
            actor: "A".to_string(),
            text: "Left note".to_string(),
        }),
        SequenceStatement::Note(Note {
            position: NotePosition::RightOf,
            actor: "B".to_string(),
            text: "Right note".to_string(),
        }),
        SequenceStatement::Note(Note {
            position: NotePosition::Over,
            actor: "C".to_string(),
            text: "Over note".to_string(),
        }),
        SequenceStatement::Activate("A".to_string()),
        SequenceStatement::Deactivate("A".to_string()),
        SequenceStatement::Create(Participant {
            actor: "D".to_string(),
            participant_type: ParticipantType::Participant,
            alias: Some("NewParticipant".to_string()),
        }),
        SequenceStatement::Destroy("D".to_string()),
    ];

    let diagram = DiagramType::Sequence(SequenceDiagram {
        title: Some("Complex Sequence".to_string()),
        accessibility: AccessibilityInfo::default(),
        participants: vec![
            Participant {
                actor: "A".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
            Participant {
                actor: "B".to_string(),
                participant_type: ParticipantType::Actor,
                alias: None,
            },
            Participant {
                actor: "C".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
        ],
        statements,
        autonumber: Some(AutoNumber {
            start: Some(1),
            step: Some(1),
            visible: true,
        }),
    });

    let output = diagram.to_mermaid();

    // Verify all statement types are present
    assert!(output.contains("sequenceDiagram"));
    assert!(output.contains("title Complex Sequence"));
    assert!(output.contains("autonumber 1 1"));
    assert!(output.contains("participant A"));
    assert!(output.contains("actor B"));
    assert!(output.contains("A -> B: Hello"));
    assert!(output.contains("loop while active"));
    assert!(output.contains("note left of A: Left note"));
    assert!(output.contains("note right of B: Right note"));
    assert!(output.contains("note over C: Over note"));
    assert!(output.contains("activate A"));
    assert!(output.contains("deactivate A"));
}

// Test simple PacketDiagram formatting (with basic structure)
#[test]
fn test_packet_diagram_basic() {
    let diagram = DiagramType::Packet(PacketDiagram {
        title: Some("Network Packet".to_string()),
        accessibility: AccessibilityInfo::default(),
        fields: vec![],
    });

    let output = diagram.to_mermaid();

    assert!(output.contains("packet-beta"));
    assert!(output.contains("title Network Packet"));
}

// Test simple RequirementDiagram formatting (with basic structure)
#[test]
fn test_requirement_diagram_basic() {
    let diagram = DiagramType::Requirement(RequirementDiagram {
        title: Some("System Requirements".to_string()),
        accessibility: AccessibilityInfo::default(),
        requirements: HashMap::new(),
        relationships: vec![],
        elements: HashMap::new(),
    });

    let output = diagram.to_mermaid();

    assert!(output.contains("requirementDiagram"));
    assert!(output.contains("title System Requirements"));
}

// Test TreemapDiagram formatting (previously untested)
#[test]
fn test_treemap_diagram_formatting() {
    let root = TreemapNode {
        name: "Company".to_string(),
        value: None,
        children: vec![
            TreemapNode {
                name: "Engineering".to_string(),
                value: Some(100.0),
                children: vec![
                    TreemapNode {
                        name: "Frontend".to_string(),
                        value: Some(60.0),
                        children: vec![],
                    },
                    TreemapNode {
                        name: "Backend".to_string(),
                        value: Some(40.0),
                        children: vec![],
                    },
                ],
            },
            TreemapNode {
                name: "Sales".to_string(),
                value: Some(50.0),
                children: vec![],
            },
        ],
    };

    let diagram = DiagramType::Treemap(TreemapDiagram {
        title: Some("Organization Treemap".to_string()),
        accessibility: AccessibilityInfo::default(),
        root,
    });

    let output = diagram.to_mermaid();

    assert!(output.contains("treemap"));
    assert!(output.contains("title Organization Treemap"));
    assert!(output.contains("Company"));
    assert!(output.contains("Engineering"));
    assert!(output.contains("Frontend"));
    assert!(output.contains("Backend"));
    assert!(output.contains("Sales"));
}

// Test RadarDiagram formatting (previously untested)
#[test]
fn test_radar_diagram_formatting() {
    let datasets = vec![
        Dataset {
            name: "Product A".to_string(),
            values: vec![10.0, 20.0, 30.0, 40.0],
        },
        Dataset {
            name: "Product B".to_string(),
            values: vec![15.0, 25.0, 35.0, 45.0],
        },
    ];

    let diagram = DiagramType::Radar(RadarDiagram {
        title: Some("Product Comparison".to_string()),
        accessibility: AccessibilityInfo::default(),
        config: RadarConfig::default(),
        axes: vec![
            "Speed".to_string(),
            "Quality".to_string(),
            "Price".to_string(),
            "Features".to_string(),
        ],
        datasets,
    });

    let output = diagram.to_mermaid();

    assert!(output.contains("radar"));
    assert!(output.contains("title Product Comparison"));
    assert!(output.contains("Speed"));
    assert!(output.contains("Quality"));
    assert!(output.contains("Price"));
    assert!(output.contains("Features"));
    assert!(output.contains("Product A"));
    assert!(output.contains("Product B"));
    assert!(output.contains("[10, 20, 30, 40]"));
    assert!(output.contains("[15, 25, 35, 45]"));
}

// Test empty diagram variants
#[test]
fn test_empty_diagram_variants() {
    // Empty flowchart
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

    // Empty sequence diagram
    let empty_sequence = DiagramType::Sequence(SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![],
        statements: vec![],
        autonumber: None,
    });
    let output = empty_sequence.to_mermaid();
    assert!(output.contains("sequenceDiagram"));

    // Empty pie diagram
    let empty_pie = DiagramType::Pie(PieDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        show_data: false,
        data: vec![],
    });
    let output = empty_pie.to_mermaid();
    assert!(output.contains("pie"));
}

// Test accessibility and title formatting
#[test]
fn test_accessibility_and_title_formatting() {
    let accessibility = AccessibilityInfo {
        title: Some("Accessible Title".to_string()),
        description: Some("This is an accessible description".to_string()),
    };

    let diagram = DiagramType::Flowchart(FlowchartDiagram {
        title: Some("Main Title".to_string()),
        accessibility,
        direction: FlowDirection::LR,
        nodes: HashMap::new(),
        edges: vec![],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let output = diagram.to_mermaid();

    assert!(output.contains("flowchart LR"));
    assert!(output.contains("title Main Title"));
    assert!(output.contains("accTitle: Accessible Title"));
    assert!(output.contains("accDescr: This is an accessible description"));
}

// Test round-trip fidelity for complex cases
#[test]
fn test_round_trip_fidelity() {
    let input = "flowchart TD\n    A[Start] --> B{Decision}\n    B -->|Yes| C[End]\n    B -->|No| D[Loop]\n    D --> A";
    let diagram = parse_diagram(input).expect("Failed to parse");
    let output = diagram.to_mermaid();

    // Parse the output again
    let reparsed = parse_diagram(&output).expect("Failed to reparse");
    let reoutput = reparsed.to_mermaid();

    // Should be functionally equivalent (though formatting may differ)
    assert!(reoutput.contains("flowchart"));
    assert!(reoutput.contains("A[Start]"));
    assert!(reoutput.contains("B{Decision}"));
    assert!(reoutput.contains("C[End]"));
    assert!(reoutput.contains("D[Loop]"));
}
