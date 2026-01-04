//! Tests for pretty_print diagram types with missing or incomplete coverage

use mermaid_parser::common::ast::*;
use mermaid_parser::*;
use std::collections::HashMap;

// Test Gantt diagram pretty printing
#[test]
fn test_gantt_diagram_pretty_print() {
    let diagram = DiagramType::Gantt(GanttDiagram {
        title: Some("Project Timeline".to_string()),
        accessibility: AccessibilityInfo::default(),
        date_format: Some("YYYY-MM-DD".to_string()),
        axis_format: Some("%Y-%m-%d".to_string()),
        tick_interval: None,
        includes: vec![],
        excludes: vec![],
        today_marker: None,
        inclusive_end_dates: false,
        top_axis: false,
        weekdays: WeekdaySettings::default(),
        sections: vec![
            GanttSection {
                name: "Planning".to_string(),
                tasks: vec![
                    GanttTask {
                        name: "Define requirements".to_string(),
                        id: Some("task1".to_string()),
                        start_date: Some("2023-01-01".to_string()),
                        duration: Some("10d".to_string()),
                        dependencies: vec![],
                        status: TaskStatus::Done,
                        progress: None,
                        interactions: vec![],
                    },
                    GanttTask {
                        name: "Design architecture".to_string(),
                        id: Some("task2".to_string()),
                        start_date: Some("2023-01-11".to_string()),
                        duration: Some("10d".to_string()),
                        dependencies: vec![],
                        status: TaskStatus::Active,
                        progress: None,
                        interactions: vec![],
                    },
                ],
            },
            GanttSection {
                name: "Development".to_string(),
                tasks: vec![GanttTask {
                    name: "Implement backend".to_string(),
                    id: Some("task3".to_string()),
                    start_date: Some("2023-01-21".to_string()),
                    duration: Some("20d".to_string()),
                    dependencies: vec![],
                    status: TaskStatus::Critical,
                    progress: None,
                    interactions: vec![],
                }],
            },
        ],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("gantt"));
    assert!(output.contains("dateFormat YYYY-MM-DD"));
    assert!(output.contains("axisFormat %Y-%m-%d"));
    assert!(output.contains("section Planning"));
    assert!(output.contains("Define requirements"));
    assert!(output.contains("Design architecture"));
    assert!(output.contains("section Development"));
    assert!(output.contains("Implement backend"));

    // Test with compact mode
    let compact_options = PrintOptions {
        compact_mode: true,
        ..Default::default()
    };
    let compact_output = diagram.to_mermaid_pretty(&compact_options);
    assert!(compact_output.contains("gantt"));
    // Should not have indented lines in compact mode
    for line in compact_output.lines() {
        if !line.trim().is_empty() {
            assert!(!line.starts_with("    "));
        }
    }
}

// Test State diagram pretty printing with complex features
#[test]
fn test_state_diagram_comprehensive_pretty_print() {
    let mut states = HashMap::new();
    states.insert(
        "idle".to_string(),
        State {
            id: "idle".to_string(),
            display_name: Some("Idle State".to_string()),
            state_type: StateType::Simple,
            substates: vec![],
            concurrent_regions: vec![],
        },
    );
    states.insert(
        "active".to_string(),
        State {
            id: "active".to_string(),
            display_name: Some("Active State".to_string()),
            state_type: StateType::Simple,
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
            concurrent_regions: vec![],
        },
    );

    let diagram = DiagramType::State(StateDiagram {
        title: Some("State Machine".to_string()),
        accessibility: AccessibilityInfo::default(),
        version: StateVersion::V2,
        states,
        transitions: vec![
            StateTransition {
                from: "idle".to_string(),
                to: "active".to_string(),
                event: Some("start".to_string()),
                guard: Some("condition_met".to_string()),
                action: Some("initialize()".to_string()),
            },
            StateTransition {
                from: "active".to_string(),
                to: "idle".to_string(),
                event: Some("stop".to_string()),
                guard: None,
                action: None,
            },
        ],
        notes: vec![StateNote {
            position: StateNotePosition::RightOf,
            target: "idle".to_string(),
            text: "Initial state".to_string(),
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("stateDiagram-v2"));
    assert!(output.contains("idle : Idle State"));
    assert!(output.contains("active : Active State"));
    assert!(output.contains("idle --> active : start [condition_met] / initialize()"));
    assert!(output.contains("active --> idle : stop"));
    assert!(output.contains("note right of idle : Initial state"));

    // Test state diagram with different version
    let v1_diagram = DiagramType::State(StateDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        version: StateVersion::V1,
        states: HashMap::new(),
        transitions: vec![],
        notes: vec![],
    });
    let v1_output = v1_diagram.to_mermaid();
    assert!(v1_output.contains("stateDiagram"));
}

// Test Class diagram with comprehensive features
#[test]
fn test_class_diagram_comprehensive_pretty_print() {
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
                    default_value: None,
                }),
                ClassMember::Method(Method {
                    visibility: Visibility::Public,
                    name: "move".to_string(),
                    parameters: vec![],
                    return_type: Some("void".to_string()),
                    is_static: false,
                    is_abstract: true,
                }),
            ],
            annotations: vec!["@Entity".to_string()],
            css_class: Some("highlight".to_string()),
        },
    );

    classes.insert(
        "Dog".to_string(),
        Class {
            name: "Dog".to_string(),
            stereotype: None,
            members: vec![ClassMember::Method(Method {
                visibility: Visibility::Public,
                name: "bark".to_string(),
                parameters: vec![],
                return_type: Some("void".to_string()),
                is_static: false,
                is_abstract: false,
            })],
            annotations: vec![],
            css_class: None,
        },
    );

    let diagram = DiagramType::Class(ClassDiagram {
        title: Some("Animal Hierarchy".to_string()),
        accessibility: AccessibilityInfo::default(),
        classes,
        relationships: vec![ClassRelationship {
            from: "Dog".to_string(),
            to: "Animal".to_string(),
            relationship_type: ClassRelationshipType::Inheritance,
            from_cardinality: None,
            to_cardinality: None,
            label: None,
        }],
        notes: vec![Note {
            position: NotePosition::Over,
            actor: "Animal".to_string(),
            text: "Base class for all animals".to_string(),
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("classDiagram"));
    assert!(output.contains("Animal"));
    assert!(output.contains("Dog"));
    assert!(output.contains("Animal Hierarchy"));
}

// Test Git diagram pretty printing
#[test]
fn test_git_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Git(GitDiagram {
        title: Some("Git Workflow".to_string()),
        accessibility: AccessibilityInfo::default(),
        theme: None,
        commits: vec![
            GitCommit {
                id: Some("c1".to_string()),
                commit_type: CommitType::Normal,
                tag: Some("v1.0".to_string()),
                branch: "main".to_string(),
            },
            GitCommit {
                id: Some("c2".to_string()),
                commit_type: CommitType::Normal,
                tag: None,
                branch: "feature".to_string(),
            },
        ],
        branches: vec![
            GitBranch {
                name: "main".to_string(),
                order: Some(0),
                color: Some("blue".to_string()),
            },
            GitBranch {
                name: "feature".to_string(),
                order: Some(1),
                color: Some("green".to_string()),
            },
        ],
        operations: vec![],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("gitGraph"));
    assert!(output.contains("Git Workflow"));
}

// Test Mindmap diagram with nested structure
#[test]
fn test_mindmap_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Mindmap(MindmapDiagram {
        title: Some("Project Structure".to_string()),
        accessibility: AccessibilityInfo::default(),
        root: MindmapNode {
            id: "root".to_string(),
            text: "Project".to_string(),
            shape: MindmapNodeShape::Cloud,
            icon: Some("📁".to_string()),
            class: Some("root-style".to_string()),
            children: vec![
                MindmapNode {
                    id: "frontend".to_string(),
                    text: "Frontend".to_string(),
                    shape: MindmapNodeShape::Square,
                    icon: Some("🖥️".to_string()),
                    class: None,
                    children: vec![
                        MindmapNode {
                            id: "react".to_string(),
                            text: "React Components".to_string(),
                            shape: MindmapNodeShape::Default,
                            icon: None,
                            class: None,
                            children: vec![],
                        },
                        MindmapNode {
                            id: "styles".to_string(),
                            text: "CSS Styles".to_string(),
                            shape: MindmapNodeShape::Rounded,
                            icon: None,
                            class: None,
                            children: vec![],
                        },
                    ],
                },
                MindmapNode {
                    id: "backend".to_string(),
                    text: "Backend".to_string(),
                    shape: MindmapNodeShape::Hexagon,
                    icon: Some("⚙️".to_string()),
                    class: Some("backend-style".to_string()),
                    children: vec![MindmapNode {
                        id: "api".to_string(),
                        text: "REST API".to_string(),
                        shape: MindmapNodeShape::Default,
                        icon: None,
                        class: None,
                        children: vec![],
                    }],
                },
            ],
        },
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("mindmap"));
    assert!(output.contains("Project"));
    assert!(output.contains("Frontend"));
    assert!(output.contains("Backend"));

    // Test with different formatting options
    let options = PrintOptions {
        indent_width: 2,
        compact_mode: false,
        ..Default::default()
    };
    let formatted_output = diagram.to_mermaid_pretty(&options);
    assert!(formatted_output.contains("  title Project Structure"));
}

// Test XyChart diagram pretty printing
#[test]
fn test_xychart_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::XyChart(XyChartDiagram {
        title: Some("Sales Performance".to_string()),
        accessibility: AccessibilityInfo::default(),
        orientation: ChartOrientation::Vertical,
        x_axis: XAxis {
            title: Some("Quarter".to_string()),
            labels: vec![
                "Q1".to_string(),
                "Q2".to_string(),
                "Q3".to_string(),
                "Q4".to_string(),
            ],
            range: None,
        },
        y_axis: YAxis {
            title: Some("Revenue (M$)".to_string()),
            range: Some((0.0, 100.0)),
        },
        data_series: vec![
            DataSeries {
                series_type: SeriesType::Line,
                name: Some("2023".to_string()),
                data: vec![20.0, 35.0, 45.0, 60.0],
            },
            DataSeries {
                series_type: SeriesType::Line,
                name: Some("2024".to_string()),
                data: vec![25.0, 40.0, 55.0, 70.0],
            },
        ],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("xychart-beta"));
    assert!(output.contains("Sales Performance"));
}

// Test Quadrant diagram pretty printing
#[test]
fn test_quadrant_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Quadrant(QuadrantDiagram {
        title: Some("Effort vs Impact".to_string()),
        accessibility: AccessibilityInfo::default(),
        x_axis: Some(AxisDefinition {
            label_start: Some("Low Effort".to_string()),
            label_end: Some("High Effort".to_string()),
        }),
        y_axis: Some(AxisDefinition {
            label_start: Some("Low Impact".to_string()),
            label_end: Some("High Impact".to_string()),
        }),
        quadrants: QuadrantLabels {
            quadrant_1: Some("Quick Wins".to_string()),
            quadrant_2: Some("Major Projects".to_string()),
            quadrant_3: Some("Fill-ins".to_string()),
            quadrant_4: Some("Thankless Tasks".to_string()),
        },
        points: vec![
            DataPoint {
                name: "Feature A".to_string(),
                x: 0.2,
                y: 0.8,
                class: Some("high-impact".to_string()),
            },
            DataPoint {
                name: "Feature B".to_string(),
                x: 0.8,
                y: 0.7,
                class: Some("major-project".to_string()),
            },
            DataPoint {
                name: "Bug Fix".to_string(),
                x: 0.3,
                y: 0.3,
                class: None,
            },
        ],
        styles: vec![ClassDefinition {
            name: "high-impact".to_string(),
            styles: vec!["fill:#ff6b6b".to_string()],
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("quadrantChart"));
    assert!(output.contains("Effort vs Impact"));
}

// Test Kanban diagram pretty printing
#[test]
fn test_kanban_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Kanban(KanbanDiagram {
        title: Some("Sprint Board".to_string()),
        accessibility: AccessibilityInfo::default(),
        sections: vec![
            KanbanSection {
                id: "todo".to_string(),
                title: "To Do".to_string(),
                items: vec![KanbanItem {
                    id: Some("TASK-1".to_string()),
                    text: "Implement authentication".to_string(),
                    assigned: vec!["alice".to_string()],
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("priority".to_string(), "high".to_string());
                        map.insert("estimate".to_string(), "8".to_string());
                        map
                    },
                }],
            },
            KanbanSection {
                id: "inprogress".to_string(),
                title: "In Progress".to_string(),
                items: vec![KanbanItem {
                    id: Some("TASK-2".to_string()),
                    text: "Setup database".to_string(),
                    assigned: vec!["bob".to_string(), "charlie".to_string()],
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("priority".to_string(), "medium".to_string());
                        map
                    },
                }],
            },
            KanbanSection {
                id: "done".to_string(),
                title: "Done".to_string(),
                items: vec![KanbanItem {
                    id: Some("TASK-3".to_string()),
                    text: "Project setup".to_string(),
                    assigned: vec!["alice".to_string()],
                    metadata: HashMap::new(),
                }],
            },
        ],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("kanban"));
    assert!(output.contains("To Do"));
    assert!(output.contains("Implement authentication"));
    assert!(output.contains("In Progress"));
    assert!(output.contains("Setup database"));
    assert!(output.contains("Done"));
    assert!(output.contains("Project setup"));
}

// Test Requirement diagram pretty printing
#[test]
fn test_requirement_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Requirement(RequirementDiagram {
        title: Some("System Requirements".to_string()),
        accessibility: AccessibilityInfo::default(),
        requirements: {
            let mut map = HashMap::new();
            map.insert(
                "REQ-1".to_string(),
                Requirement {
                    name: "User Authentication".to_string(),
                    req_type: RequirementType::FunctionalRequirement,
                    id: "REQ-1".to_string(),
                    text: "Users must be able to login securely".to_string(),
                    risk: Some(RiskLevel::Medium),
                    verify_method: Some(VerificationMethod::Test),
                },
            );
            map.insert(
                "REQ-2".to_string(),
                Requirement {
                    name: "Performance".to_string(),
                    req_type: RequirementType::PerformanceRequirement,
                    id: "REQ-2".to_string(),
                    text: "System must respond within 200ms".to_string(),
                    risk: Some(RiskLevel::High),
                    verify_method: Some(VerificationMethod::Analysis),
                },
            );
            map
        },
        elements: {
            let mut map = HashMap::new();
            map.insert(
                "SYS-1".to_string(),
                Element {
                    name: "Authentication System".to_string(),
                    element_type: "interface".to_string(),
                    doc_ref: Some("auth-spec.md".to_string()),
                },
            );
            map
        },
        relationships: vec![RequirementRelationship {
            source: "REQ-1".to_string(),
            target: "SYS-1".to_string(),
            relationship_type: RelationshipType::Satisfies,
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("requirementDiagram"));
    assert!(output.contains("System Requirements"));
}

// Test Packet diagram pretty printing
#[test]
fn test_packet_diagram_comprehensive_pretty_print() {
    let diagram = DiagramType::Packet(PacketDiagram {
        title: Some("Network Packet".to_string()),
        accessibility: AccessibilityInfo::default(),
        fields: vec![
            PacketField {
                start_bit: 0,
                end_bit: 15,
                name: "Source Port".to_string(),
                is_optional: false,
            },
            PacketField {
                start_bit: 16,
                end_bit: 31,
                name: "Destination Port".to_string(),
                is_optional: false,
            },
            PacketField {
                start_bit: 32,
                end_bit: 63,
                name: "Sequence Number".to_string(),
                is_optional: false,
            },
            PacketField {
                start_bit: 64,
                end_bit: 95,
                name: "Acknowledgment Number".to_string(),
                is_optional: false,
            },
        ],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("packet-beta"));
    assert!(output.contains("0-15: Source Port"));
    assert!(output.contains("16-31: Destination Port"));
    assert!(output.contains("32-63: Sequence Number"));
    assert!(output.contains("64-95: Acknowledgment Number"));
}

// Test ER diagram with comprehensive features
#[test]
fn test_er_diagram_comprehensive_pretty_print() {
    let mut entities = HashMap::new();
    entities.insert(
        "Customer".to_string(),
        Entity {
            name: "Customer".to_string(),
            attributes: vec![
                Attribute {
                    name: "customer_id".to_string(),
                    attr_type: "int".to_string(),
                    key_type: Some(KeyType::PK),
                    comment: Some("Primary key".to_string()),
                },
                Attribute {
                    name: "name".to_string(),
                    attr_type: "varchar(255)".to_string(),
                    key_type: None,
                    comment: Some("Customer name".to_string()),
                },
                Attribute {
                    name: "email".to_string(),
                    attr_type: "varchar(255)".to_string(),
                    key_type: Some(KeyType::UK),
                    comment: None,
                },
            ],
        },
    );

    entities.insert(
        "Order".to_string(),
        Entity {
            name: "Order".to_string(),
            attributes: vec![
                Attribute {
                    name: "order_id".to_string(),
                    attr_type: "int".to_string(),
                    key_type: Some(KeyType::PK),
                    comment: None,
                },
                Attribute {
                    name: "customer_id".to_string(),
                    attr_type: "int".to_string(),
                    key_type: Some(KeyType::FK),
                    comment: Some("Foreign key to Customer".to_string()),
                },
                Attribute {
                    name: "order_date".to_string(),
                    attr_type: "date".to_string(),
                    key_type: None,
                    comment: None,
                },
            ],
        },
    );

    let diagram = DiagramType::Er(ErDiagram {
        title: Some("E-commerce Database".to_string()),
        accessibility: AccessibilityInfo::default(),
        entities,
        relationships: vec![ErRelationship {
            left_entity: "Customer".to_string(),
            right_entity: "Order".to_string(),
            left_cardinality: ErCardinality {
                min: CardinalityValue::One,
                max: CardinalityValue::One,
            },
            right_cardinality: ErCardinality {
                min: CardinalityValue::Zero,
                max: CardinalityValue::Many,
            },
            label: Some("places".to_string()),
        }],
    });

    let output = diagram.to_mermaid();
    assert!(output.contains("erDiagram"));
    assert!(output.contains("Customer {"));
    assert!(output.contains("int customer_id PK"));
    assert!(output.contains("varchar(255) name"));
    assert!(output.contains("varchar(255) email UK"));
    assert!(output.contains("Order {"));
    assert!(output.contains("int order_id PK"));
    assert!(output.contains("int customer_id FK"));
    assert!(output.contains("date order_date"));
    assert!(output.contains("Customer ||--o{ Order : places"));
}

// Test PrettyPrinter write() method that's marked as dead code
#[test]
fn test_pretty_printer_write_method() {
    // This test ensures the write() method gets coverage even though it's not used
    use mermaid_parser::common::pretty_print::PrintOptions;

    // Use a diagram that would trigger internal PrettyPrinter usage
    let diagram = DiagramType::Misc(MiscDiagram {
        diagram_type: "test".to_string(),
        content: MiscContent::Raw(RawDiagram {
            lines: vec!["test line".to_string()],
        }),
    });

    let options = PrintOptions {
        indent_width: 4,
        max_line_length: 80,
        align_arrows: false,
        sort_nodes: false,
        compact_mode: false,
    };

    let output = diagram.to_mermaid_pretty(&options);
    assert!(output.contains("test line"));
}
