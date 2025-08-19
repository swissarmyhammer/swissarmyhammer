//! CLI Exclusion Validation System
//!
//! This module provides comprehensive validation for the CLI exclusion system,
//! helping developers maintain consistency and correctness in tool exclusions.

use crate::cli::{CliExclusionDetector, ToolCliMetadata};
use std::collections::HashMap;

/// Configuration for CLI exclusion validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Whether to check for missing exclusions
    pub check_missing_exclusions: bool,
    /// Whether to validate exclusion reasoning
    pub check_exclusion_reasoning: bool,
    /// Whether to verify CLI alternatives exist
    pub check_cli_alternatives: bool,
    /// Patterns that suggest a tool should be excluded
    pub exclusion_patterns: Vec<String>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_missing_exclusions: true,
            check_exclusion_reasoning: true,
            check_cli_alternatives: true,
            exclusion_patterns: vec![
                // Most specific patterns first
                "issue_work".to_string(),
                "issue_merge".to_string(),
                // General patterns second
                "_work".to_string(),
                "_merge".to_string(),
                "abort_".to_string(),
                "_transition".to_string(),
                "_orchestrate".to_string(),
                "_coordinate".to_string(),
                "_manage".to_string(),
                "workflow_".to_string(),
            ],
        }
    }
}

/// Types of validation issues
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationIssue {
    /// Suggest that a tool should be excluded
    SuggestExclusion {
        /// Name of the tool that should be excluded
        tool_name: String,
        /// Reason why the tool should be excluded
        reason: String,
        /// Confidence level (0.0 to 1.0) that the tool should be excluded
        confidence: f64,
    },
    /// Missing exclusion reasoning documentation
    MissingExclusionReason {
        /// Name of the tool missing exclusion reason
        tool_name: String,
    },
    /// Missing CLI alternatives documentation
    MissingCliAlternatives {
        /// Name of the tool missing CLI alternatives
        tool_name: String,
    },
    /// Inconsistent naming conventions
    InconsistentNaming {
        /// Name of the tool with inconsistent naming
        tool_name: String,
        /// Suggested consistent name for the tool
        suggested_name: String,
    },
}

/// Validation warnings (less severe than issues)
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationWarning {
    /// Tool might benefit from exclusion
    MayBenefitFromExclusion {
        /// Name of the tool that might benefit from exclusion
        tool_name: String,
        /// Reason why the tool might benefit from exclusion
        reason: String,
        /// Confidence level (0.0 to 1.0) that the tool would benefit from exclusion
        confidence: f64,
    },
    /// Exclusion reason could be more descriptive
    VagueExclusionReason {
        /// Name of the tool with a vague exclusion reason
        tool_name: String,
        /// Current exclusion reason that could be improved
        current_reason: String,
    },
}

/// Summary of validation results
#[derive(Debug, Clone)]
pub struct ValidationSummary {
    /// Total number of tools analyzed
    pub total_tools: usize,
    /// Number of tools excluded from CLI
    pub excluded_tools: usize,
    /// Number of tools eligible for CLI inclusion
    pub eligible_tools: usize,
    /// Number of validation issues found
    pub issues_found: usize,
    /// Number of validation warnings found
    pub warnings_found: usize,
}

/// Complete validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// List of validation issues found
    pub issues: Vec<ValidationIssue>,
    /// List of validation warnings found
    pub warnings: Vec<ValidationWarning>,
    /// Summary statistics of the validation
    pub summary: ValidationSummary,
}

impl ValidationReport {
    /// Create a new empty validation report
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            warnings: Vec::new(),
            summary: ValidationSummary {
                total_tools: 0,
                excluded_tools: 0,
                eligible_tools: 0,
                issues_found: 0,
                warnings_found: 0,
            },
        }
    }

    /// Add a validation issue to the report
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
        self.summary.issues_found = self.issues.len();
    }

    /// Add a validation warning to the report
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
        self.summary.warnings_found = self.warnings.len();
    }

    /// Check if the report contains any validation issues
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Check if the report contains any validation warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Tool analysis and development utilities
#[derive(Debug, Clone)]
pub struct ToolAnalysis {
    /// Tool name
    pub name: String,
    /// Whether tool is currently CLI excluded
    pub is_excluded: bool,
    /// Exclusion reason if provided
    pub exclusion_reason: Option<String>,
    /// Pattern matches and their confidence scores
    pub pattern_matches: Vec<(String, f64)>,
    /// Suggested action based on analysis
    pub suggestion: ToolSuggestion,
}

/// Suggestions for tool configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ToolSuggestion {
    /// Tool is properly configured
    CorrectlyConfigured,
    /// Tool should be excluded from CLI
    ShouldExclude { 
        /// Reason why the tool should be excluded
        reason: String, 
        /// Confidence level (0.0 to 1.0) of the recommendation
        confidence: f64 
    },
    /// Tool should include CLI usage
    ShouldInclude { 
        /// Reason why the tool should be included
        reason: String 
    },
    /// Exclusion reason needs improvement
    ImproveReason { 
        /// Current exclusion reason that needs improvement
        current_reason: String, 
        /// Suggested improved reason
        suggestion: String 
    },
}

