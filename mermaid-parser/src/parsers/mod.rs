//! Individual parsers for each Mermaid diagram type
//!
//! This module contains the specialized parser implementations for mermaid-parser.
//! Each parser is designed to understand and process a specific type of diagram efficiently.
//!
//! ## Parser Architecture
//!
//! The parser system follows a modular design where each diagram type has its own dedicated
//! parser implementation. This allows for:
//!
//! - **Specialized optimization** - Each parser is tuned for its specific diagram syntax
//! - **Independent development** - Parsers can be enhanced without affecting others
//! - **Error isolation** - Parsing failures are contained to specific diagram types
//! - **Performance optimization** - Each parser can be optimized independently
//!
//! ## Supported Parsers
//!
//! ### Core Diagram Types
//! - [`flowchart`] - Flow diagrams with nodes and edges
//! - [`sequence`] - Message flows between participants  
//! - [`class`] - Object-oriented class relationships
//! - [`state`] - State machines and transitions
//! - [`gantt`] - Project timeline visualization
//! - [`pie`] - Data distribution visualization
//! - [`er`] - Entity-relationship diagrams
//!
//! ### Specialized Visualizations
//! - [`sankey`] - Flow quantity visualization
//! - [`timeline`] - Chronological event representation
//! - [`journey`] - User experience mapping
//! - [`c4`] - Software architecture visualization
//! - [`mindmap`] - Hierarchical information structure
//! - [`quadrant`] - Four-quadrant analysis
//! - [`xy`] - Data plotting with multiple series
//!
//! ### Advanced Diagrams
//! - [`architecture`] - System architecture layouts
//! - [`block`] - Block-based representations  
//! - [`git`] - Version control branching visualization
//! - [`kanban`] - Task management workflows
//! - [`packet`] - Network packet visualization
//! - [`radar`] - Multi-dimensional data comparison
//! - [`requirement`] - Requirements and relationships
//! - [`treemap`] - Hierarchical data visualization
//!
//! ### Experimental
//! - [`misc`] - Miscellaneous and experimental diagram types
//!
//! ## Usage Example
//!
//! ```rust
//! use mermaid_parser::parsers::flowchart;
//!
//! let input = r#"
//! flowchart TD
//!     A[Start] --> B{Decision}
//!     B -->|Yes| C[Process]
//!     B -->|No| D[Skip]
//! "#;
//!
//! let diagram = flowchart::parse(input)?;
//! println!("Parsed flowchart with {} nodes", diagram.nodes.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// Core diagram parsers
pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
pub mod er;
pub mod flowchart;
pub mod gantt;
pub mod git;
pub mod journey;
pub mod kanban;
pub mod mindmap;
pub mod misc;
pub mod packet;
pub mod pie;
pub mod quadrant;
pub mod radar;
pub mod requirement;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod timeline;
pub mod treemap;
pub mod xy;
