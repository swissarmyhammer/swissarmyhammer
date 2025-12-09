# AST Types

The Abstract Syntax Tree (AST) types represent the structured representation of parsed Mermaid diagrams. Each diagram type has its own specialized AST structure optimized for that diagram's unique characteristics.

## Core AST Types

### `DiagramType`

The top-level enum containing all supported diagram types:

```rust
pub enum DiagramType {
    Sankey(SankeyDiagram),
    Timeline(TimelineDiagram),
    Journey(JourneyDiagram),
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    State(StateDiagram),
    Flowchart(FlowchartDiagram),
    Gantt(GanttDiagram),
    Pie(PieDiagram),
    Git(GitDiagram),
    Er(ErDiagram),
    C4(C4Diagram),
    Mindmap(MindmapDiagram),
    Quadrant(QuadrantDiagram),
    XyChart(XyChartDiagram),
    Kanban(KanbanDiagram),
    Block(BlockDiagram),
    Architecture(ArchitectureDiagram),
    Packet(PacketDiagram),
    Requirement(RequirementDiagram),
    Treemap(TreemapDiagram),
    Radar(RadarDiagram),
    Misc(MiscDiagram),
}
```

## Diagram-Specific AST Structures

### Sankey Diagrams

```rust
pub struct SankeyDiagram {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    pub title: Option<String>,
}

pub struct SankeyNode {
    pub id: String,
    pub label: String,
}

pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}
```

**Example:**
```rust
use mermaid_parser::{parse_diagram, DiagramType};

let input = "sankey-beta\nA,B,10\nB,C,5";
if let Ok(DiagramType::Sankey(sankey)) = parse_diagram(input) {
    for link in &sankey.links {
        println!("Flow: {} -> {} ({})", link.source, link.target, link.value);
    }
}
```

### Flowchart Diagrams

```rust
pub struct FlowchartDiagram {
    pub direction: FlowchartDirection,
    pub nodes: Vec<FlowchartNode>,
    pub edges: Vec<FlowchartEdge>,
    pub subgraphs: Vec<Subgraph>,
    pub title: Option<String>,
}

pub struct FlowchartNode {
    pub id: String,
    pub label: Option<String>,
    pub shape: NodeShape,
    pub classes: Vec<String>,
}

pub struct FlowchartEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub arrow_type: ArrowType,
    pub line_type: LineType,
}

pub enum FlowchartDirection {
    TopDown,    // TD
    TopBottom,  // TB
    BottomTop,  // BT
    RightLeft,  // RL
    LeftRight,  // LR
}

pub enum NodeShape {
    Rectangle,
    RoundedRectangle,
    Circle,
    Diamond,
    Hexagon,
    Parallelogram,
    // ... more shapes
}
```

### Sequence Diagrams

```rust
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
    pub notes: Vec<Note>,
    pub activations: Vec<Activation>,
    pub title: Option<String>,
}

pub struct Participant {
    pub id: String,
    pub label: Option<String>,
    pub actor_type: ActorType,
}

pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub message_type: MessageType,
    pub activation: bool,
}

pub enum MessageType {
    Solid,        // ->>
    Dotted,       // -->>
    Cross,        // -x
    DottedCross,  // --x
    // ... more types
}
```

### Class Diagrams

```rust
pub struct ClassDiagram {
    pub classes: Vec<Class>,
    pub relationships: Vec<Relationship>,
    pub title: Option<String>,
}

pub struct Class {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub methods: Vec<Method>,
    pub annotations: Vec<String>,
}

pub struct Attribute {
    pub visibility: Visibility,
    pub name: String,
    pub type_name: Option<String>,
}

pub struct Method {
    pub visibility: Visibility,
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
}

pub enum Visibility {
    Public,    // +
    Private,   // -
    Protected, // #
    Package,   // ~
}

pub enum Relationship {
    Inheritance(String, String),      // <|--
    Composition(String, String),      // *--
    Aggregation(String, String),      // o--
    Association(String, String),      // -->
    // ... more relationship types
}
```

### State Diagrams

```rust
pub struct StateDiagram {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub title: Option<String>,
}

pub struct State {
    pub id: String,
    pub label: Option<String>,
    pub state_type: StateType,
    pub nested_states: Vec<State>,
}

pub enum StateType {
    Simple,
    Start,
    End,
    Choice,
    Fork,
    Join,
    Composite,
}

pub struct Transition {
    pub from: String,
    pub to: String,
    pub trigger: Option<String>,
    pub guard: Option<String>,
    pub action: Option<String>,
}
```

## Common Helper Types

### Positioning and Layout

```rust
pub struct Position {
    pub x: f64,
    pub y: f64,
}

pub struct Size {
    pub width: f64,
    pub height: f64,
}

pub struct BoundingBox {
    pub position: Position,
    pub size: Size,
}
```

### Styling and Appearance

```rust
pub struct Style {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub color: Option<String>,
    pub font_size: Option<String>,
    pub font_family: Option<String>,
}

pub struct CssClass {
    pub name: String,
    pub styles: Vec<CssProperty>,
}

pub struct CssProperty {
    pub property: String,
    pub value: String,
}
```

### Text and Labels

