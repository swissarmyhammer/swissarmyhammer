use crate::common::ast::*;

/// Trait for converting AST back to Mermaid syntax
///
/// This trait enables converting parsed diagram ASTs back into valid Mermaid syntax.
/// It supports both basic conversion and pretty-printing with customizable formatting options.
///
/// # Example
///
/// ```rust
/// use mermaid_parser::common::pretty_print::{MermaidPrinter, PrintOptions};
/// use mermaid_parser::parse_diagram;
///
/// let diagram = parse_diagram("flowchart TD\n    A --> B")?;
///
/// // Basic conversion
/// let basic = diagram.to_mermaid();
///
/// // Pretty-printed with custom options
/// let options = PrintOptions {
///     indent_width: 2,
///     align_arrows: true,
///     ..Default::default()
/// };
/// let pretty = diagram.to_mermaid_pretty(&options);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait MermaidPrinter {
    /// Convert the AST to Mermaid syntax using default formatting
    fn to_mermaid(&self) -> String;

    /// Convert the AST to Mermaid syntax with custom formatting options
    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String;
}

/// Options for pretty printing Mermaid diagrams
///
/// Configures various formatting aspects when converting ASTs back to Mermaid syntax.
/// These options allow control over indentation, line length, alignment, and other
/// stylistic choices.
///
/// # Example
///
/// ```rust
/// use mermaid_parser::common::pretty_print::PrintOptions;
///
/// let options = PrintOptions {
///     indent_width: 2,        // Use 2 spaces for indentation
///     max_line_length: 100,   // Wrap lines at 100 characters
///     align_arrows: true,     // Align arrow operators
///     sort_nodes: true,       // Sort nodes alphabetically
///     compact_mode: false,    // Use readable formatting
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PrintOptions {
    /// Number of spaces to use for each indentation level
    pub indent_width: usize,
    /// Maximum line length before wrapping (0 = no limit)
    pub max_line_length: usize,
    /// Whether to align arrow operators for better readability
    pub align_arrows: bool,
    /// Whether to sort nodes alphabetically in output
    pub sort_nodes: bool,
    /// Whether to use compact formatting (minimal whitespace)
    pub compact_mode: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            indent_width: 4,
            max_line_length: 80,
            align_arrows: false,
            sort_nodes: false,
            compact_mode: false,
        }
    }
}

impl MermaidPrinter for DiagramType {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        match self {
            DiagramType::Flowchart(f) => f.to_mermaid_pretty(options),
            DiagramType::Sequence(s) => s.to_mermaid_pretty(options),
            DiagramType::Class(c) => c.to_mermaid_pretty(options),
            DiagramType::State(s) => s.to_mermaid_pretty(options),
            DiagramType::Er(e) => e.to_mermaid_pretty(options),
            DiagramType::Gantt(g) => g.to_mermaid_pretty(options),
            DiagramType::Pie(p) => p.to_mermaid_pretty(options),
            DiagramType::Git(g) => g.to_mermaid_pretty(options),
            DiagramType::Mindmap(m) => m.to_mermaid_pretty(options),
            DiagramType::Journey(j) => j.to_mermaid_pretty(options),
            DiagramType::C4(c) => c.to_mermaid_pretty(options),
            DiagramType::Timeline(t) => t.to_mermaid_pretty(options),
            DiagramType::Sankey(s) => s.to_mermaid_pretty(options),
            DiagramType::Quadrant(q) => q.to_mermaid_pretty(options),
            DiagramType::XyChart(x) => x.to_mermaid_pretty(options),
            DiagramType::Kanban(k) => k.to_mermaid_pretty(options),
            DiagramType::Block(b) => b.to_mermaid_pretty(options),
            DiagramType::Architecture(a) => a.to_mermaid_pretty(options),
            DiagramType::Packet(p) => p.to_mermaid_pretty(options),
            DiagramType::Requirement(r) => r.to_mermaid_pretty(options),
            DiagramType::Treemap(t) => t.to_mermaid_pretty(options),
            DiagramType::Radar(r) => r.to_mermaid_pretty(options),
            DiagramType::Misc(m) => m.to_mermaid_pretty(options),
        }
    }
}

// Helper struct for building formatted output
struct PrettyPrinter {
    output: String,
    options: PrintOptions,
    current_indent: usize,
}

impl PrettyPrinter {
    fn new(options: PrintOptions) -> Self {
        Self {
            output: String::new(),
            options,
            current_indent: 0,
        }
    }

    fn write_line(&mut self, content: &str) {
        if !self.output.is_empty() {
            self.output.push('\n');
        }

        if !self.options.compact_mode && self.current_indent > 0 {
            let indent = " ".repeat(self.current_indent * self.options.indent_width);
            self.output.push_str(&indent);
        }

        self.output.push_str(content);
    }

    #[allow(dead_code)]
    fn write(&mut self, content: &str) {
        self.output.push_str(content);
    }

    fn indent(&mut self) {
        if !self.options.compact_mode {
            self.current_indent += 1;
        }
    }

    fn dedent(&mut self) {
        if !self.options.compact_mode && self.current_indent > 0 {
            self.current_indent -= 1;
        }
    }

    fn finish(self) -> String {
        self.output
    }
}

