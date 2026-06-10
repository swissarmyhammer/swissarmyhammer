//! Display-time rendering of command caption templates.
//!
//! Plugins declare caption templates in their registrations (e.g.
//! `"Inspect {{entity.type}}"`); the template is the declaration, rendering
//! is the service's job. [`render_caption`] resolves `{{...}}` placeholders
//! against the [`CommandContext`] the listing surface supplies (the focused
//! scope chain, or the explicit context-menu target) so display surfaces —
//! palette, native menus, context menus — only ever see display-ready
//! strings.
//!
//! ## Engine choice
//!
//! This is a deliberately minimal resolver rather than a full template
//! engine: the workspace's Liquid engine lives in the prompts/templating
//! stack and pulling it into this crate for `{{a.b}}` substitution would be
//! a heavyweight new dependency edge. The same judgment call was made by
//! `swissarmyhammer-kanban`'s `resolve_name_template` (the legacy
//! scope-command path); this resolver generalizes it with token scanning so
//! unknown or whitespace-padded placeholders degrade cleanly instead of
//! leaking raw `{{...}}` into the UI.
//!
//! ## Fallback semantics
//!
//! A placeholder that cannot be resolved — unknown key, no entity context —
//! renders as the empty string, and the result's whitespace is tidied
//! (collapsed runs, trimmed ends), so `"Inspect {{entity.type}}"` with no
//! context becomes `"Inspect"`. A malformed token (unclosed `{{`) drops the
//! remainder of the template rather than leaking braces. A rendered caption
//! therefore NEVER contains `{{`.

use crate::types::CommandContext;

/// Render a caption template against the focused object.
///
/// Resolves each `{{ key }}` token (inner whitespace tolerated) via
/// [`resolve_placeholder`]; unresolved tokens render as empty and the
/// result is whitespace-tidied. Templates without `{{` pass through
/// untouched.
///
/// # Parameters
///
/// - `template`: the registered caption (plugin-declared `name` /
///   `menu_name`).
/// - `ctx`: the listing surface's context — `target` (context-menu entity)
///   and `scope_chain` (focused entities, innermost first).
///
/// # Returns
///
/// A display-ready string guaranteed not to contain `{{`.
pub fn render_caption(template: &str, ctx: &CommandContext) -> String {
    if !template.contains("{{") {
        return template.to_string();
    }

    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let key = after[..end].trim();
                if let Some(value) = resolve_placeholder(key, ctx) {
                    out.push_str(&value);
                }
                rest = &after[end + 2..];
            }
            None => {
                // Malformed (unclosed) token — drop the remainder rather
                // than leaking raw braces into a display surface.
                rest = "";
            }
        }
    }
    out.push_str(rest);
    tidy_whitespace(&out)
}

/// Resolve one placeholder key against the context.
///
/// Supported keys (the full inventory across builtin plugins today):
///
/// - `entity.type` — the focused entity's type, display-cased (e.g.
///   `task:01ABC` → "Task").
///
/// Unknown keys return `None` (the caller drops the token). The context
/// shape lets future keys (e.g. `entity.title`) slot in as new match arms
/// without reworking the scanner.
fn resolve_placeholder(key: &str, ctx: &CommandContext) -> Option<String> {
    match key {
        "entity.type" => focused_entity_type(ctx).map(display_case),
        _ => None,
    }
}

/// The focused entity's type token, from the explicit target moniker when
/// present (context-menu semantics: the entity the menu fired over wins),
/// otherwise from the innermost scope-chain moniker (palette semantics:
/// the focused object).
///
/// Monikers are `type:id` (the id may itself contain colons, e.g.
/// `attachment:/p.png`), so the type is the token before the FIRST colon.
/// Returns `None` when neither source yields a non-empty type.
fn focused_entity_type(ctx: &CommandContext) -> Option<&str> {
    ctx.target
        .as_deref()
        .into_iter()
        .chain(ctx.scope_chain.first().map(String::as_str))
        .find_map(moniker_type)
}

