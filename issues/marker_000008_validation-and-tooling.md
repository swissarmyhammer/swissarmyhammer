# Create Validation and Development Tooling

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Build validation tools and development utilities to help developers correctly use the CLI exclusion system, detect inconsistencies, and maintain the system over time.

## Implementation Tasks

### 1. Validation Tool

#### CLI Exclusion Validator
```rust
// swissarmyhammer-tools/src/cli/validator.rs

/// Validates CLI exclusion system consistency and correctness
pub struct ExclusionValidator {
    registry: Arc<ToolRegistry>,
    config: ValidationConfig,
}

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

impl ExclusionValidator {
    /// Validate the entire exclusion system
    pub fn validate_all(&self) -> ValidationReport {
        let mut report = ValidationReport::new();
        
        report.extend(self.validate_exclusion_consistency());
        report.extend(self.validate_exclusion_reasoning());
        report.extend(self.validate_cli_alternatives());
        report.extend(self.validate_naming_conventions());
        
        report
    }
    
    /// Check for tools that should probably be excluded but aren't
    fn validate_exclusion_consistency(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let eligible_tools = self.registry.get_cli_eligible_tools();
        
        for tool_meta in eligible_tools {
            if self.should_probably_be_excluded(&tool_meta.name) {
                issues.push(ValidationIssue::SuggestExclusion {
                    tool_name: tool_meta.name.clone(),
                    reason: self.explain_exclusion_suggestion(&tool_meta.name),
                });
            }
        }
        
        issues
    }
    
    /// Check for proper exclusion reasoning documentation
    fn validate_exclusion_reasoning(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let excluded_tools = self.registry.get_excluded_tools();
        
        for tool_meta in excluded_tools {
            if tool_meta.exclusion_reason.is_none() {
                issues.push(ValidationIssue::MissingExclusionReason {
                    tool_name: tool_meta.name.clone(),
                });
            }
        }
        
        issues
    }
    
    /// Check that excluded tools have CLI alternatives documented
    fn validate_cli_alternatives(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let excluded_tools = self.registry.get_excluded_tools();
        
        for tool_meta in excluded_tools {
            if tool_meta.cli_alternatives.is_empty() {
                issues.push(ValidationIssue::MissingCliAlternatives {
                    tool_name: tool_meta.name.clone(),
                });
            }
        }
        
        issues
    }
}
```

#### Validation Report Structure
```rust
/// Report of validation findings
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationWarning>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone)]
pub enum ValidationIssue {
    SuggestExclusion {
        tool_name: String,
        reason: String,
    },
    MissingExclusionReason {
        tool_name: String,
    },
    MissingCliAlternatives {
        tool_name: String,
    },
    InconsistentNaming {
        tool_name: String,
        suggested_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ValidationSummary {
    pub total_tools: usize,
    pub excluded_tools: usize,
    pub eligible_tools: usize,
    pub issues_found: usize,
    pub warnings_found: usize,
}
```

### 2. CLI Command for Validation

#### Add to CLI Structure
```rust
// swissarmyhammer-cli/src/commands/validate.rs

/// Validate CLI exclusion system
#[derive(Parser, Debug)]
pub struct ValidateCommand {
    /// Check for missing exclusions
    #[arg(long, default_value = "true")]
    pub check_missing: bool,
    
    /// Check exclusion reasoning
    #[arg(long, default_value = "true")]
    pub check_reasoning: bool,
    
    /// Check CLI alternatives
    #[arg(long, default_value = "true")]
    pub check_alternatives: bool,
    
    /// Output format
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
    
    /// Only show issues (no warnings)
    #[arg(long)]
    pub issues_only: bool,
}

pub async fn handle_validate_command(cmd: ValidateCommand) -> Result<(), CliError> {
    let registry = create_tool_registry().await?;
    
    let config = ValidationConfig {
        check_missing_exclusions: cmd.check_missing,
        check_exclusion_reasoning: cmd.check_reasoning,
        check_cli_alternatives: cmd.check_alternatives,
        exclusion_patterns: default_exclusion_patterns(),
    };
    
    let validator = ExclusionValidator::new(Arc::new(registry), config);
    let report = validator.validate_all();
    
    match cmd.format {
        OutputFormat::Table => print_validation_table(&report, cmd.issues_only),
        OutputFormat::Json => print_validation_json(&report),
        OutputFormat::Yaml => print_validation_yaml(&report),
    }
    
    // Exit with error code if issues found
    if !report.issues.is_empty() {
        std::process::exit(2);
    }
    
    Ok(())
}
```

