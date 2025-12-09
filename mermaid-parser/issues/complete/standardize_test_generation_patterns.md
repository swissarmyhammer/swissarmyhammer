# Standardize Test Generation Patterns

**Priority**: Medium  
**Impact**: 20+ test files affected  
**Effort**: High  

## Problem

Test files for parser coverage use repetitive patterns with extensive boilerplate code. Each parser has nearly identical test structure for validating against sample files, leading to maintenance overhead and inconsistent test patterns.

### Current Duplicated Pattern

Found in all `*_coverage_test.rs` and `*_test.rs` files:

```rust
// Example from architecture_coverage_test.rs
#[rstest]
#[case::path_01_test_architecture_architectureDetector_ts_000_mermaid(
    include_str!("../mermaid-samples/architecture/architectureDetector_ts_000.mermaid")
)]
#[case::path_02_test_architecture_architectureParser_ts_000_mermaid(
    include_str!("../mermaid-samples/architecture/architectureParser_ts_000.mermaid")
)]
// ... 38 more identical case declarations
#[case::path_38_test_architecture_tokenBuilder_ts_000_mermaid(
    include_str!("../mermaid-samples/architecture/tokenBuilder_ts_000.mermaid")
)]
fn test_architecture_files(#[case] input: &str) {
    let result = architecture::parse(input);
    match result {
        Ok(_) => {
            // Test passed - parser successfully handled the input
        }
        Err(e) => {
            eprintln!("Failed to parse architecture file: {}", e);
            panic!("Architecture parse failed");
        }
    }
}
```

### Issues with Current Approach

1. **Manual maintenance**: Adding new sample files requires manual test case additions
2. **Boilerplate repetition**: Same test structure repeated across 20+ files
3. **Inconsistent error handling**: Different parsers have slightly different test patterns
4. **Difficult refactoring**: Changes to test patterns require updating many files

## Solution

Create a comprehensive test generation system using procedural macros and build scripts.

### Approach 1: Procedural Macro System

Create a procedural macro that automatically generates test cases from directory structure:

```rust
// tests/test_macros/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};
use std::fs;
use std::path::Path;

#[proc_macro]
pub fn generate_sample_tests(input: TokenStream) -> TokenStream {
    let parser_name = parse_macro_input!(input as LitStr).value();
    
    // Read sample files from directory
    let sample_dir = format!("mermaid-samples/{}", parser_name);
    let sample_files = collect_sample_files(&sample_dir);
    
    // Generate test cases
    let test_cases = sample_files.into_iter().enumerate().map(|(i, file_path)| {
        let case_name = format!("sample_{:03}", i + 1);
        let case_ident = syn::Ident::new(&case_name, proc_macro2::Span::call_site());
        let file_content = format!("include_str!(\"../{}\")", file_path);
        let file_content_tokens: proc_macro2::TokenStream = file_content.parse().unwrap();
        
        quote! {
            #[case::#case_ident(#file_content_tokens)]
        }
    });
    
    let parser_ident = syn::Ident::new(&parser_name, proc_macro2::Span::call_site());
    let test_fn_name = syn::Ident::new(&format!("test_{}_samples", parser_name), proc_macro2::Span::call_site());
    
    let expanded = quote! {
        #[rstest]
        #(#test_cases)*
        fn #test_fn_name(#[case] input: &str) {
            let result = crate::parsers::#parser_ident::parse(input);
            assert!(result.is_ok(), "Failed to parse {} sample: {:?}", #parser_name, result.err());
        }
    };
    
    TokenStream::from(expanded)
}

fn collect_sample_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(path_str) = entry.path().to_str() {
                if path_str.ends_with(".mermaid") {
                    files.push(path_str.to_string());
                }
            }
        }
    }
    files.sort();
    files
}
```

### Approach 2: Build Script with Code Generation

Create a build script that generates test files at compile time:

```rust
// build.rs
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    
    // Generate test files for each parser
    let parsers = [
        "architecture", "block", "c4", "class", "er", "flowchart",
        "gantt", "git", "journey", "kanban", "mindmap", "misc",
        "packet", "pie", "quadrant", "radar", "requirement", 
        "sankey", "sequence", "state", "timeline", "treemap", "xy"
    ];
    
    for parser in &parsers {
        generate_parser_tests(parser, out_path);
    }
    
    println!("cargo:rerun-if-changed=mermaid-samples/");
}

fn generate_parser_tests(parser_name: &str, out_path: &Path) {
    let sample_dir = format!("mermaid-samples/{}", parser_name);
    let test_file_path = out_path.join(format!("{}_generated_tests.rs", parser_name));
    
    let mut test_file = fs::File::create(&test_file_path).unwrap();
    
    // Generate test file header
    writeln!(test_file, "// Auto-generated test file for {} parser", parser_name).unwrap();
    writeln!(test_file, "use rstest::rstest;").unwrap();
    writeln!(test_file, "use crate::parsers::{};", parser_name).unwrap();
    writeln!(test_file).unwrap();
    
    // Collect sample files
    let sample_files = collect_sample_files(&sample_dir);
    
    // Generate rstest cases
    writeln!(test_file, "#[rstest]").unwrap();
    for (i, file_path) in sample_files.iter().enumerate() {
        let case_name = format!("sample_{:03}", i + 1);
        writeln!(
            test_file, 
            "#[case::{}(include_str!(\"{}\"))]", 
            case_name, 
            file_path
        ).unwrap();
    }
    
    // Generate test function
    writeln!(test_file, "fn test_{}_samples(#[case] input: &str) {{", parser_name).unwrap();
    writeln!(test_file, "    let result = {}::parse(input);", parser_name).unwrap();
    writeln!(test_file, "    assert!(result.is_ok(), \"Failed to parse {} sample: {{:?}}\", result.err());", parser_name).unwrap();
    writeln!(test_file, "}").unwrap();
}

fn collect_sample_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(path_str) = entry.path().to_str() {
                if path_str.ends_with(".mermaid") {
                    files.push(format!("../{}", path_str));
                }
            }
        }
    }
    files.sort();
    files
}
```

### Approach 3: Macro-Based Solution (Recommended)

Create a declarative macro system for immediate use:

```rust
// tests/test_utils.rs
#[macro_export]
macro_rules! generate_parser_sample_tests {
    ($parser_name:ident, $sample_dir:literal) => {
        paste::paste! {
            mod [<$parser_name _sample_tests>] {
                use super::*;
                use rstest::rstest;
                
                // Include all sample files from directory
                $crate::include_sample_tests!($parser_name, $sample_dir);
            }
        }
    };
}

#[macro_export]  
macro_rules! include_sample_tests {
    ($parser_name:ident, $sample_dir:literal) => {
        // This would be populated by build script or manual enumeration
        // For now, we can start with manual inclusion and later automate
        
        #[rstest]
        #[case::sample_001(include_str!(concat!("../mermaid-samples/", $sample_dir, "/sample_001.mermaid")))]
        #[case::sample_002(include_str!(concat!("../mermaid-samples/", $sample_dir, "/sample_002.mermaid")))]
        // ... more cases would be auto-generated
        fn test_samples(#[case] input: &str) {
            let result = crate::parsers::$parser_name::parse(input);
            assert!(
                result.is_ok(), 
                "Failed to parse {} sample: {:?}", 
                stringify!($parser_name), 
                result.err()
            );
        }
        
        #[rstest]
        #[case::sample_001(include_str!(concat!("../mermaid-samples/", $sample_dir, "/sample_001.mermaid")))]
        fn test_samples_detailed(#[case] input: &str) {
            let result = crate::parsers::$parser_name::parse(input);
            match result {
                Ok(diagram) => {
                    // Additional validation can be added here
                    println!("Successfully parsed {} sample", stringify!($parser_name));
                }
                Err(e) => {
                    eprintln!("Parse error for {}: {}", stringify!($parser_name), e);
                    eprintln!("Input was: {}", input);
                    panic!("Parse failed");
                }
            }
        }
    };
}

// Enhanced version with custom validation
#[macro_export]
macro_rules! generate_validated_parser_tests {
    ($parser_name:ident, $sample_dir:literal, $validator:expr) => {
        paste::paste! {
            mod [<$parser_name _validated_tests>] {
                use super::*;
                use rstest::rstest;
                
                #[rstest]
                // Cases would be auto-generated here
                fn test_samples_with_validation(#[case] input: &str) {
                    let result = crate::parsers::$parser_name::parse(input);
                    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
                    
                    let diagram = result.unwrap();
                    $validator(&diagram);
                }
            }
        }
    };
}
```

## Implementation Steps

### Phase 1: Create Test Infrastructure

1. **Add dependencies to `Cargo.toml`**:
   ```toml
   [dev-dependencies]
   paste = "1.0"
   # Add proc-macro dependencies if using Approach 1
   ```

2. **Create `tests/test_utils.rs`** with macro definitions

3. **Create build script** (`build.rs`) for auto-generation

### Phase 2: Create Template System

