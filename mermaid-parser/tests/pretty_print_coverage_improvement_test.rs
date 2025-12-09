//! Comprehensive tests targeting missing coverage areas in pretty_print.rs

use mermaid_parser::common::ast::*;
use mermaid_parser::common::pretty_print::{MermaidPrinter, PrintOptions};
use std::collections::HashMap;

#[cfg(test)]
mod pretty_print_coverage_tests {
    use super::*;

    // Test PrintOptions with all variations
    #[test]
    fn test_print_options_variations() {
        let simple_flowchart = DiagramType::Flowchart(FlowchartDiagram {
            title: Some("Test".to_string()),
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes: {
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
                        shape: NodeShape::Circle,
                        classes: vec![],
                        icon: None,
                    },
                );
                nodes
            },
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: EdgeType::Arrow,
                label: Some("test".to_string()),
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        // Test compact mode
        let compact_options = PrintOptions {
            compact_mode: true,
            indent_width: 2,
            max_line_length: 80,
            align_arrows: false,
            sort_nodes: false,
        };
        let compact_output = simple_flowchart.to_mermaid_pretty(&compact_options);
        assert!(compact_output.contains("flowchart TD"));
        // In compact mode, no indentation should be present
        for line in compact_output.lines() {
            if !line.trim().is_empty() {
                assert!(!line.starts_with("    "));
            }
        }

        // Test align arrows mode
        let align_options = PrintOptions {
            compact_mode: false,
            indent_width: 4,
            max_line_length: 120,
            align_arrows: true,
            sort_nodes: false,
        };
        let aligned_output = simple_flowchart.to_mermaid_pretty(&align_options);
        assert!(aligned_output.contains("flowchart TD"));

        // Test sort nodes mode
        let sort_options = PrintOptions {
            compact_mode: false,
            indent_width: 2,
            max_line_length: 100,
            align_arrows: false,
            sort_nodes: true,
        };
        let sorted_output = simple_flowchart.to_mermaid_pretty(&sort_options);
        assert!(sorted_output.contains("flowchart TD"));

        // Test combination: compact + align + sort
        let combo_options = PrintOptions {
            compact_mode: true,
            indent_width: 8,
            max_line_length: 60,
            align_arrows: true,
            sort_nodes: true,
        };
        let combo_output = simple_flowchart.to_mermaid_pretty(&combo_options);
        assert!(combo_output.contains("flowchart TD"));
    }

    // Test PrettyPrinter write() method that's marked as dead code
    #[test]
    fn test_pretty_printer_write_method_indirect() {
        // Since the write() method is marked as dead code, we test it indirectly
        // by using diagrams that might trigger internal PrettyPrinter usage

        // Test with complex nested mindmap that uses various formatting
        let mindmap = DiagramType::Mindmap(MindmapDiagram {
            title: Some("Complex Mindmap".to_string()),
            accessibility: AccessibilityInfo::default(),
            root: MindmapNode {
                id: "root".to_string(),
                text: "Root Node".to_string(),
                shape: MindmapNodeShape::Cloud,
                icon: Some("🌟".to_string()),
                class: Some("root-class".to_string()),
                children: vec![
                    MindmapNode {
                        id: "child1".to_string(),
                        text: "".to_string(), // Empty text node with icon
                        shape: MindmapNodeShape::Square,
                        icon: Some("📁".to_string()),
                        class: None,
                        children: vec![MindmapNode {
                            id: "grandchild".to_string(),
                            text: "Deep Node".to_string(),
                            shape: MindmapNodeShape::Default,
                            icon: None,
                            class: None,
                            children: vec![],
                        }],
                    },
                    MindmapNode {
                        id: "child2".to_string(),
                        text: "Regular Child".to_string(),
                        shape: MindmapNodeShape::Hexagon,
                        icon: None,
                        class: Some("special".to_string()),
                        children: vec![],
                    },
                ],
            },
        });

        let output = mindmap.to_mermaid();
        assert!(output.contains("mindmap"));
        assert!(output.contains("root(-Root Node-)"));
        assert!(output.contains("::icon(🌟)"));
        assert!(output.contains("::::root-class"));
        assert!(output.contains("::icon(📁)"));
    }