### 3. Development Utilities

#### Tool Analysis Utility
```rust
/// Analyze tool patterns to suggest exclusions
pub struct ToolAnalyzer {
    registry: Arc<ToolRegistry>,
}

impl ToolAnalyzer {
    /// Analyze tool patterns and generate recommendations
    pub fn analyze_tools(&self) -> AnalysisReport {
        let mut report = AnalysisReport::new();
        
        // Analyze naming patterns
        report.naming_analysis = self.analyze_naming_patterns();
        
        // Analyze schema complexity
        report.complexity_analysis = self.analyze_schema_complexity();
        
        // Analyze workflow dependencies
        report.dependency_analysis = self.analyze_workflow_dependencies();
        
        report
    }
    
    /// Identify naming patterns that suggest workflow tools
    fn analyze_naming_patterns(&self) -> NamingAnalysis {
        let tool_names = self.registry.list_tool_names();
        let workflow_indicators = [
            "_work", "_merge", "_abort", "_transition", 
            "_orchestrate", "_coordinate", "_manage"
        ];
        
        let mut suggested_exclusions = Vec::new();
        
        for name in &tool_names {
            for indicator in &workflow_indicators {
                if name.contains(indicator) {
                    suggested_exclusions.push(SuggestedExclusion {
                        tool_name: name.clone(),
                        reason: format!("Contains workflow indicator: {}", indicator),
                        confidence: self.calculate_confidence(name, indicator),
                    });
                }
            }
        }
        
        NamingAnalysis { suggested_exclusions }
    }
}
```

### 4. Documentation Generator

#### Generate Exclusion Documentation
```rust
/// Generates documentation about CLI exclusion system
pub struct DocumentationGenerator {
    registry: Arc<ToolRegistry>,
}

impl DocumentationGenerator {
    /// Generate markdown documentation for excluded tools
    pub fn generate_exclusion_docs(&self) -> String {
        let mut docs = String::new();
        
        docs.push_str("# CLI Excluded Tools\n\n");
        docs.push_str("The following tools are excluded from CLI generation:\n\n");
        
        let excluded_tools = self.registry.get_excluded_tools();
        
        for tool_meta in excluded_tools {
            docs.push_str(&format!("## {}\n\n", tool_meta.name));
            
            if let Some(reason) = &tool_meta.exclusion_reason {
                docs.push_str(&format!("**Exclusion Reason**: {}\n\n", reason));
            }
            
            if !tool_meta.cli_alternatives.is_empty() {
                docs.push_str("**CLI Alternatives**:\n");
                for alt in &tool_meta.cli_alternatives {
                    docs.push_str(&format!("- `{}`\n", alt));
                }
                docs.push_str("\n");
            }
            
            if let Some(tool) = self.registry.get_tool(&tool_meta.name) {
                docs.push_str(&format!("**Description**: {}\n\n", tool.description()));
            }
        }
        
        docs
    }
    
    /// Generate CLI generation report
    pub fn generate_cli_report(&self) -> String {
        let eligible_tools = self.registry.get_cli_eligible_tools();
        let excluded_tools = self.registry.get_excluded_tools();
        
        format!(
            "# CLI Generation Report\n\n\
            - **Total Tools**: {}\n\
            - **CLI Eligible**: {}\n\
            - **CLI Excluded**: {}\n\
            - **Exclusion Rate**: {:.1}%\n\n",
            self.registry.len(),
            eligible_tools.len(),
            excluded_tools.len(),
            (excluded_tools.len() as f64 / self.registry.len() as f64) * 100.0
        )
    }
}
```

### 5. Integration with Build System

#### Build-Time Validation
```rust
// build.rs additions for validation

fn main() {
    // Existing build logic...
    
    // Validate CLI exclusion system during build
    if std::env::var("VALIDATE_CLI_EXCLUSIONS").is_ok() {
        validate_exclusion_system();
    }
}

fn validate_exclusion_system() {
    println!("cargo:warning=Validating CLI exclusion system...");
    
    // Create registry and validate
    let registry = create_build_time_registry();
    let validator = ExclusionValidator::new(registry, ValidationConfig::strict());
    let report = validator.validate_all();
    
    if !report.issues.is_empty() {
        for issue in &report.issues {
            println!("cargo:warning=CLI Exclusion Issue: {:?}", issue);
        }
        panic!("CLI exclusion validation failed");
    }
    
    println!("cargo:warning=CLI exclusion validation passed");
}
```

