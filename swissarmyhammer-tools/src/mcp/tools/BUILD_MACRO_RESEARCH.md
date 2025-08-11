# Build Macro Research for Tool Descriptions

This document outlines the research findings for implementing build macros to embed markdown descriptions into MCP tools, following the established pattern used for builtin prompts and workflows.

## Current Build System Analysis

### Existing Implementation (`build.rs`)
The project already uses a build script to embed markdown files at compile time:

1. **Build Script Pattern**: `swissarmyhammer/build.rs` (149 lines)
   - Uses `env::var("OUT_DIR")` to generate Rust code at build time
   - Recursively scans directories for `.md` and `.liquid` files
   - Embeds content as string literals using `r#"..."#` raw strings
   - Generates functions that return `Vec<(&'static str, &'static str)>`

2. **Generated Code Pattern**:
   ```rust
   // Generated in OUT_DIR/builtin_prompts.rs
   pub fn get_builtin_prompts() -> Vec<(&'static str, &'static str)> {
       vec![
           ("prompt_name", r#"markdown content here"#),
           // ...
       ]
   }
   ```

3. **Usage Pattern**:
   ```rust
   // In prompt_resolver.rs
   include!(concat!(env!("OUT_DIR"), "/builtin_prompts.rs"));
   
   fn load_builtin_prompts(&mut self) -> Result<()> {
       let builtin_prompts = get_builtin_prompts();
       for (name, content) in builtin_prompts {
           self.vfs.add_builtin(name, content);
       }
       Ok(())
   }
   ```

## Proposed Tool Description System

### Build Script Extensions

Add tool description generation to `build.rs`:

```rust
fn generate_tool_descriptions(out_dir: &str) {
    let dest_path = Path::new(&out_dir).join("tool_descriptions.rs");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let tools_dir = Path::new(&manifest_dir).join("src/mcp/tools");
    
    let mut code = String::new();
    code.push_str("// Auto-generated tool descriptions - do not edit manually\n");
    code.push_str("use std::collections::HashMap;\n\n");
    code.push_str("/// Get all tool descriptions as a HashMap\n");
    code.push_str("pub fn get_tool_descriptions() -> HashMap<&'static str, &'static str> {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    collect_tool_descriptions(&tools_dir, &mut code);
    
    code.push_str("    map\n");
    code.push_str("}\n");
    
    fs::write(&dest_path, code).unwrap();
}

fn collect_tool_descriptions(dir: &Path, code: &mut String) {
    // Recursively find description.md files
    // Extract tool name from directory structure
    // Embed content with tool name as key
}
```

### Directory Structure Integration

The new tool directory structure supports this pattern:
```
tools/
├── memoranda/
│   ├── create/
│   │   ├── mod.rs
│   │   └── description.md      # ← Embedded by build script
│   └── get/
│       ├── mod.rs
│       └── description.md      # ← Embedded by build script
├── issues/
│   └── create/
│       ├── mod.rs
│       └── description.md      # ← Embedded by build script
```

### Tool Registry Integration

The tool registry can use build-time descriptions:

```rust
// In tools/mod.rs
include!(concat!(env!("OUT_DIR"), "/tool_descriptions.rs"));

pub struct ToolRegistry {
    descriptions: std::collections::HashMap<&'static str, &'static str>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            descriptions: get_tool_descriptions(),
        }
    }
    
    pub fn get_description(&self, tool_name: &str) -> Option<&'static str> {
        self.descriptions.get(tool_name).copied()
    }
}
```

## Alternative Approaches Considered

### 1. Proc Macros
**Pros**: More flexible, can generate complex code structures
**Cons**: Complex to implement, compile time overhead, requires proc-macro crate

### 2. `include_str!` Macro
**Pros**: Simple, direct embedding
**Cons**: Requires hardcoded paths, no dynamic discovery

```rust
// Example usage (not chosen)
const MEMO_CREATE_DESCRIPTION: &str = include_str!("memoranda/create/description.md");
```

### 3. Runtime Loading
**Pros**: Dynamic, can be updated without recompilation
**Cons**: File I/O overhead, deployment complexity

## Recommended Implementation

### Phase 1: Extend Build Script
1. Add `generate_tool_descriptions()` to existing `build.rs`
2. Add `collect_tool_descriptions()` function to scan `src/mcp/tools/`
3. Generate `tool_descriptions.rs` in `OUT_DIR`

### Phase 2: Tool Registry Integration
1. Include generated descriptions in tool registry
2. Provide lookup API for MCP tool descriptions
3. Integrate with existing MCP server tool listing

### Phase 3: Validation and Testing
1. Add build-time validation for description.md files
2. Ensure descriptions match MCP tool schema requirements
3. Add tests for description loading and lookup

## Implementation Benefits

1. **Zero Runtime Cost**: Descriptions embedded at compile time
2. **Consistency**: Same pattern as existing builtin prompts/workflows
3. **Maintainability**: Descriptions co-located with tool implementation
4. **Type Safety**: Compile-time validation of description files
5. **Performance**: No file I/O during tool registration

## Next Steps

1. **Extend build.rs**: Add tool description generation function
2. **Create Tool Registry**: Implement registry pattern with description lookup
3. **Integration**: Connect tool registry to MCP server tool listing
4. **Testing**: Validate build-time description embedding

## Technical Notes

- **File Watching**: Add `cargo:rerun-if-changed=src/mcp/tools` to build script
- **Error Handling**: Handle missing description.md files gracefully
- **Naming Convention**: Extract tool names from directory structure (`memoranda/create` → `memo_create`)
- **Content Validation**: Validate markdown syntax and MCP schema compliance at build time

This approach follows established patterns in the codebase while providing the flexibility needed for the new tool organization structure.