    // Test edge cases in flowchart formatting
    #[test]
    fn test_flowchart_edge_cases() {
        // Test flowchart with all different node shapes
        let mut nodes = HashMap::new();
        let shapes = vec![
            ("rect", NodeShape::Rectangle),
            ("round", NodeShape::RoundedRectangle),
            ("stadium", NodeShape::Stadium),
            ("subroutine", NodeShape::Subroutine),
            ("cylinder", NodeShape::Cylinder),
            ("circle", NodeShape::Circle),
            ("asymmetric", NodeShape::Asymmetric),
            ("rhombus", NodeShape::Rhombus),
            ("hexagon", NodeShape::Hexagon),
            ("parallelogram", NodeShape::Parallelogram),
            ("parallelogram_alt", NodeShape::ParallelogramAlt),
            ("trapezoid", NodeShape::Trapezoid),
            ("trapezoid_alt", NodeShape::TrapezoidAlt),
            ("double_circle", NodeShape::DoubleCircle),
        ];

        let shapes_clone = shapes.clone();
        for (id, shape) in shapes {
            nodes.insert(
                id.to_string(),
                FlowNode {
                    id: id.to_string(),
                    text: Some(format!("Text {}", id)),
                    shape,
                    classes: vec![],
                    icon: None,
                },
            );
        }

        // Test all edge types
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

        let mut edges = vec![];
        for (i, edge_type) in edge_types.iter().enumerate() {
            if i < shapes_clone.len() - 1 {
                edges.push(FlowEdge {
                    from: shapes_clone[i].0.to_string(),
                    to: shapes_clone[i + 1].0.to_string(),
                    edge_type: edge_type.clone(),
                    label: Some(format!("Label {}", i)),
                    min_length: None,
                });
            }
        }

        let flowchart = DiagramType::Flowchart(FlowchartDiagram {
            title: Some("Shape Test".to_string()),
            accessibility: AccessibilityInfo {
                title: Some("Accessibility Title".to_string()),
                description: Some("Accessibility Description".to_string()),
            },
            direction: FlowDirection::LR,
            nodes,
            edges,
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        let output = flowchart.to_mermaid();
        assert!(output.contains("flowchart LR"));
        assert!(output.contains("accTitle: Accessibility Title"));
        assert!(output.contains("accDescr: Accessibility Description"));

        // Check that all shapes are properly formatted
        assert!(output.contains("rect[Text rect]"));
        assert!(output.contains("round(Text round)"));
        assert!(output.contains("stadium([Text stadium])"));
        assert!(output.contains("subroutine[[Text subroutine]]"));
        assert!(output.contains("cylinder[(Text cylinder)]"));
        assert!(output.contains("circle((Text circle)))"));
        assert!(output.contains("asymmetric>Text asymmetric]"));
        assert!(output.contains("rhombus{Text rhombus}"));
        assert!(output.contains("hexagon{{Text hexagon}}"));
        assert!(output.contains("parallelogram[/Text parallelogram\\]"));
        assert!(output.contains("parallelogram_alt[\\Text parallelogram_alt/]"));
        assert!(output.contains("trapezoid[/Text trapezoid/]"));
        assert!(output.contains("trapezoid_alt[\\Text trapezoid_alt\\]"));
        assert!(output.contains("double_circle(((Text double_circle)))"));
    }

    // Test complex flowchart with subgraphs and styles
    #[test]
    fn test_flowchart_complex_features() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Node A".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec!["highlight".to_string()],
                icon: None,
            },
        );
        nodes.insert(
            "B".to_string(),
            FlowNode {
                id: "B".to_string(),
                text: Some("Node B".to_string()),
                shape: NodeShape::Circle,
                classes: vec![],
                icon: None,
            },
        );
        nodes.insert(
            "C".to_string(),
            FlowNode {
                id: "C".to_string(),
                text: Some("Node C".to_string()),
                shape: NodeShape::Rhombus,
                classes: vec![],
                icon: None,
            },
        );

