# Mermaid Grammar Analysis

## Overview
Analysis of 17 .jison grammar files from the mermaid repository for Rust parser implementation.

## Grammar Files Summary

| Grammar | File Path | Lines | Complexity | Priority |
|---------|-----------|-------|------------|----------|
| Flow | `diagrams/flowchart/parser/flow.jison` | 631 | High | 1 |
| Class | `diagrams/class/parser/classDiagram.jison` | 420 | High | 2 |
| State | `diagrams/state/parser/stateDiagram.jison` | 336 | Medium | 3 |
| Sequence | `diagrams/sequence/parser/sequenceDiagram.jison` | 329 | Medium | 4 |
| C4 | `diagrams/c4/parser/c4Diagram.jison` | 322 | Medium | 5 |
| ER | `diagrams/er/parser/erDiagram.jison` | 293 | Medium | 6 |
| Block | `diagrams/block/parser/block.jison` | 290 | Medium | 7 |
| Requirement | `diagrams/requirement/parser/requirementDiagram.jison` | 267 | Medium | 8 |
| Gantt | `diagrams/gantt/parser/gantt.jison` | 188 | Low | 9 |
| Quadrant | `diagrams/quadrant-chart/parser/quadrant.jison` | 187 | Low | 10 |
| XY Chart | `diagrams/xychart/parser/xychart.jison` | 171 | Low | 11 |
| Kanban | `diagrams/kanban/parser/kanban.jison` | 166 | Low | 12 |
| Mindmap | `diagrams/mindmap/parser/mindmap.jison` | 127 | Low | 13 |
| Timeline | `diagrams/timeline/parser/timeline.jison` | 79 | Low | 14 |
| Journey | `diagrams/user-journey/parser/journey.jison` | 69 | Low | 15 |
| Sankey | `diagrams/sankey/parser/sankey.jison` | 66 | Low | 16 |
| Example | `mermaid-example-diagram/src/parser/exampleDiagram.jison` | 43 | Low | 17 |

## Common Patterns

### Lexical Patterns
All grammars share common lexical elements:
- `WHITESPACE`: Space, tab, newline handling
- `EOF`: End of file
- `NEWLINE`: Line breaks
- `COMMENT`: Single and multi-line comments
- `ACC_TITLE`/`ACC_DESCR`: Accessibility support
- `DIRECTIVE`: Configuration directives

### Grammar Structure
1. **Header Section**: Title, configuration
2. **Body Section**: Main diagram content
3. **Styling Section**: CSS classes, colors
4. **Interaction Section**: Click handlers, links

### Node Types
- **Flowchart**: 15+ node shapes (rectangle, circle, diamond, etc.)
- **Class**: Classes, interfaces, enums
- **State**: States, composite states, forks
- **Sequence**: Actors, lifelines, messages

## Implementation Strategy

### Phase 1: Simple Grammars (Low Complexity)
Start with simple, well-defined grammars:
1. **Sankey** - CSV-like format, minimal rules
2. **Timeline** - Simple chronological format
3. **Journey** - Basic user journey structure

### Phase 2: Medium Complexity
4. **Pie** - Simple data visualization
5. **Gantt** - Project timeline with dates
6. **Quadrant** - 2x2 matrix format

### Phase 3: Complex Grammars
7. **Sequence** - Message passing between actors
8. **State** - State machines with transitions
9. **Class** - OOP relationships and methods

### Phase 4: Most Complex
10. **Flow** - The most comprehensive grammar with many node types
11. **C4** - Architecture diagrams with multiple contexts

## Dependencies and Relationships

### Independent Grammars
These can be implemented in any order:
- Sankey, Timeline, Journey, Pie, Quadrant
- Gantt, XY Chart, Mindmap, Kanban

### Dependent Grammars
- **Block** - May share patterns with Flow
- **C4** - Builds on architectural concepts
- **Requirement** - Complex traceability relationships

## Technical Considerations

### Lexing Challenges
1. **Context Sensitivity**: Some tokens depend on current state
2. **String Handling**: Quoted strings with escaping
3. **Markdown Support**: Text with markdown formatting

### Parsing Challenges
1. **Precedence**: Operator precedence in expressions
2. **Ambiguity**: Multiple valid parse trees
3. **Error Recovery**: Graceful handling of syntax errors

### AST Design
Each grammar needs:
```rust
enum DiagramType {
    Flow(FlowDiagram),
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    // ... other types
}
```

## Test Strategy

### Test Data Sources
For each grammar, we have extracted `.mermaid` files:
- Flow: 576 test files
- Class: 212 test files  
- State: 124 test files
- Sequence: 55 test files
- And more...

### Test Approach
1. **Unit Tests**: Individual grammar rules
2. **Integration Tests**: Complete diagram parsing
3. **Regression Tests**: Known edge cases
4. **Performance Tests**: Large diagram handling

## Success Metrics
- Parse all extracted `.mermaid` samples successfully
- Performance: Parse 1000+ diagrams in <1 second
- Memory: Efficient AST representation
- Error Messages: Clear syntax error reporting