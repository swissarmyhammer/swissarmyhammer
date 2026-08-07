//! The passing fixture of the `missing-docs-rust` tool rule.
//!
//! Every public item here has documentation. The tool must report nothing. A
//! tool upgrade that reports one anyway makes the doctor mark the rule
//! unusable.

/// A documented public struct.
pub struct DocumentedItem;

/// A documented public function.
pub fn documented_function() {}
