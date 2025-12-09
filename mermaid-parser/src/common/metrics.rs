//! Comprehensive diagram metrics framework
//!
//! This module provides a framework for calculating various complexity and quality
//! metrics for Mermaid diagrams. It extends the existing visitor pattern to provide
//! detailed analysis and improvement suggestions.

use crate::common::ast::*;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Trait for calculating diagram metrics
///
/// This trait can be implemented by any diagram type to provide standardized
/// metrics calculation. The metrics help assess diagram complexity, quality,
/// and provide suggestions for improvement.
///
/// # Example
///
/// ```rust
/// // Example of how a DiagramMetrics trait implementation would look
/// // (In practice, this is implemented inside the crate)
///
/// use mermaid_parser::common::metrics::{DiagramMetrics, MetricsReport};
///
/// // Custom diagram type that implements DiagramMetrics
/// struct MyDiagram {
///     nodes: Vec<String>,
///     edges: Vec<(String, String)>,
/// }
///
/// impl DiagramMetrics for MyDiagram {
///     fn calculate_metrics(&self) -> MetricsReport {
///         // Calculate metrics based on nodes, edges, complexity, etc.
///         # unimplemented!()
///     }
/// }
/// ```
pub trait DiagramMetrics {
    /// Calculate comprehensive metrics for this diagram
    ///
    /// Returns a complete metrics report including basic statistics,
    /// complexity analysis, quality assessment, and improvement suggestions.
    fn calculate_metrics(&self) -> MetricsReport;
}

/// Comprehensive metrics report
///
/// Contains all calculated metrics for a diagram, including basic statistics,
/// complexity measures, quality assessments, and actionable suggestions for improvement.
///
/// # Example
///
/// ```rust
/// use mermaid_parser::common::metrics::MetricsReport;
///
/// fn analyze_diagram_quality(report: &MetricsReport) {
///     println!("Nodes: {}, Edges: {}", report.basic.node_count, report.basic.edge_count);
///     println!("Complexity: {}", report.complexity.cyclomatic);
///     println!("Quality: {:.2}", report.quality.maintainability);
///     
///     for suggestion in &report.suggestions {
///         println!("Suggestion: {}", suggestion.message);
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsReport {
    /// Basic structural metrics (node count, edge count, etc.)
    pub basic: BasicMetrics,
    /// Complexity analysis metrics
    pub complexity: ComplexityMetrics,
    /// Quality assessment metrics
    pub quality: QualityMetrics,
    /// List of improvement suggestions
    pub suggestions: Vec<Suggestion>,
}

/// Basic diagram metrics
///
/// Fundamental structural measurements of a diagram including counts of
/// nodes and edges, as well as dimensional properties like depth and breadth.
#[derive(Debug, Clone, PartialEq)]
pub struct BasicMetrics {
    /// Total number of nodes in the diagram
    pub node_count: usize,
    /// Total number of edges/connections in the diagram
    pub edge_count: usize,
    /// Maximum depth of the diagram (longest path from root)
    pub depth: usize,
    /// Maximum breadth of the diagram (widest level)
    pub breadth: usize,
}

/// Complexity metrics
///
/// Advanced measurements of diagram complexity using established software
/// engineering metrics adapted for diagram analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexityMetrics {
    /// Cyclomatic complexity - number of independent paths
    pub cyclomatic: usize,
    /// Cognitive complexity - difficulty of understanding (0.0-100.0)
    pub cognitive: f64,
    /// Maximum nesting depth of subgraphs or nested structures
    pub nesting_depth: usize,
    /// Coupling factor - degree of interconnectedness (0.0-1.0)
    pub coupling: f64,
}

/// Quality metrics
///
/// Assessment of diagram quality across multiple dimensions. All scores
/// range from 0.0 (poor) to 1.0 (excellent).
#[derive(Debug, Clone, PartialEq)]
pub struct QualityMetrics {
    /// Maintainability score - how easy the diagram is to modify (0.0-1.0)
    pub maintainability: f64,
    /// Readability score - how easy the diagram is to understand (0.0-1.0)
    pub readability: f64,
    /// Modularity score - how well-structured and organized (0.0-1.0)
    pub modularity: f64,
}