### 6. Developer Tools

#### cargo-sah Extension
```rust
// src/bin/cargo-sah-validate.rs

/// Cargo extension for validating CLI exclusions
#[derive(Parser)]
struct CargoSahValidate {
    #[command(subcommand)]
    command: ValidateCommands,
}

#[derive(Subcommand)]
enum ValidateCommands {
    /// Validate CLI exclusion system
    Exclusions(ExclusionValidateArgs),
    
    /// Generate exclusion documentation
    Docs(DocsGenerateArgs),
    
    /// Analyze tools for exclusion suggestions
    Analyze(AnalyzeArgs),
}

fn main() {
    let args = CargoSahValidate::parse();
    
    match args.command {
        ValidateCommands::Exclusions(args) => validate_exclusions(args),
        ValidateCommands::Docs(args) => generate_docs(args),
        ValidateCommands::Analyze(args) => analyze_tools(args),
    }
}
```

## Testing Requirements

### 1. Validator Tests
```rust
#[cfg(test)]
mod validator_tests {
    use super::*;

    #[test]
    fn test_exclusion_validation() {
        let registry = create_test_registry_with_issues();
        let validator = ExclusionValidator::new(
            Arc::new(registry),
            ValidationConfig::default()
        );
        
        let report = validator.validate_all();
        
        // Should detect missing exclusions
        assert!(report.issues.iter().any(|issue| matches!(
            issue, ValidationIssue::SuggestExclusion { .. }
        )));
        
        // Should detect missing reasoning
        assert!(report.issues.iter().any(|issue| matches!(
            issue, ValidationIssue::MissingExclusionReason { .. }
        )));
    }

    #[test]
    fn test_naming_pattern_detection() {
        let mut registry = ToolRegistry::new();
        registry.register(create_mock_tool("workflow_orchestrator")); // Should be excluded
        registry.register(create_mock_tool("simple_getter")); // Should not be excluded
        
        let validator = ExclusionValidator::new(
            Arc::new(registry),
            ValidationConfig::default()
        );
        
        let issues = validator.validate_exclusion_consistency();
        assert!(issues.iter().any(|issue| 
            matches!(issue, ValidationIssue::SuggestExclusion { tool_name, .. } 
                if tool_name == "workflow_orchestrator")
        ));
    }
}
```

### 2. CLI Integration Tests
```rust
#[tokio::test]
async fn test_validate_cli_command() {
    let cmd = ValidateCommand {
        check_missing: true,
        check_reasoning: true,
        check_alternatives: true,
        format: OutputFormat::Json,
        issues_only: false,
    };
    
    // Should not panic and should produce output
    let result = handle_validate_command(cmd).await;
    assert!(result.is_ok());
}
```

### 3. Documentation Generation Tests
```rust
#[test]
fn test_documentation_generation() {
    let registry = create_test_registry();
    let doc_gen = DocumentationGenerator::new(Arc::new(registry));
    
    let docs = doc_gen.generate_exclusion_docs();
    assert!(docs.contains("# CLI Excluded Tools"));
    assert!(docs.contains("issue_work"));
    assert!(docs.contains("issue_merge"));
    
    let report = doc_gen.generate_cli_report();
    assert!(report.contains("CLI Generation Report"));
    assert!(report.contains("Total Tools"));
}
```

## Documentation

### 1. Validation Guide
- Document all validation checks and their purpose
- Provide examples of common validation issues
- Explain how to fix validation failures

### 2. Tool Development Guide
- Guidelines for developers on exclusion decisions
- Examples of proper exclusion usage
- Integration with development workflow

### 3. CLI Usage Documentation
```bash
# Validate exclusion system
sah validate exclusions

# Generate exclusion documentation
sah validate docs --output exclusions.md

# Analyze tools for suggestions
sah validate analyze --suggest-exclusions
```

## Acceptance Criteria

- [ ] Comprehensive validation tool detects inconsistencies
- [ ] CLI command provides easy access to validation
- [ ] Development utilities help maintain system quality
- [ ] Documentation generation automates maintenance tasks
- [ ] Build-time validation prevents issues
- [ ] Comprehensive tests validate all tooling
- [ ] Documentation explains validation workflow

## Notes

This tooling ensures the CLI exclusion system remains consistent and well-maintained as the codebase evolves, providing developers with the tools they need to make correct exclusion decisions.