// Flowchart implementation
impl MermaidPrinter for FlowchartDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        // Write diagram type and direction
        let direction = match &self.direction {
            FlowDirection::TD => "TD",
            FlowDirection::TB => "TB",
            FlowDirection::BT => "BT",
            FlowDirection::RL => "RL",
            FlowDirection::LR => "LR",
        };
        printer.write_line(&format!("flowchart {}", direction));

        // Add title if present
        if let Some(title) = &self.title {
            printer.indent();
            printer.write_line(&format!("title {}", title));
            printer.dedent();
        }

        // Add accessibility info
        printer.indent();
        if let Some(title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        if options.sort_nodes {
            // Write sorted nodes first when sort_nodes is enabled
            let mut sorted_node_ids: Vec<_> = self.nodes.keys().collect();
            sorted_node_ids.sort();

            for node_id in sorted_node_ids {
                if let Some(node) = self.nodes.get(node_id) {
                    write_flow_node(&mut printer, node_id, node);
                }
            }

            // Write edges without inline definitions
            if options.align_arrows {
                write_aligned_flow_edges(&mut printer, &self.edges);
            } else {
                for edge in &self.edges {
                    write_flow_edge(&mut printer, edge);
                }
            }
        } else {
            // Write edges with inline node definitions (modern Mermaid style)
            let mut defined_nodes: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            if options.align_arrows {
                write_aligned_flow_edges_with_smart_nodes(
                    &mut printer,
                    &self.edges,
                    &self.nodes,
                    &mut defined_nodes,
                );
            } else {
                for edge in &self.edges {
                    write_flow_edge_with_smart_nodes(
                        &mut printer,
                        edge,
                        &self.nodes,
                        &mut defined_nodes,
                    );
                }
            }
        }

        // Write any standalone nodes (not part of edges)
        let mut referenced_nodes: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for edge in &self.edges {
            referenced_nodes.insert(edge.from.clone());
            referenced_nodes.insert(edge.to.clone());
        }

        for (id, node) in &self.nodes {
            if !referenced_nodes.contains(id) {
                write_flow_node(&mut printer, id, node);
            }
        }

        // Write subgraphs
        for subgraph in &self.subgraphs {
            write_subgraph(&mut printer, subgraph);
        }

        // Write styles
        for style in &self.styles {
            write_style_definition(&mut printer, style);
        }

        // Write class definitions
        for (name, class_def) in &self.class_defs {
            let styles_str = class_def
                .styles
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            printer.write_line(&format!("classDef {} {}", name, styles_str));
        }

        // Write click events
        for click in &self.clicks {
            match &click.action {
                ClickAction::Href(url, target) => {
                    let target_str = target.as_deref().unwrap_or("_self");
                    printer.write_line(&format!(
                        "click {} \"{}\" \"{}\"",
                        click.node_id, url, target_str
                    ));
                }
                ClickAction::Callback(func) => {
                    printer.write_line(&format!("click {} call {}", click.node_id, func));
                }
                ClickAction::Both(callback, url, target) => {
                    let target_str = target.as_deref().unwrap_or("_self");
                    printer.write_line(&format!(
                        "click {} call {} \"{}\" \"{}\"",
                        click.node_id, callback, url, target_str
                    ));
                }
            }
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_flow_node(printer: &mut PrettyPrinter, id: &str, node: &FlowNode) {
    let text = node.text.as_deref().unwrap_or("");
    let shape_str = match &node.shape {
        NodeShape::Rectangle => format!("{}[{}]", id, text),
        NodeShape::RoundedRectangle => format!("{}({})", id, text),
        NodeShape::Stadium => format!("{}([{}])", id, text),
        NodeShape::Subroutine => format!("{}[[{}]]", id, text),
        NodeShape::Cylinder => format!("{}[({})]", id, text),
        NodeShape::Circle => format!("{}(({})))", id, text),
        NodeShape::Asymmetric => format!("{}>{}]", id, text),
        NodeShape::Rhombus => format!("{}{{{}}}", id, text),
        NodeShape::Hexagon => format!("{}{{{{{}}}}}", id, text),
        NodeShape::Parallelogram => format!("{}[/{}\\]", id, text),
        NodeShape::ParallelogramAlt => format!("{}[\\{}/]", id, text),
        NodeShape::Trapezoid => format!("{}[/{}/]", id, text),
        NodeShape::TrapezoidAlt => format!("{}[\\{}\\]", id, text),
        NodeShape::DoubleCircle => format!("{}((({})))", id, text),
    };
    printer.write_line(&shape_str);
}

fn write_subgraph(printer: &mut PrettyPrinter, subgraph: &Subgraph) {
    if let Some(title) = &subgraph.title {
        printer.write_line(&format!("subgraph {} [{}]", subgraph.id, title));
    } else {
        printer.write_line(&format!("subgraph {}", subgraph.id));
    }
    printer.indent();

    if let Some(direction) = &subgraph.direction {
        let dir_str = match direction {
            FlowDirection::TD => "TD",
            FlowDirection::TB => "TB",
            FlowDirection::BT => "BT",
            FlowDirection::RL => "RL",
            FlowDirection::LR => "LR",
        };
        printer.write_line(&format!("direction {}", dir_str));
    }

    // Write nodes in subgraph
    for node_id in &subgraph.nodes {
        printer.write_line(node_id);
    }

    // Write edges in subgraph
    for edge in &subgraph.edges {
        write_flow_edge(printer, edge);
    }

    // Write nested subgraphs
    for nested in &subgraph.subgraphs {
        write_subgraph(printer, nested);
    }

    printer.dedent();
    printer.write_line("end");
}

fn write_flow_edge_with_smart_nodes(
    printer: &mut PrettyPrinter,
    edge: &FlowEdge,
    nodes: &std::collections::HashMap<String, FlowNode>,
    defined_nodes: &mut std::collections::HashSet<String>,
) {
    let arrow = match &edge.edge_type {
        EdgeType::Arrow => "-->",
        EdgeType::DottedArrow => "-.->",
        EdgeType::ThickArrow => "==>",
        EdgeType::OpenLink => "---",
        EdgeType::DottedLink => "-.-",
        EdgeType::ThickLink => "===",
        EdgeType::Invisible => "~~~",
        EdgeType::CircleEdge => "--o",
        EdgeType::CrossEdge => "--x",
        EdgeType::MultiDirectional => "<-->",
    };

    // Format source node - use definition if not defined yet, otherwise just ID
    let source_str = if !defined_nodes.contains(&edge.from) {
        if let Some(source_node) = nodes.get(&edge.from) {
            defined_nodes.insert(edge.from.clone());
            format_node_with_definition(&edge.from, source_node)
        } else {
            edge.from.clone()
        }
    } else {
        edge.from.clone()
    };

    // Format target node - use definition if not defined yet, otherwise just ID
    let target_str = if !defined_nodes.contains(&edge.to) {
        if let Some(target_node) = nodes.get(&edge.to) {
            defined_nodes.insert(edge.to.clone());
            format_node_with_definition(&edge.to, target_node)
        } else {
            edge.to.clone()
        }
    } else {
        edge.to.clone()
    };

    let edge_str = if let Some(label) = &edge.label {
        format!("{} {}|{}| {}", source_str, arrow, label, target_str)
    } else {
        format!("{} {} {}", source_str, arrow, target_str)
    };

    printer.write_line(&edge_str);
}

fn format_node_with_definition(id: &str, node: &FlowNode) -> String {
    let text = node.text.as_deref().unwrap_or("");
    match &node.shape {
        NodeShape::Rectangle => format!("{}[{}]", id, text),
        NodeShape::RoundedRectangle => format!("{}({})", id, text),
        NodeShape::Stadium => format!("{}([{}])", id, text),
        NodeShape::Subroutine => format!("{}[[{}]]", id, text),
        NodeShape::Cylinder => format!("{}[({})]", id, text),
        NodeShape::Circle => format!("{}(({})))", id, text),
        NodeShape::Asymmetric => format!("{}>{}]", id, text),
        NodeShape::Rhombus => format!("{}{{{}}}", id, text),
        NodeShape::Hexagon => format!("{}{{{{{}}}}}", id, text),
        NodeShape::Parallelogram => format!("{}[/{}\\]", id, text),
        NodeShape::ParallelogramAlt => format!("{}[\\{}/]", id, text),
        NodeShape::Trapezoid => format!("{}[/{}/]", id, text),
        NodeShape::TrapezoidAlt => format!("{}[\\{}\\]", id, text),
        NodeShape::DoubleCircle => format!("{}((({})))", id, text),
    }
}

fn write_aligned_flow_edges_with_smart_nodes(
    printer: &mut PrettyPrinter,
    edges: &[FlowEdge],
    nodes: &std::collections::HashMap<String, FlowNode>,
    defined_nodes: &mut std::collections::HashSet<String>,
) {
    // First pass: calculate what the source strings will be for alignment
    let mut source_strings = Vec::new();
    let mut temp_defined = defined_nodes.clone();

    for edge in edges {
        let source_str = if !temp_defined.contains(&edge.from) {
            if let Some(source_node) = nodes.get(&edge.from) {
                temp_defined.insert(edge.from.clone());
                format_node_with_definition(&edge.from, source_node)
            } else {
                edge.from.clone()
            }
        } else {
            edge.from.clone()
        };
        source_strings.push(source_str);
    }

    // Calculate max source length for alignment
    let max_source_len = source_strings.iter().map(|s| s.len()).max().unwrap_or(0);

    // Second pass: write the aligned edges
    for (i, edge) in edges.iter().enumerate() {
        let arrow = match &edge.edge_type {
            EdgeType::Arrow => "-->",
            EdgeType::DottedArrow => "-.->",
            EdgeType::ThickArrow => "==>",
            EdgeType::OpenLink => "---",
            EdgeType::DottedLink => "-.-",
            EdgeType::ThickLink => "===",
            EdgeType::Invisible => "~~~",
            EdgeType::CircleEdge => "--o",
            EdgeType::CrossEdge => "--x",
            EdgeType::MultiDirectional => "<-->",
        };

        // Use the pre-calculated source string
        let source_str = &source_strings[i];

        // Update defined_nodes for real this time
        if !defined_nodes.contains(&edge.from) && nodes.contains_key(&edge.from) {
            defined_nodes.insert(edge.from.clone());
        }

        // Format target node
        let target_str = if !defined_nodes.contains(&edge.to) {
            if let Some(target_node) = nodes.get(&edge.to) {
                defined_nodes.insert(edge.to.clone());
                format_node_with_definition(&edge.to, target_node)
            } else {
                edge.to.clone()
            }
        } else {
            edge.to.clone()
        };

        let padding = " ".repeat(max_source_len - source_str.len());

        let edge_str = if let Some(label) = &edge.label {
            format!(
                "{}{} {}|{}| {}",
                source_str, padding, arrow, label, target_str
            )
        } else {
            format!("{}{} {} {}", source_str, padding, arrow, target_str)
        };

        printer.write_line(&edge_str);
    }
}

fn write_aligned_flow_edges(printer: &mut PrettyPrinter, edges: &[FlowEdge]) {
    // Calculate the maximum length of source nodes to align arrows
    let max_source_len = edges.iter().map(|edge| edge.from.len()).max().unwrap_or(0);

    for edge in edges {
        let arrow = match &edge.edge_type {
            EdgeType::Arrow => "-->",
            EdgeType::DottedArrow => "-.->",
            EdgeType::ThickArrow => "==>",
            EdgeType::OpenLink => "---",
            EdgeType::DottedLink => "-.-",
            EdgeType::ThickLink => "===",
            EdgeType::Invisible => "~~~",
            EdgeType::CircleEdge => "--o",
            EdgeType::CrossEdge => "--x",
            EdgeType::MultiDirectional => "<-->",
        };

        let padding = " ".repeat(max_source_len - edge.from.len());

        let edge_str = if let Some(label) = &edge.label {
            format!("{}{} {} |{}| {}", edge.from, padding, arrow, label, edge.to)
        } else {
            format!("{}{} {} {}", edge.from, padding, arrow, edge.to)
        };

        printer.write_line(&edge_str);
    }
}

fn write_flow_edge(printer: &mut PrettyPrinter, edge: &FlowEdge) {
    let arrow = match &edge.edge_type {
        EdgeType::Arrow => "-->",
        EdgeType::DottedArrow => "-.->",
        EdgeType::ThickArrow => "==>",
        EdgeType::OpenLink => "---",
        EdgeType::DottedLink => "-.-",
        EdgeType::ThickLink => "===",
        EdgeType::Invisible => "~~~",
        EdgeType::CircleEdge => "--o",
        EdgeType::CrossEdge => "--x",
        EdgeType::MultiDirectional => "<-->",
    };

    let edge_str = if let Some(label) = &edge.label {
        format!("{} {} |{}| {}", edge.from, arrow, label, edge.to)
    } else {
        format!("{} {} {}", edge.from, arrow, edge.to)
    };

    printer.write_line(&edge_str);
}

fn write_style_definition(printer: &mut PrettyPrinter, style: &StyleDefinition) {
    let styles_str = style
        .styles
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<_>>()
        .join(",");

    match &style.target {
        StyleTarget::Node(id) => {
            printer.write_line(&format!("style {} {}", id, styles_str));
        }
        StyleTarget::Edge(from, to) => {
            printer.write_line(&format!("linkStyle {}--{} {}", from, to, styles_str));
        }
        StyleTarget::Subgraph(id) => {
            printer.write_line(&format!("style {} {}", id, styles_str));
        }
    }
}

// Sequence diagram implementation
impl MermaidPrinter for SequenceDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("sequenceDiagram");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write autonumber if enabled
        if let Some(auto) = &self.autonumber {
            if auto.visible {
                let mut auto_str = String::from("autonumber");
                if let Some(start) = auto.start {
                    auto_str.push_str(&format!(" {}", start));
                }
                if let Some(step) = auto.step {
                    auto_str.push_str(&format!(" {}", step));
                }
                printer.write_line(&auto_str);
            }
        }

        // Write participants
        for participant in &self.participants {
            let type_str = match participant.participant_type {
                ParticipantType::Participant => "participant",
                ParticipantType::Actor => "actor",
            };

            if let Some(alias) = &participant.alias {
                printer.write_line(&format!("{} {} as {}", type_str, participant.actor, alias));
            } else {
                printer.write_line(&format!("{} {}", type_str, participant.actor));
            }
        }

        // Write statements
        for statement in &self.statements {
            write_sequence_statement(&mut printer, statement);
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_sequence_statement(printer: &mut PrettyPrinter, statement: &SequenceStatement) {
    match statement {
        SequenceStatement::Message(msg) => {
            let arrow = match msg.arrow_type {
                ArrowType::SolidOpen => "->",
                ArrowType::SolidClosed => "->>",
                ArrowType::DottedOpen => "-->",
                ArrowType::DottedClosed => "-->>",
                ArrowType::Cross => "-x",
                ArrowType::Point => "-)",
                ArrowType::BiDirectionalSolid => "<->",
                ArrowType::BiDirectionalDotted => "<-->",
            };

            printer.write_line(&format!("{} {} {}: {}", msg.from, arrow, msg.to, msg.text));
        }
        SequenceStatement::Note(note) => {
            let position = match &note.position {
                NotePosition::LeftOf => "left of",
                NotePosition::RightOf => "right of",
                NotePosition::Over => "over",
            };
            printer.write_line(&format!("note {} {}: {}", position, note.actor, note.text));
        }
        SequenceStatement::Loop(loop_stmt) => {
            printer.write_line(&format!("loop {}", loop_stmt.condition));
            printer.indent();
            for stmt in &loop_stmt.statements {
                write_sequence_statement(printer, stmt);
            }
            printer.dedent();
            printer.write_line("end");
        }
        SequenceStatement::Alt(alt) => {
            printer.write_line(&format!("alt {}", alt.condition));
            printer.indent();
            for stmt in &alt.statements {
                write_sequence_statement(printer, stmt);
            }

            if let Some(else_branch) = &alt.else_branch {
                printer.dedent();
                if let Some(condition) = &else_branch.condition {
                    printer.write_line(&format!("else {}", condition));
                } else {
                    printer.write_line("else");
                }
                printer.indent();
                for stmt in &else_branch.statements {
                    write_sequence_statement(printer, stmt);
                }
            }

            printer.dedent();
            printer.write_line("end");
        }
        SequenceStatement::Opt(opt) => {
            printer.write_line(&format!("opt {}", opt.condition));
            printer.indent();
            for stmt in &opt.statements {
                write_sequence_statement(printer, stmt);
            }
            printer.dedent();
            printer.write_line("end");
        }
        SequenceStatement::Par(par) => {
            let first = &par.branches[0];
            if let Some(condition) = &first.condition {
                printer.write_line(&format!("par {}", condition));
            } else {
                printer.write_line("par");
            }
            printer.indent();
            for stmt in &first.statements {
                write_sequence_statement(printer, stmt);
            }

            for branch in &par.branches[1..] {
                printer.dedent();
                if let Some(condition) = &branch.condition {
                    printer.write_line(&format!("and {}", condition));
                } else {
                    printer.write_line("and");
                }
                printer.indent();
                for stmt in &branch.statements {
                    write_sequence_statement(printer, stmt);
                }
            }

            printer.dedent();
            printer.write_line("end");
        }
        SequenceStatement::Critical(crit) => {
            printer.write_line(&format!("critical {}", crit.condition));
            printer.indent();
            for stmt in &crit.statements {
                write_sequence_statement(printer, stmt);
            }

            for option in &crit.options {
                printer.dedent();
                printer.write_line(&format!("option {}", option.condition));
                printer.indent();
                for stmt in &option.statements {
                    write_sequence_statement(printer, stmt);
                }
            }

            printer.dedent();
            printer.write_line("end");
        }
        SequenceStatement::Activate(actor) => {
            printer.write_line(&format!("activate {}", actor));
        }
        SequenceStatement::Deactivate(actor) => {
            printer.write_line(&format!("deactivate {}", actor));
        }
        SequenceStatement::Create(participant) => {
            printer.write_line(&format!("create participant {}", participant.actor));
        }
        SequenceStatement::Destroy(actor) => {
            printer.write_line(&format!("destroy {}", actor));
        }
    }
}

// Class diagram implementation
impl MermaidPrinter for ClassDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("classDiagram");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write classes (sorted for deterministic output)
        let mut classes: Vec<_> = self.classes.iter().collect();
        classes.sort_by_key(|(name, _)| *name);
        for (name, class) in classes {
            write_class(&mut printer, name, class);
        }

        // Write relationships
        for rel in &self.relationships {
            write_class_relationship(&mut printer, rel);
        }

        // Write notes
        for note in &self.notes {
            printer.write_line(&format!("note \"{}\"", note.text));
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_class(printer: &mut PrettyPrinter, name: &str, class: &Class) {
    printer.write_line(&format!("class {} {{", name));
    printer.indent();

    if let Some(stereotype) = &class.stereotype {
        let stereo_str = match stereotype {
            Stereotype::Interface => "<<interface>>",
            Stereotype::Abstract => "<<abstract>>",
            Stereotype::Service => "<<service>>",
            Stereotype::Enumeration => "<<enumeration>>",
            Stereotype::Exception => "<<exception>>",
            Stereotype::Custom(s) => &format!("<<{}>>", s),
        };
        printer.write_line(stereo_str);
    }

    // Write members
    for member in &class.members {
        match member {
            ClassMember::Property(prop) => {
                let visibility = match prop.visibility {
                    Visibility::Public => "+",
                    Visibility::Private => "-",
                    Visibility::Protected => "#",
                    Visibility::Package => "~",
                };
                let static_mod = if prop.is_static { "$" } else { "" };
                let default_str = if let Some(default) = &prop.default_value {
                    format!(" = {}", default)
                } else {
                    String::new()
                };

                // Format: visibility[static]Type name[default]
                if let Some(prop_type) = &prop.prop_type {
                    printer.write_line(&format!(
                        "{}{}{} {}{}",
                        visibility, static_mod, prop_type, prop.name, default_str
                    ));
                } else {
                    printer.write_line(&format!(
                        "{}{}{}{}",
                        visibility, static_mod, prop.name, default_str
                    ));
                }
            }
            ClassMember::Method(method) => {
                let visibility = match method.visibility {
                    Visibility::Public => "+",
                    Visibility::Private => "-",
                    Visibility::Protected => "#",
                    Visibility::Package => "~",
                };
                let static_mod = if method.is_static { "$" } else { "" };
                let abstract_mod = if method.is_abstract { "*" } else { "" };

                let params_str = method
                    .parameters
                    .iter()
                    .map(|p| {
                        if let Some(t) = &p.param_type {
                            format!("{}: {}", p.name, t)
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                // Format: visibility[static][abstract]methodName(params)[ returnType]
                let method_str = if let Some(return_type) = &method.return_type {
                    format!(
                        "{}{}{}{}({}) {}",
                        visibility, static_mod, abstract_mod, method.name, params_str, return_type
                    )
                } else {
                    format!(
                        "{}{}{}{}({})",
                        visibility, static_mod, abstract_mod, method.name, params_str
                    )
                };

                printer.write_line(&method_str);
            }
        }
    }

    printer.dedent();
    printer.write_line("}");

    // Add CSS class if present
    if let Some(css_class) = &class.css_class {
        printer.write_line(&format!("class {} {}", name, css_class));
    }
}

fn write_class_relationship(printer: &mut PrettyPrinter, rel: &ClassRelationship) {
    let rel_type = match rel.relationship_type {
        ClassRelationshipType::Inheritance => "<|--",
        ClassRelationshipType::Composition => "*--",
        ClassRelationshipType::Aggregation => "o--",
        ClassRelationshipType::Association => "<--",
        ClassRelationshipType::Link => "--",
        ClassRelationshipType::DashedLink => "..",
        ClassRelationshipType::Dependency => "<..",
        ClassRelationshipType::Realization => "<|..",
    };

    let mut rel_str = String::new();
    rel_str.push_str(&rel.from);

    if let Some(from_card) = &rel.from_cardinality {
        rel_str.push_str(&format!(" \"{}\"", from_card));
    }

    rel_str.push_str(&format!(" {} ", rel_type));

    if let Some(to_card) = &rel.to_cardinality {
        rel_str.push_str(&format!("\"{}\" ", to_card));
    }

    rel_str.push_str(&rel.to);

    if let Some(label) = &rel.label {
        rel_str.push_str(&format!(" : {}", label));
    }

    printer.write_line(&rel_str);
}

// State diagram implementation
impl MermaidPrinter for StateDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        let version_str = match self.version {
            StateVersion::V1 => "stateDiagram",
            StateVersion::V2 => "stateDiagram-v2",
        };
        printer.write_line(version_str);
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write states (sorted for deterministic output)
        let mut states: Vec<_> = self.states.iter().collect();
        states.sort_by_key(|(id, _)| *id);
        for (id, state) in states {
            write_state(&mut printer, id, state);
        }

        // Write transitions
        for transition in &self.transitions {
            let mut trans_str = format!("{} --> {}", transition.from, transition.to);

            if transition.event.is_some()
                || transition.guard.is_some()
                || transition.action.is_some()
            {
                trans_str.push_str(" : ");
                if let Some(event) = &transition.event {
                    trans_str.push_str(event);
                }
                if let Some(guard) = &transition.guard {
                    trans_str.push_str(&format!(" [{}]", guard));
                }
                if let Some(action) = &transition.action {
                    trans_str.push_str(&format!(" / {}", action));
                }
            }

            printer.write_line(&trans_str);
        }

        // Write notes
        for note in &self.notes {
            let position = match note.position {
                StateNotePosition::LeftOf => "left of",
                StateNotePosition::RightOf => "right of",
                StateNotePosition::Above => "above",
                StateNotePosition::Below => "below",
            };
            printer.write_line(&format!(
                "note {} {} : {}",
                position, note.target, note.text
            ));
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_state(printer: &mut PrettyPrinter, id: &str, state: &State) {
    match &state.state_type {
        StateType::Simple => {
            if let Some(display_name) = &state.display_name {
                printer.write_line(&format!("{} : {}", id, display_name));
            } else {
                printer.write_line(id);
            }
        }
        StateType::Composite => {
            printer.write_line(&format!("state {} {{", id));
            printer.indent();

            // Write substates
            for substate_id in &state.substates {
                printer.write_line(substate_id);
            }

            // Write concurrent regions
            for (i, region) in state.concurrent_regions.iter().enumerate() {
                if i > 0 {
                    printer.write_line("--");
                }
                for state_id in region {
                    printer.write_line(state_id);
                }
            }

            printer.dedent();
            printer.write_line("}");
        }
        StateType::Start => {
            printer.write_line(&format!("[*] --> {}", id));
        }
        StateType::End => {
            printer.write_line(&format!("{} --> [*]", id));
        }
        StateType::Choice => {
            printer.write_line(&format!("state {} <<choice>>", id));
        }
        StateType::Fork => {
            printer.write_line(&format!("state {} <<fork>>", id));
        }
        StateType::Join => {
            printer.write_line(&format!("state {} <<join>>", id));
        }
    }
}

// ER diagram implementation
impl MermaidPrinter for ErDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("erDiagram");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write relationships
        for rel in &self.relationships {
            let left_card = format_er_cardinality(&rel.left_cardinality);
            let right_card = format_er_cardinality(&rel.right_cardinality);
            let label = rel.label.as_deref().unwrap_or("");
            printer.write_line(&format!(
                "{} {}--{} {} : {}",
                rel.left_entity, left_card, right_card, rel.right_entity, label
            ));
        }

        // Write entities (sorted for deterministic output)
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|(name, _)| *name);
        for (name, entity) in entities {
            printer.write_line(&format!("{} {{", name));
            printer.indent();

            for attr in &entity.attributes {
                let key_str = if let Some(key_type) = &attr.key_type {
                    match key_type {
                        KeyType::PK => " PK",
                        KeyType::FK => " FK",
                        KeyType::UK => " UK",
                    }
                } else {
                    ""
                };
                let line = if let Some(comment) = &attr.comment {
                    if comment.is_empty() {
                        format!("{} {}{}", attr.attr_type, attr.name, key_str)
                    } else {
                        format!(
                            "{} {}{} \"{}\"",
                            attr.attr_type, attr.name, key_str, comment
                        )
                    }
                } else {
                    format!("{} {}{}", attr.attr_type, attr.name, key_str)
                };
                printer.write_line(&line);
            }

            printer.dedent();
            printer.write_line("}");
        }

        printer.dedent();
        printer.finish()
    }
}

fn format_er_cardinality(card: &ErCardinality) -> &'static str {
    match (&card.min, &card.max) {
        (CardinalityValue::Zero, CardinalityValue::One) => "o|",
        (CardinalityValue::One, CardinalityValue::One) => "||",
        (CardinalityValue::Zero, CardinalityValue::Many) => "o{",
        (CardinalityValue::One, CardinalityValue::Many) => "|{",
        _ => "||", // Default
    }
}

// Pie chart implementation
impl MermaidPrinter for PieDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        if let Some(title) = &self.title {
            printer.write_line(&format!("pie title {}", title));
        } else {
            printer.write_line("pie");
        }

        printer.indent();

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write data points
        for slice in &self.data {
            printer.write_line(&format!("\"{}\" : {}", slice.label, slice.value));
        }

        printer.dedent();
        printer.finish()
    }
}

// Gantt chart implementation
impl MermaidPrinter for GanttDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("gantt");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write date format
        if let Some(fmt) = &self.date_format {
            printer.write_line(&format!("dateFormat {}", fmt));
        }

        // Write axis format
        if let Some(fmt) = &self.axis_format {
            printer.write_line(&format!("axisFormat {}", fmt));
        }

        // Write tick interval
        if let Some(interval) = &self.tick_interval {
            printer.write_line(&format!("tickInterval {}", interval));
        }

        // Write includes
        for include in &self.includes {
            printer.write_line(&format!("includes {}", include));
        }

        // Write excludes
        for exclude in &self.excludes {
            printer.write_line(&format!("excludes {}", exclude));
        }

        // Write today marker
        if let Some(marker) = &self.today_marker {
            printer.write_line(&format!("todayMarker {}", marker));
        }

        // Write sections and tasks
        for section in &self.sections {
            printer.write_line(&format!("section {}", section.name));
            printer.indent();

            for task in &section.tasks {
                let mut task_str = task.name.clone();

                // Add status tags
                let mut tags = Vec::new();
                match task.status {
                    TaskStatus::Done => tags.push("done"),
                    TaskStatus::Active => tags.push("active"),
                    TaskStatus::Critical => tags.push("crit"),
                    TaskStatus::Milestone => tags.push("milestone"),
                    TaskStatus::None => {}
                }

                if !tags.is_empty() {
                    task_str.push_str(&format!(" :{}", tags.join(", ")));
                }

                // Add ID or dependencies
                if let Some(id) = &task.id {
                    task_str.push_str(&format!(" :{}", id));
                } else if !task.dependencies.is_empty() {
                    // If no ID but has dependencies, format as ":after dep1, dep2"
                    task_str.push_str(&format!(" :after {}", task.dependencies.join(", ")));
                }

                // Add start date and duration
                if let Some(start) = &task.start_date {
                    task_str.push_str(&format!(", {}", start));
                }

                if let Some(duration) = &task.duration {
                    task_str.push_str(&format!(", {}", duration));
                }

                printer.write_line(&task_str);
            }

            printer.dedent();
        }

        printer.dedent();
        printer.finish()
    }
}

