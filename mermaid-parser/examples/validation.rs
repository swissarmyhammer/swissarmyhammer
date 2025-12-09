//! Diagram validation example for the mermaid-parser crate
//!
//! This example demonstrates how to validate parsed Mermaid diagrams
//! for semantic correctness, best practices, and potential issues.

use mermaid_parser::{parse_diagram, AstVisitor, DiagramMetrics, DiagramType, ReferenceValidator};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct ValidationIssue {
    severity: Severity,
    category: Category,
    message: String,
    location: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Severity {
    Error,   // Must fix
    Warning, // Should fix
    Info,    // Could improve
}

#[derive(Debug, Clone)]
enum Category {
    Structure,
    Naming,
    Complexity,
    #[allow(dead_code)]
    Performance,
    Accessibility,
    BestPractice,
}

struct DiagramValidator {
    issues: Vec<ValidationIssue>,
}

impl DiagramValidator {
    fn new() -> Self {
        Self { issues: Vec::new() }
    }

    fn add_issue(
        &mut self,
        severity: Severity,
        category: Category,
        message: String,
        location: Option<String>,
    ) {
        self.issues.push(ValidationIssue {
            severity,
            category,
            message,
            location,
        });
    }

    fn validate_diagram(&mut self, diagram: &DiagramType) -> Vec<ValidationIssue> {
        self.issues.clear();

        match diagram {
            DiagramType::Flowchart(flowchart) => self.validate_flowchart(flowchart),
            DiagramType::Sankey(sankey) => self.validate_sankey(sankey),
            DiagramType::Sequence(sequence) => self.validate_sequence(sequence),
            DiagramType::State(state) => self.validate_state(state),
            _ => {
                // Generic validation for other diagram types
                let metrics = diagram.calculate_metrics();
                self.validate_general_metrics(&metrics);
            }
        }

        self.issues.clone()
    }

    fn validate_flowchart(&mut self, flowchart: &mermaid_parser::common::ast::FlowchartDiagram) {
        // Check for unreachable nodes
        let mut reachable_nodes = HashSet::new();
        let mut queue = Vec::new();

        // Find entry points (nodes with no incoming edges)
        let mut has_incoming = HashSet::new();
        for edge in &flowchart.edges {
            has_incoming.insert(&edge.to);
        }

        for node_id in flowchart.nodes.keys() {
            if !has_incoming.contains(node_id) {
                queue.push(node_id.clone());
                reachable_nodes.insert(node_id.clone());
            }
        }

        // Traverse from entry points
        while let Some(current) = queue.pop() {
            for edge in &flowchart.edges {
                if edge.from == current && !reachable_nodes.contains(&edge.to) {
                    reachable_nodes.insert(edge.to.clone());
                    queue.push(edge.to.clone());
                }
            }
        }

        // Report unreachable nodes
        for node_id in flowchart.nodes.keys() {
            if !reachable_nodes.contains(node_id) {
                self.add_issue(
                    Severity::Warning,
                    Category::Structure,
                    format!("Node '{}' is unreachable", node_id),
                    Some(node_id.clone()),
                );
            }
        }

        // Check for cycles (potential infinite loops)
        if self.has_cycles(flowchart) {
            self.add_issue(
                Severity::Info,
                Category::Structure,
                "Diagram contains cycles - verify this is intentional".to_string(),
                None,
            );
        }

        // Check node naming conventions
        for (id, node) in &flowchart.nodes {
            if id.len() == 1 {
                self.add_issue(
                    Severity::Info,
                    Category::Naming,
                    format!("Consider using more descriptive name for node '{}'", id),
                    Some(id.clone()),
                );
            }

            if let Some(text) = &node.text {
                if text.len() > 50 {
                    self.add_issue(
                        Severity::Warning,
                        Category::BestPractice,
                        format!("Node '{}' has very long text ({}+ chars)", id, text.len()),
                        Some(id.clone()),
                    );
                }
            }
        }

        // Check diagram complexity
        let metrics = DiagramType::Flowchart(flowchart.clone()).calculate_metrics();
        self.validate_general_metrics(&metrics);

        // Check accessibility
        if flowchart.title.is_none() {
            self.add_issue(
                Severity::Info,
                Category::Accessibility,
                "Consider adding a title for better accessibility".to_string(),
                None,
            );
        }

        // Use reference validator
        let mut ref_validator = ReferenceValidator::new();
        ref_validator.visit_flowchart(flowchart);
    }

    fn validate_sankey(&mut self, sankey: &mermaid_parser::common::ast::SankeyDiagram) {
        // Check for mass conservation
        let mut node_flow_in = HashMap::new();
        let mut node_flow_out = HashMap::new();

        for link in &sankey.links {
            *node_flow_out.entry(&link.source).or_insert(0.0) += link.value;
            *node_flow_in.entry(&link.target).or_insert(0.0) += link.value;
        }

        // Check conservation for intermediate nodes
        for node in &sankey.nodes {
            let flow_in = node_flow_in.get(&node.id).unwrap_or(&0.0);
            let flow_out = node_flow_out.get(&node.id).unwrap_or(&0.0);

            if flow_in > &0.0 && flow_out > &0.0 {
                let difference = (flow_in - flow_out).abs();
                if difference > 0.001 {
                    // Allow small floating point differences
                    self.add_issue(
                        Severity::Warning,
                        Category::Structure,
                        format!(
                            "Flow conservation violated for node '{}': in={:.2}, out={:.2}",
                            node.name, flow_in, flow_out
                        ),
                        Some(node.id.clone()),
                    );
                }
            }
        }

        // Check for negative flows
        for link in &sankey.links {
            if link.value <= 0.0 {
                self.add_issue(
                    Severity::Error,
                    Category::Structure,
                    format!(
                        "Negative or zero flow value: {} -> {} ({})",
                        link.source, link.target, link.value
                    ),
                    None,
                );
            }
        }

        // Check for self-loops
        for link in &sankey.links {
            if link.source == link.target {
                self.add_issue(
                    Severity::Warning,
                    Category::Structure,
                    format!("Self-loop detected: {} -> {}", link.source, link.target),
                    None,
                );
            }
        }
    }

    fn validate_sequence(&mut self, sequence: &mermaid_parser::common::ast::SequenceDiagram) {
        // Check for undefined participants in messages
        let participant_ids: HashSet<_> = sequence.participants.iter().map(|p| &p.actor).collect();

        for statement in &sequence.statements {
            if let mermaid_parser::common::ast::SequenceStatement::Message(msg) = statement {
                if !participant_ids.contains(&msg.from) {
                    self.add_issue(
                        Severity::Error,
                        Category::Structure,
                        format!("Undefined participant '{}' used in message", msg.from),
                        None,
                    );
                }
                if !participant_ids.contains(&msg.to) {
                    self.add_issue(
                        Severity::Error,
                        Category::Structure,
                        format!("Undefined participant '{}' used in message", msg.to),
                        None,
                    );
                }
            }
        }

        // Check for accessibility
        if sequence.title.is_none() {
            self.add_issue(
                Severity::Info,
                Category::Accessibility,
                "Consider adding a title for better accessibility".to_string(),
                None,
            );
        }

        // Check for too many participants
        if sequence.participants.len() > 8 {
            self.add_issue(
                Severity::Warning,
                Category::Complexity,
                format!(
                    "Large number of participants ({}) may reduce readability",
                    sequence.participants.len()
                ),
                None,
            );
        }
    }

    fn validate_state(&mut self, state: &mermaid_parser::common::ast::StateDiagram) {
        // Check for unreachable states
        let mut reachable_states = HashSet::new();
        let mut queue = Vec::new();

        // Find start states
        for (state_id, state_def) in &state.states {
            if state_def.state_type == mermaid_parser::common::ast::StateType::Start {
                queue.push(state_id.clone());
                reachable_states.insert(state_id.clone());
            }
        }

        // If no explicit start states, consider states with no incoming transitions
        if queue.is_empty() {
            let mut has_incoming = HashSet::new();
            for transition in &state.transitions {
                has_incoming.insert(&transition.to);
            }

            for state_id in state.states.keys() {
                if !has_incoming.contains(state_id) {
                    queue.push(state_id.clone());
                    reachable_states.insert(state_id.clone());
                }
            }
        }

        // Traverse reachable states
        while let Some(current) = queue.pop() {
            for transition in &state.transitions {
                if transition.from == current && !reachable_states.contains(&transition.to) {
                    reachable_states.insert(transition.to.clone());
                    queue.push(transition.to.clone());
                }
            }
        }

        // Report unreachable states
        for state_id in state.states.keys() {
            if !reachable_states.contains(state_id) {
                self.add_issue(
                    Severity::Warning,
                    Category::Structure,
                    format!("State '{}' is unreachable", state_id),
                    Some(state_id.clone()),
                );
            }
        }

        // Check for states with no outgoing transitions (potential dead ends)
        let mut has_outgoing = HashSet::new();
        for transition in &state.transitions {
            has_outgoing.insert(&transition.from);
        }

        for (state_id, state_def) in &state.states {
            if !has_outgoing.contains(state_id)
                && state_def.state_type != mermaid_parser::common::ast::StateType::End
            {
                self.add_issue(
                    Severity::Warning,
                    Category::Structure,
                    format!("State '{}' has no outgoing transitions", state_id),
                    Some(state_id.clone()),
                );
            }
        }
    }

    fn validate_general_metrics(&mut self, metrics: &mermaid_parser::MetricsReport) {
        // Check complexity thresholds
        if metrics.complexity.cyclomatic > 10 {
            self.add_issue(
                Severity::Warning,
                Category::Complexity,
                format!(
                    "High cyclomatic complexity ({})",
                    metrics.complexity.cyclomatic
                ),
                None,
            );
        }

        if metrics.complexity.nesting_depth > 3 {
            self.add_issue(
                Severity::Info,
                Category::Complexity,
                format!(
                    "Deep nesting detected ({})",
                    metrics.complexity.nesting_depth
                ),
                None,
            );
        }

        // Check size thresholds
        if metrics.basic.node_count > 20 {
            self.add_issue(
                Severity::Info,
                Category::Complexity,
                format!(
                    "Large diagram with {} nodes - consider breaking into smaller parts",
                    metrics.basic.node_count
                ),
                None,
            );
        }
    }

    fn has_cycles(&self, flowchart: &mermaid_parser::common::ast::FlowchartDiagram) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node_id in flowchart.nodes.keys() {
            if !visited.contains(node_id)
                && has_cycle_util(flowchart, node_id, &mut visited, &mut rec_stack)
            {
                return true;
            }
        }
        false
    }
}