        let subgraph = Subgraph {
            id: "sub1".to_string(),
            title: Some("Subprocess".to_string()),
            direction: Some(FlowDirection::TB),
            nodes: vec!["B".to_string(), "C".to_string()],
            edges: vec![FlowEdge {
                from: "B".to_string(),
                to: "C".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![Subgraph {
                id: "nested".to_string(),
                title: None,
                direction: None,
                nodes: vec!["C".to_string()],
                edges: vec![],
                subgraphs: vec![],
            }],
        };

        let mut class_defs = HashMap::new();
        let mut highlight_styles = HashMap::new();
        highlight_styles.insert("fill".to_string(), "#ff0000".to_string());
        class_defs.insert(
            "highlight".to_string(),
            ClassDef {
                name: "highlight".to_string(),
                styles: highlight_styles,
            },
        );

        let clicks = vec![
            ClickEvent {
                node_id: "A".to_string(),
                action: ClickAction::Href(
                    "https://example.com".to_string(),
                    Some("_blank".to_string()),
                ),
            },
            ClickEvent {
                node_id: "B".to_string(),
                action: ClickAction::Callback("myFunction".to_string()),
            },
            ClickEvent {
                node_id: "C".to_string(),
                action: ClickAction::Both(
                    "callback".to_string(),
                    "https://test.com".to_string(),
                    None,
                ),
            },
        ];

        let mut node_styles = HashMap::new();
        node_styles.insert("stroke".to_string(), "#0000ff".to_string());
        let styles = vec![StyleDefinition {
            target: StyleTarget::Node("A".to_string()),
            styles: node_styles,
        }];

        let flowchart = DiagramType::Flowchart(FlowchartDiagram {
            title: None, // Test without title
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::BT,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "sub1".to_string(),
                edge_type: EdgeType::ThickArrow,
                label: Some("to subgraph".to_string()),
                min_length: Some(3),
            }],
            subgraphs: vec![subgraph],
            styles,
            class_defs,
            clicks,
        });