// Git diagram implementation
impl MermaidPrinter for GitDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("gitGraph");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write theme if present
        if let Some(theme) = &self.theme {
            printer.write_line(&format!("theme {}", theme));
        }

        // Write operations
        for op in &self.operations {
            match op {
                GitOperation::Commit {
                    id,
                    commit_type,
                    tag,
                } => {
                    let mut commit_str = String::from("commit");

                    if let Some(id_val) = id {
                        commit_str.push_str(&format!(" id: \"{}\"", id_val));
                    }

                    match commit_type {
                        CommitType::Reverse => commit_str.push_str(" type: REVERSE"),
                        CommitType::Highlight => commit_str.push_str(" type: HIGHLIGHT"),
                        _ => {}
                    }

                    if let Some(tag_val) = tag {
                        commit_str.push_str(&format!(" tag: \"{}\"", tag_val));
                    }

                    printer.write_line(&commit_str);
                }
                GitOperation::Branch { name, order } => {
                    let mut branch_str = format!("branch {}", name);
                    if let Some(order_val) = order {
                        branch_str.push_str(&format!(" order: {}", order_val));
                    }
                    printer.write_line(&branch_str);
                }
                GitOperation::Checkout { branch } => {
                    printer.write_line(&format!("checkout {}", branch));
                }
                GitOperation::Merge {
                    branch,
                    id,
                    tag,
                    commit_type,
                } => {
                    let mut merge_str = format!("merge {}", branch);

                    if let Some(id_val) = id {
                        merge_str.push_str(&format!(" id: \"{}\"", id_val));
                    }

                    match commit_type {
                        CommitType::Reverse => merge_str.push_str(" type: REVERSE"),
                        CommitType::Highlight => merge_str.push_str(" type: HIGHLIGHT"),
                        _ => {}
                    }

                    if let Some(tag_val) = tag {
                        merge_str.push_str(&format!(" tag: \"{}\"", tag_val));
                    }

                    printer.write_line(&merge_str);
                }
                GitOperation::CherryPick { id, parent, tag } => {
                    let mut cp_str = format!("cherry-pick id: \"{}\"", id);

                    if let Some(parent_val) = parent {
                        cp_str.push_str(&format!(" parent: \"{}\"", parent_val));
                    }

                    if let Some(tag_val) = tag {
                        cp_str.push_str(&format!(" tag: \"{}\"", tag_val));
                    }

                    printer.write_line(&cp_str);
                }
            }
        }

        printer.dedent();
        printer.finish()
    }
}