/// Improvement suggestion
///
/// Actionable recommendation for improving diagram quality, structure, or readability.
/// Each suggestion includes a category, descriptive message, and severity level.
///
/// # Example
///
/// ```rust
/// use mermaid_parser::common::metrics::{Suggestion, SuggestionCategory, SeverityLevel};
///
/// let suggestion = Suggestion {
///     category: SuggestionCategory::Structure,
///     message: "Consider adding labels to improve clarity".to_string(),
///     severity: SeverityLevel::Warning,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Category of the suggestion (performance, readability, etc.)
    pub category: SuggestionCategory,
    /// Human-readable description of the suggested improvement
    pub message: String,
    /// Severity level indicating importance of the suggestion
    pub severity: SeverityLevel,
}

/// Suggestion categories
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionCategory {
    Complexity,
    Structure,
    Naming,
    Organization,
}

/// Severity levels for suggestions
#[derive(Debug, Clone, PartialEq)]
pub enum SeverityLevel {
    Info,
    Warning,
    Error,
}

impl Display for MetricsReport {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        writeln!(f, "Diagram Metrics Report")?;
        writeln!(f, "=====================")?;
        writeln!(f, "Nodes: {}", self.basic.node_count)?;
        writeln!(f, "Edges: {}", self.basic.edge_count)?;
        writeln!(f, "Depth: {}", self.basic.depth)?;
        writeln!(f, "Breadth: {}", self.basic.breadth)?;
        writeln!(
            f,
            "Complexity: {} ({})",
            self.complexity.cyclomatic,
            complexity_rating(self.complexity.cyclomatic)
        )?;
        writeln!(f, "Cognitive Complexity: {:.1}", self.complexity.cognitive)?;
        writeln!(f, "Nesting Depth: {}", self.complexity.nesting_depth)?;
        writeln!(f, "Coupling: {:.2}", self.complexity.coupling)?;
        writeln!(
            f,
            "Maintainability: {:.1}%",
            self.quality.maintainability * 100.0
        )?;
        writeln!(f, "Readability: {:.1}%", self.quality.readability * 100.0)?;
        writeln!(f, "Modularity: {:.1}%", self.quality.modularity * 100.0)?;

        if !self.suggestions.is_empty() {
            writeln!(f, "\nSuggestions:")?;
            for suggestion in &self.suggestions {
                writeln!(
                    f,
                    "- {} [{}]: {}",
                    suggestion.severity_symbol(),
                    suggestion.category,
                    suggestion.message
                )?;
            }
        }

        Ok(())
    }
}

impl Display for SuggestionCategory {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            SuggestionCategory::Complexity => write!(f, "Complexity"),
            SuggestionCategory::Structure => write!(f, "Structure"),
            SuggestionCategory::Naming => write!(f, "Naming"),
            SuggestionCategory::Organization => write!(f, "Organization"),
        }
    }
}

impl Suggestion {
    fn severity_symbol(&self) -> &str {
        match self.severity {
            SeverityLevel::Info => "ℹ️",
            SeverityLevel::Warning => "⚠️",
            SeverityLevel::Error => "❌",
        }
    }
}

/// Get complexity rating string
fn complexity_rating(cyclomatic: usize) -> &'static str {
    match cyclomatic {
        0..=10 => "Low",
        11..=20 => "Moderate",
        21..=50 => "High",
        _ => "Very High",
    }
}

// Implement DiagramMetrics for each diagram type
impl DiagramMetrics for SankeyDiagram {
    fn calculate_metrics(&self) -> MetricsReport {
        let basic = BasicMetrics {
            node_count: self.nodes.len(),
            edge_count: self.links.len(),
            depth: calculate_sankey_depth(self),
            breadth: self.nodes.len(),
        };

        let complexity = ComplexityMetrics {
            cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
            cognitive: calculate_cognitive_complexity(&basic),
            nesting_depth: 1, // Sankey diagrams have no nesting
            coupling: calculate_coupling(&basic),
        };

        let quality = QualityMetrics {
            maintainability: calculate_maintainability(&basic, &complexity),
            readability: calculate_readability(&basic, &complexity),
            modularity: 1.0, // Sankey diagrams are inherently modular
        };

        let suggestions = generate_sankey_suggestions(&basic, &complexity);

        MetricsReport {
            basic,
            complexity,
            quality,
            suggestions,
        }
    }
}

