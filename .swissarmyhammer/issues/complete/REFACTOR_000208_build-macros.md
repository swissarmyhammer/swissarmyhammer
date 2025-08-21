# REFACTOR Step 6: Implement Build Macros for Tool Descriptions

## Overview
Implement a build-time macro system to compile tool description markdown files into the binary, similar to how builtin prompts are handled. This allows tool descriptions to be edited as standalone markdown files while ensuring they're available at runtime.

## Context
The specification requires that "each tool Description needs to be a markdown files brought in by a build macro much like built in prompts." Currently, builtin prompts are handled by a build script that processes markdown files and makes them available at runtime.

The goal is to:
1. Allow editing tool descriptions as standalone markdown files
2. Make the build dirty when description files change
3. Embed descriptions in the binary at compile time
4. Follow the same pattern as existing builtin prompts

## Current Builtin Prompt Pattern Analysis

### Existing Build Script
The current `build.rs` already processes builtin prompts:

```rust
// From existing build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Process builtin prompts
    collect_prompts()?;
    
    // Add tool descriptions processing
    collect_tool_descriptions()?;
    
    Ok(())
}
```

### Existing Prompt Processing
The builtin prompts are processed and made available via:
- Build script scans `builtin/prompts/` directory
- Markdown files are read and processed at build time
- Content is embedded in binary via `include_str!` or similar
- Files are tracked for rebuild when changed

## Target Architecture

### Tool Description File Structure
Each tool directory will contain a `description.md` file:

```
swissarmyhammer/src/mcp/tools/
├── issues/
│   ├── create/
│   │   ├── mod.rs
│   │   └── description.md      # Tool description
│   ├── mark_complete/
│   │   ├── mod.rs
│   │   └── description.md
│   └── ...
├── memoranda/
│   ├── create/
│   │   ├── mod.rs
│   │   └── description.md
│   └── ...
└── search/
    ├── index/
    │   ├── mod.rs
    │   └── description.md
    └── query/
        ├── mod.rs
        └── description.md
```

### Build-Time Processing
The build script will:
1. Discover all `description.md` files in tool directories
2. Process and validate markdown content
3. Generate code to embed descriptions
4. Track files for rebuild detection

## Tasks for This Step

### 1. Extend Build Script for Tool Descriptions

Add tool description processing to `build.rs`:

```rust
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

fn collect_tool_descriptions() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/mcp/tools");
    
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("tool_descriptions.rs");
    
    let mut descriptions = HashMap::new();
    let tools_dir = Path::new("src/mcp/tools");
    
    if tools_dir.exists() {
        collect_descriptions_recursive(tools_dir, "", &mut descriptions)?;
    }
    
    generate_tool_descriptions_code(&descriptions, &dest_path)?;
    
    Ok(())
}

fn collect_descriptions_recursive(
    dir: &Path,
    prefix: &str,
    descriptions: &mut HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_str().unwrap();
            let new_prefix = if prefix.is_empty() {
                dir_name.to_string()
            } else {
                format!("{}_{}", prefix, dir_name)
            };
            
            // Check for description.md in this directory
            let desc_file = path.join("description.md");
            if desc_file.exists() {
                let content = fs::read_to_string(&desc_file)?;
                descriptions.insert(new_prefix.clone(), content);
                println!("cargo:rerun-if-changed={}", desc_file.display());
            }
            
            // Recurse into subdirectories
            collect_descriptions_recursive(&path, &new_prefix, descriptions)?;
        }
    }
    
    Ok(())
}

fn generate_tool_descriptions_code(
    descriptions: &HashMap<String, String>,
    dest_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut code = String::new();
    code.push_str("// Generated tool descriptions - do not edit\n");
    code.push_str("use std::collections::HashMap;\n\n");
    code.push_str("pub fn get_tool_descriptions() -> HashMap<&'static str, &'static str> {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    for (tool_path, description) in descriptions {
        code.push_str(&format!(
            "    map.insert({:?}, {:?});\n",
            tool_path,
            description
        ));
    }
    
    code.push_str("    map\n");
    code.push_str("}\n");
    
    fs::write(dest_path, code)?;
    Ok(())
}
```

### 2. Create Tool Description Registry

Create a module to access tool descriptions at runtime:

