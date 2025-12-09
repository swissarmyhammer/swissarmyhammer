//! AST visitor pattern for traversing and analyzing Mermaid diagram structures
//!
//! This module provides visitor traits and common utility visitors for working with
//! parsed Mermaid diagrams. The visitor pattern allows users to traverse the AST
//! and perform various analyses or transformations.
//!
//! # Example Usage
//!
//! ```rust
//! use mermaid_parser::common::visitor::{AstVisitor, NodeCounter};
//! use mermaid_parser::parse_diagram;
//!
//! let input = "flowchart TD\n    A --> B\n    B --> C";
//! let diagram = parse_diagram(input).unwrap();
//!
//! let mut counter = NodeCounter::new();
//! diagram.accept(&mut counter);
//! println!("Found {} nodes and {} edges", counter.nodes(), counter.edges());
//! ```

use crate::common::ast::*;

/// Immutable visitor trait for traversing AST nodes
pub trait AstVisitor {
    type Result;

    /// Visit any diagram type
    fn visit_diagram(&mut self, diagram: &DiagramType) -> Self::Result {
        match diagram {
            DiagramType::Sankey(d) => self.visit_sankey(d),
            DiagramType::Timeline(d) => self.visit_timeline(d),
            DiagramType::Journey(d) => self.visit_journey(d),
            DiagramType::Sequence(d) => self.visit_sequence(d),
            DiagramType::Class(d) => self.visit_class(d),
            DiagramType::State(d) => self.visit_state(d),
            DiagramType::Flowchart(d) => self.visit_flowchart(d),
            DiagramType::Gantt(d) => self.visit_gantt(d),
            DiagramType::Pie(d) => self.visit_pie(d),
            DiagramType::Git(d) => self.visit_git(d),
            DiagramType::Er(d) => self.visit_er(d),
            DiagramType::C4(d) => self.visit_c4(d),
            DiagramType::Mindmap(d) => self.visit_mindmap(d),
            DiagramType::Quadrant(d) => self.visit_quadrant(d),
            DiagramType::XyChart(d) => self.visit_xychart(d),
            DiagramType::Kanban(d) => self.visit_kanban(d),
            DiagramType::Block(d) => self.visit_block(d),
            DiagramType::Architecture(d) => self.visit_architecture(d),
            DiagramType::Packet(d) => self.visit_packet(d),
            DiagramType::Requirement(d) => self.visit_requirement(d),
            DiagramType::Treemap(d) => self.visit_treemap(d),
            DiagramType::Radar(d) => self.visit_radar(d),
            DiagramType::Misc(d) => self.visit_misc(d),
        }
    }

    // Diagram type visitors
    fn visit_sankey(&mut self, diagram: &SankeyDiagram) -> Self::Result;
    fn visit_timeline(&mut self, diagram: &TimelineDiagram) -> Self::Result;
    fn visit_journey(&mut self, diagram: &JourneyDiagram) -> Self::Result;
    fn visit_sequence(&mut self, diagram: &SequenceDiagram) -> Self::Result;
    fn visit_class(&mut self, diagram: &ClassDiagram) -> Self::Result;
    fn visit_state(&mut self, diagram: &StateDiagram) -> Self::Result;
    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result;
    fn visit_gantt(&mut self, diagram: &GanttDiagram) -> Self::Result;
    fn visit_pie(&mut self, diagram: &PieDiagram) -> Self::Result;
    fn visit_git(&mut self, diagram: &GitDiagram) -> Self::Result;
    fn visit_er(&mut self, diagram: &ErDiagram) -> Self::Result;
    fn visit_c4(&mut self, diagram: &C4Diagram) -> Self::Result;
    fn visit_mindmap(&mut self, diagram: &MindmapDiagram) -> Self::Result;
    fn visit_quadrant(&mut self, diagram: &QuadrantDiagram) -> Self::Result;
    fn visit_xychart(&mut self, diagram: &XyChartDiagram) -> Self::Result;
    fn visit_kanban(&mut self, diagram: &KanbanDiagram) -> Self::Result;
    fn visit_block(&mut self, diagram: &BlockDiagram) -> Self::Result;
    fn visit_architecture(&mut self, diagram: &ArchitectureDiagram) -> Self::Result;
    fn visit_packet(&mut self, diagram: &PacketDiagram) -> Self::Result;
    fn visit_requirement(&mut self, diagram: &RequirementDiagram) -> Self::Result;
    fn visit_treemap(&mut self, diagram: &TreemapDiagram) -> Self::Result;
    fn visit_radar(&mut self, diagram: &RadarDiagram) -> Self::Result;
    fn visit_misc(&mut self, diagram: &MiscDiagram) -> Self::Result;

