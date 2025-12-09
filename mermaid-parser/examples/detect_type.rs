//! Diagram type detection example for the mermaid-parser crate
//!
//! This example demonstrates how to detect the type of Mermaid diagrams
//! from different types of input text.

use mermaid_parser::parse_diagram;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let examples = vec![
        (
            "Flowchart",
            r#"
flowchart TD
    A --> B
    B --> C
"#,
        ),
        (
            "Sequence Diagram",
            r#"
sequenceDiagram
    Alice->>Bob: Hello Bob, how are you?
    Bob-->>Alice: Great!
"#,
        ),
        (
            "Sankey Diagram",
            r#"
sankey-beta
    A,B,10
    B,C,5
    B,D,3
"#,
        ),
        (
            "Timeline",
            r#"
timeline
    title History of Social Media Platform
    2002 : LinkedIn
    2004 : Facebook
         : Google
    2005 : Youtube
    2006 : Twitter
"#,
        ),
        (
            "State Diagram",
            r#"
stateDiagram-v2
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]
"#,
        ),
        (
            "Pie Chart",
            r#"
pie title NETFLIX
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10
"#,
        ),
        (
            "Gantt Chart",
            r#"
gantt
    title A Gantt Diagram
    dateFormat  YYYY-MM-DD
    section Section
    A task           :a1, 2014-01-01, 30d
    Another task     :after a1  , 20d
"#,
        ),
    ];

    println!("Diagram Type Detection Examples");
    println!("===============================\n");

    for (description, diagram) in examples {
        println!("Testing: {}", description);
        println!("Input:");
        println!("{}", diagram.trim());

        match parse_diagram(diagram) {
            Ok(parsed) => {
                let diagram_type = match parsed {
                    mermaid_parser::DiagramType::Flowchart(_) => "Flowchart",
                    mermaid_parser::DiagramType::Sequence(_) => "Sequence Diagram",
                    mermaid_parser::DiagramType::Sankey(_) => "Sankey Diagram",
                    mermaid_parser::DiagramType::Timeline(_) => "Timeline",
                    mermaid_parser::DiagramType::Class(_) => "Class Diagram",
                    mermaid_parser::DiagramType::Er(_) => "ER Diagram",
                    mermaid_parser::DiagramType::State(_) => "State Diagram",
                    mermaid_parser::DiagramType::Pie(_) => "Pie Chart",
                    mermaid_parser::DiagramType::Gantt(_) => "Gantt Chart",
                    mermaid_parser::DiagramType::Journey(_) => "Journey",
                    mermaid_parser::DiagramType::C4(_) => "C4",
                    mermaid_parser::DiagramType::Mindmap(_) => "Mindmap",
                    mermaid_parser::DiagramType::Quadrant(_) => "Quadrant Chart",
                    mermaid_parser::DiagramType::XyChart(_) => "XY Chart",
                    mermaid_parser::DiagramType::Kanban(_) => "Kanban",
                    mermaid_parser::DiagramType::Block(_) => "Block Diagram",
                    mermaid_parser::DiagramType::Architecture(_) => "Architecture",
                    mermaid_parser::DiagramType::Packet(_) => "Packet",
                    mermaid_parser::DiagramType::Requirement(_) => "Requirement",
                    mermaid_parser::DiagramType::Treemap(_) => "Treemap",
                    mermaid_parser::DiagramType::Radar(_) => "Radar Chart",
                    mermaid_parser::DiagramType::Git(_) => "Git Graph",
                    mermaid_parser::DiagramType::Misc(_) => "Miscellaneous",
                };
                println!("✓ Detected as: {}", diagram_type);
            }
            Err(e) => {
                println!("✗ Parse error: {}", e);
            }
        }

        println!("{}\n", "-".repeat(50));
    }

    Ok(())
}
