//! Batch processing example for the mermaid-parser crate
//!
//! This example demonstrates how to process multiple Mermaid diagrams
//! in batch, collect statistics, and generate reports.

use mermaid_parser::{parse_diagram, DiagramMetrics, DiagramType};
use std::collections::HashMap;

#[derive(Default, Debug)]
struct BatchStats {
    total_files: usize,
    successful_parses: usize,
    failed_parses: usize,
    diagram_types: HashMap<String, usize>,
    total_nodes: usize,
    total_edges: usize,
    errors: Vec<String>,
}

impl BatchStats {
    fn new() -> Self {
        Self::default()
    }

    fn add_success(&mut self, diagram: &DiagramType) {
        self.successful_parses += 1;

        let (type_name, nodes, edges) = match diagram {
            DiagramType::Flowchart(d) => ("Flowchart", d.nodes.len(), d.edges.len()),
            DiagramType::Sankey(d) => ("Sankey", d.nodes.len(), d.links.len()),
            DiagramType::Sequence(d) => ("Sequence", d.participants.len(), d.statements.len()),
            DiagramType::State(d) => ("State", d.states.len(), d.transitions.len()),
            DiagramType::Timeline(d) => ("Timeline", d.sections.len(), 0),
            DiagramType::Journey(d) => ("Journey", d.sections.len(), 0),
            DiagramType::Class(d) => ("Class", d.classes.len(), d.relationships.len()),
            DiagramType::Er(d) => ("ER", d.entities.len(), d.relationships.len()),
            DiagramType::Gantt(d) => ("Gantt", d.sections.len(), 0),
            DiagramType::Pie(d) => ("Pie", d.data.len(), 0),
            DiagramType::Git(d) => ("Git", d.commits.len(), d.branches.len()),
            DiagramType::C4(d) => ("C4", d.elements.len(), d.relationships.len()),
            DiagramType::Mindmap(_d) => ("Mindmap", 1, 0), // Has root node
            DiagramType::Quadrant(d) => ("Quadrant", d.points.len(), 0),
            DiagramType::XyChart(d) => ("XyChart", d.data_series.len(), 0),
            DiagramType::Kanban(d) => ("Kanban", d.sections.len(), 0),
            DiagramType::Block(d) => ("Block", d.blocks.len(), d.connections.len()),
            DiagramType::Architecture(d) => ("Architecture", d.groups.len(), d.services.len()),
            DiagramType::Packet(d) => ("Packet", d.fields.len(), 0),
            DiagramType::Requirement(d) => {
                ("Requirement", d.requirements.len(), d.relationships.len())
            }
            DiagramType::Treemap(_d) => ("Treemap", 1, 0), // Has root node
            DiagramType::Radar(d) => ("Radar", d.datasets.len(), 0),
            DiagramType::Misc(_) => ("Misc", 0, 0),
        };

        *self.diagram_types.entry(type_name.to_string()).or_insert(0) += 1;
        self.total_nodes += nodes;
        self.total_edges += edges;
    }

    fn add_failure(&mut self, error: String) {
        self.failed_parses += 1;
        self.errors.push(error);
    }