/// Development utilities for analyzing CLI exclusion patterns
pub struct DevUtilities;

impl DevUtilities {
    /// Analyze all tools and provide development insights
    pub fn analyze_all_tools<D: CliExclusionDetector>(detector: &D) -> Vec<ToolAnalysis> {
        let all_metadata = detector.get_all_tool_metadata();
        let validator = ExclusionValidator::default();
        
        all_metadata.into_iter()
            .map(|metadata| Self::analyze_tool(&metadata, &validator))
            .collect()
    }
    
    /// Analyze a specific tool and provide insights
    pub fn analyze_tool(metadata: &ToolCliMetadata, validator: &ExclusionValidator) -> ToolAnalysis {
        let pattern_matches = Self::get_pattern_matches(&metadata.name, validator);
        let suggestion = Self::generate_suggestion(metadata, &pattern_matches);
        
        ToolAnalysis {
            name: metadata.name.clone(),
            is_excluded: metadata.is_cli_excluded,
            exclusion_reason: metadata.exclusion_reason.clone(),
            pattern_matches,
            suggestion,
        }
    }
    
    /// Get all pattern matches for a tool name
    fn get_pattern_matches(tool_name: &str, validator: &ExclusionValidator) -> Vec<(String, f64)> {
        let mut matches = Vec::new();
        
        for pattern in &validator.config.exclusion_patterns {
            if tool_name.contains(pattern) {
                let confidence = match pattern.as_str() {
                    "issue_work" | "issue_merge" => 1.0,
                    "_work" | "_merge" | "abort_" => 0.9,
                    "workflow_" => 0.8,
                    "_orchestrate" | "_coordinate" => 0.7,
                    _ => 0.6,
                };
                matches.push((pattern.clone(), confidence));
            }
        }
        
        // Sort by confidence descending
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
    }
    
    /// Generate suggestion based on tool analysis
    fn generate_suggestion(metadata: &ToolCliMetadata, pattern_matches: &[(String, f64)]) -> ToolSuggestion {
        // If tool has high-confidence pattern matches but isn't excluded
        if let Some((pattern, confidence)) = pattern_matches.first() {
            if *confidence >= 0.8 && !metadata.is_cli_excluded {
                return ToolSuggestion::ShouldExclude {
                    reason: format!("Tool contains '{}' pattern suggesting MCP-only usage", pattern),
                    confidence: *confidence,
                };
            }
        }
        
        // If tool is excluded, check for issues
        if metadata.is_cli_excluded {
            // First check if it's excluded but has no pattern matches (should include)
            if pattern_matches.is_empty() {
                return ToolSuggestion::ShouldInclude {
                    reason: "Tool is excluded but doesn't match any exclusion patterns".to_string(),
                };
            }
            
            // Then check exclusion reasoning quality
            if let Some(ref reason) = metadata.exclusion_reason {
                if reason.len() < 10 || reason.split_whitespace().count() < 3 {
                    return ToolSuggestion::ImproveReason {
                        current_reason: reason.clone(),
                        suggestion: "Provide a more descriptive reason explaining why this tool is MCP-only".to_string(),
                    };
                }
            } else {
                return ToolSuggestion::ImproveReason {
                    current_reason: "None".to_string(),
                    suggestion: "Add exclusion_reason explaining why this tool is excluded from CLI".to_string(),
                };
            }
        }
        
        ToolSuggestion::CorrectlyConfigured
    }
    
    /// Generate summary statistics about tool exclusions
    pub fn generate_statistics<D: CliExclusionDetector>(detector: &D) -> HashMap<String, usize> {
        let analyses = Self::analyze_all_tools(detector);
        let mut stats = HashMap::new();
        
        stats.insert("total_tools".to_string(), analyses.len());
        stats.insert("excluded_tools".to_string(), 
                    analyses.iter().filter(|a| a.is_excluded).count());
        stats.insert("should_exclude".to_string(), 
                    analyses.iter().filter(|a| matches!(a.suggestion, ToolSuggestion::ShouldExclude { .. })).count());
        stats.insert("should_include".to_string(), 
                    analyses.iter().filter(|a| matches!(a.suggestion, ToolSuggestion::ShouldInclude { .. })).count());
        stats.insert("need_better_reasons".to_string(), 
                    analyses.iter().filter(|a| matches!(a.suggestion, ToolSuggestion::ImproveReason { .. })).count());
        stats.insert("correctly_configured".to_string(), 
                    analyses.iter().filter(|a| matches!(a.suggestion, ToolSuggestion::CorrectlyConfigured)).count());
        
        stats
    }
    
    /// Find tools that might be missing from the exclusion system
    pub fn find_potential_exclusions<D: CliExclusionDetector>(detector: &D) -> Vec<ToolAnalysis> {
        Self::analyze_all_tools(detector)
            .into_iter()
            .filter(|analysis| matches!(analysis.suggestion, ToolSuggestion::ShouldExclude { .. }))
            .collect()
    }
    
    /// Find tools that might be incorrectly excluded
    pub fn find_potential_inclusions<D: CliExclusionDetector>(detector: &D) -> Vec<ToolAnalysis> {
        Self::analyze_all_tools(detector)
            .into_iter()
            .filter(|analysis| matches!(analysis.suggestion, ToolSuggestion::ShouldInclude { .. }))
            .collect()
    }
}

