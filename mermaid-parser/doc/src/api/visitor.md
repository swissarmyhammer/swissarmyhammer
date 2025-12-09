# Visitor Pattern API

The visitor pattern provides a powerful way to traverse and analyze Abstract Syntax Trees (ASTs) of parsed Mermaid diagrams. This API enables users to implement custom analysis, transformation, and extraction logic.

## Core Visitor Traits

### `AstVisitor`

The main visitor trait for immutable AST traversal:

```rust
pub trait AstVisitor {
    type Result;

    fn visit_diagram(&mut self, diagram: &DiagramType) -> Self::Result;
    
    // Specific diagram type visitors
    fn visit_sankey(&mut self, diagram: &SankeyDiagram) -> Self::Result;
    fn visit_timeline(&mut self, diagram: &TimelineDiagram) -> Self::Result;
    fn visit_journey(&mut self, diagram: &JourneyDiagram) -> Self::Result;
    fn visit_sequence(&mut self, diagram: &SequenceDiagram) -> Self::Result;
    fn visit_class(&mut self, diagram: &ClassDiagram) -> Self::Result;
    fn visit_state(&mut self, diagram: &StateDiagram) -> Self::Result;
    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result;
    // ... and more
}
```

### `AstVisitorMut`

Visitor trait for mutable AST traversal and transformation:

```rust
pub trait AstVisitorMut {
    type Result;

    fn visit_diagram_mut(&mut self, diagram: &mut DiagramType) -> Self::Result;
    
    // Mutable diagram type visitors
    fn visit_sankey_mut(&mut self, diagram: &mut SankeyDiagram) -> Self::Result;
    fn visit_timeline_mut(&mut self, diagram: &mut TimelineDiagram) -> Self::Result;
    // ... and more
}
```

## Built-in Visitors

### `NodeCounter`

Counts nodes and edges in any diagram type:

```rust
use mermaid_parser::{parse_diagram, NodeCounter};

let diagram = parse_diagram("flowchart TD\n    A --> B\n    B --> C")?;
let mut counter = NodeCounter::new();
diagram.accept(&mut counter);

println!("Nodes: {}, Edges: {}", counter.nodes(), counter.edges());
```

**Methods:**
- `new() -> Self` - Create a new counter
- `nodes(&self) -> usize` - Get total node count
- `edges(&self) -> usize` - Get total edge count
- `reset(&mut self)` - Reset counters to zero

### `ComplexityAnalyzer`

Analyzes diagram complexity metrics:

```rust
use mermaid_parser::{parse_diagram, ComplexityAnalyzer};

let diagram = parse_diagram(complex_flowchart)?;
let mut analyzer = ComplexityAnalyzer::new();
diagram.accept(&mut analyzer);

let metrics = analyzer.metrics();
println!("Cyclomatic complexity: {}", metrics.cyclomatic_complexity);
println!("Nesting depth: {}", metrics.max_nesting_depth);
```

**Methods:**
- `new() -> Self` - Create a new analyzer
- `metrics(&self) -> ComplexityMetrics` - Get computed metrics
- `reset(&mut self)` - Reset analysis state

### `ReferenceValidator`

Validates that all references in a diagram are properly defined:

```rust
use mermaid_parser::{parse_diagram, ReferenceValidator};

let diagram = parse_diagram(diagram_with_references)?;
let mut validator = ReferenceValidator::new();
diagram.accept(&mut validator);

if validator.has_errors() {
    for error in validator.errors() {
        println!("Reference error: {}", error);
    }
}
```

**Methods:**
- `new() -> Self` - Create a new validator
- `has_errors(&self) -> bool` - Check if validation found errors
- `errors(&self) -> &[String]` - Get list of validation errors
- `is_valid(&self) -> bool` - Check if diagram is fully valid

### `TitleSetter`

Mutable visitor for setting diagram titles:

```rust
use mermaid_parser::{parse_diagram, TitleSetter};

let mut diagram = parse_diagram("flowchart TD\n    A --> B")?;
let mut title_setter = TitleSetter::new("My Flowchart");
diagram.accept_mut(&mut title_setter);

// Diagram now has the specified title
```

**Methods:**
- `new(title: &str) -> Self` - Create with the title to set
- `with_subtitle(mut self, subtitle: &str) -> Self` - Add a subtitle

## Custom Visitor Implementation

### Basic Custom Visitor

```rust
use mermaid_parser::{AstVisitor, DiagramType, SankeyDiagram, FlowchartDiagram};

struct LabelExtractor {
    labels: Vec<String>,
}

impl LabelExtractor {
    fn new() -> Self {
        Self { labels: Vec::new() }
    }
    
    fn labels(&self) -> &[String] {
        &self.labels
    }
}

impl AstVisitor for LabelExtractor {
    type Result = ();

    fn visit_sankey(&mut self, diagram: &SankeyDiagram) -> Self::Result {
        for node in &diagram.nodes {
            self.labels.push(node.label.clone());
        }
    }

    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        for node in &diagram.nodes {
            if let Some(label) = &node.label {
                self.labels.push(label.clone());
            }
        }
    }

    // Default implementation for other diagram types
    fn visit_timeline(&mut self, _: &TimelineDiagram) -> Self::Result {}
    fn visit_journey(&mut self, _: &JourneyDiagram) -> Self::Result {}
    // ... implement or use default for other types
}

// Usage
let diagram = parse_diagram(input)?;
let mut extractor = LabelExtractor::new();
diagram.accept(&mut extractor);

for label in extractor.labels() {
    println!("Found label: {}", label);
}
```

