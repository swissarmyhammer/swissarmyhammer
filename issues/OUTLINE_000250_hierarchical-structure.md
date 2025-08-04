# OUTLINE_000250: Hierarchical Structure Builder

Refer to ./specification/outline_tool.md

## Summary

Implement the hierarchical structure builder that takes parsed symbols from individual files and organizes them into a nested tree structure that mirrors the file system hierarchy, creating the foundation for YAML output generation.

## Context

The outline tool needs to present code structure in a way that reflects both the file system organization and the nested relationships within each file. This step creates the bridge between individual file parsing results and the final structured output.

## Requirements

### 1. File System Hierarchy Mirroring
- Organize files according to their directory structure
- Preserve relative paths from the working directory
- Handle nested directories of arbitrary depth
- Group files by their containing directories
- Support cross-platform path handling

### 2. Symbol Hierarchy Building
- Maintain parent-child relationships (classes contain methods, modules contain classes)
- Handle nested scopes (nested functions, inner classes)
- Preserve source order of symbols within each scope
- Support forward references and circular dependencies
- Merge symbols from multiple files when appropriate

### 3. Tree Structure Management
- Efficient tree building for large codebases
- Memory-efficient representation of hierarchical data
- Fast lookup and traversal operations
- Support for tree modification and filtering
- Consistent ordering and sorting

## Technical Details

### Hierarchy Builder Implementation
```rust
pub struct HierarchyBuilder {
    root: OutlineDirectory,
    file_patterns: Vec<String>,
    discovered_files: Vec<DiscoveredFile>,
}

impl HierarchyBuilder {
    pub fn new(patterns: Vec<String>) -> Self;
    pub fn add_file_outline(&mut self, file_path: &Path, outline: FileOutline) -> Result<()>;
    pub fn build_hierarchy(self) -> Result<OutlineHierarchy>;
    pub fn with_sorting(mut self, sort: SortOrder) -> Self;
}

#[derive(Debug, Clone)]
pub struct OutlineHierarchy {
    pub root: OutlineDirectory,
    pub total_files: usize,
    pub total_symbols: usize,
    pub languages: HashSet<Language>,
}

#[derive(Debug, Clone)]
pub struct OutlineDirectory {
    pub name: String,
    pub path: PathBuf,
    pub files: Vec<OutlineFile>,
    pub subdirectories: Vec<OutlineDirectory>,
}

#[derive(Debug, Clone)]
pub struct OutlineFile {
    pub name: String,
    pub path: PathBuf,
    pub language: Language,
    pub symbols: Vec<OutlineNode>,
    pub parse_errors: Vec<ParseError>,
}
```

### File System Organization
```rust
impl HierarchyBuilder {
    fn organize_by_directory(&self, files: Vec<(PathBuf, FileOutline)>) -> OutlineDirectory {
        let mut directories: HashMap<PathBuf, Vec<(PathBuf, FileOutline)>> = HashMap::new();
        
        // Group files by their parent directory
        for (path, outline) in files {
            let parent = path.parent().unwrap_or(Path::new("."));
            directories.entry(parent.to_path_buf()).or_default().push((path, outline));
        }
        
        // Recursively build directory tree
        self.build_directory_tree(Path::new("."), directories)
    }
    
    fn build_directory_tree(
        &self, 
        current_path: &Path, 
        directories: HashMap<PathBuf, Vec<(PathBuf, FileOutline)>>
    ) -> OutlineDirectory {
        // Implementation for recursive directory tree building
    }
}
```

### Symbol Hierarchy Organization
```rust
impl OutlineFile {
    fn organize_symbols(mut symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // Sort symbols by source location
        symbols.sort_by_key(|node| node.line);
        
        // Build parent-child relationships
        let mut organized = Vec::new();
        let mut stack: Vec<(usize, OutlineNode)> = Vec::new();
        
        for symbol in symbols {
            // Determine nesting level based on indentation or scope
            let level = Self::calculate_nesting_level(&symbol);
            
            // Pop symbols that are not parents of current symbol
            while let Some((parent_level, _)) = stack.last() {
                if *parent_level >= level {
                    let (_, completed_symbol) = stack.pop().unwrap();
                    Self::place_symbol(&mut organized, &mut stack, completed_symbol);
                } else {
                    break;
                }
            }
            
            stack.push((level, symbol));
        }
        
        // Process remaining symbols in stack
        while let Some((_, symbol)) = stack.pop() {
            Self::place_symbol(&mut organized, &mut stack, symbol);
        }
        
        organized
    }
    
    fn calculate_nesting_level(symbol: &OutlineNode) -> usize {
        // Calculate nesting level based on symbol type and context
    }
    
    fn place_symbol(
        organized: &mut Vec<OutlineNode>,
        stack: &mut Vec<(usize, OutlineNode)>,
        mut symbol: OutlineNode
    ) {
        if let Some((_, parent)) = stack.last_mut() {
            // Add as child to parent
            parent.children.get_or_insert_with(Vec::new).push(symbol);
        } else {
            // Add as top-level symbol
            organized.push(symbol);
        }
    }
}
```

