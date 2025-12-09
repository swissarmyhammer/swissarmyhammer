//! Error handling example for the mermaid-parser crate
//!
//! This example demonstrates how to handle various types of parse errors
//! that can occur when parsing Mermaid diagrams.

use mermaid_parser::parse_diagram;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_cases = vec![
        ("Empty Input", ""),
        ("Only Whitespace", "   \n  \t  \n  "),
        ("Only Comments", "// This is a comment\n# Another comment"),
        ("Unknown Diagram Type", "unknownDiagram\nSome content here"),
        (
            "Invalid Flowchart",
            r#"
flowchart TD
    A --> 
    --> B
    C -->> D
"#,
        ),
        (
            "Malformed Sankey",
            r#"
sankey-beta
    A,B
    C,D,E,F,G
    InvalidLine
"#,
        ),
        (
            "Invalid Timeline",
            r#"
timeline
    This is not valid timeline syntax
    :: :: invalid
"#,
        ),
        (
            "Malformed State Diagram",
            r#"
stateDiagram-v2
    [*] ->
    -> [*]
    Invalid State Name With Spaces --> Another Invalid
"#,
        ),
        (
            "Incomplete Sequence",
            r#"
sequenceDiagram
    Alice->>
    Bob-->>Alice
    Alice->>Bob:
"#,
        ),
        (
            "Valid Diagram",
            r#"
flowchart TD
    A[Start] --> B[Process]
    B --> C[End]
"#,
        ),
    ];

    println!("Error Handling Examples");
    println!("=======================\n");

    for (description, input) in test_cases {
        println!("Testing: {}", description);

        if !input.is_empty() {
            println!("Input:");
            println!("{}", input);
        } else {
            println!("Input: <empty>");
        }

        match parse_diagram(input) {
            Ok(diagram) => {
                let diagram_type = match diagram {
                    mermaid_parser::DiagramType::Flowchart(_) => "Flowchart",
                    mermaid_parser::DiagramType::Sequence(_) => "Sequence Diagram",
                    mermaid_parser::DiagramType::Sankey(_) => "Sankey Diagram",
                    mermaid_parser::DiagramType::Timeline(_) => "Timeline",
                    mermaid_parser::DiagramType::Class(_) => "Class Diagram",
                    mermaid_parser::DiagramType::State(_) => "State Diagram",
                    mermaid_parser::DiagramType::Pie(_) => "Pie Chart",
                    mermaid_parser::DiagramType::Gantt(_) => "Gantt Chart",
                    _ => "Other",
                };
                println!("✓ Successfully parsed as: {}", diagram_type);
            }
            Err(error) => {
                println!("✗ Parse error occurred:");
                println!("  Error: {}", error);

                // Provide helpful suggestions based on error type
                let error_str = error.to_string();

                if error_str.contains("EmptyInput") || error_str.contains("empty") {
                    println!("  💡 Suggestion: Provide non-empty input with diagram content");
                } else if error_str.contains("unknown") || error_str.contains("Unknown") {
                    println!(
                        "  💡 Suggestion: Check diagram type keyword (flowchart, sequence, etc.)"
                    );
                } else if error_str.contains("Syntax error") {
                    println!("  💡 Suggestion: Check diagram syntax and formatting");
                } else if error_str.contains("Failed to parse") {
                    println!("  💡 Suggestion: Verify the diagram follows correct syntax rules");
                } else {
                    println!("  💡 Suggestion: Check the Mermaid documentation for proper syntax");
                }

                // Example of graceful degradation
                println!("  🔄 Fallback: Could attempt to parse as miscellaneous diagram type");
            }
        }

        println!("{}\n", "-".repeat(60));
    }

    // Demonstrate error recovery patterns
    println!("Error Recovery Patterns");
    println!("======================\n");

    let potentially_broken_diagrams = [
        r#"flowchart TD\nA --> B\nB --> C"#,
        r#"sequenceDiagram\nAlice->>Bob: Hello"#,
        r#"invalid diagram type\ncontent here"#,
    ];

    for (i, diagram) in potentially_broken_diagrams.iter().enumerate() {
        println!("Attempting to parse diagram {}", i + 1);

        match parse_diagram(diagram) {
            Ok(_parsed) => println!("✓ Successfully parsed"),
            Err(e) => {
                println!("✗ Failed to parse: {}", e);
                println!("  Attempting recovery strategies...");

                // Strategy 1: Try to extract and fix common issues
                let fixed_diagram = diagram.replace("\\n", "\n");
                match parse_diagram(&fixed_diagram) {
                    Ok(_) => println!("  ✓ Recovery successful: Fixed escape sequences"),
                    Err(_) => println!("  ✗ Recovery failed: Escape sequence fix didn't help"),
                }

                // Strategy 2: Could implement partial parsing or suggestions here
                println!("  💡 Consider implementing partial parsing for better error recovery");
            }
        }
        println!();
    }

    Ok(())
}
