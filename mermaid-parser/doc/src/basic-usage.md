# Basic Usage

This guide covers the fundamental patterns for operating your mermaid-parser neural network. Like programming AI behavior patterns, these examples will help you master the core parsing capabilities.

## Simple Diagram Parsing

### Flowchart Neural Networks

Flowcharts represent logical decision trees, similar to AI decision making processes:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let flowchart_input = r#"
flowchart TD
    Start([AI Initialization]) --> Check{Data Valid?}
    Check -->|Yes| Process[Neural Processing]
    Check -->|No| Error[Error Handling]
    Process --> Output[Generate Result]
    Error --> Output
    Output --> End([Termination])
"#;

if let Ok(DiagramType::Flowchart(diagram)) = parse_diagram(flowchart_input) {
    println!("🧠 Neural network topology loaded");
    println!("📊 Processing nodes: {}", diagram.nodes.len());
    
    // Analyze decision points (diamond nodes)
    for (id, node) in &diagram.nodes {
        if matches!(node.shape, Some(ref shape) if shape.contains("diamond")) {
            println!("🔍 Decision node detected: {}", id);
        }
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Sequence Diagrams as Communication Protocols

Sequence diagrams map message flows like inter-AI communication:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let sequence_input = r#"
sequenceDiagram
    participant AI as Main AI
    participant DB as Database
    participant Cache as Cache Layer
    
    AI->>DB: Query for data
    DB-->>AI: Return dataset
    AI->>Cache: Store in cache
    Cache-->>AI: Confirmation
    AI->>AI: Process & learn
"#;

if let Ok(DiagramType::Sequence(diagram)) = parse_diagram(sequence_input) {
    println!("🤖 Communication protocol analyzed");
    println!("👥 Participants: {}", diagram.participants.len());
    println!("📨 Messages: {}", diagram.messages.len());
    
    // Analyze message patterns
    for message in &diagram.messages {
        println!("📡 {} → {}: {}", 
                 message.from, message.to, message.text);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Batch Processing Multiple Diagrams

Process multiple diagrams like training a neural network on diverse datasets:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

fn process_diagram_batch(inputs: Vec<&str>) -> Result<(), Box<dyn std::error::Error>> {
    for (index, input) in inputs.iter().enumerate() {
        println!("🔄 Processing neural network #{}", index + 1);
        
        match parse_diagram(input) {
            Ok(DiagramType::Flowchart(diagram)) => {
                println!("  ✅ Flowchart: {} nodes", diagram.nodes.len());
            }
            Ok(DiagramType::Sequence(diagram)) => {
                println!("  ✅ Sequence: {} participants", diagram.participants.len());
            }
            Ok(DiagramType::Sankey(diagram)) => {
                println!("  ✅ Sankey: {} flow connections", diagram.links.len());
            }
            Ok(other) => {
                println!("  ✅ Parsed: {:?}", other);
            }
            Err(e) => {
                println!("  ❌ Neural network error: {}", e);
            }
        }
    }
    Ok(())
}

// Example usage
let diagram_inputs = vec![
    "flowchart TD\n    A --> B",
    "sequenceDiagram\n    Alice->>Bob: Hello",
    "sankey-beta\n    A,B,10",
];

process_diagram_batch(diagram_inputs)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Working with Complex Diagrams

### State Machine Neural Networks

State diagrams represent AI behavioral patterns:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let state_input = r#"
stateDiagram-v2
    [*] --> Idle : System Boot
    Idle --> Learning : New Data
    Learning --> Processing : Model Ready
    Processing --> Inference : Input Received
    Inference --> Learning : Update Weights
    Inference --> Idle : Task Complete
    Processing --> Error : Exception
    Error --> Idle : Reset
"#;

if let Ok(DiagramType::State(diagram)) = parse_diagram(state_input) {
    println!("🎯 Neural state machine loaded");
    println!("🔄 States: {}", diagram.states.len());
    println!("↗️  Transitions: {}", diagram.transitions.len());
    
    // Find terminal states (like convergence points)
    for state in &diagram.states {
        if state.name.contains("Error") || state.name.contains("*") {
            println!("🚨 Critical state: {}", state.name);
        }
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Class Diagrams as System Architecture

Model object relationships like AI component hierarchies:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let class_input = r#"
classDiagram
    class NeuralNetwork {
        +layers: Vec~Layer~
        +weights: Matrix
        +bias: Vector
        +forward(input: Vector) Vector
        +backward(gradient: Vector) void
        +train(dataset: Dataset) void
    }
    
    class Layer {
        +neurons: Vec~Neuron~
        +activation: ActivationFunction
        +compute(input: Vector) Vector
    }
    
    class Neuron {
        +threshold: f64
        +fire(input: f64) f64
    }
    
    NeuralNetwork --> Layer : contains
    Layer --> Neuron : contains
"#;

if let Ok(DiagramType::Class(diagram)) = parse_diagram(class_input) {
    println!("🏗️  System architecture analyzed");
    println!("📦 Classes: {}", diagram.classes.len());
    println!("🔗 Relationships: {}", diagram.relationships.len());
    
    // Analyze class complexity
    for class in &diagram.classes {
        let method_count = class.methods.len();
        let field_count = class.fields.len();
        println!("🔍 {}: {} methods, {} fields", 
                 class.name, method_count, field_count);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Data Flow Analysis

### Sankey Diagrams for Resource Tracking

Track data flows like monitoring neural network activations:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let sankey_input = r#"
sankey-beta
    Input,Preprocessing,1000
    Preprocessing,FeatureExtraction,800
    Preprocessing,DataCleaning,200
    FeatureExtraction,NeuralNetwork,800
    DataCleaning,NeuralNetwork,200
    NeuralNetwork,OutputLayer,1000
    OutputLayer,Classification,600
    OutputLayer,Regression,400
"#;

if let Ok(DiagramType::Sankey(diagram)) = parse_diagram(sankey_input) {
    println!("🌊 Data flow matrix analyzed");
    println!("🔄 Processing nodes: {}", diagram.nodes.len());
    println!("📊 Data streams: {}", diagram.links.len());
    
    // Calculate total throughput
    let total_flow: f64 = diagram.links.iter()
        .map(|link| link.value)
        .sum();
    println!("⚡ Total data throughput: {}", total_flow);
    
    // Find bottlenecks
    for link in &diagram.links {
        if link.value < 100.0 {
            println!("⚠️  Potential bottleneck: {} → {} ({})", 
                     link.source, link.target, link.value);
        }
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Error Handling Patterns

Implement robust error handling like AI system monitoring:

```rust
use mermaid_parser::{parse_diagram, ParseError};

fn safe_parse_with_fallback(input: &str) -> String {
    match parse_diagram(input) {
        Ok(diagram) => {
            format!("✅ Neural network successfully initialized: {:?}", diagram)
        }
        Err(ParseError::EmptyInput) => {
            "⚠️  No input data detected - system idle".to_string()
        }
        Err(ParseError::UnknownDiagramType(diagram_type)) => {
            format!("🔍 Unknown pattern '{}' - activating learning mode", diagram_type)
        }
        Err(ParseError::EnhancedSyntaxError { message, suggestions, .. }) => {
            let mut result = format!("🧠 Neural network detected syntax pattern: {}\n", message);
            result.push_str("💡 AI suggestions:\n");
            for suggestion in suggestions.iter() {
                result.push_str(&format!("   • {}\n", suggestion));
            }
            result
        }
        Err(e) => {
            format!("❌ Neural network error: {}", e)
        }
    }
}

// Example usage
let test_inputs = vec![
    "flowchart TD\n    A --> B",           // Valid
    "",                                    // Empty
    "unknown_diagram\n    test",           // Unknown type
    "flowchart TD\n    A => B",           // Syntax error
];

for input in test_inputs {
    println!("{}", safe_parse_with_fallback(input));
}
```

## Next Steps

Master these advanced neural network capabilities:

- **[AST Analysis](./ast-analysis.md)**: Deep exploration of parsed structures
- **[Visitor Pattern](./visitor-pattern.md)**: Systematic traversal algorithms
- **[Performance](./performance.md)**: Optimizing neural network performance
- **[Metrics](./metrics.md)**: Quality analysis and complexity measurement

Ready to explore the internal structure? Let's dive into [AST Analysis](./ast-analysis.md)!