//! Comprehensive tests targeting missing coverage areas in visitor.rs

use mermaid_parser::common::ast::*;
use mermaid_parser::common::visitor::{
    AstVisitor, AstVisitorMut, ComplexityAnalyzer, NodeCounter, ReferenceValidator, TitleSetter,
};
use std::collections::HashMap;

#[cfg(test)]
mod visitor_comprehensive_tests {
    use super::*;

    // Test individual element visitors that are likely not covered

    #[test]
    fn test_node_counter_individual_elements() {
        let mut counter = NodeCounter::new();

        // Test sankey elements
        let sankey_node = SankeyNode {
            id: "node1".to_string(),
            name: "Node 1".to_string(),
        };
        let sankey_link = SankeyLink {
            source: "node1".to_string(),
            target: "node2".to_string(),
            value: 10.0,
        };
        counter.visit_sankey_node(&sankey_node);
        counter.visit_sankey_link(&sankey_link);

        // Test flow elements
        let flow_node = FlowNode {
            id: "flow1".to_string(),
            text: Some("Flow Node".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        };
        let flow_edge = FlowEdge {
            from: "flow1".to_string(),
            to: "flow2".to_string(),
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        };
        counter.visit_flow_node(&flow_node);
        counter.visit_flow_edge(&flow_edge);

        // Test sequence elements
        let sequence_message = Message {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            text: "Hello".to_string(),
            arrow_type: ArrowType::SolidOpen,
        };
        counter.visit_sequence_message(&sequence_message);

        // Test class elements
        let class_def = Class {
            name: "MyClass".to_string(),
            stereotype: None,
            members: vec![],
            annotations: vec![],
            css_class: None,
        };
        counter.visit_class_definition(&class_def);

        // Test state elements
        let state_node = State {
            id: "state1".to_string(),
            display_name: Some("State 1".to_string()),
            state_type: StateType::Simple,
            substates: vec![],
            concurrent_regions: vec![],
        };
        let state_transition = StateTransition {
            from: "state1".to_string(),
            to: "state2".to_string(),
            event: Some("event".to_string()),
            guard: None,
            action: None,
        };
        counter.visit_state_node(&state_node);
        counter.visit_state_transition(&state_transition);

        // Check final counts
        assert_eq!(counter.nodes(), 4); // sankey_node + flow_node + class_def + state_node
        assert_eq!(counter.edges(), 3); // sankey_link + flow_edge + state_transition
        assert_eq!(counter.elements(), 1); // sequence_message
        assert_eq!(counter.total(), 8);
    }

    // Test ComplexityAnalyzer with different diagram types

    #[test]
    fn test_complexity_analyzer_sankey() {
        let diagram = SankeyDiagram {
            nodes: vec![
                SankeyNode {
                    id: "A".to_string(),
                    name: "Node A".to_string(),
                },
                SankeyNode {
                    id: "B".to_string(),
                    name: "Node B".to_string(),
                },
                SankeyNode {
                    id: "C".to_string(),
                    name: "Node C".to_string(),
                },
            ],
            links: vec![
                SankeyLink {
                    source: "A".to_string(),
                    target: "B".to_string(),
                    value: 10.0,
                },
                SankeyLink {
                    source: "B".to_string(),
                    target: "C".to_string(),
                    value: 5.0,
                },
            ],
        };

        let mut analyzer = ComplexityAnalyzer::new();
        analyzer.visit_sankey(&diagram);

        // Should count nodes and links properly
        assert!(analyzer.cyclomatic_complexity() > 1);
    }

    #[test]
    fn test_complexity_analyzer_with_state() {
        let diagram = StateDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            version: StateVersion::V1,
            states: HashMap::new(),
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
                    to: "C".to_string(),
                    event: Some("event2".to_string()),
                    guard: None,
                    action: None,
                },
            ],
            notes: vec![],
        };

        let mut analyzer = ComplexityAnalyzer::new();
        analyzer.visit_state(&diagram);

        assert_eq!(analyzer.max_depth(), 0); // No nested structures
        assert_eq!(analyzer.cyclomatic_complexity(), 3); // 2 transitions - 1 node + 2
    }

    #[test]
    fn test_complexity_analyzer_with_class() {
        let diagram = ClassDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            classes: HashMap::new(),
            relationships: vec![ClassRelationship {
                from: "ClassA".to_string(),
                to: "ClassB".to_string(),
                relationship_type: ClassRelationshipType::Inheritance,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            }],
            notes: vec![],
        };

        let mut analyzer = ComplexityAnalyzer::new();
        analyzer.visit_class(&diagram);

        assert_eq!(analyzer.max_depth(), 0);
        assert_eq!(analyzer.cyclomatic_complexity(), 2); // 1 relationship - 1 node + 2
    }

    // Test TitleSetter with different diagram types

    #[test]
    fn test_title_setter_sankey_and_misc_diagrams() {
        // Test Sankey diagram (doesn't have title)
        let mut sankey = SankeyDiagram {
            nodes: vec![],
            links: vec![],
        };

        let mut title_setter = TitleSetter::new("Should Not Be Set".to_string());
        title_setter.visit_sankey_mut(&mut sankey);
        // Sankey diagrams don't have titles, so this should not crash

        // Test Misc diagram (doesn't have title)
        let mut misc = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["test".to_string()],
            }),
            diagram_type: "info".to_string(),
        };

        title_setter.visit_misc_mut(&mut misc);
        // Misc diagrams don't have titles, so this should not crash
    }

    #[test]
    fn test_title_setter_simple_diagram_types() {
        let title = "Universal Test Title".to_string();
        let mut setter = TitleSetter::new(title.clone());

        // Test Timeline diagram
        let mut timeline = TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        setter.visit_timeline_mut(&mut timeline);
        assert_eq!(timeline.title, Some(title.clone()));

        // Test Journey diagram
        let mut journey = JourneyDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        setter.visit_journey_mut(&mut journey);
        assert_eq!(journey.title, Some(title.clone()));

        // Test Sequence diagram
        let mut sequence = SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![],
            statements: vec![],
            autonumber: None,
        };
        setter.visit_sequence_mut(&mut sequence);
        assert_eq!(sequence.title, Some(title.clone()));

        // Test Class diagram
        let mut class = ClassDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            classes: HashMap::new(),
            relationships: vec![],
            notes: vec![],
        };
        setter.visit_class_mut(&mut class);
        assert_eq!(class.title, Some(title.clone()));

        // Test State diagram
        let mut state = StateDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            version: StateVersion::V1,
            states: HashMap::new(),
            transitions: vec![],
            notes: vec![],
        };
        setter.visit_state_mut(&mut state);
        assert_eq!(state.title, Some(title.clone()));

        // Test Flowchart diagram
        let mut flowchart = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes: HashMap::new(),
            edges: vec![],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        };
        setter.visit_flowchart_mut(&mut flowchart);
        assert_eq!(flowchart.title, Some(title.clone()));
    }

    // Test default implementations for ReferenceValidator

    #[test]
    fn test_reference_validator_default_implementations() {
        let mut validator = ReferenceValidator::new();

        // Test all the default implementations that should do nothing
        validator.visit_sankey(&SankeyDiagram {
            nodes: vec![],
            links: vec![],
        });
        validator.visit_timeline(&TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });
        validator.visit_journey(&JourneyDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });
        validator.visit_sequence(&SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![],
            statements: vec![],
            autonumber: None,
        });
        validator.visit_misc(&MiscDiagram {
            content: MiscContent::Raw(RawDiagram { lines: vec![] }),
            diagram_type: "test".to_string(),
        });

        // Test element visitors
        validator.visit_sankey_node(&SankeyNode {
            id: "test".to_string(),
            name: "Test".to_string(),
        });
        validator.visit_sankey_link(&SankeyLink {
            source: "a".to_string(),
            target: "b".to_string(),
            value: 1.0,
        });
        validator.visit_flow_node(&FlowNode {
            id: "test".to_string(),
            text: None,
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        });
        validator.visit_flow_edge(&FlowEdge {
            from: "a".to_string(),
            to: "b".to_string(),
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        });
        validator.visit_sequence_message(&Message {
            from: "a".to_string(),
            to: "b".to_string(),
            text: "test".to_string(),
            arrow_type: ArrowType::SolidOpen,
        });
        validator.visit_class_definition(&Class {
            name: "Test".to_string(),
            stereotype: None,
            members: vec![],
            annotations: vec![],
            css_class: None,
        });
        validator.visit_state_node(&State {
            id: "test".to_string(),
            display_name: None,
            state_type: StateType::Simple,
            substates: vec![],
            concurrent_regions: vec![],
        });
        validator.visit_state_transition(&StateTransition {
            from: "a".to_string(),
            to: "b".to_string(),
            event: None,
            guard: None,
            action: None,
        });

        // None of these should have created any errors or references
        assert!(!validator.has_errors());
        assert_eq!(validator.undefined_references().len(), 0);
    }

    // Test some missing diagram types in the ComplexityAnalyzer with simple default implementations

    #[test]
    fn test_complexity_analyzer_various_diagram_types() {
        let mut analyzer = ComplexityAnalyzer::new();

        // Test the default implementations that just count nodes
        analyzer.visit_timeline(&TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });
        analyzer.visit_journey(&JourneyDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });
        analyzer.visit_misc(&MiscDiagram {
            content: MiscContent::Raw(RawDiagram { lines: vec![] }),
            diagram_type: "test".to_string(),
        });

        // These should all count as single nodes
        assert!(analyzer.cyclomatic_complexity() >= 1);
    }

    // Test edge cases and basic methods

    #[test]
    fn test_visitor_edge_cases() {
        // Test ComplexityAnalyzer with zero values
        let analyzer = ComplexityAnalyzer::new();
        assert_eq!(analyzer.max_depth(), 0);
        assert_eq!(analyzer.average_branching_factor(), 0.0);
        assert_eq!(analyzer.cyclomatic_complexity(), 1); // Minimum complexity

        // Test ReferenceValidator with no references
        let validator = ReferenceValidator::new();
        assert!(!validator.has_errors());
        assert_eq!(validator.errors().len(), 0);
        assert_eq!(validator.undefined_references().len(), 0);

        // Test NodeCounter totals
        let mut counter = NodeCounter::new();
        // Add some test data
        counter.visit_sankey_node(&SankeyNode {
            id: "test".to_string(),
            name: "Test".to_string(),
        });
        counter.visit_sankey_link(&SankeyLink {
            source: "A".to_string(),
            target: "B".to_string(),
            value: 10.0,
        });
        counter.visit_sequence_message(&Message {
            from: "A".to_string(),
            to: "B".to_string(),
            text: "Test".to_string(),
            arrow_type: ArrowType::SolidOpen,
        });

        assert_eq!(counter.nodes(), 1);
        assert_eq!(counter.edges(), 1);
        assert_eq!(counter.elements(), 1);
        assert_eq!(counter.total(), 3);
    }

    // Test some specific visitor methods for ComplexityAnalyzer that might not be covered

    #[test]
    fn test_complexity_analyzer_methods() {
        let mut analyzer = ComplexityAnalyzer::new();

        // Test initial state
        assert_eq!(analyzer.max_depth(), 0);
        assert_eq!(analyzer.average_branching_factor(), 0.0);
        assert_eq!(analyzer.cyclomatic_complexity(), 1);

        // Test with some data by visiting diagram types that add complexity
        let sankey = SankeyDiagram {
            nodes: vec![
                SankeyNode {
                    id: "A".to_string(),
                    name: "Node A".to_string(),
                },
                SankeyNode {
                    id: "B".to_string(),
                    name: "Node B".to_string(),
                },
            ],
            links: vec![SankeyLink {
                source: "A".to_string(),
                target: "B".to_string(),
                value: 10.0,
            }],
        };
        analyzer.visit_sankey(&sankey);

        // After visiting sankey with nodes and links, complexity should increase
        assert!(analyzer.cyclomatic_complexity() > 1);
    }

    // Test different states and transitions for reference validator

    #[test]
    fn test_reference_validator_with_state() {
        let mut states = HashMap::new();
        states.insert(
            "start".to_string(),
            State {
                id: "start".to_string(),
                display_name: Some("Start state".to_string()),
                state_type: StateType::Start,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );
        states.insert(
            "end".to_string(),
            State {
                id: "end".to_string(),
                display_name: Some("End state".to_string()),
                state_type: StateType::End,
                substates: vec![],
                concurrent_regions: vec![],
            },
        );

        let diagram = StateDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            version: StateVersion::V1,
            states,
            transitions: vec![
                StateTransition {
                    from: "start".to_string(),
                    to: "middle".to_string(), // UNDEFINED
                    event: Some("event".to_string()),
                    guard: None,
                    action: None,
                },
                StateTransition {
                    from: "middle".to_string(), // UNDEFINED
                    to: "end".to_string(),
                    event: Some("event2".to_string()),
                    guard: None,
                    action: None,
                },
            ],
            notes: vec![],
        };

        let mut validator = ReferenceValidator::new();
        validator.visit_state(&diagram);

        assert!(validator.has_errors());
        let undefined = validator.undefined_references();
        assert_eq!(undefined.len(), 1);
        assert!(undefined.contains(&"middle".to_string()));
    }

    #[test]
    fn test_reference_validator_with_class() {
        let mut classes = HashMap::new();
        classes.insert(
            "ClassA".to_string(),
            Class {
                name: "ClassA".to_string(),
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
            relationships: vec![ClassRelationship {
                from: "ClassA".to_string(),
                to: "ClassB".to_string(), // UNDEFINED
                relationship_type: ClassRelationshipType::Association,
                from_cardinality: None,
                to_cardinality: None,
                label: None,
            }],
            notes: vec![],
        };

        let mut validator = ReferenceValidator::new();
        validator.visit_class(&diagram);

        assert!(validator.has_errors());
        let undefined = validator.undefined_references();
        assert_eq!(undefined.len(), 1);
        assert!(undefined.contains(&"ClassB".to_string()));
    }

    // Test some visitor methods that might be missing from the ComplexityAnalyzer

    #[test]
    fn test_complexity_analyzer_specific_methods() {
        let mut analyzer = ComplexityAnalyzer::new();

        // Test visit_packet
        analyzer.visit_packet(&PacketDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            fields: vec![],
        });

        // Test visit_radar
        analyzer.visit_radar(&RadarDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            config: RadarConfig::default(),
            axes: vec![],
            datasets: vec![],
        });

        // All these simple visits should just count as nodes
        assert!(analyzer.cyclomatic_complexity() >= 1);
    }

    // Test NodeCounter with various diagram types

    #[test]
    fn test_node_counter_with_timeline() {
        let diagram = TimelineDiagram {
            title: Some("Test Timeline".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![
                TimelineSection {
                    name: "Section 1".to_string(),
                    items: vec![
                        TimelineItem::Event("Event 1".to_string()),
                        TimelineItem::Period("2023".to_string()),
                    ],
                },
                TimelineSection {
                    name: "Section 2".to_string(),
                    items: vec![TimelineItem::Event("Event 2".to_string())],
                },
            ],
        };

        let mut counter = NodeCounter::new();
        counter.visit_timeline(&diagram);

        assert_eq!(counter.nodes(), 0);
        assert_eq!(counter.edges(), 0);
        assert_eq!(counter.elements(), 3); // 2 + 1 items
        assert_eq!(counter.total(), 3);
    }

    #[test]
    fn test_node_counter_with_journey() {
        let diagram = JourneyDiagram {
            title: Some("Test Journey".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![JourneySection {
                name: "Section 1".to_string(),
                tasks: vec![
                    JourneyTask {
                        name: "Task 1".to_string(),
                        score: 5,
                        actors: vec!["Actor1".to_string(), "Actor2".to_string()],
                    },
                    JourneyTask {
                        name: "Task 2".to_string(),
                        score: 3,
                        actors: vec!["Actor1".to_string()],
                    },
                ],
            }],
        };

        let mut counter = NodeCounter::new();
        counter.visit_journey(&diagram);

        assert_eq!(counter.nodes(), 0);
        assert_eq!(counter.edges(), 0);
        assert_eq!(counter.elements(), 2); // 2 tasks
        assert_eq!(counter.total(), 2);
    }

    #[test]
    fn test_node_counter_with_sequence() {
        let diagram = SequenceDiagram {
            title: Some("Test Sequence".to_string()),
            accessibility: AccessibilityInfo::default(),
            participants: vec![
                Participant {
                    actor: "Alice".to_string(),
                    alias: Some("Alice Smith".to_string()),
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
                    text: "Hi there".to_string(),
                    arrow_type: ArrowType::SolidClosed,
                }),
            ],
            autonumber: None,
        };

        let mut counter = NodeCounter::new();
        counter.visit_sequence(&diagram);

        assert_eq!(counter.nodes(), 2); // 2 participants
        assert_eq!(counter.edges(), 0);
        assert_eq!(counter.elements(), 2); // 2 statements
        assert_eq!(counter.total(), 4);
    }

    #[test]
    fn test_node_counter_with_misc() {
        let diagram = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["line 1".to_string(), "line 2".to_string()],
            }),
            diagram_type: "unknown".to_string(),
        };

        let mut counter = NodeCounter::new();
        counter.visit_misc(&diagram);

        assert_eq!(counter.nodes(), 0);
        assert_eq!(counter.edges(), 0);
        assert_eq!(counter.elements(), 1); // 1 misc element
        assert_eq!(counter.total(), 1);
    }
}
