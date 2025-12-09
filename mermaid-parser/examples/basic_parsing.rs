//! Basic parsing example for the mermaid-parser crate
//!
//! This example demonstrates the fundamental usage of the parser by parsing a simple diagram.

use mermaid_parser::{parse_diagram, DiagramType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple flowchart example
    let diagram_content = r#"
flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Process A]
    B -->|No| D[Process B]
    C --> E[End]
    D --> E[End]
"#;

    // Parse the diagram
    println!("Parsing diagram...");
    match parse_diagram(diagram_content)? {
        DiagramType::Flowchart(flowchart) => {
            println!("Successfully parsed a flowchart!");
            println!("Number of nodes: {}", flowchart.nodes.len());
            println!("Number of edges: {}", flowchart.edges.len());

            // Print node information
            println!("\nNodes:");
            for (id, node) in &flowchart.nodes {
                println!("  - {}: {:?}", id, node.text);
            }

            // Print edge information
            println!("\nEdges:");
            for edge in &flowchart.edges {
                println!("  - {} -> {}", edge.from, edge.to);
                if let Some(label) = &edge.label {
                    println!("    Label: {}", label);
                }
            }
        }
        other => {
            println!("Parsed diagram type: {:?}", other);
        }
    }

    Ok(())
}