/// Documentation generator for CLI exclusion system
pub struct DocumentationGenerator;

impl DocumentationGenerator {
    /// Generate markdown documentation for all excluded tools
    pub fn generate_excluded_tools_doc<D: CliExclusionDetector>(detector: &D) -> String {
        let all_metadata = detector.get_all_tool_metadata();
        let excluded_tools: Vec<_> = all_metadata.iter()
            .filter(|m| m.is_cli_excluded)
            .collect();
        
        let mut doc = String::new();
        doc.push_str("# CLI Excluded Tools\n\n");
        doc.push_str("This document lists all MCP tools that are excluded from CLI generation.\n\n");
        doc.push_str(&format!("**Total excluded tools:** {}\n\n", excluded_tools.len()));
        
        if excluded_tools.is_empty() {
            doc.push_str("*No tools are currently excluded from CLI generation.*\n");
            return doc;
        }
        
        // Group tools by exclusion reason patterns
        let mut by_reason: HashMap<String, Vec<&ToolCliMetadata>> = HashMap::new();
        for tool in &excluded_tools {
            let key = if let Some(reason) = &tool.exclusion_reason {
                Self::categorize_reason(reason)
            } else {
                "No reason provided".to_string()
            };
            by_reason.entry(key).or_default().push(tool);
        }
        
        for (category, tools) in by_reason {
            doc.push_str(&format!("## {}\n\n", category));
            
            for tool in tools {
                doc.push_str(&format!("### `{}`\n\n", tool.name));
                if let Some(reason) = &tool.exclusion_reason {
                    doc.push_str(&format!("**Reason:** {}\n\n", reason));
                } else {
                    doc.push_str("**Reason:** *Not specified*\n\n");
                }
            }
        }
        
        doc
    }
    
    /// Generate development report with suggestions and statistics
    pub fn generate_dev_report<D: CliExclusionDetector>(detector: &D) -> String {
        let analyses = DevUtilities::analyze_all_tools(detector);
        let stats = DevUtilities::generate_statistics(detector);
        
        let mut doc = String::new();
        doc.push_str("# CLI Exclusion System Development Report\n\n");
        
        // Statistics section
        doc.push_str("## Summary Statistics\n\n");
        doc.push_str(&format!("- **Total tools:** {}\n", stats.get("total_tools").unwrap_or(&0)));
        doc.push_str(&format!("- **CLI excluded:** {}\n", stats.get("excluded_tools").unwrap_or(&0)));
        doc.push_str(&format!("- **CLI eligible:** {}\n", stats.get("total_tools").unwrap_or(&0) - stats.get("excluded_tools").unwrap_or(&0)));
        doc.push_str(&format!("- **Should exclude:** {}\n", stats.get("should_exclude").unwrap_or(&0)));
        doc.push_str(&format!("- **Should include:** {}\n", stats.get("should_include").unwrap_or(&0)));
        doc.push_str(&format!("- **Need better reasons:** {}\n", stats.get("need_better_reasons").unwrap_or(&0)));
        doc.push_str(&format!("- **Correctly configured:** {}\n", stats.get("correctly_configured").unwrap_or(&0)));
        doc.push_str("\n");
        
        // Suggestions sections
        let should_exclude: Vec<_> = analyses.iter()
            .filter(|a| matches!(a.suggestion, ToolSuggestion::ShouldExclude { .. }))
            .collect();
        
        let should_include: Vec<_> = analyses.iter()
            .filter(|a| matches!(a.suggestion, ToolSuggestion::ShouldInclude { .. }))
            .collect();
        
        let improve_reason: Vec<_> = analyses.iter()
            .filter(|a| matches!(a.suggestion, ToolSuggestion::ImproveReason { .. }))
            .collect();
        
        if !should_exclude.is_empty() {
            doc.push_str("## Tools That Should Be Excluded\n\n");
            for analysis in should_exclude {
                if let ToolSuggestion::ShouldExclude { reason, confidence } = &analysis.suggestion {
                    doc.push_str(&format!("### `{}` (confidence: {:.1})\n\n", analysis.name, confidence));
                    doc.push_str(&format!("**Reason:** {}\n\n", reason));
                    doc.push_str("**Pattern matches:**\n");
                    for (pattern, conf) in &analysis.pattern_matches {
                        doc.push_str(&format!("- `{}` (confidence: {:.1})\n", pattern, conf));
                    }
                    doc.push_str("\n");
                }
            }
        }
        
        if !should_include.is_empty() {
            doc.push_str("## Tools That Should Be Included\n\n");
            for analysis in should_include {
                if let ToolSuggestion::ShouldInclude { reason } = &analysis.suggestion {
                    doc.push_str(&format!("### `{}`\n\n", analysis.name));
                    doc.push_str(&format!("**Reason:** {}\n\n", reason));
                    if let Some(current_reason) = &analysis.exclusion_reason {
                        doc.push_str(&format!("**Current exclusion reason:** {}\n\n", current_reason));
                    }
                }
            }
        }
        
        if !improve_reason.is_empty() {
            doc.push_str("## Tools Needing Better Exclusion Reasons\n\n");
            for analysis in improve_reason {
                if let ToolSuggestion::ImproveReason { current_reason, suggestion } = &analysis.suggestion {
                    doc.push_str(&format!("### `{}`\n\n", analysis.name));
                    doc.push_str(&format!("**Current reason:** {}\n\n", current_reason));
                    doc.push_str(&format!("**Suggestion:** {}\n\n", suggestion));
                }
            }
        }
        
        doc
    }
    
