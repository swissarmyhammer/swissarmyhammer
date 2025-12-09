//! Test the visitor pattern example from documentation

use mermaid_parser::common::visitor::NodeCounter;
use mermaid_parser::parse_diagram;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "flowchart TD\n    A --> B\n    B --> C";
    let diagram = parse_diagram(input)?;

    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);
    println!(
        "Found {} nodes and {} edges",
        counter.nodes(),
        counter.edges()
    );

    Ok(())
}
