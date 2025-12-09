//! Abstract Syntax Tree definitions for all Mermaid diagram types

/// Top-level enum representing all supported Mermaid diagram types
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramType {
    /// Sankey flow diagrams
    Sankey(SankeyDiagram),
    /// Timeline diagrams
    Timeline(TimelineDiagram),
    /// User journey diagrams
    Journey(JourneyDiagram),
    /// Sequence diagrams
    Sequence(SequenceDiagram),
    /// Class diagrams
    Class(ClassDiagram),
    /// State diagrams
    State(StateDiagram),
    /// Flowchart diagrams
    Flowchart(FlowchartDiagram),
    /// Gantt charts
    Gantt(GanttDiagram),
    /// Pie charts
    Pie(PieDiagram),
    /// Git graphs
    Git(GitDiagram),
    /// Entity-relationship diagrams
    Er(ErDiagram),
    /// C4 architecture diagrams
    C4(C4Diagram),
    /// Mind maps
    Mindmap(MindmapDiagram),
    /// Quadrant charts
    Quadrant(QuadrantDiagram),
    /// XY charts
    XyChart(XyChartDiagram),
    /// Kanban boards
    Kanban(KanbanDiagram),
    /// Block diagrams
    Block(BlockDiagram),
    /// Architecture diagrams
    Architecture(ArchitectureDiagram),
    /// Packet diagrams
    Packet(PacketDiagram),
    /// Requirement diagrams
    Requirement(RequirementDiagram),
    /// Treemap diagrams
    Treemap(TreemapDiagram),
    /// Radar charts
    Radar(RadarDiagram),
    /// Miscellaneous/experimental diagrams
    Misc(MiscDiagram),
}

/// Common accessibility information used across diagram types
///
/// Provides standardized accessibility metadata that can be attached to diagrams
/// to improve screen reader support and overall accessibility compliance.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityInfo {
    /// Optional title for accessibility purposes
    pub title: Option<String>,
    /// Optional description for accessibility purposes  
    pub description: Option<String>,
}

/// Sankey flow diagram representation
///
/// Sankey diagrams visualize the flow of data, energy, or materials through a system.
/// They consist of nodes (representing entities) and weighted links (representing flows).
///
/// # Example
///
/// ```
/// use mermaid_parser::common::ast::{SankeyDiagram, SankeyNode, SankeyLink};
///
/// let diagram = SankeyDiagram {
///     nodes: vec![
///         SankeyNode { id: "A".to_string(), name: "Source".to_string() },
///         SankeyNode { id: "B".to_string(), name: "Target".to_string() },
///     ],
///     links: vec![
///         SankeyLink {
///             source: "A".to_string(),
///             target: "B".to_string(),
///             value: 10.0,
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyDiagram {
    /// Collection of nodes in the Sankey diagram
    pub nodes: Vec<SankeyNode>,
    /// Collection of weighted links between nodes
    pub links: Vec<SankeyLink>,
}

/// A node in a Sankey diagram
///
/// Represents an entity through which flow passes. Each node has a unique
/// identifier and a human-readable name.
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyNode {
    /// Unique identifier for the node
    pub id: String,
    /// Display name for the node
    pub name: String,
}

/// A weighted link between two nodes in a Sankey diagram
///
/// Represents the flow of data/energy/materials from a source node to a target node.
/// The value indicates the magnitude of the flow.
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyLink {
    /// Identifier of the source node
    pub source: String,
    /// Identifier of the target node
    pub target: String,
    /// Magnitude of the flow (must be positive)
    pub value: f64,
}

/// Timeline diagram representation
///
/// Timeline diagrams display chronological sequences of events or periods.
/// They are useful for showing historical progressions, project timelines,
/// or any time-based data.
///
/// # Example
///
/// ```
/// use mermaid_parser::common::ast::{TimelineDiagram, TimelineSection, TimelineItem, AccessibilityInfo};
///
/// let diagram = TimelineDiagram {
///     title: Some("Project Timeline".to_string()),
///     accessibility: AccessibilityInfo::default(),
///     sections: vec![
///         TimelineSection {
///             name: "Phase 1".to_string(),
///             items: vec![
///                 TimelineItem::Event("Project Start".to_string()),
///                 TimelineItem::Period("Development".to_string()),
///             ],
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineDiagram {
    /// Optional title for the timeline
    pub title: Option<String>,
    /// Accessibility information for screen readers
    pub accessibility: AccessibilityInfo,
    /// Chronological sections containing timeline items
    pub sections: Vec<TimelineSection>,
}