impl DiagramMetrics for FlowchartDiagram {
    fn calculate_metrics(&self) -> MetricsReport {
        let basic = BasicMetrics {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            depth: calculate_flowchart_depth(self),
            breadth: calculate_flowchart_breadth(self),
        };

        let complexity = ComplexityMetrics {
            cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
            cognitive: calculate_cognitive_complexity(&basic),
            nesting_depth: calculate_flowchart_nesting_depth(self),
            coupling: calculate_coupling(&basic),
        };

        let quality = QualityMetrics {
            maintainability: calculate_maintainability(&basic, &complexity),
            readability: calculate_readability(&basic, &complexity),
            modularity: calculate_flowchart_modularity(self),
        };

        let suggestions = generate_flowchart_suggestions(&basic, &complexity);

        MetricsReport {
            basic,
            complexity,
            quality,
            suggestions,
        }
    }
}

impl DiagramMetrics for SequenceDiagram {
    fn calculate_metrics(&self) -> MetricsReport {
        let basic = BasicMetrics {
            node_count: self.participants.len(),
            edge_count: count_sequence_messages(&self.statements),
            depth: calculate_sequence_depth(&self.statements),
            breadth: self.participants.len(),
        };

        let complexity = ComplexityMetrics {
            cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
            cognitive: calculate_cognitive_complexity(&basic),
            nesting_depth: calculate_sequence_nesting_depth(&self.statements),
            coupling: calculate_coupling(&basic),
        };

        let quality = QualityMetrics {
            maintainability: calculate_maintainability(&basic, &complexity),
            readability: calculate_readability(&basic, &complexity),
            modularity: 0.7, // Sequence diagrams are moderately modular
        };

        let suggestions = generate_sequence_suggestions(&basic, &complexity);

        MetricsReport {
            basic,
            complexity,
            quality,
            suggestions,
        }
    }
}

impl DiagramMetrics for ClassDiagram {
    fn calculate_metrics(&self) -> MetricsReport {
        let basic = BasicMetrics {
            node_count: self.classes.len(),
            edge_count: self.relationships.len(),
            depth: calculate_class_inheritance_depth(self),
            breadth: self.classes.len(),
        };

        let complexity = ComplexityMetrics {
            cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
            cognitive: calculate_cognitive_complexity(&basic),
            nesting_depth: 1, // Classes don't nest in most cases
            coupling: calculate_coupling(&basic),
        };

        let quality = QualityMetrics {
            maintainability: calculate_maintainability(&basic, &complexity),
            readability: calculate_readability(&basic, &complexity),
            modularity: calculate_class_modularity(self),
        };

        let suggestions = generate_class_suggestions(&basic, &complexity);

        MetricsReport {
            basic,
            complexity,
            quality,
            suggestions,
        }
    }
}

impl DiagramMetrics for StateDiagram {
    fn calculate_metrics(&self) -> MetricsReport {
        let basic = BasicMetrics {
            node_count: self.states.len(),
            edge_count: self.transitions.len(),
            depth: calculate_state_depth(self),
            breadth: self.states.len(),
        };

        let complexity = ComplexityMetrics {
            cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
            cognitive: calculate_cognitive_complexity(&basic),
            nesting_depth: calculate_state_nesting_depth(self),
            coupling: calculate_coupling(&basic),
        };

        let quality = QualityMetrics {
            maintainability: calculate_maintainability(&basic, &complexity),
            readability: calculate_readability(&basic, &complexity),
            modularity: 0.6, // State diagrams have moderate modularity
        };

        let suggestions = generate_state_suggestions(&basic, &complexity);

        MetricsReport {
            basic,
            complexity,
            quality,
            suggestions,
        }
    }
}

