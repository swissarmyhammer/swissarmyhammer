//! Todo management tools for ephemeral task tracking
//!
//! This module provides MCP tools for managing todo lists during development sessions.
//! Todo lists are temporary, stored locally as YAML files, and never committed to version control.
//!
//! ## Available Tools
//!
//! - **todo_create**: Add a new item to a todo list
//! - **todo_list**: List all todo items with optional filtering by completion status
//! - **todo_show**: Retrieve a specific todo item or the next incomplete item
//! - **todo_mark_complete**: Mark a todo item as completed
//!
//! ## Architecture
//!
//! Follows the established MCP tool pattern:
//! ```text
//! tools/todo/
//! |-- mod.rs              # This file - module exports and registration
//! |-- create/
//! |   |-- mod.rs         # CreateTodoTool implementation
//! |   +-- description.md # Tool description
//! |-- show/
//! |   |-- mod.rs         # ShowTodoTool implementation  
//! |   +-- description.md # Tool description
//! +-- mark_complete/
//!     |-- mod.rs         # MarkCompleteTodoTool implementation
//!     +-- description.md # Tool description
//! ```

use crate::mcp::tool_registry::ToolRegistry;

/// Todo item creation functionality
pub mod create;
/// Todo list display functionality
pub mod list;
/// Todo item completion functionality
pub mod mark_complete;
/// Todo item display functionality
pub mod show;

pub use create::CreateTodoTool;
pub use list::ListTodoTool;
pub use mark_complete::MarkCompleteTodoTool;
pub use show::ShowTodoTool;

/// Register all todo tools with the tool registry
///
/// This function registers all todo-related MCP tools, following the same pattern
/// used by issues, memoranda, and other tool modules.
pub fn register_todo_tools(registry: &mut ToolRegistry) {
    registry.register(CreateTodoTool);
    registry.register(ListTodoTool);
    registry.register(ShowTodoTool);
    registry.register(MarkCompleteTodoTool);
}