// Mindmap implementation
impl MermaidPrinter for MindmapDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("mindmap");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write root node
        write_mindmap_node(&mut printer, &self.root, 0, true);

        printer.dedent();
        printer.finish()
    }
}

fn write_mindmap_node(
    printer: &mut PrettyPrinter,
    node: &MindmapNode,
    depth: usize,
    is_root: bool,
) {
    let indent = "  ".repeat(depth);

    let (shape_start, shape_end) = match node.shape {
        MindmapNodeShape::Default => ("", ""),
        MindmapNodeShape::Square => ("[", "]"),
        MindmapNodeShape::Rounded => ("(", ")"),
        MindmapNodeShape::Circle => ("((", "))"),
        MindmapNodeShape::Cloud => ("(-", "-)"),
        MindmapNodeShape::Bang => ("))", "(("),
        MindmapNodeShape::Hexagon => ("{{", "}}"),
    };

    // For the root node, prefix with "root"
    let node_text = if is_root {
        format!("root{}{}{}", shape_start, node.text, shape_end)
    } else if node.text.is_empty() && node.icon.is_some() {
        // Skip nodes with empty text that only have icons - they should be handled differently
        if let Some(icon) = &node.icon {
            printer.write_line(&format!("{}::icon({})", indent, icon));
        }
        // Write children
        for child in &node.children {
            write_mindmap_node(printer, child, depth + 1, false);
        }
        return;
    } else {
        format!("{}{}{}", shape_start, node.text, shape_end)
    };

    printer.write_line(&format!("{}{}", indent, node_text));

    if let Some(icon) = &node.icon {
        if !node.text.is_empty() {
            printer.write_line(&format!("{}::icon({})", indent, icon));
        }
    }

    if let Some(class) = &node.class {
        printer.write_line(&format!("{}::::{}", indent, class));
    }

    // Write children
    for child in &node.children {
        write_mindmap_node(printer, child, depth + 1, false);
    }
}