// Implement for DiagramType enum
impl DiagramMetrics for DiagramType {
    fn calculate_metrics(&self) -> MetricsReport {
        match self {
            DiagramType::Sankey(d) => d.calculate_metrics(),
            DiagramType::Flowchart(d) => d.calculate_metrics(),
            DiagramType::Sequence(d) => d.calculate_metrics(),
            DiagramType::Class(d) => d.calculate_metrics(),
            DiagramType::State(d) => d.calculate_metrics(),
            // For other types, provide basic metrics
            _ => calculate_generic_metrics(self),
        }
    }
}

// Helper functions for metric calculations
fn calculate_cyclomatic_complexity(edges: usize, nodes: usize) -> usize {
    if nodes == 0 {
        1
    } else {
        // Cyclomatic complexity = E - N + 2, but ensure minimum of 1
        (edges + 2).saturating_sub(nodes).max(1)
    }
}

fn calculate_cognitive_complexity(basic: &BasicMetrics) -> f64 {
    // Simple cognitive complexity based on structural complexity
    let base_complexity = basic.node_count as f64 * 0.1;
    let edge_complexity = basic.edge_count as f64 * 0.2;
    let depth_complexity = basic.depth as f64 * 0.5;

    base_complexity + edge_complexity + depth_complexity
}

fn calculate_coupling(basic: &BasicMetrics) -> f64 {
    if basic.node_count == 0 {
        0.0
    } else {
        basic.edge_count as f64 / basic.node_count as f64
    }
}

fn calculate_maintainability(basic: &BasicMetrics, complexity: &ComplexityMetrics) -> f64 {
    let complexity_factor = 1.0 - (complexity.cyclomatic as f64 / 100.0).min(1.0);
    let size_factor = 1.0 - (basic.node_count as f64 / 50.0).min(1.0);

    (complexity_factor + size_factor) / 2.0
}

fn calculate_readability(basic: &BasicMetrics, complexity: &ComplexityMetrics) -> f64 {
    let complexity_factor = 1.0 - (complexity.cognitive / 20.0).min(1.0);
    let density_factor = if basic.node_count > 0 {
        1.0 - (basic.edge_count as f64 / basic.node_count as f64 / 3.0).min(1.0)
    } else {
        1.0
    };

    (complexity_factor + density_factor) / 2.0
}

// Sankey-specific calculations
fn calculate_sankey_depth(_diagram: &SankeyDiagram) -> usize {
    // For Sankey, depth is the maximum path length through the flow
    1 // Simplified implementation
}

fn generate_sankey_suggestions(
    basic: &BasicMetrics,
    complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if basic.node_count > 20 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Complexity,
            message: "Consider grouping related nodes to reduce visual complexity".to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    if complexity.coupling > 3.0 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Structure,
            message: "High coupling detected. Consider breaking into smaller flows".to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    suggestions
}

// Flowchart-specific calculations
fn calculate_flowchart_depth(diagram: &FlowchartDiagram) -> usize {
    // Maximum depth including subgraph nesting
    let subgraph_depth = diagram
        .subgraphs
        .iter()
        .map(calculate_subgraph_depth)
        .max()
        .unwrap_or(0);

    subgraph_depth + 1
}

fn calculate_subgraph_depth(subgraph: &Subgraph) -> usize {
    let nested_depth = subgraph
        .subgraphs
        .iter()
        .map(calculate_subgraph_depth)
        .max()
        .unwrap_or(0);

    nested_depth + 1
}

fn calculate_flowchart_breadth(diagram: &FlowchartDiagram) -> usize {
    // Simplified: max nodes at any level
    diagram.nodes.len()
}

fn calculate_flowchart_nesting_depth(diagram: &FlowchartDiagram) -> usize {
    diagram
        .subgraphs
        .iter()
        .map(calculate_subgraph_depth)
        .max()
        .unwrap_or(0)
}