/// The `type` token of a `type:id` moniker, or `None` when the moniker has
/// no colon or an empty type.
fn moniker_type(moniker: &str) -> Option<&str> {
    match moniker.split_once(':') {
        Some((entity_type, _)) if !entity_type.is_empty() => Some(entity_type),
        _ => None,
    }
}

/// Display-case an entity type token: `_`/`-` separators become spaces and
/// each word's first character is uppercased (`task` → "Task",
/// `saved_search` → "Saved Search"). An empty token renders as the empty
/// string.
///
/// Public because it is the ONE canonical casing rule for `{{entity.type}}`
/// across every display surface: [`render_caption`] uses it for the palette
/// and menu listing paths, and `swissarmyhammer-kanban`'s
/// `resolve_name_template` (the OS menu's focus-driven refresh) reuses it so
/// the two resolvers can never drift apart on casing.
pub fn display_case(entity_type: &str) -> String {
    entity_type
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collapse whitespace runs to single spaces and trim the ends, so a
/// dropped placeholder leaves no dangling gaps ("Inspect " → "Inspect").
fn tidy_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a context with the given scope chain and no target.
    fn ctx_with_chain(chain: &[&str]) -> CommandContext {
        CommandContext {
            scope_chain: chain.iter().map(|s| s.to_string()).collect(),
            ..CommandContext::default()
        }
    }

    #[test]
    fn known_placeholder_resolves_from_innermost_scope_chain_moniker() {
        let ctx = ctx_with_chain(&["task:01ABC", "board:01X"]);
        assert_eq!(
            render_caption("Inspect {{entity.type}}", &ctx),
            "Inspect Task"
        );
    }

    #[test]
    fn target_takes_precedence_over_scope_chain() {
        let mut ctx = ctx_with_chain(&["task:01ABC"]);
        ctx.target = Some("tag:01T".to_string());
        assert_eq!(render_caption("Delete {{entity.type}}", &ctx), "Delete Tag");
    }

    #[test]
    fn empty_context_falls_back_to_clean_generic_caption() {
        let ctx = CommandContext::default();
        assert_eq!(render_caption("Inspect {{entity.type}}", &ctx), "Inspect");
    }

    #[test]
    fn unknown_placeholder_is_dropped_never_raw() {
        let ctx = ctx_with_chain(&["task:01ABC"]);
        let rendered = render_caption("Reticulate {{entity.frobnicate}} now", &ctx);
        assert_eq!(rendered, "Reticulate now");
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn inner_whitespace_in_braces_is_tolerated() {
        let ctx = ctx_with_chain(&["task:01ABC"]);
        assert_eq!(render_caption("Cut {{ entity.type }}", &ctx), "Cut Task");
    }

    #[test]
    fn template_free_captions_pass_through_untouched() {
        let ctx = ctx_with_chain(&["task:01ABC"]);
        assert_eq!(render_caption("Close Inspector", &ctx), "Close Inspector");
    }

    #[test]
    fn malformed_unclosed_token_never_leaks_braces() {
        let ctx = ctx_with_chain(&["task:01ABC"]);
        let rendered = render_caption("Inspect {{entity.type", &ctx);
        assert_eq!(rendered, "Inspect");
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn multi_word_entity_types_display_case_each_word() {
        let ctx = ctx_with_chain(&["saved_search:01S"]);
        assert_eq!(
            render_caption("Open {{entity.type}}", &ctx),
            "Open Saved Search"
        );
    }

    #[test]
    fn moniker_without_colon_yields_no_entity_context() {
        let ctx = ctx_with_chain(&["weird-moniker"]);
        assert_eq!(render_caption("Inspect {{entity.type}}", &ctx), "Inspect");
    }

    #[test]
    fn path_shaped_moniker_ids_keep_the_type_before_the_first_colon() {
        let ctx = ctx_with_chain(&["attachment:/some/path:with:colons.png"]);
        assert_eq!(
            render_caption("Open {{entity.type}}", &ctx),
            "Open Attachment"
        );
    }
}
