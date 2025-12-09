//! Comprehensive tests for metrics module to improve coverage

use mermaid_parser::common::ast::*;
use mermaid_parser::common::metrics::*;
use std::collections::HashMap;

#[test]
fn test_comprehensive_sequence_diagram_metrics() {
    // Test sequence diagram with complex nested statements
    let participants = vec![
        Participant {
            actor: "Alice".to_string(),
            alias: None,
            participant_type: ParticipantType::Actor,
        },
        Participant {
            actor: "Bob".to_string(),
            alias: None,
            participant_type: ParticipantType::Actor,
        },
        Participant {
            actor: "Charlie".to_string(),
            alias: None,
            participant_type: ParticipantType::Actor,
        },
    ];

    // Create complex sequence with nested structures
    let statements = vec![
        SequenceStatement::Message(Message {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            text: "Start process".to_string(),
            arrow_type: ArrowType::SolidOpen,
        }),
        SequenceStatement::Loop(Loop {
            condition: "while active".to_string(),
            statements: vec![SequenceStatement::Alt(Alternative {
                condition: "if condition".to_string(),
                statements: vec![SequenceStatement::Message(Message {
                    from: "Bob".to_string(),
                    to: "Charlie".to_string(),
                    text: "Success case".to_string(),
                    arrow_type: ArrowType::SolidOpen,
                })],
                else_branch: Some(ElseBranch {
                    condition: Some("else".to_string()),
                    statements: vec![SequenceStatement::Message(Message {
                        from: "Bob".to_string(),
                        to: "Alice".to_string(),
                        text: "Error case".to_string(),
                        arrow_type: ArrowType::SolidOpen,
                    })],
                }),
            })],
        }),
        SequenceStatement::Opt(Optional {
            condition: "if cleanup needed".to_string(),
            statements: vec![SequenceStatement::Message(Message {
                from: "Charlie".to_string(),
                to: "Alice".to_string(),
                text: "Cleanup".to_string(),
                arrow_type: ArrowType::SolidOpen,
            })],
        }),
        SequenceStatement::Par(Parallel {
            branches: vec![
                ParallelBranch {
                    condition: Some("branch 1".to_string()),
                    statements: vec![SequenceStatement::Message(Message {
                        from: "Alice".to_string(),
                        to: "Bob".to_string(),
                        text: "Parallel 1".to_string(),
                        arrow_type: ArrowType::SolidOpen,
                    })],
                },
                ParallelBranch {
                    condition: Some("branch 2".to_string()),
                    statements: vec![SequenceStatement::Message(Message {
                        from: "Alice".to_string(),
                        to: "Charlie".to_string(),
                        text: "Parallel 2".to_string(),
                        arrow_type: ArrowType::SolidOpen,
                    })],
                },
            ],
        }),
        SequenceStatement::Critical(Critical {
            condition: "critical section".to_string(),
            statements: vec![SequenceStatement::Message(Message {
                from: "Bob".to_string(),
                to: "Charlie".to_string(),
                text: "Critical operation".to_string(),
                arrow_type: ArrowType::SolidOpen,
            })],
            options: vec![
                CriticalOption {
                    condition: "option 1".to_string(),
                    statements: vec![SequenceStatement::Message(Message {
                        from: "Charlie".to_string(),
                        to: "Alice".to_string(),
                        text: "Option 1 response".to_string(),
                        arrow_type: ArrowType::SolidOpen,
                    })],
                },
                CriticalOption {
                    condition: "option 2".to_string(),
                    statements: vec![
                        SequenceStatement::Message(Message {
                            from: "Charlie".to_string(),
                            to: "Bob".to_string(),
                            text: "Option 2a".to_string(),
                            arrow_type: ArrowType::SolidOpen,
                        }),
                        SequenceStatement::Message(Message {
                            from: "Bob".to_string(),
                            to: "Charlie".to_string(),
                            text: "Option 2b".to_string(),
                            arrow_type: ArrowType::SolidOpen,
                        }),
                    ],
                },
            ],
        }),
        // Test other sequence statement types for coverage
        SequenceStatement::Note(Note {
            position: NotePosition::Over,
            actor: "Alice".to_string(),
            text: "Important note".to_string(),
        }),
        SequenceStatement::Activate("Bob".to_string()),
        SequenceStatement::Deactivate("Bob".to_string()),
        SequenceStatement::Create(Participant {
            actor: "Dynamic".to_string(),
            alias: None,
            participant_type: ParticipantType::Participant,
        }),
        SequenceStatement::Destroy("Dynamic".to_string()),
    ];

    let diagram = SequenceDiagram {
        title: Some("Complex Sequence".to_string()),
        accessibility: AccessibilityInfo::default(),
        participants,
        statements,
        autonumber: Some(AutoNumber {
            start: Some(1),
            step: Some(1),
            visible: true,
        }),
    };

    let metrics = diagram.calculate_metrics();

    // Verify basic metrics
    assert_eq!(metrics.basic.node_count, 3); // 3 participants
    assert!(metrics.basic.edge_count > 5); // Many messages
    assert!(metrics.basic.depth > 1); // Nested structures

    // Verify complexity metrics
    assert!(metrics.complexity.cyclomatic >= 1);
    assert!(metrics.complexity.cognitive > 0.0);
    assert!(metrics.complexity.nesting_depth > 1); // Due to nested Alt in Loop

    // Verify quality metrics
    assert!(metrics.quality.maintainability > 0.0);
    assert!(metrics.quality.readability > 0.0);
    assert!(metrics.quality.modularity > 0.0);

    // Test that suggestions are generated for complex diagrams
    if metrics.basic.edge_count > 50 || metrics.complexity.nesting_depth > 4 {
        assert!(!metrics.suggestions.is_empty());
    }
}

