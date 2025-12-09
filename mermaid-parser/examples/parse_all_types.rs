//! Parse multiple diagram types example for the mermaid-parser crate
//!
//! This example demonstrates parsing different types of Mermaid diagrams
//! and extracting specific information from each type.

use mermaid_parser::{parse_diagram, DiagramType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let examples = vec![
        (
            "Simple Flowchart",
            r#"
flowchart LR
    A[Start] --> B{Decision}
    B -->|Yes| C[Process]
    B -->|No| D[End]
    C --> D
"#,
        ),
        (
            "Sankey Flow",
            r#"
sankey-beta
    Agriculture,Manufacturing,5
    Manufacturing,Services,3
    Agriculture,Services,2
    Services,Export,4
"#,
        ),
        (
            "Timeline",
            r#"
timeline
    title Development Timeline
    2023    : Planning
            : Design
    2024    : Implementation
            : Testing
            : Release
"#,
        ),
        (
            "State Machine",
            r#"
stateDiagram-v2
    [*] --> Idle
    Idle --> Active : start
    Active --> Processing : process
    Processing --> Active : continue
    Processing --> [*] : finish
    Active --> Idle : stop
"#,
        ),
        (
            "User Journey",
            r#"
journey
    title My working day
    section Go to work
      Make tea: 5: Me
      Go upstairs: 3: Me
      Do work: 1: Me, Cat
    section Go home
      Go downstairs: 5: Me
      Sit down: 5: Me
"#,
        ),
    ];

    println!("Parsing Multiple Diagram Types");
    println!("==============================\n");

    for (name, diagram_source) in examples {
        println!("Processing: {}", name);
        println!("Source:");
        println!("{}", diagram_source.trim());

        match parse_diagram(diagram_source) {
            Ok(diagram) => {
                match diagram {
                    DiagramType::Flowchart(flowchart) => {
                        println!("✓ Flowchart parsed successfully!");
                        println!("  Direction: {:?}", flowchart.direction);
                        println!("  Nodes: {}", flowchart.nodes.len());
                        println!("  Edges: {}", flowchart.edges.len());
                        println!("  Subgraphs: {}", flowchart.subgraphs.len());

                        // List first few nodes
                        for (node_count, (id, node)) in flowchart.nodes.iter().enumerate() {
                            if node_count >= 3 {
                                break;
                            }
                            println!("    Node '{}': {:?}", id, node.text);
                        }
                    }
                    DiagramType::Sankey(sankey) => {
                        println!("✓ Sankey diagram parsed successfully!");
                        println!("  Nodes: {}", sankey.nodes.len());
                        println!("  Links: {}", sankey.links.len());

                        // Calculate total flow
                        let total_flow: f64 = sankey.links.iter().map(|link| link.value).sum();
                        println!("  Total flow value: {}", total_flow);

                        // List nodes
                        for node in &sankey.nodes {
                            println!("    Node: '{}' ({})", node.id, node.name);
                        }
                    }
                    DiagramType::Timeline(timeline) => {
                        println!("✓ Timeline parsed successfully!");
                        if let Some(title) = &timeline.title {
                            println!("  Title: {}", title);
                        }
                        println!("  Sections: {}", timeline.sections.len());

                        for section in &timeline.sections {
                            println!(
                                "    Section '{}': {} items",
                                section.name,
                                section.items.len()
                            );
                        }
                    }
                    DiagramType::State(state) => {
                        println!("✓ State diagram parsed successfully!");
                        println!("  States: {}", state.states.len());
                        println!("  Transitions: {}", state.transitions.len());

                        // List first few states
                        for (state_count, (id, state_def)) in state.states.iter().enumerate() {
                            if state_count >= 3 {
                                break;
                            }
                            println!("    State: '{}' (type: {:?})", id, state_def.state_type);
                        }
                    }
                    DiagramType::Journey(journey) => {
                        println!("✓ Journey diagram parsed successfully!");
                        if let Some(title) = &journey.title {
                            println!("  Title: {}", title);
                        }
                        println!("  Sections: {}", journey.sections.len());

                        for section in &journey.sections {
                            println!(
                                "    Section '{}': {} tasks",
                                section.name,
                                section.tasks.len()
                            );
                            for task in &section.tasks {
                                println!(
                                    "      Task '{}': score {}, actors: {:?}",
                                    task.name, task.score, task.actors
                                );
                            }
                        }
                    }
                    other => {
                        println!("✓ Parsed as: {:?}", other);
                    }
                }
            }
            Err(e) => {
                println!("✗ Parse error: {}", e);
            }
        }

        println!("{}\n", "=".repeat(60));
    }

    Ok(())
}
