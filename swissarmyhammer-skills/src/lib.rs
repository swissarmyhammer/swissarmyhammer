//! SwissArmyHammer Skills
//!
//! Core crate for parsing, validating, resolving, and storing Agent Skills.
//! Used by both the MCP server (llama-agent) and CLI (init/doctor).
//!
//! ## Overview
//!
//! Skills are defined as SKILL.md files following the [Agent Skills spec](https://agentskills.io/specification).
//! They provide specialized instructions that extend an agent's capabilities.
//!
//! ## Resolution Precedence
//!
//! Skills are resolved from multiple sources with later sources overriding earlier:
//! 1. **Builtin** — embedded in the binary from `builtin/skills/`
//! 2. **Local** — `.skills/` or `.swissarmyhammer/skills/` in the project
//! 3. **User** — `~/.skills/` or `~/.swissarmyhammer/skills/`

pub mod context;
pub mod error;
pub mod operations;
pub mod parse;
pub mod schema;
pub mod skill;
pub mod skill_library;
pub mod skill_loader;
pub mod skill_resolver;
pub mod validation;

// Re-export key types
pub use context::SkillContext;
pub use error::SkillError;
pub use operations::{ListSkills, SearchSkill, UseSkill};
pub use parse::{parse_input, SkillOperation};
pub use schema::generate_skill_mcp_schema;
pub use skill::{Skill, SkillName, SkillResources, SkillSource};
pub use skill_library::SkillLibrary;
pub use skill_resolver::SkillResolver;

// Re-export Execute trait from operations crate
pub use swissarmyhammer_operations::{async_trait, Execute, ExecutionResult, Operation};