    /// Categorize exclusion reasons for grouping
    fn categorize_reason(reason: &str) -> String {
        let lower = reason.to_lowercase();
        if lower.contains("workflow") || lower.contains("orchestrat") || lower.contains("mcp") {
            "MCP Workflow Tools".to_string()
        } else if lower.contains("internal") || lower.contains("private") {
            "Internal Tools".to_string()
        } else if lower.contains("experimental") || lower.contains("test") {
            "Experimental/Testing Tools".to_string()
        } else if lower.contains("deprecated") || lower.contains("legacy") {
            "Deprecated Tools".to_string()
        } else {
            "Other Exclusions".to_string()
        }
    }
}

/// Main validator for CLI exclusion system
pub struct ExclusionValidator {
    config: ValidationConfig,
}

impl ExclusionValidator {
    /// Create a new validator with the given configuration
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Create a validator with default configuration
    pub fn default() -> Self {
        Self::new(ValidationConfig::default())
    }

    /// Validate the entire exclusion system
    pub fn validate_all<D: CliExclusionDetector>(&self, detector: &D) -> ValidationReport {
        let mut report = ValidationReport::new();
        
        let all_metadata = detector.get_all_tool_metadata();
        
        // Update summary with basic counts
        report.summary.total_tools = all_metadata.len();
        report.summary.excluded_tools = all_metadata.iter()
            .filter(|m| m.is_cli_excluded)
            .count();
        report.summary.eligible_tools = all_metadata.iter()
            .filter(|m| !m.is_cli_excluded)
            .count();
        
        if self.config.check_missing_exclusions {
            self.validate_exclusion_consistency(&all_metadata, &mut report);
        }
        
        if self.config.check_exclusion_reasoning {
            self.validate_exclusion_reasoning(&all_metadata, &mut report);
        }
        
        if self.config.check_cli_alternatives {
            self.validate_cli_alternatives(&all_metadata, &mut report);
        }
        
        report
    }

    /// Check for tools that should probably be excluded but aren't
    fn validate_exclusion_consistency(&self, metadata: &[ToolCliMetadata], report: &mut ValidationReport) {
        for tool_meta in metadata {
            if !tool_meta.is_cli_excluded {
                if let Some((reason, confidence)) = self.should_probably_be_excluded(&tool_meta.name) {
                    if confidence >= 0.8 {
                        report.add_issue(ValidationIssue::SuggestExclusion {
                            tool_name: tool_meta.name.clone(),
                            reason,
                            confidence,
                        });
                    } else if confidence >= 0.5 {
                        report.add_warning(ValidationWarning::MayBenefitFromExclusion {
                            tool_name: tool_meta.name.clone(),
                            reason,
                            confidence,
                        });
                    }
                }
            }
        }
    }

    /// Check for proper exclusion reasoning documentation
    fn validate_exclusion_reasoning(&self, metadata: &[ToolCliMetadata], report: &mut ValidationReport) {
        for tool_meta in metadata {
            if tool_meta.is_cli_excluded && tool_meta.exclusion_reason.is_none() {
                report.add_issue(ValidationIssue::MissingExclusionReason {
                    tool_name: tool_meta.name.clone(),
                });
            } else if let Some(ref reason) = tool_meta.exclusion_reason {
                // Check if reason is too vague
                if reason.len() < 10 || reason.split_whitespace().count() < 3 {
                    report.add_warning(ValidationWarning::VagueExclusionReason {
                        tool_name: tool_meta.name.clone(),
                        current_reason: reason.clone(),
                    });
                }
            }
        }
    }

    /// Check that excluded tools have CLI alternatives documented
    fn validate_cli_alternatives(&self, metadata: &[ToolCliMetadata], _report: &mut ValidationReport) {
        for tool_meta in metadata {
            // Note: CLI alternatives are not yet part of ToolCliMetadata
            // This is a placeholder for future implementation
            if tool_meta.is_cli_excluded {
                // For now, we'll skip this validation since cli_alternatives isn't implemented
                // report.add_issue(ValidationIssue::MissingCliAlternatives {
                //     tool_name: tool_meta.name.clone(),
                // });
            }
        }
    }