fn has_cycle_util(
    flowchart: &mermaid_parser::common::ast::FlowchartDiagram,
    node: &str,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
) -> bool {
    visited.insert(node.to_string());
    rec_stack.insert(node.to_string());

    for edge in &flowchart.edges {
        if edge.from == node {
            if !visited.contains(&edge.to) {
                if has_cycle_util(flowchart, &edge.to, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(&edge.to) {
                return true;
            }
        }
    }

    rec_stack.remove(node);
    false
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_diagrams = vec![
        (
            "Good Flowchart",
            r#"
flowchart TD
    title: User Registration Process
    Start([User Registration]) --> ValidateInput{Validate Input}
    ValidateInput -->|Valid| CreateAccount[Create Account]
    ValidateInput -->|Invalid| ShowError[Show Error Message]
    CreateAccount --> SendConfirmation[Send Confirmation Email]
    SendConfirmation --> Complete([Registration Complete])
    ShowError --> Start
"#,
        ),
        (
            "Problematic Flowchart",
            r#"
flowchart TD
    A --> B
    C --> D
    E --> F
    B --> G
    G --> B
"#,
        ),
        (
            "Valid Sankey",
            r#"
sankey-beta
    Input,Processing,100
    Processing,Output1,60
    Processing,Output2,40
"#,
        ),
        (
            "Invalid Sankey",
            r#"
sankey-beta
    A,B,100
    B,C,150
    A,A,10
    D,E,-5
"#,
        ),
        (
            "Good Sequence",
            r#"
sequenceDiagram
    participant User
    participant System
    participant Database
    
    User->>System: Login Request
    System->>Database: Validate Credentials
    Database-->>System: User Data
    System-->>User: Login Success
"#,
        ),
        (
            "Bad Sequence",
            r#"
sequenceDiagram
    participant User
    participant System
    
    User->>NonExistent: Message
    System->>AlsoMissing: Another Message
"#,
        ),
    ];

    println!("Diagram Validation Examples");
    println!("===========================\n");

    let mut validator = DiagramValidator::new();

    for (name, diagram_source) in test_diagrams {
        println!("Validating: {}", name);
        println!("{}", "-".repeat(40));

        match parse_diagram(diagram_source) {
            Ok(diagram) => {
                let issues = validator.validate_diagram(&diagram);

                if issues.is_empty() {
                    println!("✅ No validation issues found!");
                } else {
                    println!("Found {} validation issue(s):", issues.len());

                    for (i, issue) in issues.iter().enumerate() {
                        let severity_icon = match issue.severity {
                            Severity::Error => "🔴",
                            Severity::Warning => "🟡",
                            Severity::Info => "🔵",
                        };

                        println!(
                            "  {}. {} [{:?}] {:?}: {}",
                            i + 1,
                            severity_icon,
                            issue.severity,
                            issue.category,
                            issue.message
                        );

                        if let Some(location) = &issue.location {
                            println!("      Location: {}", location);
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ Parse error: {}", e);
            }
        }

        println!();
    }

    // Demonstrate validation report
    println!("Validation Summary");
    println!("==================");

    let complex_flowchart = r#"
flowchart TD
    A[Start] --> B{Check User}
    B -->|Authorized| C[Process Request]
    B -->|Unauthorized| D[Show Login]
    C --> E{Validate Data}
    E -->|Valid| F[Save to Database]
    E -->|Invalid| G[Show Error]
    F --> H[Send Notification]
    G --> C
    H --> I[Update UI]
    I --> J[Log Activity]
    J --> K{More Requests?}
    K -->|Yes| C
    K -->|No| L[End]
    D --> M[Authenticate]
    M --> N{Auth Success?}
    N -->|Yes| C
    N -->|No| D
"#;

    match parse_diagram(complex_flowchart) {
        Ok(diagram) => {
            let issues = validator.validate_diagram(&diagram);

            let error_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Error)
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Warning)
                .count();
            let info_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Info)
                .count();

            println!("Complex flowchart validation results:");
            println!("  🔴 Errors: {}", error_count);
            println!("  🟡 Warnings: {}", warning_count);
            println!("  🔵 Info: {}", info_count);
            println!("  ✅ Total issues: {}", issues.len());

            if issues.is_empty() {
                println!("\n🎉 Diagram passes all validation checks!");
            } else {
                println!("\n📋 Recommendations:");
                for issue in issues.iter().take(3) {
                    println!("  • {}", issue.message);
                }
                if issues.len() > 3 {
                    println!("  ... and {} more", issues.len() - 3);
                }
            }
        }
        Err(e) => {
            println!("Failed to parse complex diagram: {}", e);
        }
    }

    Ok(())
}
