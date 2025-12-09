# Utilities

The mermaid-parser provides a comprehensive set of utility functions and types to support diagram parsing, analysis, and manipulation.

## Metrics and Analysis

### `DiagramMetrics`

Comprehensive metrics analysis for diagrams:

```rust
use mermaid_parser::{parse_diagram, DiagramMetrics};

let diagram = parse_diagram(input)?;
let metrics = DiagramMetrics::analyze(&diagram);

println!("Basic metrics: {:?}", metrics.basic);
println!("Complexity: {:?}", metrics.complexity);
println!("Quality score: {:.2}", metrics.quality.maintainability);
```

#### `MetricsReport`

```rust
pub struct MetricsReport {
    pub basic: BasicMetrics,
    pub complexity: ComplexityMetrics,
    pub quality: QualityMetrics,
    pub suggestions: Vec<Suggestion>,
}
```

#### `BasicMetrics`

```rust
pub struct BasicMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub depth: usize,
    pub breadth: usize,
}
```

#### `ComplexityMetrics`

```rust
pub struct ComplexityMetrics {
    pub cyclomatic: usize,      // Cyclomatic complexity
    pub cognitive: f64,         // Cognitive complexity
    pub nesting_depth: usize,   // Maximum nesting depth
    pub coupling: f64,          // Inter-node coupling
}
```

#### `QualityMetrics`

```rust
pub struct QualityMetrics {
    pub maintainability: f64,   // 0.0 - 1.0
    pub readability: f64,       // 0.0 - 1.0
    pub modularity: f64,        // 0.0 - 1.0
}
```

### `Suggestion`

Quality improvement suggestions:

```rust
pub struct Suggestion {
    pub category: SuggestionCategory,
    pub severity: SeverityLevel,
    pub message: String,
    pub location: Option<Location>,
}

pub enum SuggestionCategory {
    Performance,
    Readability,
    Maintainability,
    Structure,
    Style,
}

pub enum SeverityLevel {
    Info,
    Warning,
    Error,
}
```

**Example:**
```rust
let metrics = DiagramMetrics::analyze(&diagram);
for suggestion in &metrics.suggestions {
    match suggestion.severity {
        SeverityLevel::Error => println!("❌ {}", suggestion.message),
        SeverityLevel::Warning => println!("⚠️  {}", suggestion.message),
        SeverityLevel::Info => println!("💡 {}", suggestion.message),
    }
}
```

## Pretty Printing

### `MermaidPrinter`

Convert AST back to Mermaid syntax:

```rust
use mermaid_parser::{parse_diagram, MermaidPrinter, PrintOptions};

let diagram = parse_diagram(input)?;
let printer = MermaidPrinter::new();
let output = printer.print(&diagram, &PrintOptions::default())?;
println!("{}", output);
```

#### `PrintOptions`

```rust
pub struct PrintOptions {
    pub indent: String,
    pub line_width: usize,
    pub preserve_comments: bool,
    pub format_style: FormatStyle,
    pub include_metadata: bool,
}

pub enum FormatStyle {
    Compact,
    Readable,
    Verbose,
}
```

**Example:**
```rust
let options = PrintOptions {
    indent: "  ".to_string(),
    line_width: 80,
    preserve_comments: true,
    format_style: FormatStyle::Readable,
    include_metadata: true,
};

let formatted = printer.print(&diagram, &options)?;
```

## Validation Utilities

### `ReferenceValidator`

Validates diagram references and dependencies:

```rust
use mermaid_parser::{parse_diagram, ReferenceValidator};

let diagram = parse_diagram(input)?;
let mut validator = ReferenceValidator::new();
diagram.accept(&mut validator);

if !validator.is_valid() {
    for error in validator.errors() {
        println!("Validation error: {}", error);
    }
}
```

**Methods:**
- `new() -> Self` - Create a new validator
- `is_valid(&self) -> bool` - Check if diagram is valid
- `errors(&self) -> &[String]` - Get validation errors
- `warnings(&self) -> &[String]` - Get validation warnings

### `StructuralValidator`

Validates diagram structure and semantics:

```rust
use mermaid_parser::StructuralValidator;

let mut validator = StructuralValidator::new()
    .check_cycles(true)
    .check_unreachable_nodes(true)
    .check_naming_conventions(true);

diagram.accept(&mut validator);
let report = validator.report();
```

## Transformation Utilities

### `DiagramTransformer`

Transform diagrams with common operations:

```rust
use mermaid_parser::DiagramTransformer;

let transformer = DiagramTransformer::new()
    .normalize_ids()
    .add_missing_labels()
    .optimize_layout();

let transformed = transformer.transform(diagram)?;
```

### `NodeIdNormalizer`

Normalize node identifiers across diagrams:

```rust
use mermaid_parser::NodeIdNormalizer;

let mut normalizer = NodeIdNormalizer::new()
    .with_prefix("node_")
    .with_suffix_counter(true);

diagram.accept_mut(&mut normalizer);
```

## Parsing Utilities

### `KeyType` and `CardinalityValue`

Helper types for diagram parsing:

```rust
pub enum KeyType {
    Primary,
    Foreign,
    Unique,
}

pub enum CardinalityValue {
    ZeroOrOne,     // 0..1
    ExactlyOne,    // 1
    ZeroOrMany,    // 0..*
    OneOrMany,     // 1..*
    Custom(String), // Custom cardinality
}
```

### `TokenStream`

Low-level token stream utilities:

