//! Minimal tests targeting specific missing coverage areas in visitor.rs

use mermaid_parser::common::ast::*;
use mermaid_parser::common::visitor::*;

#[cfg(test)]
mod visitor_minimal_coverage_tests {
    use super::*;

    // Test TitleSetter on misc diagram (should do nothing but exercise the code path)
    #[test]
    fn test_title_setter_misc_diagram() {
        let mut diagram = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["test".to_string()],
            }),
            diagram_type: "info".to_string(),
        };

        let mut setter = TitleSetter::new("Test Title".to_string());
        setter.visit_misc_mut(&mut diagram);

        // Misc diagrams don't have titles, but this tests the method doesn't crash
    }

    // Test TitleSetter on sankey diagram (should do nothing but exercise the code path)
    #[test]
    fn test_title_setter_sankey_diagram() {
        let mut diagram = SankeyDiagram {
            nodes: vec![],
            links: vec![],
        };

        let mut setter = TitleSetter::new("Test Title".to_string());
        setter.visit_sankey_mut(&mut diagram);

        // Sankey diagrams don't have titles, but this tests the method doesn't crash
    }

    // Test ComplexityAnalyzer with basic diagrams to hit branches
    #[test]
    fn test_complexity_analyzer_basic_branches() {
        let mut analyzer = ComplexityAnalyzer::new();

        // Test timeline
        let timeline = TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        analyzer.visit_timeline(&timeline);

        // Test journey
        let journey = JourneyDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        analyzer.visit_journey(&journey);

        // Test misc
        let misc = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["test".to_string()],
            }),
            diagram_type: "info".to_string(),
        };
        analyzer.visit_misc(&misc);

        // Verify initial state is maintained
        assert_eq!(analyzer.cyclomatic_complexity(), 1);
    }

    // Test ReferenceValidator default implementations (empty methods)
    #[test]
    fn test_reference_validator_default_implementations() {
        let mut validator = ReferenceValidator::new();

        // Test sankey
        let sankey = SankeyDiagram {
            nodes: vec![],
            links: vec![],
        };
        validator.visit_sankey(&sankey);

        // Test timeline
        let timeline = TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        validator.visit_timeline(&timeline);

        // Test journey
        let journey = JourneyDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };
        validator.visit_journey(&journey);

        // Test sequence
        let sequence = SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![],
            statements: vec![],
            autonumber: None,
        };
        validator.visit_sequence(&sequence);

        // Test misc
        let misc = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["test".to_string()],
            }),
            diagram_type: "info".to_string(),
        };
        validator.visit_misc(&misc);

        // Individual visitor methods
        let sankey_node = SankeyNode {
            id: "test".to_string(),
            name: "Test".to_string(),
        };
        validator.visit_sankey_node(&sankey_node);

        let sankey_link = SankeyLink {
            source: "A".to_string(),
            target: "B".to_string(),
            value: 10.0,
        };
        validator.visit_sankey_link(&sankey_link);

        let flow_node = FlowNode {
            id: "test".to_string(),
            text: Some("Test".to_string()),
            shape: NodeShape::Rectangle,
            classes: vec![],
            icon: None,
        };
        validator.visit_flow_node(&flow_node);

        let flow_edge = FlowEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            edge_type: EdgeType::Arrow,
            label: None,
            min_length: None,
        };
        validator.visit_flow_edge(&flow_edge);

        let message = Message {
            from: "A".to_string(),
            to: "B".to_string(),
            text: "Test".to_string(),
            arrow_type: ArrowType::SolidOpen,
        };
        validator.visit_sequence_message(&message);

        let class = Class {
            name: "Test".to_string(),
            stereotype: None,
            members: vec![],
            annotations: vec![],
            css_class: None,
        };
        validator.visit_class_definition(&class);

        // All these should execute without errors
        assert!(!validator.has_errors());
    }

    // Test ComplexityAnalyzer average_branching_factor edge case
    #[test]
    fn test_complexity_analyzer_branching_factor_zero_connections() {
        let analyzer = ComplexityAnalyzer::new();
        assert_eq!(analyzer.average_branching_factor(), 0.0);
    }

    // Test ComplexityAnalyzer cyclomatic_complexity edge case
    #[test]
    fn test_complexity_analyzer_cyclomatic_complexity_zero_connections() {
        let analyzer = ComplexityAnalyzer::new();
        assert_eq!(analyzer.cyclomatic_complexity(), 1); // Minimum complexity
    }

    // Test TitleSetter set_title method functionality
    #[test]
    fn test_title_setter_set_title_functionality() {
        let mut timeline = TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        };

        let mut setter = TitleSetter::new("New Title".to_string());
        setter.visit_timeline_mut(&mut timeline);

        assert_eq!(timeline.title, Some("New Title".to_string()));
    }

    // Test NodeCounter with timeline items
    #[test]
    fn test_node_counter_timeline_with_items() {
        let mut counter = NodeCounter::new();

        let timeline = TimelineDiagram {
            title: Some("Test".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![TimelineSection {
                name: "Phase 1".to_string(),
                items: vec![
                    TimelineItem::Event("Event 1".to_string()),
                    TimelineItem::Period("2024".to_string()),
                ],
            }],
        };
        counter.visit_timeline(&timeline);
        assert_eq!(counter.elements(), 2);
    }

    // Test NodeCounter with journey tasks
    #[test]
    fn test_node_counter_journey_with_tasks() {
        let mut counter = NodeCounter::new();

        let journey = JourneyDiagram {
            title: Some("Journey".to_string()),
            accessibility: AccessibilityInfo::default(),
            sections: vec![JourneySection {
                name: "Section".to_string(),
                tasks: vec![JourneyTask {
                    name: "Task 1".to_string(),
                    score: 5,
                    actors: vec!["User".to_string()],
                }],
            }],
        };
        counter.visit_journey(&journey);
        assert_eq!(counter.elements(), 1);
    }

    // Test NodeCounter misc
    #[test]
    fn test_node_counter_misc() {
        let mut counter = NodeCounter::new();

        let misc = MiscDiagram {
            content: MiscContent::Raw(RawDiagram {
                lines: vec!["test".to_string()],
            }),
            diagram_type: "info".to_string(),
        };
        counter.visit_misc(&misc);
        assert_eq!(counter.elements(), 1);
    }
}
