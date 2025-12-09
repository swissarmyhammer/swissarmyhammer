# Implement diagram complexity metrics

## Description
Add functionality to calculate various metrics and complexity measures for parsed diagrams. This helps users understand diagram complexity and identify potential improvements.

## Requirements
1. Define relevant metrics for each diagram type
2. Implement metrics calculation framework
3. Support custom metric definitions
4. Generate metric reports
5. Provide improvement suggestions

## Metrics Framework
```rust
pub trait DiagramMetrics {
    fn calculate_metrics(&self) -> MetricsReport;
}

pub struct MetricsReport {
    pub basic: BasicMetrics,
    pub complexity: ComplexityMetrics,
    pub quality: QualityMetrics,
    pub suggestions: Vec<Suggestion>,
}

pub struct BasicMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub depth: usize,
    pub breadth: usize,
}

pub struct ComplexityMetrics {
    pub cyclomatic: usize,
    pub cognitive: f64,
    pub nesting_depth: usize,
    pub coupling: f64,
}
```

## Metrics by Diagram Type

### Flowchart
- Cyclomatic complexity
- Maximum path length
- Node fan-in/fan-out
- Subgraph nesting depth

### Sequence
- Message complexity
- Participant interactions
- Nested block depth
- Lifeline coverage

### Class
- Inheritance depth
- Coupling between classes
- Methods per class
- Relationship complexity

### State
- State complexity
- Transition density
- Parallel region count
- Hierarchical depth

## Report Generation
```rust
impl Display for MetricsReport {
    fn fmt(&self, f: &mut Formatter) -> Result {
        writeln!(f, "Diagram Metrics Report")?;
        writeln!(f, "=====================")?;
        writeln!(f, "Nodes: {}", self.basic.node_count)?;
        writeln!(f, "Complexity: {} ({})", 
            self.complexity.cyclomatic,
            complexity_rating(self.complexity.cyclomatic))?;
        // ... more metrics
        
        if !self.suggestions.is_empty() {
            writeln!(f, "\nSuggestions:")?;
            for suggestion in &self.suggestions {
                writeln!(f, "- {}", suggestion)?;
            }
        }
    }
}
```

## Success Criteria
- Meaningful metrics for all diagram types
- Clear complexity ratings
- Actionable improvement suggestions
- Fast calculation performance
- Extensible metric system