        let output = flowchart.to_mermaid();
        assert!(output.contains("flowchart BT"));
        assert!(output.contains("subgraph sub1 [Subprocess]"));
        assert!(output.contains("direction TB"));
        assert!(output.contains("subgraph nested"));
        assert!(output.contains("end"));
        assert!(output.contains("style A stroke:#0000ff"));
        assert!(output.contains("classDef highlight fill:#ff0000"));
        assert!(output.contains("click A \"https://example.com\" \"_blank\""));
        assert!(output.contains("click B call myFunction"));
        assert!(output.contains("click C call callback \"https://test.com\" \"_self\""));
    }

    // Test sequence diagram complex features
    #[test]
    fn test_sequence_diagram_comprehensive() {
        let participants = vec![
            Participant {
                actor: "Alice".to_string(),
                alias: Some("A".to_string()),
                participant_type: ParticipantType::Actor,
            },
            Participant {
                actor: "Bob".to_string(),
                alias: None,
                participant_type: ParticipantType::Participant,
            },
        ];

        let statements = vec![
            SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Bob".to_string(),
                text: "Hello".to_string(),
                arrow_type: ArrowType::SolidOpen,
            }),
            SequenceStatement::Note(Note {
                position: NotePosition::RightOf,
                actor: "Bob".to_string(),
                text: "Thinking...".to_string(),
            }),
            SequenceStatement::Loop(Loop {
                condition: "x < 10".to_string(),
                statements: vec![SequenceStatement::Message(Message {
                    from: "Bob".to_string(),
                    to: "Alice".to_string(),
                    text: "Counter".to_string(),
                    arrow_type: ArrowType::DottedClosed,
                })],
            }),
            SequenceStatement::Alt(Alternative {
                condition: "success".to_string(),
                statements: vec![SequenceStatement::Message(Message {
                    from: "Alice".to_string(),
                    to: "Bob".to_string(),
                    text: "OK".to_string(),
                    arrow_type: ArrowType::SolidClosed,
                })],
                else_branch: Some(ElseBranch {
                    condition: Some("failure".to_string()),
                    statements: vec![SequenceStatement::Message(Message {
                        from: "Alice".to_string(),
                        to: "Bob".to_string(),
                        text: "Error".to_string(),
                        arrow_type: ArrowType::Cross,
                    })],
                }),
            }),
            SequenceStatement::Opt(Optional {
                condition: "optional".to_string(),
                statements: vec![
                    SequenceStatement::Activate("Bob".to_string()),
                    SequenceStatement::Message(Message {
                        from: "Bob".to_string(),
                        to: "Alice".to_string(),
                        text: "Processing".to_string(),
                        arrow_type: ArrowType::Point,
                    }),
                    SequenceStatement::Deactivate("Bob".to_string()),
                ],
            }),
            SequenceStatement::Par(Parallel {
                branches: vec![
                    ParallelBranch {
                        condition: Some("branch1".to_string()),
                        statements: vec![SequenceStatement::Message(Message {
                            from: "Alice".to_string(),
                            to: "Bob".to_string(),
                            text: "Parallel 1".to_string(),
                            arrow_type: ArrowType::BiDirectionalSolid,
                        })],
                    },
                    ParallelBranch {
                        condition: None,
                        statements: vec![SequenceStatement::Message(Message {
                            from: "Bob".to_string(),
                            to: "Alice".to_string(),
                            text: "Parallel 2".to_string(),
                            arrow_type: ArrowType::BiDirectionalDotted,
                        })],
                    },
                ],
            }),
            SequenceStatement::Critical(Critical {
                condition: "critical section".to_string(),
                statements: vec![SequenceStatement::Create(Participant {
                    actor: "System".to_string(),
                    alias: None,
                    participant_type: ParticipantType::Participant,
                })],
                options: vec![CriticalOption {
                    condition: "option1".to_string(),
                    statements: vec![SequenceStatement::Destroy("System".to_string())],
                }],
            }),
        ];

        let sequence = DiagramType::Sequence(SequenceDiagram {
            title: Some("Complex Sequence".to_string()),
            accessibility: AccessibilityInfo {
                title: Some("Sequence Access Title".to_string()),
                description: Some("Sequence Description".to_string()),
            },
            participants,
            statements,
            autonumber: Some(AutoNumber {
                visible: true,
                start: Some(5),
                step: Some(2),
            }),
        });

        let output = sequence.to_mermaid();
        assert!(output.contains("sequenceDiagram"));
        assert!(output.contains("title Complex Sequence"));
        assert!(output.contains("accTitle: Sequence Access Title"));
        assert!(output.contains("accDescr: Sequence Description"));
        assert!(output.contains("autonumber 5 2"));
        assert!(output.contains("actor Alice as A"));
        assert!(output.contains("participant Bob"));
        assert!(output.contains("Alice -> Bob: Hello"));
        assert!(output.contains("note right of Bob: Thinking..."));
        assert!(output.contains("loop x < 10"));
        assert!(output.contains("Bob -->> Alice: Counter"));
        assert!(output.contains("alt success"));
        assert!(output.contains("Alice ->> Bob: OK"));
        assert!(output.contains("else failure"));
        assert!(output.contains("Alice -x Bob: Error"));
        assert!(output.contains("opt optional"));
        assert!(output.contains("activate Bob"));
        assert!(output.contains("Bob -) Alice: Processing"));
        assert!(output.contains("deactivate Bob"));
        assert!(output.contains("par branch1"));
        assert!(output.contains("Alice <-> Bob: Parallel 1"));
        assert!(output.contains("and"));
        assert!(output.contains("Bob <--> Alice: Parallel 2"));
        assert!(output.contains("critical critical section"));
        assert!(output.contains("create participant System"));
        assert!(output.contains("option option1"));
        assert!(output.contains("destroy System"));
        assert!(output.contains("end"));
    }

    // Test class diagram with all features
    #[test]
    fn test_class_diagram_comprehensive() {
        let mut classes = HashMap::new();
        classes.insert(
            "Animal".to_string(),
            Class {
                name: "Animal".to_string(),
                stereotype: Some(Stereotype::Abstract),
                members: vec![
                    ClassMember::Property(Property {
                        name: "name".to_string(),
                        prop_type: Some("String".to_string()),
                        visibility: Visibility::Protected,
                        is_static: false,
                        default_value: Some("'Unknown'".to_string()),
                    }),
                    ClassMember::Property(Property {
                        name: "count".to_string(),
                        prop_type: None, // Test without type
                        visibility: Visibility::Private,
                        is_static: true,
                        default_value: None,
                    }),
                    ClassMember::Method(Method {
                        visibility: Visibility::Public,
                        name: "move".to_string(),
                        parameters: vec![
                            Parameter {
                                name: "distance".to_string(),
                                param_type: Some("int".to_string()),
                            },
                            Parameter {
                                name: "direction".to_string(),
                                param_type: None, // Test parameter without type
                            },
                        ],
                        return_type: Some("void".to_string()),
                        is_static: false,
                        is_abstract: true,
                    }),
                    ClassMember::Method(Method {
                        visibility: Visibility::Package,
                        name: "staticMethod".to_string(),
                        parameters: vec![],
                        return_type: None, // Test method without return type
                        is_static: true,
                        is_abstract: false,
                    }),
                ],
                annotations: vec!["@Entity".to_string(), "@Serializable".to_string()],
                css_class: Some("highlight".to_string()),
            },
        );

        classes.insert(
            "Dog".to_string(),
            Class {
                name: "Dog".to_string(),
                stereotype: Some(Stereotype::Custom("pet".to_string())),
                members: vec![],
                annotations: vec![],
                css_class: None,
            },
        );

        let relationships = vec![
            ClassRelationship {
                from: "Dog".to_string(),
                to: "Animal".to_string(),
                relationship_type: ClassRelationshipType::Inheritance,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            },
            ClassRelationship {
                from: "Owner".to_string(),
                to: "Dog".to_string(),
                relationship_type: ClassRelationshipType::Composition,
                from_cardinality: Some("1".to_string()),
                to_cardinality: Some("*".to_string()),
                label: Some("owns".to_string()),
            },
        ];

        let notes = vec![Note {
            position: NotePosition::Over,
            actor: "Animal".to_string(),
            text: "Base class for all animals".to_string(),
        }];

        let class_diagram = DiagramType::Class(ClassDiagram {
            title: Some("Animal Hierarchy".to_string()),
            accessibility: AccessibilityInfo::default(),
            classes,
            relationships,
            notes,
        });

        let output = class_diagram.to_mermaid();
        assert!(output.contains("classDiagram"));
        assert!(output.contains("title Animal Hierarchy"));
        assert!(output.contains("class Animal {"));
        assert!(output.contains("<<abstract>>"));
        assert!(output.contains("#String name = 'Unknown'"));
        assert!(output.contains("-$count"));
        assert!(output.contains("+*move(distance: int, direction) void"));
        assert!(output.contains("~$staticMethod()"));
        assert!(output.contains("class Dog {"));
        assert!(output.contains("<<pet>>"));
        assert!(output.contains("Dog <|-- Animal"));
        assert!(output.contains("Owner \"1\" *-- \"*\" Dog : owns"));
        assert!(output.contains("note \"Base class for all animals\""));
        assert!(output.contains("class Animal highlight"));
    }

    // Test state diagram with all state types
    #[test]
    fn test_state_diagram_comprehensive() {
        let mut states = HashMap::new();
        states.insert(
            "start".to_string(),
            State {
                id: "start".to_string(),
                display_name: None,
                state_type: StateType::Start,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "end".to_string(),
            State {
                id: "end".to_string(),
                display_name: None,
                state_type: StateType::End,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "choice".to_string(),
            State {
                id: "choice".to_string(),
                display_name: Some("Decision Point".to_string()),
                state_type: StateType::Choice,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "fork".to_string(),
            State {
                id: "fork".to_string(),
                display_name: None,
                state_type: StateType::Fork,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "join".to_string(),
            State {
                id: "join".to_string(),
                display_name: None,
                state_type: StateType::Join,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "composite".to_string(),
            State {
                id: "composite".to_string(),
                display_name: Some("Composite State".to_string()),
                state_type: StateType::Composite,
                substates: vec!["sub1".to_string(), "sub2".to_string()],
                concurrent_regions: vec![
                    vec!["region1_state1".to_string(), "region1_state2".to_string()],
                    vec!["region2_state1".to_string()],
                ],
            },
        );

        let transitions = vec![
            StateTransition {
                from: "start".to_string(),
                to: "choice".to_string(),
                event: None,
                guard: None,
                action: None,
            },
            StateTransition {
                from: "choice".to_string(),
                to: "composite".to_string(),
                event: Some("event1".to_string()),
                guard: Some("condition".to_string()),
                action: Some("doAction()".to_string()),
            },
        ];

        let notes = vec![
            StateNote {
                position: StateNotePosition::LeftOf,
                target: "choice".to_string(),
                text: "Decision point".to_string(),
            },
            StateNote {
                position: StateNotePosition::Above,
                target: "composite".to_string(),
                text: "Complex state".to_string(),
            },
            StateNote {
                position: StateNotePosition::Below,
                target: "end".to_string(),
                text: "Final state".to_string(),
            },
        ];

        // Test both V1 and V2
        let state_v1 = DiagramType::State(StateDiagram {
            title: Some("State Machine V1".to_string()),
            accessibility: AccessibilityInfo::default(),
            version: StateVersion::V1,
            states: states.clone(),
            transitions: transitions.clone(),
            notes: notes.clone(),
        });

        let state_v2 = DiagramType::State(StateDiagram {
            title: Some("State Machine V2".to_string()),
            accessibility: AccessibilityInfo::default(),
            version: StateVersion::V2,
            states,
            transitions,
            notes,
        });

        let output_v1 = state_v1.to_mermaid();
        assert!(output_v1.contains("stateDiagram"));
        assert!(!output_v1.contains("stateDiagram-v2"));
        assert!(output_v1.contains("[*] --> start"));
        assert!(output_v1.contains("end --> [*]"));
        assert!(output_v1.contains("state choice <<choice>>"));
        assert!(output_v1.contains("state fork <<fork>>"));
        assert!(output_v1.contains("state join <<join>>"));
        assert!(output_v1.contains("state composite {"));
        assert!(output_v1.contains("sub1"));
        assert!(output_v1.contains("sub2"));
        assert!(output_v1.contains("--"));
        assert!(output_v1.contains("region1_state1"));
        assert!(output_v1.contains("region2_state1"));
        assert!(output_v1.contains("choice --> composite : event1 [condition] / doAction()"));
        assert!(output_v1.contains("note left of choice : Decision point"));
        assert!(output_v1.contains("note above composite : Complex state"));
        assert!(output_v1.contains("note below end : Final state"));

        let output_v2 = state_v2.to_mermaid();
        assert!(output_v2.contains("stateDiagram-v2"));
    }

    // Test ER diagram comprehensive features
    #[test]
    fn test_er_diagram_comprehensive() {
        let mut entities = HashMap::new();
        entities.insert(
            "Customer".to_string(),
            Entity {
                name: "Customer".to_string(),
                attributes: vec![
                    Attribute {
                        name: "id".to_string(),
                        attr_type: "int".to_string(),
                        key_type: Some(KeyType::PK),
                        comment: Some("Primary key".to_string()),
                    },
                    Attribute {
                        name: "email".to_string(),
                        attr_type: "varchar(255)".to_string(),
                        key_type: Some(KeyType::UK),
                        comment: None, // Test without comment
                    },
                    Attribute {
                        name: "foreign_key".to_string(),
                        attr_type: "int".to_string(),
                        key_type: Some(KeyType::FK),
                        comment: Some("".to_string()), // Test empty comment
                    },
                ],
            },
        );

        let relationships = vec![
            ErRelationship {
                left_entity: "Customer".to_string(),
                right_entity: "Order".to_string(),
                left_cardinality: ErCardinality {
                    min: CardinalityValue::Zero,
                    max: CardinalityValue::One,
                },
                right_cardinality: ErCardinality {
                    min: CardinalityValue::One,
                    max: CardinalityValue::Many,
                },
                label: Some("places".to_string()),
            },
            ErRelationship {
                left_entity: "Order".to_string(),
                right_entity: "Product".to_string(),
                left_cardinality: ErCardinality {
                    min: CardinalityValue::One,
                    max: CardinalityValue::One,
                },
                right_cardinality: ErCardinality {
                    min: CardinalityValue::Zero,
                    max: CardinalityValue::Many,
                },
                label: None, // Test without label
            },
        ];

        let er_diagram = DiagramType::Er(ErDiagram {
            title: Some("E-commerce".to_string()),
            accessibility: AccessibilityInfo::default(),
            entities,
            relationships,
        });

        let output = er_diagram.to_mermaid();
        assert!(output.contains("erDiagram"));
        assert!(output.contains("title E-commerce"));
        assert!(output.contains("Customer o|--|{ Order : places"));
        assert!(output.contains("Order ||--o{ Product :"));
        assert!(output.contains("Customer {"));
        assert!(output.contains("int id PK \"Primary key\""));
        assert!(output.contains("varchar(255) email UK"));
        assert!(output.contains("int foreign_key FK"));
    }

    // Test all remaining diagram types for coverage
    #[test]
    fn test_remaining_diagram_types() {
        // Test Git diagram with all operation types
        let git = DiagramType::Git(GitDiagram {
            title: Some("Git Flow".to_string()),
            accessibility: AccessibilityInfo::default(),
            theme: Some("base".to_string()),
            commits: vec![GitCommit {
                id: Some("c1".to_string()),
                commit_type: CommitType::Normal,
                tag: Some("v1.0".to_string()),
                branch: "main".to_string(),
            }],
            branches: vec![GitBranch {
                name: "main".to_string(),
                order: Some(0),
                color: Some("blue".to_string()),
            }],
            operations: vec![
                GitOperation::Commit {
                    id: Some("c1".to_string()),
                    commit_type: CommitType::Reverse,
                    tag: Some("v1.0".to_string()),
                },
                GitOperation::Commit {
                    id: Some("c2".to_string()),
                    commit_type: CommitType::Highlight,
                    tag: None,
                },
                GitOperation::Branch {
                    name: "feature".to_string(),
                    order: Some(1),
                },
                GitOperation::Checkout {
                    branch: "feature".to_string(),
                },
                GitOperation::Merge {
                    branch: "feature".to_string(),
                    id: Some("merge1".to_string()),
                    tag: Some("merge-tag".to_string()),
                    commit_type: CommitType::Normal,
                },
                GitOperation::CherryPick {
                    id: "cherry1".to_string(),
                    parent: Some("parent1".to_string()),
                    tag: Some("cherry-tag".to_string()),
                },
            ],
        });

        let git_output = git.to_mermaid();
        assert!(git_output.contains("gitGraph"));
        assert!(git_output.contains("title Git Flow"));
        assert!(git_output.contains("theme base"));
        assert!(git_output.contains("commit id: \"c1\" type: REVERSE tag: \"v1.0\""));
        assert!(git_output.contains("commit id: \"c2\" type: HIGHLIGHT"));
        assert!(git_output.contains("branch feature order: 1"));
        assert!(git_output.contains("checkout feature"));
        assert!(git_output.contains("merge feature id: \"merge1\" tag: \"merge-tag\""));
        assert!(git_output
            .contains("cherry-pick id: \"cherry1\" parent: \"parent1\" tag: \"cherry-tag\""));

        // Test Timeline with empty sections
        let timeline = DiagramType::Timeline(TimelineDiagram {
            title: Some("Project Timeline".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![
                TimelineSection {
                    name: "Phase 1".to_string(),
                    items: vec![
                        TimelineItem::Period("2023".to_string()),
                        TimelineItem::Event("Started project".to_string()),
                    ],
                },
                TimelineSection {
                    name: "Phase 2".to_string(),
                    items: vec![],
                },
            ],
        });

        let timeline_output = timeline.to_mermaid();
        assert!(timeline_output.contains("timeline"));
        assert!(timeline_output.contains("title Project Timeline"));
        assert!(timeline_output.contains("section Phase 1"));
        assert!(timeline_output.contains("2023"));
        assert!(timeline_output.contains(": Started project"));
        assert!(timeline_output.contains("section Phase 2"));

        // Test Journey with multiple actors
        let journey = DiagramType::Journey(JourneyDiagram {
            title: Some("User Journey".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![JourneySection {
                name: "Discovery".to_string(),
                tasks: vec![JourneyTask {
                    name: "Research".to_string(),
                    score: 5,
                    actors: vec!["User".to_string(), "System".to_string()],
                }],
            }],
        });

        let journey_output = journey.to_mermaid();
        assert!(journey_output.contains("journey"));
        assert!(journey_output.contains("title User Journey"));
        assert!(journey_output.contains("section Discovery"));
        assert!(journey_output.contains("Research: 5: User, System"));
    }

    // Test edge cases with null/empty values
    #[test]
    fn test_edge_cases_null_empty() {
        // Test flowchart with empty node text
        let mut nodes = HashMap::new();
        nodes.insert(
            "empty".to_string(),
            FlowNode {
                id: "empty".to_string(),
                text: None, // No text
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let flowchart = DiagramType::Flowchart(FlowchartDiagram {
            title: None, // No title
            accessibility: AccessibilityInfo {
                title: None, // No accessibility
                description: None,
            },
            direction: FlowDirection::TD,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        let output = flowchart.to_mermaid();
        assert!(output.contains("flowchart TD"));
        assert!(output.contains("empty[]")); // Empty node text

        // Test sequence without autonumber
        let sequence = DiagramType::Sequence(SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![],
            statements: vec![],
            autonumber: None,
        });

        let seq_output = sequence.to_mermaid();
        assert!(seq_output.contains("sequenceDiagram"));
        assert!(!seq_output.contains("autonumber"));

        // Test pie without title
        let pie = DiagramType::Pie(PieDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            show_data: false,
            data: vec![],
        });

        let pie_output = pie.to_mermaid();
        assert!(pie_output.contains("pie"));
        assert!(!pie_output.contains("title"));
    }
}
