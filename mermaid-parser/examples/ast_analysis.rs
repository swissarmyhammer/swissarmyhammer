//! AST analysis example for the mermaid-parser crate
//!
//! This example demonstrates how to analyze and traverse parsed AST structures
//! to extract meaningful information and perform various analyses.

use mermaid_parser::{
    parse_diagram, AstVisitor, ComplexityAnalyzer, DiagramMetrics, DiagramType, NodeCounter,
    ReferenceValidator,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let complex_flowchart = r#"
flowchart TD
    Start([Start Process]) --> InputValidation{Validate Input}
    InputValidation -->|Valid| ProcessData[Process Data]
    InputValidation -->|Invalid| ErrorHandler[Handle Error]
    
    ProcessData --> DatabaseCheck{Check Database}
    DatabaseCheck -->|Found| UpdateRecord[Update Record]
    DatabaseCheck -->|Not Found| CreateRecord[Create New Record]
    
    UpdateRecord --> AuditLog[Write Audit Log]
    CreateRecord --> AuditLog
    
    AuditLog --> SendNotification[Send Notification]
    SendNotification --> End([End Process])
    
    ErrorHandler --> LogError[Log Error]
    LogError --> SendAlert[Send Alert]
    SendAlert --> End
    
    subgraph DataProcessing
        ProcessData
        DatabaseCheck
        UpdateRecord
        CreateRecord
    end
    
    subgraph ErrorHandling
        ErrorHandler
        LogError
        SendAlert
    end
"#;

    let sankey_example = r#"
sankey-beta
    Source1,Intermediate1,100
    Source1,Intermediate2,50
    Source2,Intermediate1,80
    Source2,Intermediate3,120
    Intermediate1,Target1,90
    Intermediate1,Target2,90
    Intermediate2,Target1,30
    Intermediate2,Target3,20
    Intermediate3,Target2,60
    Intermediate3,Target3,60
"#;

    println!("AST Analysis Examples");
    println!("====================\n");

    // Analyze the complex flowchart
    println!("1. Analyzing Complex Flowchart");
    println!("------------------------------");

    match parse_diagram(complex_flowchart)? {
        DiagramType::Flowchart(flowchart) => {
            println!("Basic Statistics:");
            println!("  Nodes: {}", flowchart.nodes.len());
            println!("  Edges: {}", flowchart.edges.len());
            println!("  Subgraphs: {}", flowchart.subgraphs.len());

            // Use the NodeCounter visitor
            let mut node_counter = NodeCounter::new();
            node_counter.visit_flowchart(&flowchart);
            println!("  Nodes counted by visitor: {}", node_counter.nodes());
            println!("  Edges counted by visitor: {}", node_counter.edges());
            println!("  Total elements: {}", node_counter.elements());

            // Use the ComplexityAnalyzer visitor
            let mut complexity_analyzer = ComplexityAnalyzer::new();
            complexity_analyzer.visit_flowchart(&flowchart);
            println!("  Max nesting depth: {}", complexity_analyzer.max_depth());

            // Analyze node types
            let mut shape_counts = std::collections::HashMap::new();
            for node in flowchart.nodes.values() {
                *shape_counts.entry(format!("{:?}", node.shape)).or_insert(0) += 1;
            }

            println!("  Node shapes:");
            for (shape, count) in shape_counts {
                println!("    {}: {}", shape, count);
            }

            // Analyze edge types
            let mut edge_type_counts = std::collections::HashMap::new();
            for edge in &flowchart.edges {
                *edge_type_counts
                    .entry(format!("{:?}", edge.edge_type))
                    .or_insert(0) += 1;
            }

            println!("  Edge types:");
            for (edge_type, count) in edge_type_counts {
                println!("    {}: {}", edge_type, count);
            }

            // Find terminal nodes (nodes with no outgoing edges)
            let mut has_outgoing = std::collections::HashSet::new();
            for edge in &flowchart.edges {
                has_outgoing.insert(&edge.from);
            }

            let terminal_nodes: Vec<_> = flowchart
                .nodes
                .keys()
                .filter(|id| !has_outgoing.contains(id))
                .collect();

            println!("  Terminal nodes: {:?}", terminal_nodes);

            // Find entry nodes (nodes with no incoming edges)
            let mut has_incoming = std::collections::HashSet::new();
            for edge in &flowchart.edges {
                has_incoming.insert(&edge.to);
            }

            let entry_nodes: Vec<_> = flowchart
                .nodes
                .keys()
                .filter(|id| !has_incoming.contains(id))
                .collect();

            println!("  Entry nodes: {:?}", entry_nodes);

            // Use ReferenceValidator to check for broken references
            let mut validator = ReferenceValidator::new();
            validator.visit_flowchart(&flowchart);
            println!("  Reference validation completed");
        }
        _ => println!("Expected flowchart but got different diagram type"),
    }

    println!("\n{}\n", "=".repeat(60));

    // Analyze the Sankey diagram
    println!("2. Analyzing Sankey Diagram");
    println!("---------------------------");

    match parse_diagram(sankey_example)? {
        DiagramType::Sankey(sankey) => {
            println!("Basic Statistics:");
            println!("  Nodes: {}", sankey.nodes.len());
            println!("  Links: {}", sankey.links.len());

            // Calculate flow statistics
            let total_flow: f64 = sankey.links.iter().map(|link| link.value).sum();
            let avg_flow = total_flow / sankey.links.len() as f64;
            let max_flow = sankey
                .links
                .iter()
                .map(|link| link.value)
                .fold(0.0, f64::max);
            let min_flow = sankey
                .links
                .iter()
                .map(|link| link.value)
                .fold(f64::INFINITY, f64::min);

            println!("  Flow Statistics:");
            println!("    Total flow: {:.2}", total_flow);
            println!("    Average flow: {:.2}", avg_flow);
            println!("    Maximum flow: {:.2}", max_flow);
            println!("    Minimum flow: {:.2}", min_flow);

            // Analyze node connectivity
            let mut incoming_flow = std::collections::HashMap::new();
            let mut outgoing_flow = std::collections::HashMap::new();

            for link in &sankey.links {
                *outgoing_flow.entry(&link.source).or_insert(0.0) += link.value;
                *incoming_flow.entry(&link.target).or_insert(0.0) += link.value;
            }

            println!("  Node Analysis:");
            for node in &sankey.nodes {
                let incoming = incoming_flow.get(&node.id).unwrap_or(&0.0);
                let outgoing = outgoing_flow.get(&node.id).unwrap_or(&0.0);
                let balance = incoming - outgoing;

                println!(
                    "    {}: in={:.1}, out={:.1}, balance={:.1}",
                    node.name, incoming, outgoing, balance
                );
            }

            // Find source and sink nodes
            let sources: Vec<_> = sankey
                .nodes
                .iter()
                .filter(|node| !incoming_flow.contains_key(&node.id))
                .map(|node| &node.name)
                .collect();

            let sinks: Vec<_> = sankey
                .nodes
                .iter()
                .filter(|node| !outgoing_flow.contains_key(&node.id))
                .map(|node| &node.name)
                .collect();

            println!("  Sources: {:?}", sources);
            println!("  Sinks: {:?}", sinks);
        }
        _ => println!("Expected Sankey diagram but got different diagram type"),
    }

    println!("\n{}\n", "=".repeat(60));

    // Demonstrate metrics collection
    println!("3. Diagram Metrics Analysis");
    println!("---------------------------");

    let test_diagrams = vec![
        ("Simple Flowchart", r#"flowchart LR; A --> B --> C"#),
        ("Complex Flowchart", complex_flowchart),
        ("Sankey", sankey_example),
    ];

    for (name, diagram_source) in test_diagrams {
        println!("Analyzing: {}", name);

        match parse_diagram(diagram_source) {
            Ok(diagram) => {
                let metrics = diagram.calculate_metrics();

                println!("  Basic Metrics:");
                println!("    Nodes: {}", metrics.basic.node_count);
                println!("    Edges: {}", metrics.basic.edge_count);
                println!("    Depth: {}", metrics.basic.depth);
                println!("    Breadth: {}", metrics.basic.breadth);

                println!("  Complexity Metrics:");
                println!(
                    "    Cyclomatic Complexity: {}",
                    metrics.complexity.cyclomatic
                );
                println!(
                    "    Cognitive Complexity: {:.2}",
                    metrics.complexity.cognitive
                );
                println!("    Nesting Depth: {}", metrics.complexity.nesting_depth);
                println!("    Coupling: {:.2}", metrics.complexity.coupling);

                println!("  Quality Metrics:");
                println!(
                    "    Maintainability: {:.2}",
                    metrics.quality.maintainability
                );
                println!("    Readability: {:.2}", metrics.quality.readability);
                println!("    Modularity: {:.2}", metrics.quality.modularity);

                if !metrics.suggestions.is_empty() {
                    println!("  Suggestions:");
                    for suggestion in &metrics.suggestions {
                        println!("    - {:?}: {}", suggestion.category, suggestion.message);
                    }
                }
            }
            Err(e) => {
                println!("  Failed to parse: {}", e);
            }
        }

        println!();
    }

    Ok(())
}
