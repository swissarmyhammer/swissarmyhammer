//! The Claude Code settings-file schema, in one place.
//!
//! Claude Code reads its denied tools from a `permissions.deny` array in a
//! settings file. That path is an external contract: it belongs to Claude
//! Code, and only Claude Code decides what it is called. Two parts of this
//! crate address it — [`crate::strategy::ClaudeCodeStrategy`] denies and
//! allows one tool at a time, and [`crate::install::profile`] installs and
//! strips the edit-redirect fragment — so the keys and the JSON pointer built
//! from them live here, where each part reads the same declaration.
//!
//! The module sits in the strategy layer because that layer already owns every
//! agent-specific fact (see the [`crate::strategy`] module docs). The generic
//! primitives in [`crate::settings`] stay agent-agnostic: a caller hands them
//! a pointer, and this module is where a Claude Code caller gets one.

/// The `permissions` object key of the Claude Code settings shape.
pub(crate) const POINTER_KEY_PERMISSIONS: &str = "permissions";

/// The `deny` array key inside [`POINTER_KEY_PERMISSIONS`].
pub(crate) const POINTER_KEY_DENY: &str = "deny";

/// RFC 6901 JSON pointer for the `permissions.deny` array.
///
/// Built from [`POINTER_KEY_PERMISSIONS`] and [`POINTER_KEY_DENY`] instead of
/// spelled out, because a pointer that restates the keys is a second source of
/// them: a change to either key would leave the pointer addressing the old
/// path while the documents built from the keys carry the new one. A `const`
/// cannot join other `const` strings, so this is a function.
pub(crate) fn permissions_deny_pointer() -> String {
    format!("/{POINTER_KEY_PERMISSIONS}/{POINTER_KEY_DENY}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The pointer must address the array the two keys name.
    ///
    /// This is the guard that keeps the pointer from becoming a second source
    /// of the keys: build a settings document out of `POINTER_KEY_PERMISSIONS`
    /// and `POINTER_KEY_DENY`, then resolve the pointer against it. A pointer
    /// that spells the old path out cannot find the array once either key
    /// changes, and this test goes red.
    #[test]
    fn pointer_resolves_the_array_the_keys_name() {
        let denied = json!(["Bash"]);
        let settings = json!({
            POINTER_KEY_PERMISSIONS: {
                POINTER_KEY_DENY: denied.clone(),
            }
        });

        assert_eq!(
            settings.pointer(&permissions_deny_pointer()),
            Some(&denied),
            "permissions_deny_pointer() must address the array named by \
             POINTER_KEY_PERMISSIONS and POINTER_KEY_DENY"
        );
    }
}