```rust
pub struct Label {
    pub text: String,
    pub position: Option<Position>,
    pub style: Option<Style>,
}

pub struct MultilineText {
    pub lines: Vec<String>,
    pub alignment: TextAlignment,
}

pub enum TextAlignment {
    Left,
    Center,
    Right,
}
```

### Metadata and Annotations

```rust
pub struct Metadata {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub version: Option<String>,
}

pub struct Annotation {
    pub annotation_type: AnnotationType,
    pub target: String,
    pub content: String,
}

pub enum AnnotationType {
    Note,
    Comment,
    Link,
    Tooltip,
}
```

## Advanced AST Features

### Generic Node Structure

Many diagram types share common node characteristics:

```rust
pub trait Node {
    fn id(&self) -> &str;
    fn label(&self) -> Option<&str>;
    fn position(&self) -> Option<&Position>;
    fn style(&self) -> Option<&Style>;
}

pub trait Edge {
    fn from(&self) -> &str;
    fn to(&self) -> &str;
    fn label(&self) -> Option<&str>;
}
```

### Visitor Pattern Support

All AST types implement visitor pattern support:

```rust
impl DiagramType {
    pub fn accept<V: AstVisitor>(&self, visitor: &mut V) -> V::Result {
        visitor.visit_diagram(self)
    }
    
    pub fn accept_mut<V: AstVisitorMut>(&mut self, visitor: &mut V) -> V::Result {
        visitor.visit_diagram_mut(self)
    }
}
```

### Serialization Support

AST types support serialization for persistence and interchange:

```rust
use serde::{Serialize, Deserialize};

// Most AST types derive these traits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SankeyDiagram {
    // ... fields
}
```

## Working with AST Types

### Pattern Matching

```rust
use mermaid_parser::{parse_diagram, DiagramType};

match parse_diagram(input)? {
    DiagramType::Flowchart(flowchart) => {
        println!("Flowchart has {} nodes", flowchart.nodes.len());
        for node in &flowchart.nodes {
            println!("Node {}: {:?}", node.id, node.shape);
        }
    }
    DiagramType::Sequence(sequence) => {
        println!("Sequence has {} participants", sequence.participants.len());
        for message in &sequence.messages {
            println!("Message: {} -> {}: {}", message.from, message.to, message.text);
        }
    }
    other => println!("Other diagram type: {:?}", other),
}
```

### AST Transformation

```rust
fn add_styling_to_flowchart(mut flowchart: FlowchartDiagram) -> FlowchartDiagram {
    for node in &mut flowchart.nodes {
        if node.classes.is_empty() {
            node.classes.push("default-style".to_string());
        }
    }
    flowchart
}
```

### AST Analysis

```rust
fn analyze_flowchart_complexity(flowchart: &FlowchartDiagram) -> f64 {
    let node_count = flowchart.nodes.len() as f64;
    let edge_count = flowchart.edges.len() as f64;
    let subgraph_count = flowchart.subgraphs.len() as f64;
    
    // Simple complexity metric
    node_count + (edge_count * 0.5) + (subgraph_count * 2.0)
}
```

### Building AST Programmatically

```rust
use mermaid_parser::{FlowchartDiagram, FlowchartNode, FlowchartEdge, NodeShape, ArrowType, LineType, FlowchartDirection};

fn create_simple_flowchart() -> FlowchartDiagram {
    FlowchartDiagram {
        direction: FlowchartDirection::TopDown,
        nodes: vec![
            FlowchartNode {
                id: "A".to_string(),
                label: Some("Start".to_string()),
                shape: NodeShape::RoundedRectangle,
                classes: vec![],
            },
            FlowchartNode {
                id: "B".to_string(),
                label: Some("End".to_string()),
                shape: NodeShape::RoundedRectangle,
                classes: vec![],
            },
        ],
        edges: vec![
            FlowchartEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                arrow_type: ArrowType::Arrow,
                line_type: LineType::Solid,
            },
        ],
        subgraphs: vec![],
        title: Some("Simple Flow".to_string()),
    }
}
```

## Type Safety and Validation

The AST types provide compile-time type safety and runtime validation:

```rust
// Compile-time safety
fn process_flowchart(diagram: FlowchartDiagram) {
    // We know this is definitely a flowchart
    for node in &diagram.nodes {
        // Type-safe access to flowchart-specific fields
        match node.shape {
            NodeShape::Diamond => println!("Decision node: {}", node.id),
            _ => println!("Regular node: {}", node.id),
        }
    }
}

// Runtime validation
fn validate_flowchart(diagram: &FlowchartDiagram) -> Vec<String> {
    let mut errors = Vec::new();
    
    // Check for orphaned edges
    let node_ids: HashSet<_> = diagram.nodes.iter().map(|n| &n.id).collect();
    for edge in &diagram.edges {
        if !node_ids.contains(&edge.from) {
            errors.push(format!("Edge references unknown node: {}", edge.from));
        }
        if !node_ids.contains(&edge.to) {
            errors.push(format!("Edge references unknown node: {}", edge.to));
        }
    }
    
    errors
}
```

This AST design provides a balance of type safety, expressiveness, and performance for working with parsed Mermaid diagrams.