// Timeline implementation
impl MermaidPrinter for TimelineDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("timeline");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write sections
        for section in &self.sections {
            printer.write_line("");
            printer.write_line(&format!("section {}", section.name));
            printer.indent();

            for item in &section.items {
                match item {
                    TimelineItem::Period(period) => {
                        printer.write_line(period);
                    }
                    TimelineItem::Event(event) => {
                        printer.write_line(&format!(": {}", event));
                    }
                }
            }

            printer.dedent();
        }

        printer.dedent();
        printer.finish()
    }
}

// Journey diagram implementation
impl MermaidPrinter for JourneyDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("journey");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write sections
        for section in &self.sections {
            printer.write_line(&format!("section {}", section.name));
            printer.indent();

            for task in &section.tasks {
                let actors = task.actors.join(", ");
                printer.write_line(&format!("{}: {}: {}", task.name, task.score, actors));
            }

            printer.dedent();
        }

        printer.dedent();
        printer.finish()
    }
}

// Sankey diagram implementation
impl MermaidPrinter for SankeyDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("sankey-beta");
        printer.indent();

        // Write links
        for link in &self.links {
            printer.write_line(&format!("{},{},{}", link.source, link.target, link.value));
        }

        printer.dedent();
        printer.finish()
    }
}

// C4 diagram implementation
impl MermaidPrinter for C4Diagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        let diagram_type = match self.diagram_type {
            C4DiagramType::Context => "C4Context",
            C4DiagramType::Container => "C4Container",
            C4DiagramType::Component => "C4Component",
            C4DiagramType::Deployment => "C4Deployment",
            C4DiagramType::Dynamic => "C4Dynamic",
        };

        printer.write_line(diagram_type);
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write elements
        for element in self.elements.values() {
            write_c4_element(&mut printer, element);
        }

        // Write boundaries
        for boundary in &self.boundaries {
            write_c4_boundary(&mut printer, boundary);
        }

        // Write relationships
        for rel in &self.relationships {
            let mut rel_str = format!("Rel({}, {}", rel.from, rel.to);

            if let Some(label) = &rel.label {
                rel_str.push_str(&format!(", \"{}\"", label));
            }

            if let Some(tech) = &rel.technology {
                rel_str.push_str(&format!(", \"{}\"", tech));
            }

            rel_str.push(')');
            printer.write_line(&rel_str);
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_c4_element(printer: &mut PrettyPrinter, element: &C4Element) {
    let elem_type = match &element.element_type {
        C4ElementType::Person => "Person",
        C4ElementType::System => "System",
        C4ElementType::SystemDb => "SystemDb",
        C4ElementType::SystemQueue => "SystemQueue",
        C4ElementType::Container => "Container",
        C4ElementType::ContainerDb => "ContainerDb",
        C4ElementType::ContainerQueue => "ContainerQueue",
        C4ElementType::Component => "Component",
        C4ElementType::ComponentDb => "ComponentDb",
        C4ElementType::ComponentQueue => "ComponentQueue",
        C4ElementType::Node => "Node",
        C4ElementType::DeploymentNode => "DeploymentNode",
    };

    let ext_suffix = if element.is_external { "_Ext" } else { "" };

    let mut elem_str = format!(
        "{}{}({}, \"{}\"",
        elem_type, ext_suffix, element.id, element.name
    );

    if let Some(desc) = &element.description {
        elem_str.push_str(&format!(", \"{}\"", desc));
    }

    if let Some(tech) = &element.technology {
        elem_str.push_str(&format!(", \"{}\"", tech));
    }

    elem_str.push(')');
    printer.write_line(&elem_str);
}

fn write_c4_boundary(printer: &mut PrettyPrinter, boundary: &C4Boundary) {
    let boundary_type = match boundary.boundary_type {
        C4BoundaryType::System => "System_Boundary",
        C4BoundaryType::Container => "Container_Boundary",
        C4BoundaryType::Enterprise => "Enterprise_Boundary",
        C4BoundaryType::Generic => "Boundary",
    };

    printer.write_line(&format!(
        "{}({}, \"{}\") {{",
        boundary_type, boundary.id, boundary.label
    ));
    printer.indent();

    // Write elements in boundary
    for elem_id in &boundary.elements {
        printer.write_line(elem_id);
    }

    // Write nested boundaries
    for nested in &boundary.boundaries {
        write_c4_boundary(printer, nested);
    }

    printer.dedent();
    printer.write_line("}");
}

// Quadrant chart implementation
impl MermaidPrinter for QuadrantDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("quadrantChart");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write axis labels
        if let Some(x_axis) = &self.x_axis {
            let x_start = x_axis.label_start.as_deref().unwrap_or("");
            let x_end = x_axis.label_end.as_deref().unwrap_or("");
            printer.write_line(&format!("x-axis \"{}\" --> \"{}\"", x_start, x_end));
        }

        if let Some(y_axis) = &self.y_axis {
            let y_start = y_axis.label_start.as_deref().unwrap_or("");
            let y_end = y_axis.label_end.as_deref().unwrap_or("");
            printer.write_line(&format!("y-axis \"{}\" --> \"{}\"", y_start, y_end));
        }

        // Write quadrant labels
        if let Some(q1) = &self.quadrants.quadrant_1 {
            printer.write_line(&format!("quadrant-1 {}", q1));
        }
        if let Some(q2) = &self.quadrants.quadrant_2 {
            printer.write_line(&format!("quadrant-2 {}", q2));
        }
        if let Some(q3) = &self.quadrants.quadrant_3 {
            printer.write_line(&format!("quadrant-3 {}", q3));
        }
        if let Some(q4) = &self.quadrants.quadrant_4 {
            printer.write_line(&format!("quadrant-4 {}", q4));
        }

        // Write points
        for point in &self.points {
            printer.write_line(&format!("{}: [{}, {}]", point.name, point.x, point.y));
        }

        printer.dedent();
        printer.finish()
    }
}