/// A section within a timeline diagram
///
/// Groups related timeline items under a common heading or time period.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSection {
    /// Name or heading for this section
    pub name: String,
    /// Items (events or periods) within this section
    pub items: Vec<TimelineItem>,
}

/// Individual items that can appear in a timeline
///
/// Timeline items represent either discrete events or time periods.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineItem {
    /// A specific time period or duration
    Period(String),
    /// A discrete event that occurred at a point in time
    Event(String),
}

/// User journey diagram representation
///
/// Journey diagrams map user experiences through a process or service,
/// showing tasks, satisfaction scores, and the actors involved at each step.
///
/// # Example
///
/// ```
/// use mermaid_parser::common::ast::{JourneyDiagram, JourneySection, JourneyTask, AccessibilityInfo};
///
/// let diagram = JourneyDiagram {
///     title: Some("Customer Journey".to_string()),
///     accessibility: AccessibilityInfo::default(),
///     sections: vec![
///         JourneySection {
///             name: "Online Shopping".to_string(),
///             tasks: vec![
///                 JourneyTask {
///                     name: "Browse Products".to_string(),
///                     score: 5,
///                     actors: vec!["Customer".to_string()],
///                 },
///             ],
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct JourneyDiagram {
    /// Optional title for the journey
    pub title: Option<String>,
    /// Accessibility information for screen readers
    pub accessibility: AccessibilityInfo,
    /// Sections grouping related journey tasks
    pub sections: Vec<JourneySection>,
}

/// A section within a journey diagram
///
/// Groups related tasks or steps in the user journey under a common theme.
#[derive(Debug, Clone, PartialEq)]
pub struct JourneySection {
    /// Name or heading for this section of the journey
    pub name: String,
    /// Tasks performed within this section
    pub tasks: Vec<JourneyTask>,
}

/// A specific task or step in a user journey
///
/// Represents an action taken by users, with an associated satisfaction score
/// and the actors involved in performing the task.
#[derive(Debug, Clone, PartialEq)]
pub struct JourneyTask {
    /// Name or description of the task
    pub name: String,
    /// Satisfaction score (typically 1-5, where 5 is most satisfied)
    pub score: i32,
    /// List of actors (roles/personas) involved in this task
    pub actors: Vec<String>,
}

/// Sequence diagram representation
///
/// Sequence diagrams show interactions between participants over time,
/// displaying the order of message exchanges and method calls.
///
/// # Example
///
/// ```
/// use mermaid_parser::common::ast::{SequenceDiagram, Participant, ParticipantType, AccessibilityInfo};
///
/// let diagram = SequenceDiagram {
///     title: Some("API Interaction".to_string()),
///     accessibility: AccessibilityInfo::default(),
///     participants: vec![
///         Participant {
///             actor: "Client".to_string(),
///             alias: None,
///             participant_type: ParticipantType::Actor,
///         },
///     ],
///     statements: vec![],
///     autonumber: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceDiagram {
    /// Optional title for the sequence diagram
    pub title: Option<String>,
    /// Accessibility information for screen readers
    pub accessibility: AccessibilityInfo,
    /// List of participants in the sequence
    pub participants: Vec<Participant>,
    /// Sequence of statements (messages, notes, etc.)
    pub statements: Vec<SequenceStatement>,
    /// Optional automatic numbering configuration
    pub autonumber: Option<AutoNumber>,
}