```rust
// swissarmyhammer/src/mcp/tool_descriptions.rs
include!(concat!(env!("OUT_DIR"), "/tool_descriptions.rs"));

use std::collections::HashMap;

lazy_static::lazy_static! {
    static ref TOOL_DESCRIPTIONS: HashMap<&'static str, &'static str> = get_tool_descriptions();
}

pub fn get_description(tool_path: &str) -> Option<&'static str> {
    TOOL_DESCRIPTIONS.get(tool_path).copied()
}

pub fn list_all_descriptions() -> Vec<(&'static str, &'static str)> {
    TOOL_DESCRIPTIONS.iter().map(|(&k, &v)| (k, v)).collect()
}

/// Get description for a tool by noun and verb
/// e.g., get_tool_description("issues", "create") -> description for issues/create
pub fn get_tool_description(noun: &str, verb: &str) -> Option<&'static str> {
    let tool_path = format!("{}_{}", noun, verb);
    get_description(&tool_path)
}
```

### 3. Update Tool Implementations to Use Build-Time Descriptions

Modify the `McpTool` trait implementation pattern:

```rust
// Before (using include_str! directly)
impl McpTool for CreateIssueTool {
    fn description(&self) -> &'static str {
        include_str!("description.md")
    }
}

// After (using build-time registry)
impl McpTool for CreateIssueTool {
    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "create")
            .unwrap_or("No description available")
    }
}
```

### 4. Alternative Approach: Direct include_str! with Build Dependencies

If the registry approach is too complex, use direct `include_str!` with proper build dependencies:

```rust
// In build.rs - just ensure files are tracked
fn track_tool_descriptions() -> Result<(), Box<dyn std::error::Error>> {
    let tools_dir = Path::new("src/mcp/tools");
    
    if tools_dir.exists() {
        track_descriptions_recursive(tools_dir)?;
    }
    
    Ok(())
}

fn track_descriptions_recursive(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let desc_file = path.join("description.md");
            if desc_file.exists() {
                println!("cargo:rerun-if-changed={}", desc_file.display());
            }
            track_descriptions_recursive(&path)?;
        }
    }
    
    Ok(())
}

// Then tools can use include_str! directly
impl McpTool for CreateIssueTool {
    fn description(&self) -> &'static str {
        include_str!("description.md")
    }
}
```

### 5. Implement Description Validation

Add validation to ensure description files are well-formed:

```rust
fn validate_tool_description(content: &str, tool_path: &str) -> Result<(), String> {
    // Basic validation
    if content.trim().is_empty() {
        return Err(format!("Description for {} is empty", tool_path));
    }
    
    // Check for required sections
    if !content.contains("# ") {
        return Err(format!("Description for {} missing main heading", tool_path));
    }
    
    // Validate markdown syntax if needed
    // Could use a markdown parser here for more thorough validation
    
    Ok(())
}
```

### 6. Update Module Dependencies

Ensure the tool descriptions module is properly integrated:

```rust
// In src/mcp/mod.rs
pub mod tool_descriptions;

// Or if using include_str! approach, no additional module needed
```

### 7. Handle Missing Descriptions Gracefully

Ensure tools work even if descriptions are missing:

```rust
impl McpTool for SomeToolWithMissingDescription {
    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("some", "tool")
            .unwrap_or("Tool description not available")
    }
}
```

### 8. Add Build Script Documentation

Document the build process for tool descriptions:

```rust
// At the top of build.rs
//! Build script for swissarmyhammer
//! 
//! This script processes builtin prompts and tool descriptions at build time:
//! 
//! 1. Scans builtin/prompts/ for prompt templates
//! 2. Scans src/mcp/tools/ for tool description.md files
//! 3. Embeds content in binary for runtime access
//! 4. Sets up rebuild triggers when files change
//! 
//! Tool descriptions follow the pattern:
//! src/mcp/tools/{noun}/{verb}/description.md -> tool_descriptions::{noun}_{verb}
```

### 9. Testing the Build System

Create tests to verify the build system works:

```rust
#[cfg(test)]
mod build_tests {
    use super::*;
    
    #[test]
    fn test_tool_descriptions_available() {
        // Test that descriptions can be loaded
        let descriptions = crate::mcp::tool_descriptions::list_all_descriptions();
        assert!(!descriptions.is_empty(), "No tool descriptions found");
        
        // Test specific tools
        assert!(crate::mcp::tool_descriptions::get_tool_description("issues", "create").is_some());
        assert!(crate::mcp::tool_descriptions::get_tool_description("memoranda", "create").is_some());
    }
    
    #[test]
    fn test_description_content_quality() {
        let create_issue_desc = crate::mcp::tool_descriptions::get_tool_description("issues", "create")
            .expect("Create issue description should exist");
            
        assert!(create_issue_desc.contains("# "), "Description should have a title");
        assert!(create_issue_desc.contains("Parameters"), "Description should document parameters");
        assert!(create_issue_desc.len() > 100, "Description should be substantial");
    }
}
```

### 10. Documentation and Examples

Create documentation for maintaining tool descriptions:

```markdown
<!-- In CONTRIBUTING.md or similar -->
## Tool Descriptions

Each MCP tool must have a `description.md` file in its directory. This file:

1. Documents the tool's purpose and parameters
2. Provides usage examples
3. Explains return values and error conditions
4. Is embedded in the binary at build time

### Format Requirements

- Start with a main heading (`# Tool Name`)
- Include a `## Parameters` section
- Include an `## Examples` section
- Include a `## Returns` section if applicable

### Build Process

Tool descriptions are processed by the build script:
- Changes to any `description.md` file trigger a rebuild
- Content is validated for basic structure
- Descriptions are embedded as static strings

### Testing

Run `cargo test build_tests` to verify descriptions are properly embedded.
```

## Success Criteria
- [ ] Build script processes tool description files
- [ ] Tool descriptions are embedded in binary at compile time
- [ ] Build triggers when description files change
- [ ] All existing tool descriptions are accessible at runtime
- [ ] Tools use build-time descriptions instead of `include_str!`
- [ ] Build validation ensures description quality
- [ ] Tests verify description availability
- [ ] Documentation explains the system

## Implementation Decision

Choose between two approaches:
1. **Registry Approach**: Build script generates code with HashMap of descriptions
2. **Direct Include Approach**: Use `include_str!` with build dependency tracking

The registry approach provides more flexibility but is more complex. The direct include approach is simpler and follows existing patterns.

## Integration Points
- Follows same pattern as existing builtin prompt processing
- Uses existing `build.rs` infrastructure
- Integrates with `McpTool` trait implementations
- Works with existing tool directory structure

## Next Steps
After implementing build macros:
1. Update CLI to use same tool implementations
2. Remove old implementation from main match statement
3. Clean up duplicate code across codebase
4. Finalize tool organization

## Risk Mitigation
- Start with simple direct include approach
- Ensure build fails fast if descriptions are invalid
- Provide fallback descriptions for missing files
- Test build process in CI/CD pipeline
- Document build requirements clearly


## Proposed Solution

After analyzing the current build.rs and tool structure, I propose implementing the build-time macro system as follows:

### Analysis of Current State

1. **Existing Build System**: The `build.rs` already processes builtin prompts and workflows from `../builtin/` directories using a recursive collection approach
2. **Tool Structure**: Tools are organized in `src/mcp/tools/{noun}/{verb}/` directories, each containing:
   - `mod.rs` - tool implementation 
   - `description.md` - tool description (already exists for all tools)
3. **Current Implementation**: Tools use `include_str!("description.md")` which works but doesn't follow the build macro pattern

### Implementation Plan

1. **Extend build.rs**: Add `collect_tool_descriptions()` function similar to existing `collect_prompts()` and `collect_workflows()`
2. **Generate tool_descriptions.rs**: Create a registry that maps tool paths to embedded descriptions
3. **Create access module**: Add `src/mcp/tool_descriptions.rs` to provide runtime access to descriptions
4. **Update tool implementations**: Replace `include_str!()` calls with build-time registry lookups
5. **Add validation**: Ensure description files are well-formed markdown

### Advantages of this approach

- Follows existing patterns in the codebase
- Centralizes tool description access
- Provides build-time validation
- Makes descriptions searchable and listable at runtime  
- Maintains compatibility with existing tool structure

### Implementation Steps

1. Extend build.rs with tool description processing
2. Create tool_descriptions.rs module for runtime access
3. Update all tool implementations to use the registry
4. Add comprehensive tests
5. Validate build process works correctly

This approach maintains the same directory structure but provides centralized, validated access to tool descriptions at runtime while embedding them at build-time.