### Sorting and Ordering
```rust
#[derive(Debug, Clone, Copy)]
pub enum SortOrder {
    SourceOrder,    // Maintain original source order
    Alphabetical,   // Sort by name alphabetically
    ByKind,        // Group by symbol kind, then alphabetical
    ByVisibility,  // Public symbols first, then private
}

impl OutlineDirectory {
    fn sort_contents(&mut self, sort_order: SortOrder) {
        match sort_order {
            SortOrder::SourceOrder => {
                // Keep original order
            }
            SortOrder::Alphabetical => {
                self.files.sort_by(|a, b| a.name.cmp(&b.name));
                self.subdirectories.sort_by(|a, b| a.name.cmp(&b.name));
                for file in &mut self.files {
                    file.sort_symbols_alphabetically();
                }
            }
            SortOrder::ByKind => {
                for file in &mut self.files {
                    file.sort_symbols_by_kind();
                }
            }
            SortOrder::ByVisibility => {
                for file in &mut self.files {
                    file.sort_symbols_by_visibility();
                }
            }
        }
        
        // Recursively sort subdirectories
        for subdir in &mut self.subdirectories {
            subdir.sort_contents(sort_order);
        }
    }
}
```

## Implementation Steps

1. Create `src/outline/hierarchy.rs` module
2. Implement basic directory tree organization
3. Add file system path handling and normalization
4. Implement symbol hierarchy organization within files
5. Add support for different sorting strategies  
6. Implement tree traversal and modification operations
7. Add validation and error handling for malformed hierarchies
8. Create comprehensive unit tests with various directory structures
9. Add integration tests with real project structures

## Testing Requirements

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_directory_organization() {
        // Test organizing files into directory structure
    }
    
    #[test]
    fn test_nested_symbol_hierarchy() {
        // Test organizing symbols with parent-child relationships
    }
    
    #[test]
    fn test_different_sort_orders() {
        // Test all sorting strategies
    }
    
    #[test]
    fn test_cross_platform_paths() {
        // Test Windows, Unix, and other path formats
    }
}
```

### Integration Tests
- Test with real project directory structures
- Verify handling of deeply nested hierarchies
- Test performance with large numbers of files
- Validate cross-platform compatibility

### Sample Test Structure
```
test_project/
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── helpers.rs
│   │   └── validators.rs
│   └── services/
│       ├── user_service.rs
│       └── data_service.rs
└── tests/
    ├── integration_tests.rs
    └── unit_tests.rs
```

Expected hierarchy:
```yaml
src:
  main.rs:
    children:
      - name: main
        kind: function
  lib.rs:
    children:
      - name: Config
        kind: struct
  utils:
    mod.rs:
      children:
        - name: utils
          kind: module
    helpers.rs:
      children:
        - name: Helper
          kind: struct
          children:
            - name: new
              kind: method
```

## Integration Points

### With File Discovery
- Receive list of discovered files with their paths
- Maintain file system hierarchy relationships
- Handle symbolic links and aliases appropriately

### With Symbol Extraction
- Receive parsed symbols from each file
- Organize symbols into hierarchical relationships
- Maintain source location information

### With YAML Output
- Provide structured hierarchy for YAML generation
- Support different output formats and filtering
- Enable efficient traversal for output generation

## Performance Considerations

- Efficient tree building algorithms for large codebases
- Memory-efficient storage of hierarchical data
- Fast lookups for symbol relationships
- Lazy evaluation where possible
- Optimize for common traversal patterns

## Error Handling

- Handle malformed or incomplete symbol hierarchies
- Graceful degradation when file system structure is unclear
- Clear error messages for hierarchy building failures
- Recovery from partial parsing results

## Success Criteria

- Correctly organizes files into directory hierarchy
- Maintains proper parent-child symbol relationships
- Supports all required sorting strategies
- Handles large codebases efficiently
- Provides clean API for hierarchy manipulation
- Comprehensive test coverage
- Cross-platform compatibility
- Clean integration with parsing and output modules

## Dependencies

- Core outline types and structures
- File discovery module
- Symbol extraction results
- Standard library path handling
- Platform-specific path utilities

## Notes

The hierarchy builder is a critical component that affects both performance and output quality. Consider memory usage carefully as large codebases may have thousands of files and symbols. The implementation should be extensible to support additional organizational strategies in the future.