/// A participant in a sequence diagram
///
/// Represents an actor, object, or system component that can send and receive messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    /// The name/identifier of the participant
    pub actor: String,
    /// Optional alias or display name for the participant
    pub alias: Option<String>,
    /// Type of participant (actor, boundary, control, entity, etc.)
    pub participant_type: ParticipantType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticipantType {
    Participant,
    Actor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SequenceStatement {
    Message(Message),
    Note(Note),
    Loop(Loop),
    Alt(Alternative),
    Opt(Optional),
    Par(Parallel),
    Critical(Critical),
    Activate(String),
    Deactivate(String),
    Create(Participant),
    Destroy(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub arrow_type: ArrowType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowType {
    SolidOpen,
    SolidClosed,
    DottedOpen,
    DottedClosed,
    Cross,
    Point,
    BiDirectionalSolid,
    BiDirectionalDotted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub position: NotePosition,
    pub actor: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePosition {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    pub condition: String,
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    pub condition: String,
    pub statements: Vec<SequenceStatement>,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElseBranch {
    pub condition: Option<String>,
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Optional {
    pub condition: String,
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parallel {
    pub branches: Vec<ParallelBranch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParallelBranch {
    pub condition: Option<String>,
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Critical {
    pub condition: String,
    pub statements: Vec<SequenceStatement>,
    pub options: Vec<CriticalOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CriticalOption {
    pub condition: String,
    pub statements: Vec<SequenceStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoNumber {
    pub start: Option<i32>,
    pub step: Option<i32>,
    pub visible: bool,
}

// Placeholder types for other diagram types
// These will be expanded as parsers are implemented

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub classes: std::collections::HashMap<String, Class>,
    pub relationships: Vec<ClassRelationship>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub stereotype: Option<Stereotype>,
    pub members: Vec<ClassMember>,
    pub annotations: Vec<String>,
    pub css_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stereotype {
    Interface,
    Abstract,
    Service,
    Enumeration,
    Exception,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Property(Property),
    Method(Method),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub prop_type: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,    // +
    Private,   // -
    Protected, // #
    Package,   // ~
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassRelationship {
    pub from: String,
    pub to: String,
    pub relationship_type: ClassRelationshipType,
    pub from_cardinality: Option<String>,
    pub to_cardinality: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassRelationshipType {
    Inheritance, // <|--
    Composition, // *--
    Aggregation, // o--
    Association, // <--
    Link,        // --
    DashedLink,  // ..
    Dependency,  // <..
    Realization, // <|..
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub version: StateVersion,
    pub states: std::collections::HashMap<String, State>,
    pub transitions: Vec<StateTransition>,
    pub notes: Vec<StateNote>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: String,
    pub display_name: Option<String>,
    pub state_type: StateType,
    pub substates: Vec<String>,               // IDs of child states
    pub concurrent_regions: Vec<Vec<String>>, // For parallel states
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateType {
    Simple,
    Composite,
    Start,  // [*] as source
    End,    // [*] as target
    Choice, // <<choice>>
    Fork,   // <<fork>>
    Join,   // <<join>>
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub event: Option<String>,
    pub guard: Option<String>,
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateNote {
    pub position: StateNotePosition,
    pub target: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateNotePosition {
    LeftOf,
    RightOf,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowchartDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub direction: FlowDirection,
    pub nodes: std::collections::HashMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>,
    pub styles: Vec<StyleDefinition>,
    pub class_defs: std::collections::HashMap<String, ClassDef>,
    pub clicks: Vec<ClickEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowDirection {
    TB, // Top to Bottom (same as TD)
    TD, // Top Down
    BT, // Bottom to Top
    RL, // Right to Left
    LR, // Left to Right
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub text: Option<String>,
    pub shape: NodeShape,
    pub classes: Vec<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Rectangle,        // [text]
    RoundedRectangle, // (text)
    Stadium,          // ([text])
    Subroutine,       // [[text]]
    Cylinder,         // [(text)]
    Circle,           // ((text))
    Asymmetric,       // >text]
    Rhombus,          // {text}
    Hexagon,          // {{text}}
    Parallelogram,    // [/text/]
    ParallelogramAlt, // [\text\]
    Trapezoid,        // [/text\]
    TrapezoidAlt,     // [\text/]
    DoubleCircle,     // (((text)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub min_length: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeType {
    Arrow,            // -->
    DottedArrow,      // -.->
    ThickArrow,       // ==>
    OpenLink,         // ---
    DottedLink,       // -.-
    ThickLink,        // ===
    Invisible,        // ~~~
    CircleEdge,       // --o
    CrossEdge,        // --x
    MultiDirectional, // <-->
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<String>, // Node IDs
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>, // Nested subgraphs
    pub direction: Option<FlowDirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDefinition {
    pub target: StyleTarget,
    pub styles: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StyleTarget {
    Node(String),
    Edge(String, String),
    Subgraph(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub styles: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClickEvent {
    pub node_id: String,
    pub action: ClickAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    Href(String, Option<String>),         // URL, target
    Callback(String),                     // Function name
    Both(String, String, Option<String>), // Callback, URL, target
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub date_format: Option<String>,
    pub axis_format: Option<String>,
    pub tick_interval: Option<String>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub today_marker: Option<String>,
    pub inclusive_end_dates: bool,
    pub top_axis: bool,
    pub weekdays: WeekdaySettings,
    pub sections: Vec<GanttSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttTask {
    pub name: String,
    pub id: Option<String>,
    pub start_date: Option<String>,
    pub duration: Option<String>,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub progress: Option<f32>,
    pub interactions: Vec<TaskInteraction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Active,
    Done,
    Critical,
    Milestone,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskInteraction {
    Click {
        task_id: String,
    },
    Href {
        url: String,
    },
    Call {
        function: String,
        args: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeekdaySettings {
    pub start_day: Option<Weekday>,
    pub weekend: Vec<Weekday>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub show_data: bool,
    pub data: Vec<PieSlice>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub theme: Option<String>,
    pub commits: Vec<GitCommit>,
    pub branches: Vec<GitBranch>,
    pub operations: Vec<GitOperation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitCommit {
    pub id: Option<String>,
    pub commit_type: CommitType,
    pub tag: Option<String>,
    pub branch: String, // Which branch this commit is on
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitType {
    Normal,
    Reverse,
    Highlight,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitBranch {
    pub name: String,
    pub order: Option<i32>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitOperation {
    Commit {
        id: Option<String>,
        commit_type: CommitType,
        tag: Option<String>,
    },
    Branch {
        name: String,
        order: Option<i32>,
    },
    Checkout {
        branch: String,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tag: Option<String>,
        commit_type: CommitType,
    },
    CherryPick {
        id: String,
        parent: Option<String>,
        tag: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub entities: std::collections::HashMap<String, Entity>,
    pub relationships: Vec<ErRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub attr_type: String,
    pub key_type: Option<KeyType>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyType {
    PK, // Primary Key
    FK, // Foreign Key
    UK, // Unique Key
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErRelationship {
    pub left_entity: String,
    pub right_entity: String,
    pub left_cardinality: ErCardinality,
    pub right_cardinality: ErCardinality,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErCardinality {
    pub min: CardinalityValue,
    pub max: CardinalityValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardinalityValue {
    Zero,
    One,
    Many,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Diagram {
    pub diagram_type: C4DiagramType,
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub elements: std::collections::HashMap<String, C4Element>,
    pub boundaries: Vec<C4Boundary>,
    pub relationships: Vec<C4Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4DiagramType {
    Context,
    Container,
    Component,
    Dynamic,
    Deployment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Element {
    pub id: String,
    pub element_type: C4ElementType,
    pub name: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub tags: Vec<String>,
    pub is_external: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4ElementType {
    Person,
    System,
    SystemDb,
    SystemQueue,
    Container,
    ContainerDb,
    ContainerQueue,
    Component,
    ComponentDb,
    ComponentQueue,
    Node,
    DeploymentNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Boundary {
    pub id: String,
    pub boundary_type: C4BoundaryType,
    pub label: String,
    pub tags: Vec<String>,
    pub elements: Vec<String>,       // Element IDs
    pub boundaries: Vec<C4Boundary>, // Nested boundaries
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4BoundaryType {
    System,
    Container,
    Enterprise,
    Generic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Relationship {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub technology: Option<String>,
    pub direction: C4RelationshipDirection,
    pub is_bidirectional: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4RelationshipDirection {
    Default,
    Up,
    Down,
    Left,
    Right,
    Back,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub root: MindmapNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapNode {
    pub id: String,
    pub text: String,
    pub shape: MindmapNodeShape,
    pub icon: Option<String>,
    pub class: Option<String>,
    pub children: Vec<MindmapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MindmapNodeShape {
    Default, // No brackets
    Square,  // [text]
    Rounded, // (text)
    Circle,  // ((text))
    Cloud,   // (-text-)
    Bang,    // ))text((
    Hexagon, // {{text}}
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuadrantDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub x_axis: Option<AxisDefinition>,
    pub y_axis: Option<AxisDefinition>,
    pub quadrants: QuadrantLabels,
    pub points: Vec<DataPoint>,
    pub styles: Vec<ClassDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxisDefinition {
    pub label_start: Option<String>,
    pub label_end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuadrantLabels {
    pub quadrant_1: Option<String>, // Top-right
    pub quadrant_2: Option<String>, // Top-left
    pub quadrant_3: Option<String>, // Bottom-left
    pub quadrant_4: Option<String>, // Bottom-right
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataPoint {
    pub name: String,
    pub x: f64, // 0.0 to 1.0
    pub y: f64, // 0.0 to 1.0
    pub class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDefinition {
    pub name: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XyChartDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub orientation: ChartOrientation,
    pub x_axis: XAxis,
    pub y_axis: YAxis,
    pub data_series: Vec<DataSeries>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartOrientation {
    Vertical, // Default
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XAxis {
    pub title: Option<String>,
    pub labels: Vec<String>,
    pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YAxis {
    pub title: Option<String>,
    pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataSeries {
    pub series_type: SeriesType,
    pub name: Option<String>,
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeriesType {
    Line,
    Bar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub sections: Vec<KanbanSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanSection {
    pub id: String,
    pub title: String,
    pub items: Vec<KanbanItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanItem {
    pub id: Option<String>,
    pub text: String,
    pub assigned: Vec<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub columns: Option<i32>,
    pub blocks: Vec<Block>,
    pub connections: Vec<BlockConnection>,
    pub styles: Vec<BlockStyleDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Simple {
        id: String,
        label: Option<String>,
        shape: BlockShape,
    },
    Composite {
        id: String,
        label: Option<String>,
        blocks: Vec<Block>,
    },
    Space {
        size: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockShape {
    Rectangle,      // Basic block
    RoundedRect,    // Rounded corners
    Rhombus,        // Diamond shape
    Circle,         // Circular
    Ellipse,        // Oval
    Cylinder,       // Database-style
    Custom(String), // Custom shape definition
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockConnection {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub arrow_type: BlockArrowType,
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockArrowType {
    Normal,        // -->
    Dotted,        // -.->
    Thick,         // ==>
    Invisible,     // ~~~
    Bidirectional, // <-->
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockStyleDefinition {
    pub target: String,
    pub properties: Vec<BlockStyleProperty>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockStyleProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchitectureDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub direction: ArchDirection,
    pub services: std::collections::HashMap<String, Service>,
    pub groups: std::collections::HashMap<String, Group>,
    pub junctions: std::collections::HashMap<String, Junction>,
    pub edges: Vec<ArchEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchDirection {
    TB, // Top to Bottom
    BT, // Bottom to Top
    LR, // Left to Right
    RL, // Right to Left
}

#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    pub id: String,
    pub icon: Option<String>,
    pub title: String,
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: String,
    pub in_group: Option<String>, // For nested groups
}

#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    pub id: String,
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchEdge {
    pub from: EdgeEndpoint,
    pub to: EdgeEndpoint,
    pub label: Option<String>,
    pub edge_type: ArchEdgeType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeEndpoint {
    pub id: String,
    pub port: Option<Port>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Port {
    Left,   // L
    Right,  // R
    Top,    // T
    Bottom, // B
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchEdgeType {
    Solid,   // --
    Dotted,  // ..
    Arrow,   // ->
    BiArrow, // <->
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub fields: Vec<PacketField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketField {
    pub start_bit: u32,
    pub end_bit: u32,
    pub name: String,
    pub is_optional: bool, // Indicated by parentheses
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub requirements: std::collections::HashMap<String, Requirement>,
    pub elements: std::collections::HashMap<String, Element>,
    pub relationships: Vec<RequirementRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    pub name: String,
    pub req_type: RequirementType,
    pub id: String,
    pub text: String,
    pub risk: Option<RiskLevel>,
    pub verify_method: Option<VerificationMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequirementType {
    Requirement,
    FunctionalRequirement,
    PerformanceRequirement,
    InterfaceRequirement,
    PhysicalRequirement,
    DesignConstraint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerificationMethod {
    Analysis,
    Inspection,
    Test,
    Demonstration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub element_type: String,
    pub doc_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementRelationship {
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub root: TreemapNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNode {
    pub name: String,
    pub value: Option<f64>,
    pub children: Vec<TreemapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub config: RadarConfig,
    pub axes: Vec<String>,
    pub datasets: Vec<Dataset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarConfig {
    pub background_color: Option<String>,
    pub grid_color: Option<String>,
    pub scale_max: f64,
    pub scale_min: f64,
}

impl Default for RadarConfig {
    fn default() -> Self {
        RadarConfig {
            background_color: None,
            grid_color: None,
            scale_max: 100.0,
            scale_min: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiscDiagram {
    pub diagram_type: String,
    pub content: MiscContent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MiscContent {
    Info(InfoDiagram),
    GitGraph(GitGraphAlt),
    Raw(RawDiagram),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InfoDiagram {
    pub command: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphAlt {
    pub commits: Vec<MiscGitCommit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiscGitCommit {
    pub action: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawDiagram {
    pub lines: Vec<String>,
}