```rust
use mermaid_parser::TokenStream;

let tokens = TokenStream::tokenize(input)?;
for token in tokens {
    println!("{:?} at {}:{}", token.kind, token.line, token.column);
}
```

## String and Text Utilities

### `StringEscaping`

Handle Mermaid string escaping and unescaping:

```rust
use mermaid_parser::StringEscaping;

let escaped = StringEscaping::escape("Text with \"quotes\" and newlines\n");
let unescaped = StringEscaping::unescape(&escaped)?;
```

### `LabelFormatter`

Format labels and text content:

```rust
use mermaid_parser::LabelFormatter;

let formatter = LabelFormatter::new()
    .max_length(30)
    .word_wrap(true)
    .preserve_formatting(false);

let formatted = formatter.format("Very long label text that needs formatting");
```

## Layout and Positioning

### `LayoutEngine`

Automatic layout calculation for diagrams:

```rust
use mermaid_parser::LayoutEngine;

let engine = LayoutEngine::new()
    .algorithm(LayoutAlgorithm::Hierarchical)
    .node_spacing(50.0)
    .level_spacing(100.0);

let positioned_diagram = engine.layout(diagram)?;
```

### `BoundingBoxCalculator`

Calculate bounding boxes for diagram elements:

```rust
use mermaid_parser::BoundingBoxCalculator;

let calculator = BoundingBoxCalculator::new();
let bbox = calculator.calculate(&diagram);
println!("Diagram size: {}x{}", bbox.width, bbox.height);
```

## Export Utilities

### `DiagramExporter`

Export diagrams to various formats:

```rust
use mermaid_parser::{DiagramExporter, ExportFormat};

let exporter = DiagramExporter::new();

// Export to JSON
let json = exporter.export(&diagram, ExportFormat::Json)?;

// Export to DOT (Graphviz)
let dot = exporter.export(&diagram, ExportFormat::Dot)?;

// Export to PlantUML
let plantuml = exporter.export(&diagram, ExportFormat::PlantUml)?;
```

### `StatisticsCollector`

Collect comprehensive diagram statistics:

```rust
use mermaid_parser::StatisticsCollector;

let mut collector = StatisticsCollector::new();
diagram.accept(&mut collector);

let stats = collector.statistics();
println!("Diagram types: {:?}", stats.diagram_types);
println!("Total complexity: {:.2}", stats.total_complexity);
```

## Error Utilities

### `ErrorFormatter`

Format parse errors for display:

```rust
use mermaid_parser::{ErrorFormatter, ParseError};

let formatter = ErrorFormatter::new()
    .show_source_context(true)
    .highlight_error_location(true)
    .include_suggestions(true);

match parse_diagram(input) {
    Err(error) => {
        let formatted = formatter.format(&error, input);
        println!("{}", formatted);
    }
    Ok(diagram) => { /* ... */ }
}
```

### `ErrorRecovery`

Advanced error recovery strategies:

```rust
use mermaid_parser::ErrorRecovery;

let recovery = ErrorRecovery::new()
    .try_partial_parsing(true)
    .skip_invalid_lines(true)
    .provide_suggestions(true);

let result = recovery.parse_with_recovery(input);
match result {
    Ok(diagram) => println!("Parsed successfully"),
    PartialOk(diagram, errors) => {
        println!("Partial parse with {} errors", errors.len());
        process_diagram(diagram);
    }
    Err(errors) => println!("Parse failed completely"),
}
```

## Performance Utilities

### `ParsingProfiler`

Profile parsing performance:

```rust
use mermaid_parser::ParsingProfiler;

let profiler = ParsingProfiler::new();
let (diagram, profile) = profiler.parse_with_profiling(input)?;

println!("Parse time: {:?}", profile.total_time);
println!("Tokenization: {:?}", profile.tokenization_time);
println!("AST construction: {:?}", profile.ast_time);
println!("Memory usage: {} bytes", profile.peak_memory);
```

### `CacheManager`

Manage parsing caches for performance:

```rust
use mermaid_parser::CacheManager;

let cache = CacheManager::new()
    .max_entries(1000)
    .ttl(Duration::from_secs(300));

// Parse with caching
let diagram = cache.parse_cached(input)?;
```

## Utility Functions

### General-Purpose Helpers

```rust
use mermaid_parser::utils;

// Check if input appears to be a Mermaid diagram
if utils::is_mermaid_content(input) {
    let diagram = parse_diagram(input)?;
}

// Extract all text content from a diagram
let text_content = utils::extract_text(&diagram);

// Calculate diagram hash for caching
let hash = utils::diagram_hash(&diagram);

// Merge multiple diagrams (if compatible)
let merged = utils::merge_diagrams(&[diagram1, diagram2])?;
```

### Debugging Utilities

```rust
use mermaid_parser::debug;

// Pretty-print AST structure
debug::print_ast_structure(&diagram);

// Dump parsing tokens
debug::dump_tokens(input)?;

// Trace parsing steps
let diagram = debug::parse_with_trace(input)?;
```

## Configuration

### `ParserConfig`

Global parser configuration:

```rust
use mermaid_parser::ParserConfig;

let config = ParserConfig::new()
    .strict_mode(false)
    .max_recursion_depth(100)
    .enable_extensions(true)
    .timeout(Duration::from_secs(30));

// Apply configuration globally
config.apply_globally();

// Or use for specific parsing
let diagram = config.parse_diagram(input)?;
```

These utilities provide comprehensive support for working with Mermaid diagrams beyond basic parsing, enabling sophisticated analysis, transformation, and integration workflows.