    // Common element visitors
    fn visit_sankey_node(&mut self, node: &SankeyNode) -> Self::Result;
    fn visit_sankey_link(&mut self, link: &SankeyLink) -> Self::Result;
    fn visit_flow_node(&mut self, node: &FlowNode) -> Self::Result;
    fn visit_flow_edge(&mut self, edge: &FlowEdge) -> Self::Result;
    fn visit_sequence_message(&mut self, message: &Message) -> Self::Result;
    fn visit_class_definition(&mut self, class: &Class) -> Self::Result;
    fn visit_state_node(&mut self, state: &State) -> Self::Result;
    fn visit_state_transition(&mut self, transition: &StateTransition) -> Self::Result;
}

/// Mutable visitor trait for modifying AST nodes
pub trait AstVisitorMut {
    type Result;

    /// Visit any diagram type with mutable access
    fn visit_diagram_mut(&mut self, diagram: &mut DiagramType) -> Self::Result {
        match diagram {
            DiagramType::Sankey(d) => self.visit_sankey_mut(d),
            DiagramType::Timeline(d) => self.visit_timeline_mut(d),
            DiagramType::Journey(d) => self.visit_journey_mut(d),
            DiagramType::Sequence(d) => self.visit_sequence_mut(d),
            DiagramType::Class(d) => self.visit_class_mut(d),
            DiagramType::State(d) => self.visit_state_mut(d),
            DiagramType::Flowchart(d) => self.visit_flowchart_mut(d),
            DiagramType::Gantt(d) => self.visit_gantt_mut(d),
            DiagramType::Pie(d) => self.visit_pie_mut(d),
            DiagramType::Git(d) => self.visit_git_mut(d),
            DiagramType::Er(d) => self.visit_er_mut(d),
            DiagramType::C4(d) => self.visit_c4_mut(d),
            DiagramType::Mindmap(d) => self.visit_mindmap_mut(d),
            DiagramType::Quadrant(d) => self.visit_quadrant_mut(d),
            DiagramType::XyChart(d) => self.visit_xychart_mut(d),
            DiagramType::Kanban(d) => self.visit_kanban_mut(d),
            DiagramType::Block(d) => self.visit_block_mut(d),
            DiagramType::Architecture(d) => self.visit_architecture_mut(d),
            DiagramType::Packet(d) => self.visit_packet_mut(d),
            DiagramType::Requirement(d) => self.visit_requirement_mut(d),
            DiagramType::Treemap(d) => self.visit_treemap_mut(d),
            DiagramType::Radar(d) => self.visit_radar_mut(d),
            DiagramType::Misc(d) => self.visit_misc_mut(d),
        }
    }

    // Mutable diagram type visitors
    fn visit_sankey_mut(&mut self, diagram: &mut SankeyDiagram) -> Self::Result;
    fn visit_timeline_mut(&mut self, diagram: &mut TimelineDiagram) -> Self::Result;
    fn visit_journey_mut(&mut self, diagram: &mut JourneyDiagram) -> Self::Result;
    fn visit_sequence_mut(&mut self, diagram: &mut SequenceDiagram) -> Self::Result;
    fn visit_class_mut(&mut self, diagram: &mut ClassDiagram) -> Self::Result;
    fn visit_state_mut(&mut self, diagram: &mut StateDiagram) -> Self::Result;
    fn visit_flowchart_mut(&mut self, diagram: &mut FlowchartDiagram) -> Self::Result;
    fn visit_gantt_mut(&mut self, diagram: &mut GanttDiagram) -> Self::Result;
    fn visit_pie_mut(&mut self, diagram: &mut PieDiagram) -> Self::Result;
    fn visit_git_mut(&mut self, diagram: &mut GitDiagram) -> Self::Result;
    fn visit_er_mut(&mut self, diagram: &mut ErDiagram) -> Self::Result;
    fn visit_c4_mut(&mut self, diagram: &mut C4Diagram) -> Self::Result;
    fn visit_mindmap_mut(&mut self, diagram: &mut MindmapDiagram) -> Self::Result;
    fn visit_quadrant_mut(&mut self, diagram: &mut QuadrantDiagram) -> Self::Result;
    fn visit_xychart_mut(&mut self, diagram: &mut XyChartDiagram) -> Self::Result;
    fn visit_kanban_mut(&mut self, diagram: &mut KanbanDiagram) -> Self::Result;
    fn visit_block_mut(&mut self, diagram: &mut BlockDiagram) -> Self::Result;
    fn visit_architecture_mut(&mut self, diagram: &mut ArchitectureDiagram) -> Self::Result;
    fn visit_packet_mut(&mut self, diagram: &mut PacketDiagram) -> Self::Result;
    fn visit_requirement_mut(&mut self, diagram: &mut RequirementDiagram) -> Self::Result;
    fn visit_treemap_mut(&mut self, diagram: &mut TreemapDiagram) -> Self::Result;
    fn visit_radar_mut(&mut self, diagram: &mut RadarDiagram) -> Self::Result;
    fn visit_misc_mut(&mut self, diagram: &mut MiscDiagram) -> Self::Result;
}