```rust
// tests/parser_test_template.rs
use crate::test_utils::*;

// Template that can be copied and customized for each parser
generate_parser_sample_tests!(architecture, "architecture");

// For parsers that need custom validation
generate_validated_parser_tests!(
    architecture, 
    "architecture",
    |diagram: &ArchitectureDiagram| {
        assert!(!diagram.services.is_empty(), "Architecture should have services");
    }
);
```

### Phase 3: Migrate Existing Tests

1. **Convert existing coverage tests** to use new macros
2. **Remove boilerplate code** from test files  
3. **Standardize error messages** and test patterns
4. **Add enhanced validation** where appropriate

### Phase 4: Auto-Discovery System

```rust
// build.rs enhancement for auto-discovery
fn discover_and_generate_tests() {
    let sample_dirs = fs::read_dir("mermaid-samples").unwrap();
    
    for dir_entry in sample_dirs {
        if let Ok(entry) = dir_entry {
            if entry.file_type().unwrap().is_dir() {
                let parser_name = entry.file_name().to_str().unwrap();
                
                // Check if parser exists
                let parser_file = format!("src/parsers/{}.rs", parser_name);
                if Path::new(&parser_file).exists() {
                    generate_parser_tests(parser_name, &out_path);
                }
            }
        }
    }
}
```

## Benefits

- **Automatic test generation**: New sample files automatically become test cases
- **Consistent test patterns**: All parsers use identical test structure
- **Reduced maintenance**: Test updates happen in one place
- **Better error reporting**: Standardized error messages and debugging info
- **Extensible validation**: Easy to add custom validation per parser type
- **Build-time optimization**: Tests generated at compile time

## Enhanced Features

### Custom Validation Support

```rust
// Custom validators for different diagram types
fn validate_architecture_diagram(diagram: &ArchitectureDiagram) {
    assert!(!diagram.services.is_empty(), "Should have services");
    assert!(diagram.title.is_some() || !diagram.services.is_empty(), "Should have title or services");
}

fn validate_flowchart_diagram(diagram: &FlowchartDiagram) {
    assert!(!diagram.nodes.is_empty(), "Should have nodes");
    // Could validate that edges reference valid nodes
}

// Usage
generate_validated_parser_tests!(architecture, "architecture", validate_architecture_diagram);
generate_validated_parser_tests!(flowchart, "flowchart", validate_flowchart_diagram);
```

### Performance Testing Integration

```rust
#[macro_export]
macro_rules! generate_performance_tests {
    ($parser_name:ident, $sample_dir:literal) => {
        #[cfg(test)]
        mod performance_tests {
            use super::*;
            use std::time::Instant;
            
            #[test]
            fn test_parse_performance() {
                let samples = include_sample_files!($sample_dir);
                
                for sample in samples {
                    let start = Instant::now();
                    let result = crate::parsers::$parser_name::parse(sample);
                    let duration = start.elapsed();
                    
                    assert!(result.is_ok(), "Parse failed");
                    assert!(duration.as_millis() < 100, "Parse too slow: {:?}", duration);
                }
            }
        }
    };
}
```

## Files to Create/Modify

### New Files
- `tests/test_utils.rs` (macro definitions)
- `build.rs` (test generation)
- `tests/parser_test_template.rs` (template example)

### Modified Files
- `Cargo.toml` (add dependencies)
- All `tests/*_coverage_test.rs` files (convert to use macros)
- All `tests/*_test.rs` files (convert to use macros)

### Removed Files (Eventually)
- Boilerplate code from existing test files

## Migration Strategy

1. **Phase 1**: Implement macro system alongside existing tests
2. **Phase 2**: Convert one parser at a time (start with architecture)
3. **Phase 3**: Add enhanced validation and performance tests
4. **Phase 4**: Remove old boilerplate code
5. **Phase 5**: Enable auto-discovery for new parsers

## Testing the Test System

Create meta-tests to ensure the test generation works correctly:

```rust
#[cfg(test)]
mod test_generation_tests {
    use super::*;
    
    #[test]
    fn test_macro_expansion() {
        // Verify that macros expand correctly
        // Could use trybuild or similar for compile-time testing
    }
    
    #[test]
    fn test_sample_file_discovery() {
        // Test that sample files are discovered correctly
        let files = collect_sample_files("mermaid-samples/architecture");
        assert!(!files.is_empty(), "Should find sample files");
    }
}
```

## Risk Assessment

**Medium Risk**: 
- Complex macro system could be difficult to debug
- Build script changes affect compile process
- Need to ensure all existing tests continue to work

**Mitigation**:
- Implement incrementally alongside existing tests
- Extensive testing of macro system
- Clear documentation and examples
- Fallback to manual test definition if needed