    /// Determine if a tool should probably be excluded based on patterns
    fn should_probably_be_excluded(&self, tool_name: &str) -> Option<(String, f64)> {
        let mut best_match: Option<(String, f64, &str)> = None;
        
        // Check all patterns and find the best match (highest confidence)
        for pattern in &self.config.exclusion_patterns {
            if tool_name.contains(pattern) {
                let confidence = match pattern.as_str() {
                    "issue_work" | "issue_merge" => 1.0,
                    "_work" | "_merge" | "abort_" => 0.9,
                    "workflow_" => 0.8,
                    "_orchestrate" | "_coordinate" => 0.7,
                    _ => 0.6,
                };
                
                // Update best match if this has higher confidence
                if let Some((_, current_confidence, _)) = &best_match {
                    if confidence > *current_confidence {
                        best_match = Some((
                            format!(
                                "Contains workflow indicator '{}' suggesting MCP-only usage",
                                pattern
                            ),
                            confidence,
                            pattern
                        ));
                    }
                } else {
                    best_match = Some((
                        format!(
                            "Contains workflow indicator '{}' suggesting MCP-only usage",
                            pattern
                        ),
                        confidence,
                        pattern
                    ));
                }
            }
        }
        
        best_match.map(|(reason, confidence, _)| (reason, confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{RegistryCliExclusionDetector, ToolCliMetadata};
    use std::collections::HashMap;

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert!(config.check_missing_exclusions);
        assert!(config.check_exclusion_reasoning);
        assert!(config.check_cli_alternatives);
        assert!(!config.exclusion_patterns.is_empty());
    }

    #[test]
    fn test_validation_report_creation() {
        let report = ValidationReport::new();
        assert!(report.issues.is_empty());
        assert!(report.warnings.is_empty());
        assert_eq!(report.summary.total_tools, 0);
        assert!(!report.has_issues());
        assert!(!report.has_warnings());
    }

    #[test]
    fn test_validation_report_add_issue() {
        let mut report = ValidationReport::new();
        let issue = ValidationIssue::SuggestExclusion {
            tool_name: "test_tool".to_string(),
            reason: "Test reason".to_string(),
            confidence: 0.9,
        };
        
        report.add_issue(issue);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.summary.issues_found, 1);
        assert!(report.has_issues());
    }

    #[test]
    fn test_exclusion_validator_creation() {
        let config = ValidationConfig::default();
        let validator = ExclusionValidator::new(config);
        assert!(validator.config.check_missing_exclusions);
        
        let default_validator = ExclusionValidator::default();
        assert!(default_validator.config.check_exclusion_reasoning);
    }

    #[test] 
    fn test_should_probably_be_excluded() {
        let validator = ExclusionValidator::default();
        
        let (reason, confidence) = validator.should_probably_be_excluded("issue_work").unwrap();
        
        // Should now match the exact "issue_work" pattern with confidence 1.0
        assert_eq!(confidence, 1.0);
        assert!(reason.contains("issue_work"));
        
        // Test pattern matching - only matches "_work" pattern  
        let (reason, confidence) = validator.should_probably_be_excluded("tool_work").unwrap();
        assert_eq!(confidence, 0.9); // _work pattern
        assert!(reason.contains("_work"));
        
        // Test workflow pattern
        let (reason, confidence) = validator.should_probably_be_excluded("workflow_orchestrator").unwrap();
        assert_eq!(confidence, 0.8); // workflow_ pattern
        assert!(reason.contains("workflow_"));
        
        // Test no match
        assert!(validator.should_probably_be_excluded("normal_tool").is_none());
    }

    #[test]
    fn test_validate_exclusion_consistency() {
        let validator = ExclusionValidator::default();
        
        // Create test metadata
        let mut metadata_map = HashMap::new();
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::included("issue_work"));
        metadata_map.insert("normal_tool".to_string(), ToolCliMetadata::included("normal_tool"));
        metadata_map.insert("excluded_tool".to_string(), ToolCliMetadata::excluded("excluded_tool", "Test reason"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let report = validator.validate_all(&detector);
        
        // Should suggest exclusion for issue_work
        assert!(report.has_issues());
        let suggest_issues: Vec<_> = report.issues.iter()
            .filter(|issue| matches!(issue, ValidationIssue::SuggestExclusion { .. }))
            .collect();
        assert_eq!(suggest_issues.len(), 1);
        
        if let ValidationIssue::SuggestExclusion { tool_name, confidence, .. } = &suggest_issues[0] {
            assert_eq!(tool_name, "issue_work");
            assert_eq!(*confidence, 1.0); // Now matches "issue_work" pattern exactly
        }
    }

    #[test]
    fn test_validate_exclusion_reasoning() {
        let validator = ExclusionValidator::default();
        
        // Create test metadata with missing reasoning
        let mut metadata_map = HashMap::new();
        metadata_map.insert("excluded_no_reason".to_string(), ToolCliMetadata {
            name: "excluded_no_reason".to_string(),
            is_cli_excluded: true,
            exclusion_reason: None,
        });
        metadata_map.insert("excluded_vague_reason".to_string(), ToolCliMetadata {
            name: "excluded_vague_reason".to_string(),
            is_cli_excluded: true,
            exclusion_reason: Some("Bad".to_string()),
        });
        metadata_map.insert("excluded_good_reason".to_string(), ToolCliMetadata::excluded(
            "excluded_good_reason", 
            "This tool is designed for MCP workflow orchestration only"
        ));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let report = validator.validate_all(&detector);
        
        // Should have issue for missing reason
        let missing_reason_issues: Vec<_> = report.issues.iter()
            .filter(|issue| matches!(issue, ValidationIssue::MissingExclusionReason { .. }))
            .collect();
        assert_eq!(missing_reason_issues.len(), 1);
        
        // Should have warning for vague reason
        let vague_reason_warnings: Vec<_> = report.warnings.iter()
            .filter(|warning| matches!(warning, ValidationWarning::VagueExclusionReason { .. }))
            .collect();
        assert_eq!(vague_reason_warnings.len(), 1);
    }

    #[test]
    fn test_validate_all_summary() {
        let validator = ExclusionValidator::default();
        
        let mut metadata_map = HashMap::new();
        metadata_map.insert("tool1".to_string(), ToolCliMetadata::included("tool1"));
        metadata_map.insert("tool2".to_string(), ToolCliMetadata::included("tool2"));
        metadata_map.insert("excluded1".to_string(), ToolCliMetadata::excluded("excluded1", "Good reason"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let report = validator.validate_all(&detector);
        
        assert_eq!(report.summary.total_tools, 3);
        assert_eq!(report.summary.excluded_tools, 1);
        assert_eq!(report.summary.eligible_tools, 2);
    }

    #[test]
    fn test_dev_utilities_tool_analysis() {
        let validator = ExclusionValidator::default();
        
        // Test tool that should be excluded
        let should_exclude_metadata = ToolCliMetadata::included("issue_work");
        let analysis = DevUtilities::analyze_tool(&should_exclude_metadata, &validator);
        
        assert_eq!(analysis.name, "issue_work");
        assert!(!analysis.is_excluded);
        assert!(analysis.pattern_matches.len() > 0);
        assert!(matches!(analysis.suggestion, ToolSuggestion::ShouldExclude { .. }));
        
        if let ToolSuggestion::ShouldExclude { confidence, .. } = analysis.suggestion {
            assert_eq!(confidence, 1.0); // Exact match
        }
    }

    #[test]
    fn test_dev_utilities_correctly_configured() {
        let validator = ExclusionValidator::default();
        
        // Test properly excluded tool
        let excluded_metadata = ToolCliMetadata::excluded("issue_work", "MCP workflow management only");
        let analysis = DevUtilities::analyze_tool(&excluded_metadata, &validator);
        
        assert!(analysis.is_excluded);
        assert!(matches!(analysis.suggestion, ToolSuggestion::CorrectlyConfigured));
        
        // Test normal tool that should remain included
        let normal_metadata = ToolCliMetadata::included("normal_tool");
        let normal_analysis = DevUtilities::analyze_tool(&normal_metadata, &validator);
        
        assert!(!normal_analysis.is_excluded);
        assert!(normal_analysis.pattern_matches.is_empty());
        assert!(matches!(normal_analysis.suggestion, ToolSuggestion::CorrectlyConfigured));
    }

    #[test]
    fn test_dev_utilities_improve_reason() {
        let validator = ExclusionValidator::default();
        
        // Test excluded tool with poor reason but has pattern matches
        let poor_reason_metadata = ToolCliMetadata::excluded("tool_work", "Bad");
        let analysis = DevUtilities::analyze_tool(&poor_reason_metadata, &validator);
        
        assert!(matches!(analysis.suggestion, ToolSuggestion::ImproveReason { .. }));
        
        // Test excluded tool with no reason but has pattern matches
        let no_reason_metadata = ToolCliMetadata {
            name: "tool_work".to_string(),
            is_cli_excluded: true,
            exclusion_reason: None,
        };
        let no_reason_analysis = DevUtilities::analyze_tool(&no_reason_metadata, &validator);
        
        assert!(matches!(no_reason_analysis.suggestion, ToolSuggestion::ImproveReason { .. }));
    }

    #[test]
    fn test_dev_utilities_should_include() {
        let validator = ExclusionValidator::default();
        
        // Test tool that is excluded but has no pattern matches
        let excluded_no_pattern = ToolCliMetadata::excluded("normal_excluded_tool", "Some reason");
        let analysis = DevUtilities::analyze_tool(&excluded_no_pattern, &validator);
        
        assert!(analysis.pattern_matches.is_empty());
        assert!(matches!(analysis.suggestion, ToolSuggestion::ShouldInclude { .. }));
    }

    #[test]
    fn test_dev_utilities_statistics() {
        let mut metadata_map = HashMap::new();
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::included("issue_work"));
        metadata_map.insert("tool_work".to_string(), ToolCliMetadata::included("tool_work"));
        metadata_map.insert("properly_excluded".to_string(), ToolCliMetadata::excluded("properly_excluded", "Good reason for exclusion"));
        metadata_map.insert("poorly_excluded_work".to_string(), ToolCliMetadata::excluded("poorly_excluded_work", "Bad"));
        metadata_map.insert("normal_tool".to_string(), ToolCliMetadata::included("normal_tool"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let stats = DevUtilities::generate_statistics(&detector);
        
        assert_eq!(stats.get("total_tools"), Some(&5));
        assert_eq!(stats.get("excluded_tools"), Some(&2));
        assert_eq!(stats.get("should_exclude"), Some(&2)); // issue_work and tool_work
        assert_eq!(stats.get("should_include"), Some(&1)); // properly_excluded (no pattern matches)
        assert_eq!(stats.get("need_better_reasons"), Some(&1)); // poorly_excluded_work (has pattern matches but poor reason)
        assert_eq!(stats.get("correctly_configured"), Some(&1)); // normal_tool only
    }

    #[test]
    fn test_dev_utilities_find_potential_exclusions() {
        let mut metadata_map = HashMap::new();
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::included("issue_work"));
        metadata_map.insert("workflow_orchestrator".to_string(), ToolCliMetadata::included("workflow_orchestrator"));
        metadata_map.insert("normal_tool".to_string(), ToolCliMetadata::included("normal_tool"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let potential_exclusions = DevUtilities::find_potential_exclusions(&detector);
        
        assert_eq!(potential_exclusions.len(), 2);
        let names: Vec<&String> = potential_exclusions.iter().map(|a| &a.name).collect();
        assert!(names.contains(&&"issue_work".to_string()));
        assert!(names.contains(&&"workflow_orchestrator".to_string()));
    }

    #[test]
    fn test_dev_utilities_find_potential_inclusions() {
        let mut metadata_map = HashMap::new();
        metadata_map.insert("excluded_no_pattern".to_string(), ToolCliMetadata::excluded("excluded_no_pattern", "Some reason"));
        metadata_map.insert("properly_excluded".to_string(), ToolCliMetadata::excluded("issue_work", "Good reason"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let potential_inclusions = DevUtilities::find_potential_inclusions(&detector);
        
        assert_eq!(potential_inclusions.len(), 1);
        assert_eq!(potential_inclusions[0].name, "excluded_no_pattern");
    }

    #[test]
    fn test_documentation_generator_excluded_tools_doc() {
        let mut metadata_map = HashMap::new();
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::excluded("issue_work", "MCP workflow management only"));
        metadata_map.insert("workflow_orchestrator".to_string(), ToolCliMetadata::excluded("workflow_orchestrator", "Internal workflow coordination"));
        metadata_map.insert("normal_tool".to_string(), ToolCliMetadata::included("normal_tool"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let doc = DocumentationGenerator::generate_excluded_tools_doc(&detector);
        
        assert!(doc.contains("# CLI Excluded Tools"));
        assert!(doc.contains("**Total excluded tools:** 2"));
        assert!(doc.contains("`issue_work`"));
        assert!(doc.contains("`workflow_orchestrator`"));
        assert!(doc.contains("MCP workflow management only"));
        assert!(doc.contains("Internal workflow coordination"));
        assert!(doc.contains("## MCP Workflow Tools")); // Categorization
    }

    #[test]
    fn test_documentation_generator_empty_exclusions() {
        let metadata_map = HashMap::new();
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let doc = DocumentationGenerator::generate_excluded_tools_doc(&detector);
        
        assert!(doc.contains("# CLI Excluded Tools"));
        assert!(doc.contains("**Total excluded tools:** 0"));
        assert!(doc.contains("*No tools are currently excluded from CLI generation.*"));
    }

    #[test]
    fn test_documentation_generator_dev_report() {
        let mut metadata_map = HashMap::new();
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::included("issue_work")); // Should exclude
        metadata_map.insert("excluded_no_pattern".to_string(), ToolCliMetadata::excluded("excluded_no_pattern", "Some reason")); // Should include
        metadata_map.insert("tool_work".to_string(), ToolCliMetadata::excluded("tool_work", "Bad")); // Improve reason
        metadata_map.insert("normal_tool".to_string(), ToolCliMetadata::included("normal_tool")); // Correctly configured
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        let doc = DocumentationGenerator::generate_dev_report(&detector);
        
        assert!(doc.contains("# CLI Exclusion System Development Report"));
        assert!(doc.contains("## Summary Statistics"));
        assert!(doc.contains("**Total tools:** 4"));
        assert!(doc.contains("**CLI excluded:** 2"));
        assert!(doc.contains("**Should exclude:** 1"));
        assert!(doc.contains("**Should include:** 1"));
        assert!(doc.contains("**Need better reasons:** 1"));
        assert!(doc.contains("**Correctly configured:** 1"));
        
        // Check sections for suggestions
        assert!(doc.contains("## Tools That Should Be Excluded"));
        assert!(doc.contains("`issue_work`"));
        assert!(doc.contains("## Tools That Should Be Included"));
        assert!(doc.contains("`excluded_no_pattern`"));
        assert!(doc.contains("## Tools Needing Better Exclusion Reasons"));
        assert!(doc.contains("`tool_work`"));
    }

    #[test]
    fn test_documentation_generator_categorize_reason() {
        assert_eq!(DocumentationGenerator::categorize_reason("MCP workflow management"), "MCP Workflow Tools");
        assert_eq!(DocumentationGenerator::categorize_reason("workflow orchestration only"), "MCP Workflow Tools");
        assert_eq!(DocumentationGenerator::categorize_reason("internal use only"), "Internal Tools");
        assert_eq!(DocumentationGenerator::categorize_reason("experimental feature"), "Experimental/Testing Tools");
        assert_eq!(DocumentationGenerator::categorize_reason("deprecated API"), "Deprecated Tools");
        assert_eq!(DocumentationGenerator::categorize_reason("some other reason"), "Other Exclusions");
    }

    #[test]
    fn test_comprehensive_cli_exclusion_system_integration() {
        // Create a comprehensive test scenario with various tool types
        let mut metadata_map = HashMap::new();
        
        // Tools that should be excluded but aren't
        metadata_map.insert("issue_work".to_string(), ToolCliMetadata::included("issue_work"));
        metadata_map.insert("workflow_orchestrator".to_string(), ToolCliMetadata::included("workflow_orchestrator"));
        
        // Properly excluded tools
        metadata_map.insert("issue_merge".to_string(), ToolCliMetadata::excluded("issue_merge", "MCP workflow state management requiring coordinated transitions"));
        metadata_map.insert("abort_create".to_string(), ToolCliMetadata::excluded("abort_create", "MCP-specific error handling mechanism for workflow termination"));
        
        // Poorly documented exclusions
        metadata_map.insert("tool_work".to_string(), ToolCliMetadata::excluded("tool_work", "Bad"));
        metadata_map.insert("workflow_coord".to_string(), ToolCliMetadata {
            name: "workflow_coord".to_string(),
            is_cli_excluded: true,
            exclusion_reason: None,
        });
        
        // Tools that shouldn't be excluded but are
        metadata_map.insert("normal_excluded".to_string(), ToolCliMetadata::excluded("normal_excluded", "Some reason"));
        
        // Correctly configured tools
        metadata_map.insert("files_read".to_string(), ToolCliMetadata::included("files_read"));
        metadata_map.insert("web_search".to_string(), ToolCliMetadata::included("web_search"));
        
        let detector = RegistryCliExclusionDetector::new(metadata_map);
        
        // Test validation system
        let validator = ExclusionValidator::default();
        let report = validator.validate_all(&detector);
        
        assert_eq!(report.summary.total_tools, 9);
        assert_eq!(report.summary.excluded_tools, 5); // issue_merge, abort_create, tool_work, workflow_coord, normal_excluded
        assert_eq!(report.summary.eligible_tools, 4); // issue_work, workflow_orchestrator, files_read, web_search
        assert!(report.has_issues());
        assert!(report.has_warnings());
        
        // Test development utilities
        let analyses = DevUtilities::analyze_all_tools(&detector);
        let stats = DevUtilities::generate_statistics(&detector);
        
        assert_eq!(stats.get("total_tools"), Some(&9));
        assert_eq!(stats.get("should_exclude"), Some(&2)); // issue_work, workflow_orchestrator
        assert_eq!(stats.get("should_include"), Some(&1)); // normal_excluded should be included
        assert_eq!(stats.get("need_better_reasons"), Some(&2)); // tool_work, workflow_coord
        assert_eq!(stats.get("excluded_tools"), Some(&5)); // issue_merge, abort_create, tool_work, workflow_coord, normal_excluded
        assert_eq!(stats.get("correctly_configured"), Some(&4)); // issue_merge, abort_create are correctly excluded; files_read, web_search are correctly included
        
        let potential_exclusions = DevUtilities::find_potential_exclusions(&detector);
        assert_eq!(potential_exclusions.len(), 2);
        
        let potential_inclusions = DevUtilities::find_potential_inclusions(&detector);
        assert_eq!(potential_inclusions.len(), 1);
        
        // Test documentation generator
        let excluded_doc = DocumentationGenerator::generate_excluded_tools_doc(&detector);
        assert!(excluded_doc.contains("# CLI Excluded Tools"));
        assert!(excluded_doc.contains("**Total excluded tools:** 5"));
        
        let dev_report = DocumentationGenerator::generate_dev_report(&detector);
        assert!(dev_report.contains("# CLI Exclusion System Development Report"));
        assert!(dev_report.contains("**Total tools:** 9"));
        assert!(dev_report.contains("## Tools That Should Be Excluded"));
        assert!(dev_report.contains("## Tools That Should Be Included"));
        assert!(dev_report.contains("## Tools Needing Better Exclusion Reasons"));
        
        // Verify pattern matching system
        for analysis in &analyses {
            match analysis.name.as_str() {
                "issue_work" => {
                    assert!(matches!(analysis.suggestion, ToolSuggestion::ShouldExclude { confidence, .. } if confidence == 1.0));
                    assert!(analysis.pattern_matches.iter().any(|(pattern, _)| pattern == "issue_work"));
                }
                "workflow_orchestrator" => {
                    assert!(matches!(analysis.suggestion, ToolSuggestion::ShouldExclude { confidence, .. } if confidence >= 0.8));
                    assert!(analysis.pattern_matches.iter().any(|(pattern, _)| pattern == "workflow_"));
                }
                "normal_excluded" => {
                    assert!(matches!(analysis.suggestion, ToolSuggestion::ShouldInclude { .. }));
                    assert!(analysis.pattern_matches.is_empty());
                }
                "tool_work" => {
                    assert!(matches!(analysis.suggestion, ToolSuggestion::ImproveReason { .. }));
                }
                "files_read" | "web_search" => {
                    assert!(matches!(analysis.suggestion, ToolSuggestion::CorrectlyConfigured));
                }
                _ => {}
            }
        }
    }
}