fn calculate_flowchart_modularity(diagram: &FlowchartDiagram) -> f64 {
    if diagram.subgraphs.is_empty() {
        0.5 // No modular structure
    } else {
        // Higher modularity with more organized subgraphs
        (diagram.subgraphs.len() as f64 / (diagram.nodes.len() as f64 + 1.0)).min(1.0)
    }
}

fn generate_flowchart_suggestions(
    basic: &BasicMetrics,
    complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if complexity.cyclomatic > 20 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Complexity,
            message: "High cyclomatic complexity. Consider breaking into smaller flowcharts"
                .to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    if complexity.nesting_depth > 3 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Structure,
            message: "Deep nesting detected. Consider flattening the structure".to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    if basic.node_count > 30 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Organization,
            message: "Large diagram detected. Consider using subgraphs for organization"
                .to_string(),
            severity: SeverityLevel::Info,
        });
    }

    suggestions
}

// Sequence diagram helper functions
fn count_sequence_messages(statements: &[SequenceStatement]) -> usize {
    statements.iter().map(count_messages_in_statement).sum()
}

fn count_messages_in_statement(statement: &SequenceStatement) -> usize {
    match statement {
        SequenceStatement::Message(_) => 1,
        SequenceStatement::Loop(loop_stmt) => loop_stmt
            .statements
            .iter()
            .map(count_messages_in_statement)
            .sum(),
        SequenceStatement::Alt(alt) => {
            let main_count: usize = alt.statements.iter().map(count_messages_in_statement).sum();
            let else_count: usize = alt
                .else_branch
                .as_ref()
                .map(|eb| eb.statements.iter().map(count_messages_in_statement).sum())
                .unwrap_or(0);
            main_count + else_count
        }
        SequenceStatement::Opt(opt) => opt.statements.iter().map(count_messages_in_statement).sum(),
        SequenceStatement::Par(par) => par
            .branches
            .iter()
            .map(|branch| {
                branch
                    .statements
                    .iter()
                    .map(count_messages_in_statement)
                    .sum::<usize>()
            })
            .sum(),
        SequenceStatement::Critical(critical) => {
            let main_count: usize = critical
                .statements
                .iter()
                .map(count_messages_in_statement)
                .sum();
            let option_count: usize = critical
                .options
                .iter()
                .map(|option| {
                    option
                        .statements
                        .iter()
                        .map(count_messages_in_statement)
                        .sum::<usize>()
                })
                .sum();
            main_count + option_count
        }
        _ => 0,
    }
}

fn calculate_sequence_depth(statements: &[SequenceStatement]) -> usize {
    statements
        .iter()
        .map(calculate_statement_depth)
        .max()
        .unwrap_or(1)
}

