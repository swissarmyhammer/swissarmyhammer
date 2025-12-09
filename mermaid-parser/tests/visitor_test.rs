//! Comprehensive tests for the visitor pattern module

use mermaid_parser::common::ast::*;
use mermaid_parser::common::visitor::*;
use std::collections::HashMap;

// Test the ReferenceValidator visitor
#[test]
fn test_reference_validator_valid_flowchart() {
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
            label: None,
            min_length: None,
        }],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let mut validator = ReferenceValidator::new();
    diagram.accept(&mut validator);

    assert!(validator.errors().is_empty());
}

#[test]
fn test_reference_validator_undefined_node() {
    let nodes = HashMap::new(); // No nodes defined

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

    let mut validator = ReferenceValidator::new();
    diagram.accept(&mut validator);

    assert_eq!(validator.errors().len(), 2); // Both A and B are undefined
}

#[test]
fn test_reference_validator_sequence_diagram() {
    let diagram = DiagramType::Sequence(SequenceDiagram {
        title: None,
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

    let mut validator = ReferenceValidator::new();
    diagram.accept(&mut validator);

    assert!(validator.errors().is_empty());
}

// Test the ComplexityAnalyzer visitor
#[test]
fn test_complexity_analyzer_flowchart() {
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
            text: Some("Decision".to_string()),
            shape: NodeShape::Rhombus,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "C".to_string(),
        FlowNode {
            id: "C".to_string(),
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
        edges: vec![
            FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            },
            FlowEdge {
                from: "B".to_string(),
                to: "C".to_string(),
                edge_type: EdgeType::Arrow,
                label: Some("Yes".to_string()),
                min_length: None,
            },
        ],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let mut analyzer = ComplexityAnalyzer::new();
    diagram.accept(&mut analyzer);

    let complexity = analyzer.cyclomatic_complexity();
    assert!(complexity > 0);
}

#[test]
fn test_complexity_analyzer_sequence_diagram() {
    let diagram = DiagramType::Sequence(SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![
            Participant {
                actor: "A".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
            Participant {
                actor: "B".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
        ],
        statements: vec![SequenceStatement::Loop(Loop {
            condition: "While true".to_string(),
            statements: vec![SequenceStatement::Message(Message {
                from: "A".to_string(),
                to: "B".to_string(),
                text: "Request".to_string(),
                arrow_type: ArrowType::SolidOpen,
            })],
        })],
        autonumber: None,
    });

    let mut analyzer = ComplexityAnalyzer::new();
    diagram.accept(&mut analyzer);

    let complexity = analyzer.cyclomatic_complexity();
    assert!(complexity > 1); // Loop adds complexity
}

// Test the TitleSetter visitor
#[test]
fn test_title_setter_flowchart() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let mut diagram = DiagramType::Flowchart(FlowchartDiagram {
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

    let mut setter = TitleSetter::new("Test Flowchart".to_string());
    diagram.accept_mut(&mut setter);

    if let DiagramType::Flowchart(fc) = &diagram {
        assert_eq!(fc.title, Some("Test Flowchart".to_string()));
    }
}

#[test]
fn test_title_setter_sequence_diagram() {
    let mut diagram = DiagramType::Sequence(SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![],
        statements: vec![],
        autonumber: None,
    });

    let mut setter = TitleSetter::new("Test Sequence".to_string());
    diagram.accept_mut(&mut setter);

    if let DiagramType::Sequence(seq) = &diagram {
        assert_eq!(seq.title, Some("Test Sequence".to_string()));
    }
}

#[test]
fn test_title_setter_timeline() {
    let mut diagram = DiagramType::Timeline(TimelineDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        sections: vec![],
    });

    let mut setter = TitleSetter::new("Test Timeline".to_string());
    diagram.accept_mut(&mut setter);

    if let DiagramType::Timeline(timeline) = &diagram {
        assert_eq!(timeline.title, Some("Test Timeline".to_string()));
    }
}

// Test the NodeCounter visitor
#[test]
fn test_node_counter_flowchart() {
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
    nodes.insert(
        "C".to_string(),
        FlowNode {
            id: "C".to_string(),
            text: Some("Node C".to_string()),
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
        edges: vec![
            FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            },
            FlowEdge {
                from: "B".to_string(),
                to: "C".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            },
        ],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    });

    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);

    assert_eq!(counter.nodes(), 3);
    assert_eq!(counter.edges(), 2);
}

#[test]
fn test_node_counter_sequence_diagram() {
    let diagram = DiagramType::Sequence(SequenceDiagram {
        title: None,
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
            Participant {
                actor: "Charlie".to_string(),
                participant_type: ParticipantType::Participant,
                alias: None,
            },
        ],
        statements: vec![
            SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Bob".to_string(),
                text: "Hello Bob".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
            SequenceStatement::Message(Message {
                from: "Bob".to_string(),
                to: "Charlie".to_string(),
                text: "Hello Charlie".to_string(),
                arrow_type: ArrowType::SolidClosed,
            }),
        ],
        autonumber: None,
    });

    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);

    assert_eq!(counter.nodes(), 3); // participants
    assert_eq!(counter.elements(), 2); // statements
}

#[test]
fn test_node_counter_empty_diagrams() {
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

    let mut counter = NodeCounter::new();
    empty_flowchart.accept(&mut counter);

    assert_eq!(counter.nodes(), 0);
    assert_eq!(counter.edges(), 0);

    // Test empty sequence diagram
    let empty_sequence = DiagramType::Sequence(SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![],
        statements: vec![],
        autonumber: None,
    });

    let mut counter2 = NodeCounter::new();
    empty_sequence.accept(&mut counter2);

    assert_eq!(counter2.nodes(), 0);
    assert_eq!(counter2.edges(), 0);
}

#[test]
fn test_complex_nested_structure() {
    // Test a complex mindmap with nested structure
    let diagram = DiagramType::Mindmap(MindmapDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        root: MindmapNode {
            id: "root".to_string(),
            text: "Central Topic".to_string(),
            shape: MindmapNodeShape::Cloud,
            icon: None,
            class: None,
            children: vec![
                MindmapNode {
                    id: "branch1".to_string(),
                    text: "Branch 1".to_string(),
                    shape: MindmapNodeShape::Square,
                    icon: None,
                    class: None,
                    children: vec![MindmapNode {
                        id: "leaf1".to_string(),
                        text: "Leaf 1".to_string(),
                        shape: MindmapNodeShape::Default,
                        icon: None,
                        class: None,
                        children: vec![],
                    }],
                },
                MindmapNode {
                    id: "branch2".to_string(),
                    text: "Branch 2".to_string(),
                    shape: MindmapNodeShape::Rounded,
                    icon: None,
                    class: None,
                    children: vec![],
                },
            ],
        },
    });

    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);

    assert_eq!(counter.nodes(), 4); // root + 2 branches + 1 leaf

    let mut analyzer = ComplexityAnalyzer::new();
    diagram.accept(&mut analyzer);

    let complexity = analyzer.cyclomatic_complexity();
    assert!(complexity > 0); // Nested structure should have complexity
}