/// Add accept methods to DiagramType for visitor pattern
impl DiagramType {
    /// Accept an immutable visitor
    pub fn accept<V: AstVisitor>(&self, visitor: &mut V) -> V::Result {
        visitor.visit_diagram(self)
    }

    /// Accept a mutable visitor
    pub fn accept_mut<V: AstVisitorMut>(&mut self, visitor: &mut V) -> V::Result {
        visitor.visit_diagram_mut(self)
    }
}

/// Simple node and edge counter visitor
#[derive(Debug, Default)]
pub struct NodeCounter {
    nodes: usize,
    edges: usize,
    elements: usize,
}

impl NodeCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn nodes(&self) -> usize {
        self.nodes
    }

    pub fn edges(&self) -> usize {
        self.edges
    }

    pub fn elements(&self) -> usize {
        self.elements
    }

    pub fn total(&self) -> usize {
        self.nodes + self.edges + self.elements
    }
}

/// Complexity analyzer visitor that calculates diagram complexity metrics
#[derive(Debug, Default)]
pub struct ComplexityAnalyzer {
    depth: usize,
    max_depth: usize,
    current_depth: usize,
    branching_factor: usize,
    total_connections: usize,
}

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub fn average_branching_factor(&self) -> f64 {
        if self.total_connections == 0 {
            0.0
        } else {
            self.branching_factor as f64 / self.total_connections as f64
        }
    }

    pub fn cyclomatic_complexity(&self) -> usize {
        // Basic cyclomatic complexity: edges - nodes + 2
        if self.total_connections > 0 {
            self.total_connections.saturating_sub(self.depth) + 2
        } else {
            1
        }
    }

    fn enter_scope(&mut self) {
        self.current_depth += 1;
        self.max_depth = self.max_depth.max(self.current_depth);
    }

    fn exit_scope(&mut self) {
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    fn count_connection(&mut self) {
        self.total_connections += 1;
    }

    fn count_node(&mut self) {
        self.depth += 1;
    }
}

impl AstVisitor for ComplexityAnalyzer {
    type Result = ();