// XY chart implementation
impl MermaidPrinter for XyChartDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        let orientation = match self.orientation {
            ChartOrientation::Vertical => "xychart-beta",
            ChartOrientation::Horizontal => "xychart-beta horizontal",
        };
        printer.write_line(orientation);
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title \"{}\"", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write x-axis
        let mut x_str = String::from("x-axis");
        if let Some(title) = &self.x_axis.title {
            x_str.push_str(&format!(" \"{}\"", title));
        }
        if !self.x_axis.labels.is_empty() {
            let labels = self
                .x_axis
                .labels
                .iter()
                .map(|l| format!("\"{}\"", l))
                .collect::<Vec<_>>()
                .join(", ");
            x_str.push_str(&format!(" [{}]", labels));
        }
        if let Some((min, max)) = &self.x_axis.range {
            x_str.push_str(&format!(" {} --> {}", min, max));
        }
        printer.write_line(&x_str);

        // Write y-axis
        let mut y_str = String::from("y-axis");
        if let Some(title) = &self.y_axis.title {
            y_str.push_str(&format!(" \"{}\"", title));
        }
        if let Some((min, max)) = &self.y_axis.range {
            y_str.push_str(&format!(" {} --> {}", min, max));
        }
        printer.write_line(&y_str);

        // Write series
        for series in &self.data_series {
            let series_type = match series.series_type {
                SeriesType::Line => "line",
                SeriesType::Bar => "bar",
            };

            let data_str = series
                .data
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            if let Some(name) = &series.name {
                printer.write_line(&format!("{} \"{}\" [{}]", series_type, name, data_str));
            } else {
                printer.write_line(&format!("{} [{}]", series_type, data_str));
            }
        }

        printer.dedent();
        printer.finish()
    }
}

