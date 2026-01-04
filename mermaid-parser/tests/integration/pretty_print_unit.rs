//! Unit tests for pretty_print module focusing on internal functionality

#[cfg(test)]
mod tests {
    use mermaid_parser::*;

    // Test the internal PrettyPrinter struct indentation logic
    #[test]
    fn test_pretty_printer_indentation() {
        let options = PrintOptions {
            indent_width: 4,
            max_line_length: 80,
            align_arrows: false,
            sort_nodes: false,
            compact_mode: false,
        };

        // Test by pretty printing a simple flowchart
        let input = "flowchart TD\nA[Start] --> B[End]";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid_pretty(&options);

        // Check that indentation is applied
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines[0].starts_with("flowchart TD"));
        assert!(lines[1].starts_with("    ")); // Should be indented
    }

    #[test]
    fn test_pretty_printer_compact_mode() {
        let compact_options = PrintOptions {
            indent_width: 4,
            max_line_length: 80,
            align_arrows: false,
            sort_nodes: false,
            compact_mode: true,
        };

        let input = "flowchart TD\nA[Start] --> B[End]";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid_pretty(&compact_options);

        // In compact mode, there should be no indentation
        let lines: Vec<&str> = output.lines().collect();
        for line in lines {
            assert!(
                !line.starts_with("    "),
                "Line should not be indented in compact mode: {}",
                line
            );
        }
    }

    #[test]
    fn test_all_diagram_types_to_mermaid() {
        // Test that all diagram types implement to_mermaid
        let test_cases = vec![
            ("flowchart TD\nA --> B", "flowchart"),
            ("sequenceDiagram\nA->>B: Hi", "sequenceDiagram"),
            ("classDiagram\nclass Animal", "classDiagram"),
            ("stateDiagram-v2\n[*] --> Active", "stateDiagram-v2"),
            ("erDiagram\nCUSTOMER ||--o{ ORDER : places", "erDiagram"),
            ("gantt\ntitle Test\nsection A\nTask 1 :a1, 2023-01-01, 1d", "gantt"),
            ("pie\n\"A\" : 10\n\"B\" : 20", "pie"),
            ("mindmap\nroot((Root))\n  Branch", "mindmap"),
            ("gitGraph\ncommit", "gitGraph"),
            ("journey\ntitle Test\nsection A\nStep: 5: Me", "journey"),
            ("C4Context\ntitle Test", "C4Context"),
            ("sankey-beta\nA,B,10", "sankey-beta"),
            ("quadrantChart\ntitle Test\nA: [0.1, 0.2]", "quadrantChart"),
            ("xychart-beta\ntitle \"Test\"\nx-axis [a, b]\ny-axis 0 --> 10\nbar [5, 7]", "xychart-beta"),
            ("kanban\nTo Do", "kanban"),
            ("block-beta\ncolumns 1\na", "block-beta"),
            ("architecture-beta\ngroup api", "architecture-beta"),
            ("packet-beta\n0-7: Field", "packet-beta"),
            ("requirementDiagram\nrequirement req1 {\nid: 1\ntext: Test\nrisk: low\nverifymethod: test\n}", "requirementDiagram"),
            ("treemap\nA[Root]\n  B[Child]", "treemap"),
            ("radar\ntitle Test\nA\nB\nData [1, 2]", "radar"),
            ("timeline\ntitle Test\nsection A\n2023 : Event", "timeline"),
        ];

        for (input, expected_type) in test_cases {
            let diagram =
                parse_diagram(input).unwrap_or_else(|_| panic!("Failed to parse: {}", input));
            let output = diagram.to_mermaid();
            assert!(
                output.contains(expected_type),
                "Output for {} should contain '{}', but got: {}",
                input,
                expected_type,
                output
            );
        }
    }

    #[test]
    fn test_flowchart_node_shapes_formatting() {
        let input = "flowchart TD\nA[Rectangle]\nB(Round)\nC{Diamond}\nD((Circle))\nE(((Triple)))\nF[[Subroutine]]\nG{{Hexagon}}";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        // Check all shapes are preserved
        assert!(output.contains("A[Rectangle]"));
        assert!(output.contains("B(Round)"));
        assert!(output.contains("C{Diamond}"));
        assert!(output.contains("D((Circle)))"));
        assert!(output.contains("E(((Triple)))"));
        assert!(output.contains("F[[Subroutine]]"));
        assert!(output.contains("G{{Hexagon}}"));
    }

    #[test]
    fn test_sequence_participant_types() {
        let input = "sequenceDiagram\nparticipant A\nactor B\nA->>B: Hello";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("participant A"));
        assert!(output.contains("actor B"));
    }

    #[test]
    fn test_sequence_arrow_types_formatting() {
        let input = "sequenceDiagram\nA->>B: Arrow\nA-->>B: Dotted\nA-xB: Cross\nA-)B: Point";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("A ->> B: Arrow"));
        assert!(output.contains("A -->> B: Dotted"));
        assert!(output.contains("A -x B: Cross"));
        assert!(output.contains("A -) B: Point"));
    }

    #[test]
    fn test_print_options_debug() {
        let options = PrintOptions::default();
        let debug_str = format!("{:?}", options);
        assert!(debug_str.contains("PrintOptions"));
        assert!(debug_str.contains("indent_width"));
        assert!(debug_str.contains("max_line_length"));
    }

    #[test]
    fn test_diagram_type_to_mermaid_delegates() {
        // Test that DiagramType's to_mermaid delegates to to_mermaid_pretty with default options
        let input = "pie\n\"A\" : 50\n\"B\" : 50";
        let diagram = parse_diagram(input).expect("Failed to parse");

        let default_output = diagram.to_mermaid();
        let pretty_output = diagram.to_mermaid_pretty(&PrintOptions::default());

        assert_eq!(default_output, pretty_output);
    }

    #[test]
    fn test_empty_diagrams() {
        // Test that empty diagrams still produce valid output
        let test_cases = vec![
            "flowchart TD",
            "sequenceDiagram",
            "classDiagram",
            "stateDiagram-v2",
            "erDiagram",
            "pie",
            "mindmap\nroot((Root))",
            "gitGraph",
            "kanban",
        ];

        for input in test_cases {
            let diagram =
                parse_diagram(input).unwrap_or_else(|_| panic!("Failed to parse: {}", input));
            let output = diagram.to_mermaid();
            assert!(
                !output.is_empty(),
                "Empty diagram should still produce output: {}",
                input
            );
        }
    }

    #[test]
    fn test_timeline_sections_and_items() {
        let input = r#"timeline
    title Test Timeline
    section First
        2020 : Event A
        2021 : Event B
    section Second
        2022 : Event C"#;

        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("title Test Timeline"));
        assert!(output.contains("section First"));
        assert!(output.contains("2020 : Event A"));
        assert!(output.contains("section Second"));
    }

    #[test]
    fn test_mindmap_indentation() {
        let input = r#"mindmap
    root((Central))
        Branch1
            Leaf1
            Leaf2
        Branch2
            Leaf3"#;

        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("root((Central))"));
        assert!(output.contains("Branch1"));
        assert!(output.contains("Leaf1"));
    }

    #[test]
    fn test_gitgraph_operations() {
        let input = r#"gitGraph
    commit
    branch develop
    checkout develop
    commit
    checkout main
    merge develop
    commit"#;

        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("commit"));
        assert!(output.contains("branch develop"));
        assert!(output.contains("checkout"));
        assert!(output.contains("merge"));
    }

    #[test]
    fn test_state_diagram_transitions() {
        let input = r#"stateDiagram-v2
    [*] --> State1
    State1 --> State2 : Transition
    State2 --> State3
    State3 --> [*]"#;

        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("[*] --> State1"));
        assert!(output.contains("State1 --> State2 : Transition"));
        assert!(output.contains("State3 --> [*]"));
    }

    #[test]
    fn test_er_diagram_relationships() {
        let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name
        string address
    }
    ORDER {
        int orderId
        date orderDate
    }"#;

        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid();

        assert!(output.contains("CUSTOMER ||--o{ ORDER : places"));
        assert!(output.contains("CUSTOMER {"));
        assert!(output.contains("string name"));
        assert!(output.contains("ORDER {"));
    }

    #[test]
    fn test_sorted_nodes_option() {
        let options = PrintOptions {
            indent_width: 4,
            max_line_length: 80,
            align_arrows: false,
            sort_nodes: true,
            compact_mode: false,
        };

        let input = "flowchart TD\nC[Node C]\nA[Node A]\nB[Node B]";
        let diagram = parse_diagram(input).expect("Failed to parse");
        let output = diagram.to_mermaid_pretty(&options);

        // When sorted, nodes should appear in alphabetical order
        let a_pos = output.find("A[Node A]").expect("Should find Node A");
        let b_pos = output.find("B[Node B]").expect("Should find Node B");
        let c_pos = output.find("C[Node C]").expect("Should find Node C");

        assert!(a_pos < b_pos, "Node A should come before Node B");
        assert!(b_pos < c_pos, "Node B should come before Node C");
    }
}