    fn visit_sequence(&mut self, diagram: &SequenceDiagram) -> Self::Result {
        self.count_node();
        for statement in &diagram.statements {
            self.visit_sequence_statement(statement);
        }
    }

    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        self.count_node();
        for _edge in &diagram.edges {
            self.count_connection();
        }
        for subgraph in &diagram.subgraphs {
            self.enter_scope();
            self.visit_subgraph(subgraph);
            self.exit_scope();
        }
    }

    fn visit_state(&mut self, diagram: &StateDiagram) -> Self::Result {
        self.count_node();
        for _transition in &diagram.transitions {
            self.count_connection();
        }
    }

    fn visit_class(&mut self, diagram: &ClassDiagram) -> Self::Result {
        self.count_node();
        for _relationship in &diagram.relationships {
            self.count_connection();
        }
    }

    // Default implementations for other diagram types
    fn visit_sankey(&mut self, diagram: &SankeyDiagram) -> Self::Result {
        self.depth += diagram.nodes.len();
        self.total_connections += diagram.links.len();
    }

    fn visit_timeline(&mut self, _diagram: &TimelineDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_journey(&mut self, _diagram: &JourneyDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_gantt(&mut self, _diagram: &GanttDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_pie(&mut self, _diagram: &PieDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_git(&mut self, diagram: &GitDiagram) -> Self::Result {
        self.depth += diagram.commits.len();
        self.total_connections += diagram.operations.len();
    }

    fn visit_er(&mut self, diagram: &ErDiagram) -> Self::Result {
        self.depth += diagram.entities.len();
        self.total_connections += diagram.relationships.len();
    }

    fn visit_c4(&mut self, diagram: &C4Diagram) -> Self::Result {
        self.depth += diagram.elements.len();
        self.total_connections += diagram.relationships.len();
    }

    fn visit_mindmap(&mut self, diagram: &MindmapDiagram) -> Self::Result {
        self.visit_mindmap_node(&diagram.root);
    }

    fn visit_quadrant(&mut self, _diagram: &QuadrantDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_xychart(&mut self, _diagram: &XyChartDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_kanban(&mut self, _diagram: &KanbanDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_block(&mut self, diagram: &BlockDiagram) -> Self::Result {
        self.depth += diagram.blocks.len();
        self.total_connections += diagram.connections.len();
    }

    fn visit_architecture(&mut self, diagram: &ArchitectureDiagram) -> Self::Result {
        self.depth += diagram.services.len();
        self.total_connections += diagram.edges.len();
    }

    fn visit_packet(&mut self, _diagram: &PacketDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_requirement(&mut self, diagram: &RequirementDiagram) -> Self::Result {
        self.depth += diagram.requirements.len();
        self.total_connections += diagram.relationships.len();
    }

    fn visit_treemap(&mut self, diagram: &TreemapDiagram) -> Self::Result {
        self.visit_treemap_node(&diagram.root);
    }

    fn visit_radar(&mut self, _diagram: &RadarDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_misc(&mut self, _diagram: &MiscDiagram) -> Self::Result {
        self.count_node();
    }

    fn visit_sankey_node(&mut self, _node: &SankeyNode) -> Self::Result {
        self.count_node();
    }

    fn visit_sankey_link(&mut self, _link: &SankeyLink) -> Self::Result {
        self.count_connection();
    }

    fn visit_flow_node(&mut self, _node: &FlowNode) -> Self::Result {
        self.count_node();
    }

    fn visit_flow_edge(&mut self, _edge: &FlowEdge) -> Self::Result {
        self.count_connection();
    }

    fn visit_sequence_message(&mut self, _message: &Message) -> Self::Result {
        self.count_connection();
    }

    fn visit_class_definition(&mut self, _class: &Class) -> Self::Result {
        self.count_node();
    }

    fn visit_state_node(&mut self, _state: &State) -> Self::Result {
        self.count_node();
    }

    fn visit_state_transition(&mut self, _transition: &StateTransition) -> Self::Result {
        self.count_connection();
    }
}

impl ComplexityAnalyzer {
    fn visit_sequence_statement(&mut self, statement: &SequenceStatement) {
        match statement {
            SequenceStatement::Message(_) => self.count_connection(),
            SequenceStatement::Loop(loop_stmt) => {
                self.enter_scope();
                for stmt in &loop_stmt.statements {
                    self.visit_sequence_statement(stmt);
                }
                self.exit_scope();
            }
            SequenceStatement::Alt(alt) => {
                self.enter_scope();
                for stmt in &alt.statements {
                    self.visit_sequence_statement(stmt);
                }
                if let Some(else_branch) = &alt.else_branch {
                    for stmt in &else_branch.statements {
                        self.visit_sequence_statement(stmt);
                    }
                }
                self.exit_scope();
            }
            SequenceStatement::Opt(opt) => {
                self.enter_scope();
                for stmt in &opt.statements {
                    self.visit_sequence_statement(stmt);
                }
                self.exit_scope();
            }
            SequenceStatement::Par(par) => {
                self.enter_scope();
                for branch in &par.branches {
                    for stmt in &branch.statements {
                        self.visit_sequence_statement(stmt);
                    }
                }
                self.exit_scope();
            }
            SequenceStatement::Critical(critical) => {
                self.enter_scope();
                for stmt in &critical.statements {
                    self.visit_sequence_statement(stmt);
                }
                for option in &critical.options {
                    for stmt in &option.statements {
                        self.visit_sequence_statement(stmt);
                    }
                }
                self.exit_scope();
            }
            _ => {} // Other statement types
        }
    }

    fn visit_subgraph(&mut self, subgraph: &Subgraph) {
        self.count_node();
        for _edge in &subgraph.edges {
            self.count_connection();
        }
        for nested in &subgraph.subgraphs {
            self.enter_scope();
            self.visit_subgraph(nested);
            self.exit_scope();
        }
    }

    fn visit_mindmap_node(&mut self, node: &MindmapNode) {
        self.count_node();
        self.enter_scope();
        for child in &node.children {
            self.visit_mindmap_node(child);
        }
        self.exit_scope();
    }

    fn visit_treemap_node(&mut self, node: &TreemapNode) {
        self.count_node();
        self.enter_scope();
        for child in &node.children {
            self.visit_treemap_node(child);
        }
        self.exit_scope();
    }
}

/// Reference validator visitor that checks for undefined references
#[derive(Debug, Default)]
pub struct ReferenceValidator {
    errors: Vec<String>,
    defined_ids: std::collections::HashSet<String>,
    referenced_ids: std::collections::HashSet<String>,
}

impl ReferenceValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn undefined_references(&self) -> Vec<String> {
        self.referenced_ids
            .difference(&self.defined_ids)
            .cloned()
            .collect()
    }

    fn define_id(&mut self, id: &str) {
        self.defined_ids.insert(id.to_string());
    }

    fn reference_id(&mut self, id: &str) {
        self.referenced_ids.insert(id.to_string());
    }

    fn validate_references(&mut self) {
        for undefined in self.undefined_references() {
            self.errors
                .push(format!("Undefined reference: {}", undefined));
        }
    }
}

impl AstVisitor for ReferenceValidator {
    type Result = ();

    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        // Define all node IDs
        for id in diagram.nodes.keys() {
            self.define_id(id);
        }

        // Check edge references
        for edge in &diagram.edges {
            self.reference_id(&edge.from);
            self.reference_id(&edge.to);
        }

        self.validate_references();
    }

    fn visit_state(&mut self, diagram: &StateDiagram) -> Self::Result {
        // Define all state IDs
        for id in diagram.states.keys() {
            self.define_id(id);
        }

        // Check transition references
        for transition in &diagram.transitions {
            self.reference_id(&transition.from);
            self.reference_id(&transition.to);
        }

        self.validate_references();
    }

    fn visit_class(&mut self, diagram: &ClassDiagram) -> Self::Result {
        // Define all class names
        for name in diagram.classes.keys() {
            self.define_id(name);
        }

        // Check relationship references
        for relationship in &diagram.relationships {
            self.reference_id(&relationship.from);
            self.reference_id(&relationship.to);
        }

        self.validate_references();
    }

    fn visit_er(&mut self, diagram: &ErDiagram) -> Self::Result {
        // Define all entity names
        for name in diagram.entities.keys() {
            self.define_id(name);
        }

        // Check relationship references
        for relationship in &diagram.relationships {
            self.reference_id(&relationship.left_entity);
            self.reference_id(&relationship.right_entity);
        }

        self.validate_references();
    }

    // Default implementations for other diagram types
    fn visit_sankey(&mut self, _diagram: &SankeyDiagram) -> Self::Result {}
    fn visit_timeline(&mut self, _diagram: &TimelineDiagram) -> Self::Result {}
    fn visit_journey(&mut self, _diagram: &JourneyDiagram) -> Self::Result {}
    fn visit_sequence(&mut self, _diagram: &SequenceDiagram) -> Self::Result {}
    fn visit_gantt(&mut self, _diagram: &GanttDiagram) -> Self::Result {}
    fn visit_pie(&mut self, _diagram: &PieDiagram) -> Self::Result {}
    fn visit_git(&mut self, _diagram: &GitDiagram) -> Self::Result {}
    fn visit_c4(&mut self, _diagram: &C4Diagram) -> Self::Result {}
    fn visit_mindmap(&mut self, _diagram: &MindmapDiagram) -> Self::Result {}
    fn visit_quadrant(&mut self, _diagram: &QuadrantDiagram) -> Self::Result {}
    fn visit_xychart(&mut self, _diagram: &XyChartDiagram) -> Self::Result {}
    fn visit_kanban(&mut self, _diagram: &KanbanDiagram) -> Self::Result {}
    fn visit_block(&mut self, _diagram: &BlockDiagram) -> Self::Result {}
    fn visit_architecture(&mut self, _diagram: &ArchitectureDiagram) -> Self::Result {}
    fn visit_packet(&mut self, _diagram: &PacketDiagram) -> Self::Result {}
    fn visit_requirement(&mut self, _diagram: &RequirementDiagram) -> Self::Result {}
    fn visit_treemap(&mut self, _diagram: &TreemapDiagram) -> Self::Result {}
    fn visit_radar(&mut self, _diagram: &RadarDiagram) -> Self::Result {}
    fn visit_misc(&mut self, _diagram: &MiscDiagram) -> Self::Result {}

    fn visit_sankey_node(&mut self, _node: &SankeyNode) -> Self::Result {}
    fn visit_sankey_link(&mut self, _link: &SankeyLink) -> Self::Result {}
    fn visit_flow_node(&mut self, _node: &FlowNode) -> Self::Result {}
    fn visit_flow_edge(&mut self, _edge: &FlowEdge) -> Self::Result {}
    fn visit_sequence_message(&mut self, _message: &Message) -> Self::Result {}
    fn visit_class_definition(&mut self, _class: &Class) -> Self::Result {}
    fn visit_state_node(&mut self, _state: &State) -> Self::Result {}
    fn visit_state_transition(&mut self, _transition: &StateTransition) -> Self::Result {}
}

/// A simple mutable visitor that can set titles on diagrams that support them
#[derive(Debug)]
pub struct TitleSetter {
    pub title: String,
}

impl TitleSetter {
    pub fn new(title: String) -> Self {
        Self { title }
    }

    fn set_title(&self, title_field: &mut Option<String>) {
        *title_field = Some(self.title.clone());
    }
}

impl AstVisitorMut for TitleSetter {
    type Result = ();

    fn visit_sankey_mut(&mut self, _diagram: &mut SankeyDiagram) -> Self::Result {
        // Sankey diagrams don't have titles in the current AST
    }

    fn visit_timeline_mut(&mut self, diagram: &mut TimelineDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_journey_mut(&mut self, diagram: &mut JourneyDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_sequence_mut(&mut self, diagram: &mut SequenceDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_class_mut(&mut self, diagram: &mut ClassDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_state_mut(&mut self, diagram: &mut StateDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_flowchart_mut(&mut self, diagram: &mut FlowchartDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_gantt_mut(&mut self, diagram: &mut GanttDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_pie_mut(&mut self, diagram: &mut PieDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_git_mut(&mut self, diagram: &mut GitDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_er_mut(&mut self, diagram: &mut ErDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_c4_mut(&mut self, diagram: &mut C4Diagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_mindmap_mut(&mut self, diagram: &mut MindmapDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_quadrant_mut(&mut self, diagram: &mut QuadrantDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_xychart_mut(&mut self, diagram: &mut XyChartDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_kanban_mut(&mut self, diagram: &mut KanbanDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_block_mut(&mut self, diagram: &mut BlockDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_architecture_mut(&mut self, diagram: &mut ArchitectureDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_packet_mut(&mut self, diagram: &mut PacketDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_requirement_mut(&mut self, diagram: &mut RequirementDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_treemap_mut(&mut self, diagram: &mut TreemapDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_radar_mut(&mut self, diagram: &mut RadarDiagram) -> Self::Result {
        self.set_title(&mut diagram.title);
    }

    fn visit_misc_mut(&mut self, _diagram: &mut MiscDiagram) -> Self::Result {
        // Misc diagrams don't have titles in the current AST
    }
}

impl AstVisitor for NodeCounter {
    type Result = ();

    fn visit_sankey(&mut self, diagram: &SankeyDiagram) -> Self::Result {
        self.nodes += diagram.nodes.len();
        self.edges += diagram.links.len();
    }

    fn visit_timeline(&mut self, diagram: &TimelineDiagram) -> Self::Result {
        for section in &diagram.sections {
            self.elements += section.items.len();
        }
    }

    fn visit_journey(&mut self, diagram: &JourneyDiagram) -> Self::Result {
        for section in &diagram.sections {
            self.elements += section.tasks.len();
        }
    }

    fn visit_sequence(&mut self, diagram: &SequenceDiagram) -> Self::Result {
        self.nodes += diagram.participants.len();
        self.elements += diagram.statements.len();
    }

    fn visit_class(&mut self, diagram: &ClassDiagram) -> Self::Result {
        self.nodes += diagram.classes.len();
        self.edges += diagram.relationships.len();
    }

    fn visit_state(&mut self, diagram: &StateDiagram) -> Self::Result {
        self.nodes += diagram.states.len();
        self.edges += diagram.transitions.len();
    }

    fn visit_flowchart(&mut self, diagram: &FlowchartDiagram) -> Self::Result {
        self.nodes += diagram.nodes.len();
        self.edges += diagram.edges.len();
    }

    fn visit_gantt(&mut self, diagram: &GanttDiagram) -> Self::Result {
        for section in &diagram.sections {
            self.elements += section.tasks.len();
        }
    }

    fn visit_pie(&mut self, diagram: &PieDiagram) -> Self::Result {
        self.elements += diagram.data.len();
    }

    fn visit_git(&mut self, diagram: &GitDiagram) -> Self::Result {
        self.nodes += diagram.commits.len();
        self.elements += diagram.operations.len();
    }

    fn visit_er(&mut self, diagram: &ErDiagram) -> Self::Result {
        self.nodes += diagram.entities.len();
        self.edges += diagram.relationships.len();
    }

    fn visit_c4(&mut self, diagram: &C4Diagram) -> Self::Result {
        self.nodes += diagram.elements.len();
        self.edges += diagram.relationships.len();
        self.elements += diagram.boundaries.len();
    }

    fn visit_mindmap(&mut self, diagram: &MindmapDiagram) -> Self::Result {
        self.visit_mindmap_node(&diagram.root);
    }

    fn visit_quadrant(&mut self, diagram: &QuadrantDiagram) -> Self::Result {
        self.elements += diagram.points.len();
    }

    fn visit_xychart(&mut self, diagram: &XyChartDiagram) -> Self::Result {
        self.elements += diagram.data_series.len();
    }

    fn visit_kanban(&mut self, diagram: &KanbanDiagram) -> Self::Result {
        for section in &diagram.sections {
            self.elements += section.items.len();
        }
    }

    fn visit_block(&mut self, diagram: &BlockDiagram) -> Self::Result {
        self.nodes += diagram.blocks.len();
        self.edges += diagram.connections.len();
    }

    fn visit_architecture(&mut self, diagram: &ArchitectureDiagram) -> Self::Result {
        self.nodes += diagram.services.len();
        self.edges += diagram.edges.len();
        self.elements += diagram.groups.len() + diagram.junctions.len();
    }

    fn visit_packet(&mut self, diagram: &PacketDiagram) -> Self::Result {
        self.elements += diagram.fields.len();
    }

    fn visit_requirement(&mut self, diagram: &RequirementDiagram) -> Self::Result {
        self.nodes += diagram.requirements.len();
        self.elements += diagram.elements.len();
        self.edges += diagram.relationships.len();
    }

    fn visit_treemap(&mut self, diagram: &TreemapDiagram) -> Self::Result {
        self.visit_treemap_node(&diagram.root);
    }

    fn visit_radar(&mut self, diagram: &RadarDiagram) -> Self::Result {
        self.elements += diagram.datasets.len();
    }

    fn visit_misc(&mut self, _diagram: &MiscDiagram) -> Self::Result {
        self.elements += 1;
    }

    fn visit_sankey_node(&mut self, _node: &SankeyNode) -> Self::Result {
        self.nodes += 1;
    }

    fn visit_sankey_link(&mut self, _link: &SankeyLink) -> Self::Result {
        self.edges += 1;
    }

    fn visit_flow_node(&mut self, _node: &FlowNode) -> Self::Result {
        self.nodes += 1;
    }

    fn visit_flow_edge(&mut self, _edge: &FlowEdge) -> Self::Result {
        self.edges += 1;
    }

    fn visit_sequence_message(&mut self, _message: &Message) -> Self::Result {
        self.elements += 1;
    }

    fn visit_class_definition(&mut self, _class: &Class) -> Self::Result {
        self.nodes += 1;
    }

    fn visit_state_node(&mut self, _state: &State) -> Self::Result {
        self.nodes += 1;
    }

    fn visit_state_transition(&mut self, _transition: &StateTransition) -> Self::Result {
        self.edges += 1;
    }
}

impl NodeCounter {
    fn visit_mindmap_node(&mut self, node: &MindmapNode) {
        self.nodes += 1;
        for child in &node.children {
            self.visit_mindmap_node(child);
        }
    }

    fn visit_treemap_node(&mut self, node: &TreemapNode) {
        self.nodes += 1;
        for child in &node.children {
            self.visit_treemap_node(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_counter_with_sankey() {
        let diagram = SankeyDiagram {
            nodes: vec![
                SankeyNode {
                    id: "A".to_string(),
                    name: "Node A".to_string(),
                },
                SankeyNode {
                    id: "B".to_string(),
                    name: "Node B".to_string(),
                },
            ],
            links: vec![SankeyLink {
                source: "A".to_string(),
                target: "B".to_string(),
                value: 10.0,
            }],
        };

        let mut counter = NodeCounter::new();
        counter.visit_sankey(&diagram);

        assert_eq!(counter.nodes(), 2);
        assert_eq!(counter.edges(), 1);
        assert_eq!(counter.elements(), 0);
        assert_eq!(counter.total(), 3);
    }

    #[test]
    fn test_diagram_accept_method() {
        let diagram = DiagramType::Sankey(SankeyDiagram {
            nodes: vec![SankeyNode {
                id: "A".to_string(),
                name: "Node A".to_string(),
            }],
            links: vec![],
        });

        let mut counter = NodeCounter::new();
        diagram.accept(&mut counter);

        assert_eq!(counter.nodes(), 1);
        assert_eq!(counter.edges(), 0);
    }

    #[test]
    fn test_complexity_analyzer_with_flowchart() {
        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: crate::common::ast::FlowDirection::TD,
            nodes: std::collections::HashMap::new(),
            edges: vec![
                FlowEdge {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    edge_type: crate::common::ast::EdgeType::Arrow,
                    label: None,
                    min_length: None,
                },
                FlowEdge {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    edge_type: crate::common::ast::EdgeType::Arrow,
                    label: None,
                    min_length: None,
                },
            ],
            subgraphs: vec![],
            styles: vec![],
            class_defs: std::collections::HashMap::new(),
            clicks: vec![],
        };

        let mut analyzer = ComplexityAnalyzer::new();
        analyzer.visit_flowchart(&diagram);

        assert_eq!(analyzer.cyclomatic_complexity(), 3); // 2 edges - 1 node + 2
        assert_eq!(analyzer.max_depth(), 0); // No nested structures
    }

    #[test]
    fn test_reference_validator_with_valid_flowchart() {
        use std::collections::HashMap;

        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Node A".to_string()),
                shape: crate::common::ast::NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );
        nodes.insert(
            "B".to_string(),
            FlowNode {
                id: "B".to_string(),
                text: Some("Node B".to_string()),
                shape: crate::common::ast::NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: crate::common::ast::FlowDirection::TD,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: crate::common::ast::EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: std::collections::HashMap::new(),
            clicks: vec![],
        };

        let mut validator = ReferenceValidator::new();
        validator.visit_flowchart(&diagram);

        assert!(!validator.has_errors());
        assert_eq!(validator.undefined_references().len(), 0);
    }

    #[test]
    fn test_reference_validator_with_invalid_flowchart() {
        use std::collections::HashMap;

        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Node A".to_string()),
                shape: crate::common::ast::NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: crate::common::ast::FlowDirection::TD,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "UNDEFINED".to_string(), // This should trigger an error
                edge_type: crate::common::ast::EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: std::collections::HashMap::new(),
            clicks: vec![],
        };

        let mut validator = ReferenceValidator::new();
        validator.visit_flowchart(&diagram);

        assert!(validator.has_errors());
        assert_eq!(validator.undefined_references().len(), 1);
        assert_eq!(validator.undefined_references()[0], "UNDEFINED");
    }

    #[test]
    fn test_multiple_visitors_on_same_diagram() {
        let diagram = DiagramType::Sankey(SankeyDiagram {
            nodes: vec![
                SankeyNode {
                    id: "A".to_string(),
                    name: "Node A".to_string(),
                },
                SankeyNode {
                    id: "B".to_string(),
                    name: "Node B".to_string(),
                },
                SankeyNode {
                    id: "C".to_string(),
                    name: "Node C".to_string(),
                },
            ],
            links: vec![
                SankeyLink {
                    source: "A".to_string(),
                    target: "B".to_string(),
                    value: 10.0,
                },
                SankeyLink {
                    source: "B".to_string(),
                    target: "C".to_string(),
                    value: 5.0,
                },
            ],
        });

        // Test NodeCounter
        let mut counter = NodeCounter::new();
        diagram.accept(&mut counter);
        assert_eq!(counter.nodes(), 3);
        assert_eq!(counter.edges(), 2);
        assert_eq!(counter.total(), 5);

        // Test ComplexityAnalyzer
        let mut analyzer = ComplexityAnalyzer::new();
        diagram.accept(&mut analyzer);
        // For Sankey: 3 nodes, 2 links -> depth=3, total_connections=2
        // Cyclomatic complexity = max(0, 2 - 3) + 2 = 0 + 2 = 2
        assert_eq!(analyzer.cyclomatic_complexity(), 2);
    }

    #[test]
    fn test_mutable_visitor_title_setter() {
        let mut diagram = DiagramType::Timeline(TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });

        let mut title_setter = TitleSetter::new("My Custom Title".to_string());
        diagram.accept_mut(&mut title_setter);

        if let DiagramType::Timeline(timeline) = &diagram {
            assert_eq!(timeline.title, Some("My Custom Title".to_string()));
        } else {
            panic!("Expected timeline diagram");
        }
    }

    #[test]
    fn test_mutable_visitor_title_setter_multiple_types() {
        let mut flowchart = DiagramType::Flowchart(FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: crate::common::ast::FlowDirection::TD,
            nodes: std::collections::HashMap::new(),
            edges: vec![],
            subgraphs: vec![],
            styles: vec![],
            class_defs: std::collections::HashMap::new(),
            clicks: vec![],
        });

        let mut sequence = DiagramType::Sequence(SequenceDiagram {
            title: Some("Old Title".to_string()),
            accessibility: AccessibilityInfo::default(),
            participants: vec![],
            statements: vec![],
            autonumber: None,
        });

        let mut title_setter = TitleSetter::new("Universal Title".to_string());

        flowchart.accept_mut(&mut title_setter);
        sequence.accept_mut(&mut title_setter);

        if let DiagramType::Flowchart(fc) = &flowchart {
            assert_eq!(fc.title, Some("Universal Title".to_string()));
        } else {
            panic!("Expected flowchart diagram");
        }

        if let DiagramType::Sequence(seq) = &sequence {
            assert_eq!(seq.title, Some("Universal Title".to_string()));
        } else {
            panic!("Expected sequence diagram");
        }
    }

    #[test]
    fn test_complexity_analyzer_all_methods() {
        let mut analyzer = ComplexityAnalyzer::new();

        // Test initial state
        assert_eq!(analyzer.max_depth(), 0);
        assert_eq!(analyzer.average_branching_factor(), 0.0);
        assert_eq!(analyzer.cyclomatic_complexity(), 1);

        // Test internal counting methods
        analyzer.count_node();
        analyzer.count_connection();
        analyzer.enter_scope();
        analyzer.enter_scope();
        assert_eq!(analyzer.max_depth(), 2);

        analyzer.exit_scope();
        analyzer.exit_scope();

        // Test average branching factor calculation
        // branching_factor / total_connections = 0 / 1 = 0.0 initially
        assert_eq!(analyzer.average_branching_factor(), 0.0);
        assert_eq!(analyzer.cyclomatic_complexity(), 2); // 1 - 1 + 2 = 2
    }

    #[test]
    fn test_reference_validator_error_methods() {
        let mut validator = ReferenceValidator::new();

        // Test initial state
        assert!(!validator.has_errors());
        assert_eq!(validator.errors().len(), 0);
        assert_eq!(validator.undefined_references().len(), 0);

        // Add some test data
        validator.define_id("existing");
        validator.reference_id("existing");
        validator.reference_id("missing");

        // Check undefined references
        let undefined = validator.undefined_references();
        assert_eq!(undefined.len(), 1);
        assert!(undefined.contains(&"missing".to_string()));

        // Validate and check errors
        validator.validate_references();
        assert!(validator.has_errors());
        assert_eq!(validator.errors().len(), 1);
        assert!(validator.errors()[0].contains("missing"));
    }

    #[test]
    fn test_title_setter_new() {
        let title = "Test Title".to_string();
        let setter = TitleSetter::new(title.clone());
        assert_eq!(setter.title, title);
    }
}
