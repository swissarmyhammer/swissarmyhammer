# Supported Diagrams

The mermaid-parser crate supports parsing the following Mermaid diagram types:

## Fully Supported Diagrams

| Type | Keywords | Description |
|------|----------|-------------|
| **Sankey** | `sankey-beta`, `sankey` | Flow data visualization with weighted connections |
| **Architecture** | `architecture-beta`, `architecture` | System architecture diagrams |
| **Block** | `block-beta`, `block` | Block diagrams for system design |
| **C4** | `c4context`, `c4container`, `c4component`, `c4dynamic`, `c4deployment` | C4 model architecture diagrams |
| **Class** | `classDiagram` | Object-oriented class relationships |
| **Entity Relationship** | `erDiagram`, `erDiagramTitleText` | Database schema diagrams |
| **Flowchart** | `flowchart`, `graph` | General-purpose flow diagrams |
| **Gantt** | `gantt`, `ganttTestClick` | Project timeline charts |
| **Git** | `gitgraph` | Git branching visualization |
| **Journey** | `journey` | User experience journey mapping |
| **Kanban** | `kanban` | Kanban board representation |
| **Mindmap** | `mindmap` | Mind mapping diagrams |
| **Packet** | `packet-beta`, `packet` | Network packet diagrams |
| **Pie** | `pie` | Pie chart data visualization |
| **Quadrant** | `quadrant`, `quadrantChart` | Quadrant analysis charts |
| **Radar** | `radar` | Radar/spider chart visualization |
| **Requirement** | `requirement`, `requirementDiagram` | Requirements documentation |
| **Sequence** | `sequenceDiagram` | Message passing between actors |
| **State** | `stateDiagram`, `stateDiagram-v2` | State machine representations |
| **Timeline** | `timeline` | Chronological event sequences |
| **Treemap** | `treemap-beta`, `treemap` | Hierarchical data visualization |
| **XY Chart** | `xychart-beta`, `xychart` | X-Y coordinate plotting |

## Additional Support

- **Misc Parser**: Handles unknown diagram types and provides basic parsing fallback
- **Comments**: All parsers support `//` and `#` style comments
- **Beta Syntax**: Many diagrams support both standard and beta syntax variations

## Usage Example

```rust
use mermaid_parser::parse_diagram;

// Any supported diagram type can be parsed
let result = parse_diagram("flowchart TD\n    A --> B");
assert!(result.is_ok());

let result = parse_diagram("sankey-beta\n    A,B,10");
assert!(result.is_ok());
```

The parser automatically detects the diagram type from the opening keyword and routes to the appropriate specialized parser.