fn calculate_statement_depth(statement: &SequenceStatement) -> usize {
    match statement {
        SequenceStatement::Message(_) => 1,
        SequenceStatement::Loop(loop_stmt) => {
            1 + loop_stmt
                .statements
                .iter()
                .map(calculate_statement_depth)
                .max()
                .unwrap_or(0)
        }
        SequenceStatement::Alt(alt) => {
            let main_depth = alt
                .statements
                .iter()
                .map(calculate_statement_depth)
                .max()
                .unwrap_or(0);
            let else_depth = alt
                .else_branch
                .as_ref()
                .map(|eb| {
                    eb.statements
                        .iter()
                        .map(calculate_statement_depth)
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            1 + main_depth.max(else_depth)
        }
        SequenceStatement::Opt(opt) => {
            1 + opt
                .statements
                .iter()
                .map(calculate_statement_depth)
                .max()
                .unwrap_or(0)
        }
        SequenceStatement::Par(par) => {
            1 + par
                .branches
                .iter()
                .map(|branch| {
                    branch
                        .statements
                        .iter()
                        .map(calculate_statement_depth)
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0)
        }
        SequenceStatement::Critical(critical) => {
            let main_depth = critical
                .statements
                .iter()
                .map(calculate_statement_depth)
                .max()
                .unwrap_or(0);
            let option_depth = critical
                .options
                .iter()
                .map(|option| {
                    option
                        .statements
                        .iter()
                        .map(calculate_statement_depth)
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            1 + main_depth.max(option_depth)
        }
        _ => 1,
    }
}

fn calculate_sequence_nesting_depth(statements: &[SequenceStatement]) -> usize {
    calculate_sequence_depth(statements)
}

fn generate_sequence_suggestions(
    basic: &BasicMetrics,
    complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if basic.edge_count > 50 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Complexity,
            message: "High message count. Consider breaking into smaller sequence diagrams"
                .to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    if complexity.nesting_depth > 4 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Structure,
            message: "Deep nesting in sequence blocks. Consider simplifying control flow"
                .to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    suggestions
}

// Class diagram helper functions
fn calculate_class_inheritance_depth(_diagram: &ClassDiagram) -> usize {
    // Simplified: would need to analyze inheritance relationships
    1
}

fn calculate_class_modularity(_diagram: &ClassDiagram) -> f64 {
    // Simplified: based on relationship density
    0.8
}

fn generate_class_suggestions(
    basic: &BasicMetrics,
    complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if basic.node_count > 25 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Organization,
            message: "Large number of classes. Consider using packages or modules".to_string(),
            severity: SeverityLevel::Info,
        });
    }

    if complexity.coupling > 2.5 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Structure,
            message: "High coupling between classes. Consider reducing dependencies".to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    suggestions
}

// State diagram helper functions
fn calculate_state_depth(_diagram: &StateDiagram) -> usize {
    // Simplified: would need to analyze state hierarchy
    1
}

fn calculate_state_nesting_depth(_diagram: &StateDiagram) -> usize {
    // Simplified: would need to analyze composite states
    1
}

fn generate_state_suggestions(
    basic: &BasicMetrics,
    complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if basic.node_count > 20 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Complexity,
            message: "Large state space. Consider using composite states".to_string(),
            severity: SeverityLevel::Info,
        });
    }

    if complexity.coupling > 3.0 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Structure,
            message: "High transition density. Consider simplifying state machine".to_string(),
            severity: SeverityLevel::Warning,
        });
    }

    suggestions
}

// Generic metrics for unsupported diagram types
fn calculate_generic_metrics(diagram: &DiagramType) -> MetricsReport {
    // Use the existing visitor pattern for basic counts
    use crate::common::visitor::NodeCounter;
    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);

    let basic = BasicMetrics {
        node_count: counter.nodes(),
        edge_count: counter.edges(),
        depth: 1,
        breadth: counter.nodes(),
    };

    let complexity = ComplexityMetrics {
        cyclomatic: calculate_cyclomatic_complexity(basic.edge_count, basic.node_count),
        cognitive: calculate_cognitive_complexity(&basic),
        nesting_depth: 1,
        coupling: calculate_coupling(&basic),
    };

    let quality = QualityMetrics {
        maintainability: calculate_maintainability(&basic, &complexity),
        readability: calculate_readability(&basic, &complexity),
        modularity: 0.5, // Default moderate modularity
    };

    let suggestions = generate_generic_suggestions(&basic, &complexity);

    MetricsReport {
        basic,
        complexity,
        quality,
        suggestions,
    }
}