    fn print_report(&self) {
        println!("Batch Processing Report");
        println!("======================");
        println!("Total files processed: {}", self.total_files);
        println!("Successful parses: {}", self.successful_parses);
        println!("Failed parses: {}", self.failed_parses);
        println!(
            "Success rate: {:.1}%",
            (self.successful_parses as f64 / self.total_files as f64) * 100.0
        );

        println!("\nDiagram Type Distribution:");
        for (diagram_type, count) in &self.diagram_types {
            let percentage = (*count as f64 / self.successful_parses as f64) * 100.0;
            println!("  {}: {} ({:.1}%)", diagram_type, count, percentage);
        }

        println!("\nAggregate Statistics:");
        println!("  Total nodes: {}", self.total_nodes);
        println!("  Total edges: {}", self.total_edges);

        if self.successful_parses > 0 {
            println!(
                "  Average nodes per diagram: {:.1}",
                self.total_nodes as f64 / self.successful_parses as f64
            );
            println!(
                "  Average edges per diagram: {:.1}",
                self.total_edges as f64 / self.successful_parses as f64
            );
        }

        if !self.errors.is_empty() {
            println!("\nErrors encountered:");
            for (i, error) in self.errors.iter().enumerate() {
                if i < 5 {
                    // Limit to first 5 errors
                    println!("  {}: {}", i + 1, error);
                }
            }
            if self.errors.len() > 5 {
                println!("  ... and {} more errors", self.errors.len() - 5);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simulate batch processing with various diagram examples
    let test_diagrams = vec![
        (
            "flowchart_simple.mmd",
            r#"
flowchart TD
    A[Start] --> B[Process]
    B --> C[End]
"#,
        ),
        (
            "flowchart_complex.mmd",
            r#"
flowchart LR
    subgraph Frontend
        A[User Interface]
        B[Form Validation]
    end
    
    subgraph Backend
        C[API Gateway]
        D[Business Logic]
        E[Database]
    end
    
    A --> B
    B --> C
    C --> D
    D --> E
    E --> D
    D --> C
    C --> A
"#,
        ),
        (
            "sankey.mmd",
            r#"
sankey-beta
    Source1,Processing,100
    Source2,Processing,80
    Processing,Output1,120
    Processing,Output2,60
"#,
        ),
        (
            "sequence.mmd",
            r#"
sequenceDiagram
    participant A as Alice
    participant B as Bob
    participant C as Charlie
    
    A->>B: Authentication Request
    B->>C: Forward Request
    C-->>B: Authentication Response
    B-->>A: Response
"#,
        ),
        (
            "state.mmd",
            r#"
stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : start
    Processing --> Complete : finish
    Processing --> Error : fail
    Complete --> [*]
    Error --> Idle : retry
    Error --> [*] : abort
"#,
        ),
        (
            "timeline.mmd",
            r#"
timeline
    title Project Timeline
    2023 Q1 : Planning
            : Requirements
    2023 Q2 : Development
            : Testing
    2023 Q3 : Deployment
            : Monitoring
"#,
        ),
        (
            "invalid.mmd",
            r#"
invalid-diagram-type
    This is not a valid diagram
    And should cause a parse error
"#,
        ),
        (
            "malformed.mmd",
            r#"
flowchart TD
    A --> 
    --> B
    C <-- D
"#,
        ),
    ];

    println!("Processing {} diagram files...\n", test_diagrams.len());

    let mut stats = BatchStats::new();
    stats.total_files = test_diagrams.len();

    // Process each diagram
    for (filename, content) in &test_diagrams {
        println!("Processing: {}", filename);

        match parse_diagram(content) {
            Ok(diagram) => {
                let type_name = match &diagram {
                    DiagramType::Flowchart(_) => "Flowchart",
                    DiagramType::Sankey(_) => "Sankey",
                    DiagramType::Sequence(_) => "Sequence",
                    DiagramType::State(_) => "State",
                    DiagramType::Timeline(_) => "Timeline",
                    DiagramType::Journey(_) => "Journey",
                    _ => "Other",
                };

                println!("  ✓ Successfully parsed as {}", type_name);

                // Calculate and display metrics for each diagram
                let metrics = diagram.calculate_metrics();
                println!(
                    "    Nodes: {}, Edges: {}, Complexity: {}",
                    metrics.basic.node_count,
                    metrics.basic.edge_count,
                    metrics.complexity.cyclomatic
                );

                stats.add_success(&diagram);
            }
            Err(e) => {
                println!("  ✗ Parse failed: {}", e);
                stats.add_failure(format!("{}: {}", filename, e));
            }
        }

        println!();
    }

    println!("{}\n", "=".repeat(60));

    // Print comprehensive batch statistics
    stats.print_report();

    println!("\n{}\n", "=".repeat(60));

    // Demonstrate parallel processing concept
    println!("Parallel Processing Simulation");
    println!("==============================");

    let large_batch = [
        r#"flowchart TD; A --> B"#,
        r#"sequenceDiagram; A->>B: msg"#,
        r#"sankey-beta; A,B,10"#,
        r#"stateDiagram-v2; [*] --> A"#,
        r#"pie title Test; "A": 30; "B": 70"#,
    ];

    println!(
        "Processing {} diagrams in simulated parallel batch...",
        large_batch.len()
    );

    let mut parallel_stats = BatchStats::new();
    parallel_stats.total_files = large_batch.len();

    // Simulate parallel processing by processing all at once
    for (i, diagram_source) in large_batch.iter().enumerate() {
        match parse_diagram(diagram_source) {
            Ok(diagram) => {
                parallel_stats.add_success(&diagram);
                println!("  Worker {}: ✓ Parse successful", i + 1);
            }
            Err(e) => {
                parallel_stats.add_failure(format!("Worker {}: {}", i + 1, e));
                println!("  Worker {}: ✗ Parse failed", i + 1);
            }
        }
    }

    println!("\nParallel processing results:");
    println!(
        "  Successful: {}/{}",
        parallel_stats.successful_parses, parallel_stats.total_files
    );
    println!(
        "  Success rate: {:.1}%",
        (parallel_stats.successful_parses as f64 / parallel_stats.total_files as f64) * 100.0
    );

    Ok(())
}