### Advanced Custom Visitor with State

```rust
use std::collections::HashMap;
use mermaid_parser::{AstVisitor, DiagramType};

struct StatisticsCollector {
    diagram_types: HashMap<String, usize>,
    total_nodes: usize,
    total_edges: usize,
    max_depth: usize,
    current_depth: usize,
}

impl StatisticsCollector {
    fn new() -> Self {
        Self {
            diagram_types: HashMap::new(),
            total_nodes: 0,
            total_edges: 0,
            max_depth: 0,
            current_depth: 0,
        }
    }
    
    fn report(&self) -> String {
        format!(
            "Statistics:\n  Diagram types: {:?}\n  Total nodes: {}\n  Total edges: {}\n  Max depth: {}",
            self.diagram_types, self.total_nodes, self.total_edges, self.max_depth
        )
    }
}

impl AstVisitor for StatisticsCollector {
    type Result = ();

    fn visit_diagram(&mut self, diagram: &DiagramType) -> Self::Result {
        let diagram_name = match diagram {
            DiagramType::Sankey(_) => "sankey",
            DiagramType::Flowchart(_) => "flowchart",
            DiagramType::Sequence(_) => "sequence",
            // ... other types
            _ => "other",
        };
        
        *self.diagram_types.entry(diagram_name.to_string()).or_insert(0) += 1;
        
        // Continue with specific visitor
        match diagram {
            DiagramType::Sankey(d) => self.visit_sankey(d),
            DiagramType::Flowchart(d) => self.visit_flowchart(d),
            // ... other types
            _ => {}
        }
    }

    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        self.total_nodes += diagram.nodes.len();
        self.total_edges += diagram.edges.len();
        
        // Calculate depth by traversing the flowchart structure
        self.current_depth = 0;
        // ... depth calculation logic
        self.max_depth = self.max_depth.max(self.current_depth);
    }

    // ... implement other diagram types
}
```

## Visitor Usage Patterns

### Single Diagram Analysis

```rust
use mermaid_parser::{parse_diagram, NodeCounter, ComplexityAnalyzer};

fn analyze_diagram(input: &str) -> Result<(), Box<dyn std::error::Error>> {
    let diagram = parse_diagram(input)?;
    
    // Count nodes and edges
    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);
    
    // Analyze complexity
    let mut analyzer = ComplexityAnalyzer::new();
    diagram.accept(&mut analyzer);
    
    println!("Diagram has {} nodes, {} edges", counter.nodes(), counter.edges());
    println!("Complexity metrics: {:?}", analyzer.metrics());
    
    Ok(())
}
```

### Batch Processing Multiple Diagrams

```rust
use mermaid_parser::{parse_diagram, NodeCounter};

fn analyze_multiple_diagrams(inputs: &[&str]) {
    let mut total_counter = NodeCounter::new();
    
    for input in inputs {
        if let Ok(diagram) = parse_diagram(input) {
            let mut counter = NodeCounter::new();
            diagram.accept(&mut counter);
            
            println!("Diagram: {} nodes, {} edges", counter.nodes(), counter.edges());
            
            // Accumulate totals
            // Note: This is conceptual - actual implementation would need accumulation logic
        }
    }
    
    println!("Total across all diagrams: {} nodes", total_counter.nodes());
}
```

### Diagram Transformation

```rust
use mermaid_parser::{parse_diagram, TitleSetter};

fn add_titles_to_diagrams(inputs: &[&str], titles: &[&str]) {
    for (input, title) in inputs.iter().zip(titles.iter()) {
        if let Ok(mut diagram) = parse_diagram(input) {
            let mut title_setter = TitleSetter::new(title);
            diagram.accept_mut(&mut title_setter);
            
            // Process the modified diagram
            println!("Added title '{}' to diagram", title);
        }
    }
}
```

## Performance Considerations

### Memory Efficiency

```rust
// For large diagrams, prefer streaming visitors that don't store all data
struct StreamingNodeCounter {
    count: usize,
}

impl AstVisitor for StreamingNodeCounter {
    type Result = ();
    
    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        // Just count, don't store nodes
        self.count += diagram.nodes.len();
    }
    
    // ... other diagram types
}
```

### Visitor Composition

```rust
// Combine multiple visitors for efficient single-pass analysis
struct CompositeVisitor {
    counter: NodeCounter,
    analyzer: ComplexityAnalyzer,
    validator: ReferenceValidator,
}

impl AstVisitor for CompositeVisitor {
    type Result = ();
    
    fn visit_diagram(&mut self, diagram: &DiagramType) -> Self::Result {
        self.counter.visit_diagram(diagram);
        self.analyzer.visit_diagram(diagram);
        self.validator.visit_diagram(diagram);
    }
}
```

## Error Handling in Visitors

```rust
use mermaid_parser::{AstVisitor, DiagramType};

struct SafeVisitor {
    errors: Vec<String>,
}

impl AstVisitor for SafeVisitor {
    type Result = Result<(), String>;
    
    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        if diagram.nodes.is_empty() {
            let error = "Flowchart has no nodes".to_string();
            self.errors.push(error.clone());
            return Err(error);
        }
        
        // Process diagram safely
        Ok(())
    }
    
    // ... other diagram types
}
```

This visitor pattern API provides a flexible and powerful way to work with parsed Mermaid diagrams, enabling everything from simple analysis to complex transformations.