// Kanban implementation
impl MermaidPrinter for KanbanDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("kanban");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write sections
        for section in &self.sections {
            printer.write_line(&section.title);
            printer.indent();

            for item in &section.items {
                let mut item_str = item.text.clone();

                if !item.assigned.is_empty() {
                    item_str.push_str(&format!(" @{}", item.assigned.join(",")));
                }

                // Add metadata if present
                for (key, value) in &item.metadata {
                    item_str.push_str(&format!(" #{}:{}", key, value));
                }

                printer.write_line(&item_str);
            }

            printer.dedent();
        }

        printer.dedent();
        printer.finish()
    }
}

// Block diagram implementation
impl MermaidPrinter for BlockDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("block-beta");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write columns if specified
        if let Some(cols) = self.columns {
            printer.write_line(&format!("columns {}", cols));
        }

        // Write blocks
        for block in &self.blocks {
            write_block(&mut printer, block);
        }

        // Write connections
        for conn in &self.connections {
            let arrow = match conn.arrow_type {
                BlockArrowType::Normal => "-->",
                BlockArrowType::Dotted => "-.->",
                BlockArrowType::Thick => "==>",
                BlockArrowType::Invisible => "~~~",
                BlockArrowType::Bidirectional => "<-->",
            };

            if let Some(label) = &conn.label {
                printer.write_line(&format!("{} {}|{}| {}", conn.from, arrow, label, conn.to));
            } else {
                printer.write_line(&format!("{} {} {}", conn.from, arrow, conn.to));
            }
        }

        printer.dedent();
        printer.finish()
    }
}

fn write_block(printer: &mut PrettyPrinter, block: &Block) {
    match block {
        Block::Simple { id, label, shape } => {
            let shape_str = match shape {
                BlockShape::Rectangle => "",
                BlockShape::RoundedRect => "()",
                BlockShape::Rhombus => "{{}}",
                BlockShape::Circle => "(())",
                BlockShape::Ellipse => "([])",
                BlockShape::Cylinder => "[()]",
                BlockShape::Custom(s) => s,
            };

            if let Some(label_text) = label {
                printer.write_line(&format!("{}{} \"{}\"", id, shape_str, label_text));
            } else {
                printer.write_line(&format!("{}{}", id, shape_str));
            }
        }
        Block::Composite { id, label, blocks } => {
            if let Some(label_text) = label {
                printer.write_line(&format!("block:{} \"{}\"", id, label_text));
            } else {
                printer.write_line(&format!("block:{}", id));
            }
            printer.indent();

            for child in blocks {
                write_block(printer, child);
            }

            printer.dedent();
            printer.write_line("end");
        }
        Block::Space { size } => {
            if let Some(s) = size {
                printer.write_line(&format!("space:{}", s));
            } else {
                printer.write_line("space");
            }
        }
    }
}