fn generate_generic_suggestions(
    basic: &BasicMetrics,
    _complexity: &ComplexityMetrics,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    if basic.node_count > 20 {
        suggestions.push(Suggestion {
            category: SuggestionCategory::Organization,
            message: "Consider organizing into smaller, focused diagrams".to_string(),
            severity: SeverityLevel::Info,
        });
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sankey_metrics_calculation() {
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
        };

        let metrics = diagram.calculate_metrics();

        assert_eq!(metrics.basic.node_count, 3);
        assert_eq!(metrics.basic.edge_count, 2);
        assert_eq!(metrics.complexity.cyclomatic, 1); // 2 - 3 + 2 = 1 (with saturation)
        assert!(metrics.quality.maintainability > 0.0);
        assert!(metrics.quality.readability > 0.0);
    }

    #[test]
    fn test_flowchart_metrics_calculation() {
        use std::collections::HashMap;

        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Start".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );
        nodes.insert(
            "B".to_string(),
            FlowNode {
                id: "B".to_string(),
                text: Some("Process".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        };

        let metrics = diagram.calculate_metrics();

        assert_eq!(metrics.basic.node_count, 2);
        assert_eq!(metrics.basic.edge_count, 1);
        assert_eq!(metrics.complexity.cyclomatic, 1); // 1 - 2 + 2 = 1 (with saturation)
        assert!(metrics.quality.maintainability > 0.0);
    }

    #[test]
    fn test_metrics_report_display() {
        let report = MetricsReport {
            basic: BasicMetrics {
                node_count: 5,
                edge_count: 4,
                depth: 2,
                breadth: 3,
            },
            complexity: ComplexityMetrics {
                cyclomatic: 6,
                cognitive: 2.5,
                nesting_depth: 1,
                coupling: 0.8,
            },
            quality: QualityMetrics {
                maintainability: 0.85,
                readability: 0.75,
                modularity: 0.6,
            },
            suggestions: vec![Suggestion {
                category: SuggestionCategory::Complexity,
                message: "Consider simplification".to_string(),
                severity: SeverityLevel::Warning,
            }],
        };

        let output = format!("{}", report);
        assert!(output.contains("Diagram Metrics Report"));
        assert!(output.contains("Nodes: 5"));
        assert!(output.contains("Complexity: 6 (Low)"));
        assert!(output.contains("Suggestions:"));
        assert!(output.contains("Consider simplification"));
    }

    #[test]
    fn test_complexity_rating() {
        assert_eq!(complexity_rating(5), "Low");
        assert_eq!(complexity_rating(15), "Moderate");
        assert_eq!(complexity_rating(25), "High");
        assert_eq!(complexity_rating(100), "Very High");
    }

    #[test]
    fn test_suggestion_severity_symbol() {
        let info_suggestion = Suggestion {
            category: SuggestionCategory::Structure,
            message: "Info message".to_string(),
            severity: SeverityLevel::Info,
        };
        assert_eq!(info_suggestion.severity_symbol(), "ℹ️");

        let warning_suggestion = Suggestion {
            category: SuggestionCategory::Complexity,
            message: "Warning message".to_string(),
            severity: SeverityLevel::Warning,
        };
        assert_eq!(warning_suggestion.severity_symbol(), "⚠️");

        let error_suggestion = Suggestion {
            category: SuggestionCategory::Naming,
            message: "Error message".to_string(),
            severity: SeverityLevel::Error,
        };
        assert_eq!(error_suggestion.severity_symbol(), "❌");
    }

    #[test]
    fn test_sequence_diagram_metrics() {
        let diagram = SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![
                Participant {
                    actor: "Alice".to_string(),
                    alias: None,
                    participant_type: ParticipantType::Actor,
                },
                Participant {
                    actor: "Bob".to_string(),
                    alias: None,
                    participant_type: ParticipantType::Actor,
                },
            ],
            statements: vec![SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Bob".to_string(),
                text: "Hello".to_string(),
                arrow_type: ArrowType::SolidOpen,
            })],
            autonumber: None,
        };

        let metrics = diagram.calculate_metrics();

        assert_eq!(metrics.basic.node_count, 2);
        assert_eq!(metrics.basic.edge_count, 1);
        assert!(metrics.quality.maintainability > 0.0);
    }

    #[test]
    fn test_diagram_type_metrics() {
        let diagram = DiagramType::Sankey(SankeyDiagram {
            nodes: vec![SankeyNode {
                id: "A".to_string(),
                name: "Node A".to_string(),
            }],
            links: vec![],
        });

        let metrics = diagram.calculate_metrics();

        assert_eq!(metrics.basic.node_count, 1);
        assert_eq!(metrics.basic.edge_count, 0);
    }

    #[test]
    fn test_generic_metrics_calculation() {
        let diagram = DiagramType::Timeline(TimelineDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });

        let metrics = diagram.calculate_metrics();

        // Should use generic metrics for timeline diagrams
        assert_eq!(metrics.basic.depth, 1);
        assert_eq!(metrics.quality.modularity, 0.5);
    }
}
