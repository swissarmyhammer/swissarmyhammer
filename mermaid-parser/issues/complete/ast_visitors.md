# Implement AST visitor utilities

## Description
Create utility functions and traits for traversing and analyzing AST structures. This makes it easier for users to work with parsed diagrams.

## Requirements
1. Design visitor trait pattern
2. Implement for all AST node types
3. Create common analysis utilities
4. Support both immutable and mutable visitors
5. Add transformation capabilities

## Visitor Pattern Design
```rust
pub trait AstVisitor {
    type Result;
    
    fn visit_flowchart(&mut self, ast: &FlowchartDiagram) -> Self::Result;
    fn visit_sequence(&mut self, ast: &SequenceDiagram) -> Self::Result;
    // ... for all diagram types
    
    fn visit_node(&mut self, node: &Node) -> Self::Result;
    fn visit_edge(&mut self, edge: &Edge) -> Self::Result;
    // ... for all element types
}

pub trait AstVisitorMut {
    type Result;
    
    fn visit_flowchart_mut(&mut self, ast: &mut FlowchartDiagram) -> Self::Result;
    // ... mutable versions
}
```

## Common Utilities to Implement
- Node counter (count elements by type)
- Complexity analyzer (cyclomatic complexity)
- Reference validator (check undefined references)
- Style extractor (collect all styles)
- Metrics calculator (depth, breadth, connections)
- AST transformer (modify structure)
- AST optimizer (simplify structure)

## Example Usage
```rust
struct NodeCounter {
    nodes: usize,
    edges: usize,
}

impl AstVisitor for NodeCounter {
    type Result = ();
    
    fn visit_node(&mut self, _: &Node) {
        self.nodes += 1;
    }
    
    fn visit_edge(&mut self, _: &Edge) {
        self.edges += 1;
    }
}
```

## Success Criteria
- Easy to implement custom visitors
- Common analyses provided out-of-box
- Support for all AST node types
- Both read and write visitors
- Well-documented patterns