// Architecture diagram implementation
impl MermaidPrinter for ArchitectureDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("architecture-beta");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write direction
        let dir_str = match self.direction {
            ArchDirection::TB => "TB",
            ArchDirection::BT => "BT",
            ArchDirection::LR => "LR",
            ArchDirection::RL => "RL",
        };
        printer.write_line(&format!("direction {}", dir_str));

        // Write groups
        for (id, group) in &self.groups {
            printer.write_line(&format!("group {} {}", id, group.title));
            printer.indent();

            if let Some(icon) = &group.icon {
                printer.write_line(&format!("icon {}", icon));
            }

            printer.dedent();
        }

        // Write services
        for (id, service) in &self.services {
            let mut service_str = format!("service {}", id);

            if let Some(icon) = &service.icon {
                service_str.push_str(&format!("({}) ", icon));
            }

            service_str.push_str(&format!(" \"{}\"", service.title));

            if let Some(group) = &service.in_group {
                service_str.push_str(&format!(" in {}", group));
            }

            printer.write_line(&service_str);
        }

        // Write junctions
        for (id, junction) in &self.junctions {
            let mut junc_str = format!("junction {}", id);

            if let Some(group) = &junction.in_group {
                junc_str.push_str(&format!(" in {}", group));
            }

            printer.write_line(&junc_str);
        }

        // Write edges
        for edge in &self.edges {
            let from_str = format_edge_endpoint(&edge.from);
            let to_str = format_edge_endpoint(&edge.to);

            let edge_type_str = match edge.edge_type {
                ArchEdgeType::Solid => "--",
                ArchEdgeType::Dotted => "..",
                ArchEdgeType::Arrow => "->",
                ArchEdgeType::BiArrow => "<->",
            };

            if let Some(label) = &edge.label {
                printer.write_line(&format!(
                    "{} {}|{}| {}",
                    from_str, edge_type_str, label, to_str
                ));
            } else {
                printer.write_line(&format!("{} {} {}", from_str, edge_type_str, to_str));
            }
        }

        printer.dedent();
        printer.finish()
    }
}

fn format_edge_endpoint(endpoint: &EdgeEndpoint) -> String {
    if let Some(port) = &endpoint.port {
        let port_str = match port {
            Port::Left => ":L",
            Port::Right => ":R",
            Port::Top => ":T",
            Port::Bottom => ":B",
        };
        format!("{}{}", endpoint.id, port_str)
    } else {
        endpoint.id.clone()
    }
}

// Packet diagram implementation
impl MermaidPrinter for PacketDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("packet-beta");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write fields
        for field in &self.fields {
            let field_str = if field.is_optional {
                format!("{}-{}: ({})", field.start_bit, field.end_bit, field.name)
            } else {
                format!("{}-{}: {}", field.start_bit, field.end_bit, field.name)
            };
            printer.write_line(&field_str);
        }

        printer.dedent();
        printer.finish()
    }
}

// Requirement diagram implementation
impl MermaidPrinter for RequirementDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("requirementDiagram");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write requirements
        for req in self.requirements.values() {
            let req_type = match req.req_type {
                RequirementType::Requirement => "requirement",
                RequirementType::FunctionalRequirement => "functionalRequirement",
                RequirementType::PerformanceRequirement => "performanceRequirement",
                RequirementType::InterfaceRequirement => "interfaceRequirement",
                RequirementType::PhysicalRequirement => "physicalRequirement",
                RequirementType::DesignConstraint => "designConstraint",
            };

            printer.write_line(&format!("{} {} {{", req_type, req.id));
            printer.indent();

            printer.write_line(&format!("id: {}", req.id));
            printer.write_line(&format!("text: {}", req.text));

            if let Some(risk) = &req.risk {
                let risk_str = match risk {
                    RiskLevel::Low => "low",
                    RiskLevel::Medium => "medium",
                    RiskLevel::High => "high",
                };
                printer.write_line(&format!("risk: {}", risk_str));
            }

            if let Some(method) = &req.verify_method {
                let method_str = match method {
                    VerificationMethod::Analysis => "analysis",
                    VerificationMethod::Inspection => "inspection",
                    VerificationMethod::Test => "test",
                    VerificationMethod::Demonstration => "demonstration",
                };
                printer.write_line(&format!("verifymethod: {}", method_str));
            }

            printer.dedent();
            printer.write_line("}");
        }

        // Write elements
        for (id, elem) in &self.elements {
            printer.write_line(&format!("element {} {{", id));
            printer.indent();

            printer.write_line(&format!("type: \"{}\"", elem.element_type));

            if let Some(doc_ref) = &elem.doc_ref {
                printer.write_line(&format!("docref: {}", doc_ref));
            }

            printer.dedent();
            printer.write_line("}");
        }

        // Write relationships
        for rel in &self.relationships {
            let rel_type = match rel.relationship_type {
                RelationshipType::Contains => "contains",
                RelationshipType::Copies => "copies",
                RelationshipType::Derives => "derives",
                RelationshipType::Satisfies => "satisfies",
                RelationshipType::Verifies => "verifies",
                RelationshipType::Refines => "refines",
                RelationshipType::Traces => "traces",
            };

            printer.write_line(&format!("{} - {} -> {}", rel.source, rel_type, rel.target));
        }

        printer.dedent();
        printer.finish()
    }
}

// Treemap diagram implementation
impl MermaidPrinter for TreemapDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("treemap");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write root
        write_treemap_node(&mut printer, &self.root, 0);

        printer.dedent();
        printer.finish()
    }
}

fn write_treemap_node(printer: &mut PrettyPrinter, node: &TreemapNode, depth: usize) {
    let indent = "  ".repeat(depth);

    if let Some(value) = node.value {
        printer.write_line(&format!("{}{}({})", indent, node.name, value));
    } else {
        printer.write_line(&format!("{}{}", indent, node.name));
    }

    // Write children
    for child in &node.children {
        write_treemap_node(printer, child, depth + 1);
    }
}

// Radar chart implementation
impl MermaidPrinter for RadarDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String {
        let mut printer = PrettyPrinter::new(options.clone());

        printer.write_line("radar");
        printer.indent();

        // Write title
        if let Some(title) = &self.title {
            printer.write_line(&format!("title {}", title));
        }

        // Write accessibility info
        if let Some(acc_title) = &self.accessibility.title {
            printer.write_line(&format!("accTitle: {}", acc_title));
        }
        if let Some(desc) = &self.accessibility.description {
            printer.write_line(&format!("accDescr: {}", desc));
        }

        // Write config
        if self.config.background_color.is_some()
            || self.config.grid_color.is_some()
            || self.config.scale_min != 0.0
            || self.config.scale_max != 100.0
        {
            printer.write_line("config");
            printer.indent();

            if let Some(bg) = &self.config.background_color {
                printer.write_line(&format!("backgroundColor: {}", bg));
            }
            if let Some(grid) = &self.config.grid_color {
                printer.write_line(&format!("gridColor: {}", grid));
            }
            printer.write_line(&format!(
                "scale: [{}, {}]",
                self.config.scale_min, self.config.scale_max
            ));

            printer.dedent();
        }

        // Write axes
        let axes_str = self.axes.join(", ");
        printer.write_line(&axes_str);

        // Write datasets
        for dataset in &self.datasets {
            let values_str = dataset
                .values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            printer.write_line(&format!("{}: [{}]", dataset.name, values_str));
        }

        printer.dedent();
        printer.finish()
    }
}

// Misc diagram implementation (for unsupported diagrams)
impl MermaidPrinter for MiscDiagram {
    fn to_mermaid(&self) -> String {
        self.to_mermaid_pretty(&PrintOptions::default())
    }

    fn to_mermaid_pretty(&self, _options: &PrintOptions) -> String {
        match &self.content {
            MiscContent::Info(info) => {
                format!("info\n{}", info.command)
            }
            MiscContent::GitGraph(git) => {
                let mut output = String::from("gitGraph:\n");
                for commit in &git.commits {
                    output.push_str(&format!(
                        "    {} {}\n",
                        commit.action,
                        commit.params.join(" ")
                    ));
                }
                output
            }
            MiscContent::Raw(raw) => raw.lines.join("\n"),
        }
    }
}