#[test]
fn test_complex_flowchart_metrics() {
    let mut nodes = HashMap::new();
    for i in 1..=15 {
        nodes.insert(
            format!("node{}", i),
            FlowNode {
                id: format!("node{}", i),
                text: Some(format!("Node {}", i)),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );
    }

    let mut edges = Vec::new();
    for i in 1..14 {
        edges.push(FlowEdge {
            from: format!("node{}", i),
            to: format!("node{}", i + 1),
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        });
    }
    // Add some complex edges for higher cyclomatic complexity
    edges.push(FlowEdge {
        from: "node5".to_string(),
        to: "node10".to_string(),
        edge_type: EdgeType::Arrow,
        label: Some("branch".to_string()),
        min_length: None,
    });
    edges.push(FlowEdge {
        from: "node8".to_string(),
        to: "node3".to_string(),
        edge_type: EdgeType::DottedArrow,
        label: Some("loop back".to_string()),
        min_length: None,
    });

    let nested_subgraph = Subgraph {
        id: "nested".to_string(),
        title: Some("Nested Level".to_string()),
        nodes: vec!["node1".to_string()],
        edges: vec![],
        subgraphs: vec![],
        direction: Some(FlowDirection::LR),
    };

    let main_subgraph = Subgraph {
        id: "main".to_string(),
        title: Some("Main Group".to_string()),
        nodes: vec!["node2".to_string(), "node3".to_string()],
        edges: vec![],
        subgraphs: vec![nested_subgraph],
        direction: Some(FlowDirection::TD),
    };

    let diagram = FlowchartDiagram {
        title: Some("Complex Flowchart".to_string()),
        accessibility: AccessibilityInfo::default(),
        direction: FlowDirection::TD,
        nodes,
        edges,
        subgraphs: vec![main_subgraph],
        styles: vec![StyleDefinition {
            target: StyleTarget::Node("node1".to_string()),
            styles: {
                let mut style_map = HashMap::new();
                style_map.insert("fill".to_string(), "#f9f".to_string());
                style_map
            },
        }],
        class_defs: HashMap::new(),
        clicks: vec![],
    };

    let metrics = diagram.calculate_metrics();

    assert_eq!(metrics.basic.node_count, 15);
    assert_eq!(metrics.basic.edge_count, 15);
    assert!(metrics.basic.depth >= 2); // Due to nested subgraphs
    assert!(metrics.basic.breadth > 0);

    // Should have reasonable complexity
    assert!(metrics.complexity.cyclomatic > 1);
    assert!(metrics.complexity.cognitive > 0.0);
    assert!(metrics.complexity.nesting_depth >= 1); // Due to subgraphs

    // Quality metrics should be reasonable
    assert!(metrics.quality.maintainability > 0.0 && metrics.quality.maintainability <= 1.0);
    assert!(metrics.quality.readability > 0.0 && metrics.quality.readability <= 1.0);
    assert!(metrics.quality.modularity > 0.0 && metrics.quality.modularity <= 1.0);
}

#[test]
fn test_complex_class_diagram_metrics() {
    let mut classes = HashMap::new();

    // Create a complex class hierarchy
    for i in 1..=10 {
        let members = vec![
            ClassMember::Property(Property {
                name: format!("field{}", i),
                prop_type: Some("String".to_string()),
                visibility: Visibility::Private,
                is_static: false,
                default_value: None,
            }),
            ClassMember::Method(Method {
                name: format!("method{}", i),
                visibility: Visibility::Public,
                parameters: vec![],
                return_type: Some("void".to_string()),
                is_static: false,
                is_abstract: false,
            }),
        ];

        classes.insert(
            format!("Class{}", i),
            Class {
                name: format!("Class{}", i),
                stereotype: if i % 3 == 0 {
                    Some(Stereotype::Abstract)
                } else {
                    None
                },
                members,
                annotations: vec![format!("@Component{}", i)],
                css_class: Some(format!("class-style-{}", i)),
            },
        );
    }

    let mut relationships = Vec::new();
    // Create inheritance and other relationships
    for i in 2..=10 {
        relationships.push(ClassRelationship {
            from: format!("Class{}", i),
            to: format!("Class{}", i - 1),
            relationship_type: if i % 2 == 0 {
                ClassRelationshipType::Inheritance
            } else {
                ClassRelationshipType::Composition
            },
            from_cardinality: Some("1".to_string()),
            to_cardinality: Some("*".to_string()),
            label: Some(format!("rel{}", i)),
        });
    }

    let diagram = ClassDiagram {
        title: Some("Complex Class System".to_string()),
        accessibility: AccessibilityInfo::default(),
        classes,
        relationships,
        notes: vec![
            Note {
                position: NotePosition::Over,
                actor: "Class1".to_string(),
                text: "This is the base class".to_string(),
            },
            Note {
                position: NotePosition::LeftOf,
                actor: "Class5".to_string(),
                text: "Important class".to_string(),
            },
        ],
    };

    let metrics = diagram.calculate_metrics();

    assert_eq!(metrics.basic.node_count, 10);
    assert_eq!(metrics.basic.edge_count, 9); // 9 relationships
    assert!(metrics.complexity.cyclomatic >= 1);
    assert!(metrics.quality.maintainability > 0.0);

    // Should have suggestions for large class diagrams
    if metrics.basic.node_count > 25 {
        assert!(metrics
            .suggestions
            .iter()
            .any(|s| s.message.contains("Large number of classes")));
    }
}

#[test]
fn test_large_sankey_diagram_metrics() {
    let mut nodes = Vec::new();
    let mut links = Vec::new();

    // Create a large Sankey diagram to trigger suggestions
    for i in 1..=25 {
        nodes.push(SankeyNode {
            id: format!("node{}", i),
            name: format!("Node {}", i),
        });
    }

    // Create connections with high coupling
    for i in 1..=20 {
        for j in (i + 1)..=(i + 4).min(25) {
            links.push(SankeyLink {
                source: format!("node{}", i),
                target: format!("node{}", j),
                value: (i * j) as f64,
            });
        }
    }

    let diagram = SankeyDiagram { nodes, links };

    let metrics = diagram.calculate_metrics();

    assert_eq!(metrics.basic.node_count, 25);
    assert!(metrics.basic.edge_count > 20); // Many links
    assert!(metrics.complexity.coupling > 0.0);

    // Should generate suggestions for large diagrams
    assert!(metrics
        .suggestions
        .iter()
        .any(|s| s.message.contains("grouping related nodes")
            || s.category == SuggestionCategory::Complexity));
}

#[test]
fn test_metrics_report_display_with_no_suggestions() {
    let report = MetricsReport {
        basic: BasicMetrics {
            node_count: 3,
            edge_count: 2,
            depth: 1,
            breadth: 2,
        },
        complexity: ComplexityMetrics {
            cyclomatic: 2,
            cognitive: 1.5,
            nesting_depth: 1,
            coupling: 0.67,
        },
        quality: QualityMetrics {
            maintainability: 0.95,
            readability: 0.85,
            modularity: 0.7,
        },
        suggestions: vec![], // No suggestions
    };

    let output = format!("{}", report);
    assert!(output.contains("Diagram Metrics Report"));
    assert!(output.contains("Nodes: 3"));
    assert!(output.contains("Edges: 2"));
    assert!(output.contains("Complexity: 2 (Low)"));
    assert!(output.contains("Cognitive Complexity: 1.5"));
    assert!(output.contains("Maintainability: 95.0%"));
    assert!(output.contains("Readability: 85.0%"));
    assert!(output.contains("Modularity: 70.0%"));

    // Should not contain suggestions section
    assert!(!output.contains("Suggestions:"));
}

#[test]
fn test_metrics_report_display_with_suggestions() {
    let report = MetricsReport {
        basic: BasicMetrics {
            node_count: 50,
            edge_count: 75,
            depth: 5,
            breadth: 10,
        },
        complexity: ComplexityMetrics {
            cyclomatic: 35,
            cognitive: 15.5,
            nesting_depth: 4,
            coupling: 1.5,
        },
        quality: QualityMetrics {
            maintainability: 0.4,
            readability: 0.3,
            modularity: 0.8,
        },
        suggestions: vec![
            Suggestion {
                category: SuggestionCategory::Complexity,
                message: "High complexity detected".to_string(),
                severity: SeverityLevel::Error,
            },
            Suggestion {
                category: SuggestionCategory::Organization,
                message: "Consider restructuring".to_string(),
                severity: SeverityLevel::Warning,
            },
            Suggestion {
                category: SuggestionCategory::Structure,
                message: "Simplify relationships".to_string(),
                severity: SeverityLevel::Info,
            },
        ],
    };

    let output = format!("{}", report);
    assert!(output.contains("Suggestions:"));
    assert!(output.contains("❌ [Complexity]: High complexity detected"));
    assert!(output.contains("⚠️ [Organization]: Consider restructuring"));
    assert!(output.contains("ℹ️ [Structure]: Simplify relationships"));
    assert!(output.contains("Complexity: 35 (High)"));
}

#[test]
fn test_diagram_type_metrics_coverage() {
    // Test various diagram types that should use generic metrics
    let timeline = DiagramType::Timeline(TimelineDiagram {
        title: Some("Test Timeline".to_string()),
        accessibility: AccessibilityInfo::default(),
        sections: vec![TimelineSection {
            name: "Phase 1".to_string(),
            items: vec![
                TimelineItem::Event("Start".to_string()),
                TimelineItem::Period("Q1".to_string()),
            ],
        }],
    });

    let pie = DiagramType::Pie(PieDiagram {
        title: Some("Test Pie".to_string()),
        accessibility: AccessibilityInfo::default(),
        show_data: true,
        data: vec![
            PieSlice {
                label: "A".to_string(),
                value: 30.0,
            },
            PieSlice {
                label: "B".to_string(),
                value: 70.0,
            },
        ],
    });

    let journey = DiagramType::Journey(JourneyDiagram {
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

    // Test that all return valid metrics
    for diagram in [timeline, pie, journey] {
        let metrics = diagram.calculate_metrics();

        // No need to check >= 0 for usize types
        // Just verify they exist
        assert_eq!(metrics.basic.depth, 1); // Generic depth
        assert_eq!(metrics.quality.modularity, 0.5); // Generic modularity
        assert!(metrics.quality.maintainability > 0.0);
        assert!(metrics.quality.readability > 0.0);
    }
}

#[test]
fn test_all_severity_levels_and_categories() {
    // Test all combinations of severity levels and categories
    let severities = [
        SeverityLevel::Info,
        SeverityLevel::Warning,
        SeverityLevel::Error,
    ];
    let categories = [
        SuggestionCategory::Complexity,
        SuggestionCategory::Structure,
        SuggestionCategory::Naming,
        SuggestionCategory::Organization,
    ];

    for severity in &severities {
        for category in &categories {
            let suggestion = Suggestion {
                category: category.clone(),
                message: format!("Test {} {:?}", category, severity),
                severity: severity.clone(),
            };

            // Verify that suggestion was created with correct severity and category
            assert_eq!(suggestion.severity, *severity);
            assert_eq!(suggestion.category, *category);

            // Test category display
            let category_str = format!("{}", category);
            match category {
                SuggestionCategory::Complexity => assert_eq!(category_str, "Complexity"),
                SuggestionCategory::Structure => assert_eq!(category_str, "Structure"),
                SuggestionCategory::Naming => assert_eq!(category_str, "Naming"),
                SuggestionCategory::Organization => assert_eq!(category_str, "Organization"),
            }
        }
    }
}

#[test]
fn test_edge_case_empty_diagrams() {
    // Test with minimal/empty diagrams
    let empty_sankey = SankeyDiagram {
        nodes: vec![],
        links: vec![],
    };

    let empty_sequence = SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: vec![],
        statements: vec![],
        autonumber: None,
    };

    let empty_class = ClassDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        classes: HashMap::new(),
        relationships: vec![],
        notes: vec![],
    };

    let empty_flowchart = FlowchartDiagram {
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

    // Test that empty diagrams don't crash and return sensible metrics
    for diagram in [
        DiagramType::Sankey(empty_sankey),
        DiagramType::Sequence(empty_sequence),
        DiagramType::Class(empty_class),
        DiagramType::Flowchart(empty_flowchart),
    ] {
        let metrics = diagram.calculate_metrics();

        assert_eq!(metrics.basic.node_count, 0);
        assert_eq!(metrics.basic.edge_count, 0);
        assert!(metrics.complexity.cyclomatic >= 1); // Should have minimum complexity
        assert!(metrics.quality.maintainability > 0.0);
        assert!(metrics.quality.readability > 0.0);

        // Empty diagrams shouldn't trigger size-based suggestions
        assert!(
            metrics.suggestions.is_empty()
                || !metrics
                    .suggestions
                    .iter()
                    .any(|s| s.message.contains("Large"))
        );
    }
}
