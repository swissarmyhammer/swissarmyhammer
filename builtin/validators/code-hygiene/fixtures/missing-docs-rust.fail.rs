//! The failing fixture of the `missing-docs-rust` tool rule.
//!
//! It holds one undocumented public item. The tool must report it. A tool
//! upgrade that stops reporting it makes the doctor mark the rule unusable.

/// A documented public struct, so the fixture fails only on the item below.
pub struct DocumentedNeighbor;

pub struct UndocumentedItem;
