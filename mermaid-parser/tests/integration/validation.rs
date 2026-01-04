use mermaid_parser::common::ast::*;
use mermaid_parser::common::validation::*;
use std::collections::HashMap;

#[test]
fn test_flowchart_validation_comprehensive() {
    // Create a flowchart with multiple validation issues
    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Node A".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec!["undefined-class".to_string()],
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
            text: Some("Isolated Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let diagram = FlowchartDiagram {
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
                to: "UNDEFINED".to_string(), // Undefined node
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            },
        ],
        subgraphs: vec![
            Subgraph {
                id: "sub1".to_string(),
                title: Some("Duplicate".to_string()),
                nodes: vec![],
                edges: vec![],
                subgraphs: vec![],
                direction: None,
            },
            Subgraph {
                id: "sub2".to_string(),
                title: Some("Duplicate".to_string()), // Duplicate name
                nodes: vec![],
                edges: vec![],
                subgraphs: vec![],
                direction: None,
            },
        ],
        styles: vec![],
        class_defs: HashMap::new(), // No class definitions
        clicks: vec![],
    };

    let validator = FlowchartValidator::new();
    let result = validator.validate(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();

    // Should have multiple errors
    assert!(errors.len() >= 3);

    // Check for specific error types
    let error_rules: Vec<_> = errors.iter().map(|e| e.rule).collect();
    assert!(error_rules.contains(&"undefined_node_reference"));
    assert!(error_rules.contains(&"duplicate_subgraph_name"));
    assert!(error_rules.contains(&"undefined_style_class"));
    assert!(error_rules.contains(&"isolated_node"));

    // Test error severities
    assert!(errors.iter().any(|e| e.severity == Severity::Error));
    assert!(errors.iter().any(|e| e.severity == Severity::Warning));
}

#[test]
fn test_sequence_validation_comprehensive() {
    let diagram = SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![
            Participant {
                actor: "Alice".to_string(),
                alias: None,
                participant_type: ParticipantType::Actor,
            },
            Participant {
                actor: "Bob".to_string(),
                alias: None,
                participant_type: ParticipantType::Participant,
            },
        ],
        statements: vec![
            SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Charlie".to_string(), // Undefined participant
                text: "Hello".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
            SequenceStatement::Activate("Bob".to_string()),
            SequenceStatement::Message(Message {
                from: "Bob".to_string(),
                to: "Alice".to_string(),
                text: "Response".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
            // Missing deactivate for Bob - should cause unbalanced activation error
        ],
        autonumber: None,
    };

    let validator = SequenceValidator::new();
    let result = validator.validate(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();

    // Should have errors for undefined participant and unbalanced activation
    let error_rules: Vec<_> = errors.iter().map(|e| e.rule).collect();
    assert!(error_rules.contains(&"undefined_participant"));
    assert!(error_rules.contains(&"unbalanced_activation"));
}

#[test]
fn test_class_validation_comprehensive() {
    let mut classes = HashMap::new();
    classes.insert(
        "A".to_string(),
        Class {
            name: "A".to_string(),
            stereotype: None,
            members: vec![
                ClassMember::Property(Property {
                    name: "field1".to_string(),
                    prop_type: Some("String".to_string()),
                    visibility: Visibility::Public,
                    is_static: false,
                    default_value: None,
                }),
                ClassMember::Property(Property {
                    name: "field1".to_string(), // Duplicate member
                    prop_type: Some("String".to_string()),
                    visibility: Visibility::Private,
                    is_static: false,
                    default_value: None,
                }),
            ],
            annotations: vec![],
            css_class: None,
        },
    );
    classes.insert(
        "B".to_string(),
        Class {
            name: "B".to_string(),
            stereotype: None,
            members: vec![],
            annotations: vec![],
            css_class: None,
        },
    );
    classes.insert(
        "C".to_string(),
        Class {
            name: "C".to_string(),
            stereotype: None,
            members: vec![],
            annotations: vec![],
            css_class: None,
        },
    );

    let diagram = ClassDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        classes,
        relationships: vec![
            ClassRelationship {
                from: "A".to_string(),
                to: "B".to_string(),
                relationship_type: ClassRelationshipType::Inheritance,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            },
            ClassRelationship {
                from: "B".to_string(),
                to: "C".to_string(),
                relationship_type: ClassRelationshipType::Inheritance,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            },
            ClassRelationship {
                from: "C".to_string(),
                to: "A".to_string(), // Creates circular inheritance
                relationship_type: ClassRelationshipType::Inheritance,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            },
            ClassRelationship {
                from: "A".to_string(),
                to: "UNDEFINED".to_string(), // Undefined class
                relationship_type: ClassRelationshipType::Association,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            },
        ],
        notes: vec![],
    };

    let validator = ClassValidator::new();
    let result = validator.validate(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();

    let error_rules: Vec<_> = errors.iter().map(|e| e.rule).collect();
    assert!(error_rules.contains(&"circular_inheritance"));
    assert!(error_rules.contains(&"undefined_class_reference"));
    assert!(error_rules.contains(&"duplicate_member"));
}

#[test]
fn test_state_validation_comprehensive() {
    let mut states = HashMap::new();
    states.insert(
        "A".to_string(),
        State {
            id: "A".to_string(),
            display_name: Some("State A".to_string()),
            state_type: StateType::Start,
            substates: vec![],
            concurrent_regions: vec![],
        },
    );
    states.insert(
        "B".to_string(),
        State {
            id: "B".to_string(),
            display_name: Some("State B".to_string()),
            state_type: StateType::End,
            substates: vec![],
            concurrent_regions: vec![],
        },
    );
    states.insert(
        "C".to_string(),
        State {
            id: "C".to_string(),
            display_name: Some("Unreachable State".to_string()),
            state_type: StateType::Simple,
            substates: vec![],
            concurrent_regions: vec![],
        },
    );

    let diagram = StateDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        version: StateVersion::V2,
        states,
        transitions: vec![
            StateTransition {
                from: "A".to_string(),
                to: "B".to_string(),
                event: Some("event1".to_string()),
                guard: None,
                action: None,
            },
            StateTransition {
                from: "B".to_string(),
                to: "UNDEFINED".to_string(), // Undefined state
                event: Some("event2".to_string()),
                guard: None,
                action: None,
            },
            StateTransition {
                from: "B".to_string(), // End state with outgoing transition
                to: "A".to_string(),
                event: Some("event3".to_string()),
                guard: None,
                action: None,
            },
            // State C is unreachable
        ],
        notes: vec![],
    };

    let validator = StateValidator::new();
    let result = validator.validate(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();

    let error_rules: Vec<_> = errors.iter().map(|e| e.rule).collect();
    assert!(error_rules.contains(&"undefined_state_reference"));
    assert!(error_rules.contains(&"unreachable_state"));
    assert!(error_rules.contains(&"end_state_with_outgoing_transition"));
}

#[test]
fn test_universal_validator() {
    // Test with a flowchart that has validation issues
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

    let flowchart = FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges: vec![FlowEdge {
            from: "A".to_string(),
            to: "UNDEFINED".to_string(), // Error: undefined node reference
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        }],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    };

    let diagram = DiagramType::Flowchart(flowchart);
    let validator = UniversalValidator::new();
    let result = validator.validate_any(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.rule == "undefined_node_reference"));
}

#[test]
fn test_validation_config() {
    // Test with custom configuration that ignores certain rules
    let mut config = ValidationConfig {
        min_severity: Severity::Error, // Only show errors, not warnings
        ..Default::default()
    };
    config.ignore_rules.insert("isolated_node"); // Ignore isolated node warnings

    let mut nodes = HashMap::new();
    nodes.insert(
        "A".to_string(),
        FlowNode {
            id: "A".to_string(),
            text: Some("Connected Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "B".to_string(),
        FlowNode {
            id: "B".to_string(),
            text: Some("Connected Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );
    nodes.insert(
        "C".to_string(),
        FlowNode {
            id: "C".to_string(),
            text: Some("Isolated Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        },
    );

    let diagram = FlowchartDiagram {
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
                to: "UNDEFINED".to_string(), // Error: undefined reference
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            },
        ],
        subgraphs: vec![],
        styles: vec![],
        class_defs: HashMap::new(),
        clicks: vec![],
    };

    let validator = FlowchartValidator::with_config(config);
    let result = validator.validate(&diagram);

    assert!(result.is_err());
    let errors = result.unwrap_err();

    // Should only have the error (undefined reference), not the warning (isolated node)
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].rule, "undefined_node_reference");
    assert_eq!(errors[0].severity, Severity::Error);
}

#[test]
fn test_valid_diagrams_pass_validation() {
    // Test that valid diagrams pass validation

    // Valid flowchart
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

    let flowchart = FlowchartDiagram {
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
    };

    let validator = FlowchartValidator::new();
    let result = validator.validate(&flowchart);
    assert!(result.is_ok());

    // Valid sequence diagram
    let sequence = SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![
            Participant {
                actor: "Alice".to_string(),
                alias: None,
                participant_type: ParticipantType::Actor,
            },
            Participant {
                actor: "Bob".to_string(),
                alias: None,
                participant_type: ParticipantType::Participant,
            },
        ],
        statements: vec![
            SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Bob".to_string(),
                text: "Hello".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
            SequenceStatement::Message(Message {
                from: "Bob".to_string(),
                to: "Alice".to_string(),
                text: "Hi".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
        ],
        autonumber: None,
    };

    let seq_validator = SequenceValidator::new();
    let result = seq_validator.validate(&sequence);
    assert!(result.is_ok());
}

#[test]
fn test_validation_error_display() {
    let location = Location::with_element(10, 5, "node_id".to_string());
    let error = ValidationError::with_location(
        "test_rule",
        "Test error message".to_string(),
        Severity::Error,
        location,
    );

    let display_str = format!("{}", error);
    assert!(display_str.contains("ERROR"));
    assert!(display_str.contains("test_rule"));
    assert!(display_str.contains("Test error message"));
    assert!(display_str.contains("10:5"));
    assert!(display_str.contains("node_id"));
}
