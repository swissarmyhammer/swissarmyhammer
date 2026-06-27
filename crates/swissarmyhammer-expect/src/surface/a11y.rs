//! The shared accessibility (a11y) drive dialect, used by every surface that
//! drives and observes through an **accessibility tree** — `browser` (the web
//! a11y tree over CDP) and `gui` (the native a11y tree over the OS AX/UIA/AT-SPI
//! APIs).
//!
//! Both surfaces speak the *same* pixel-free dialect from `ideas/expect.md`
//! §"Locators are a per-surface dialect": a control is addressed by its
//! accessible `role[name=…]` (an [`A11ySelector`]), never by coordinates, so a
//! genuine control rename surfaces as honest structural drift rather than the
//! everything-screams-on-a-cosmetic-change noise of a screenshot diff. The
//! *observe* side of that dialect (the `within` / `ancestor` tree relationships)
//! lives in the [assertion compiler](crate::assertion); this module owns the
//! *drive* side — turning a `When` step into a concrete [`A11yAction`] — so the
//! grammar has a single parser shared by both surfaces. A control rename ⇒
//! structural drift is therefore the same behavior across browser and gui by
//! construction, not by two parsers kept in lockstep by hand.

use std::time::Duration;

use crate::assertion::A11ySelector;
use crate::error::ExpectError;

/// The default per-action wall-clock budget when an a11y surface adapter is built
/// without an explicit timeout. Shared by the browser and gui adapters so the
/// default is one value, not two that can drift.
pub(crate) const DEFAULT_ACTION_TIMEOUT: Duration = Duration::from_secs(30);

/// The leading keywords of a "press the control" drive step (synonyms).
const PRESS_KEYWORDS: &[&str] = &["press", "click", "tap"];

/// The leading keywords of a "type into the control" drive step (synonyms).
const TYPE_KEYWORDS: &[&str] = &["type", "enter", "fill"];

/// The separator between the typed value and its target selector in a `type`
/// step (`type "hello" into textbox[name="Email"]`).
const TYPE_TARGET_SEPARATOR: &str = " into ";

/// One mechanical action an a11y surface can drive against the accessibility tree.
///
/// The drive dialect is `role[name=…]`-addressed and pixel-free: press a control
/// by role and name, or type a value into one. Parsed by [`A11yAction::parse`]
/// (a pure function, unit-tested without a browser or a native app).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A11yAction {
    /// Press (activate) the node matching `selector`.
    Press {
        /// The `role[name=…]` selector for the control to press.
        selector: A11ySelector,
    },
    /// Type `value` into the node matching `selector`.
    Type {
        /// The `role[name=…]` selector for the control to type into.
        selector: A11ySelector,
        /// The text to insert.
        value: String,
    },
}

impl A11yAction {
    /// Parse a `When` step in the a11y drive dialect, or `None` when it is not a
    /// recognized action.
    ///
    /// The dialect is `press <selector>` / `click <selector>` / `tap <selector>`
    /// for a press, and `type <value> into <selector>` (with `enter`/`fill` as
    /// synonyms) for typing; `<value>` may be quoted to carry spaces, and
    /// `<selector>` is a single `role[name=…]` selector.
    pub fn parse(when_step: &str) -> Option<Self> {
        let (keyword, rest) = split_first_word(when_step.trim());
        let keyword = keyword.to_ascii_lowercase();
        if PRESS_KEYWORDS.contains(&keyword.as_str()) {
            return A11ySelector::parse_exact(rest).map(|selector| A11yAction::Press { selector });
        }
        if TYPE_KEYWORDS.contains(&keyword.as_str()) {
            let separator = find_ascii(rest, TYPE_TARGET_SEPARATOR)?;
            let value = strip_quotes(rest[..separator].trim());
            let target = &rest[separator + TYPE_TARGET_SEPARATOR.len()..];
            return A11ySelector::parse_exact(target)
                .map(|selector| A11yAction::Type { selector, value });
        }
        None
    }
}

/// Whether `when_step` resolves to a concrete a11y action the adapter can drive
/// mechanically, or is a blank no-op.
///
/// This is the per-step gate every a11y surface returns from
/// [`resolves_mechanically`](crate::surface::SurfaceAdapter::resolves_mechanically):
/// a blank step is a mechanical no-op, and any other step must parse into a
/// concrete `role[name=…]` action; an unparseable step returns `false` and routes
/// to the agent fallback.
pub(crate) fn step_resolves_mechanically(when_step: &str) -> bool {
    when_step.trim().is_empty() || A11yAction::parse(when_step).is_some()
}

/// Split `text` into its first whitespace-delimited word and the trimmed
/// remainder (`("", "")` for blank input).
fn split_first_word(text: &str) -> (&str, &str) {
    let trimmed = text.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim_start()),
        None => (trimmed, ""),
    }
}

/// The byte offset of the first occurrence of the ASCII `needle` in `haystack`,
/// case-insensitively. The offset is valid in `haystack` because ASCII-lowercasing
/// is a length-preserving, 1:1 byte mapping.
fn find_ascii(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_ascii_lowercase().find(needle)
}

/// Strip a matching pair of surrounding quotes from `value`, else return it
/// unchanged.
fn strip_quotes(value: &str) -> String {
    let first = value.chars().next();
    let last = value.chars().next_back();
    if value.len() >= 2
        && matches!(
            (first, last),
            (Some('"'), Some('"')) | (Some('\''), Some('\''))
        )
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

/// A surface error for a drive selector that matched no accessibility node.
///
/// Shared by every a11y surface so an unbound drive selector reports identically
/// whether it failed to bind in a web a11y tree or a native one.
pub(crate) fn unbound(selector: &A11ySelector) -> ExpectError {
    ExpectError::Surface(format!(
        "no accessibility node matched `{selector}` to drive"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector(role: &str, name: Option<&str>) -> A11ySelector {
        A11ySelector {
            role: role.to_string(),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn parses_press_synonyms_into_a_press_action() {
        for keyword in PRESS_KEYWORDS {
            let step = format!("{keyword} button[name=\"Go\"]");
            assert_eq!(
                A11yAction::parse(&step),
                Some(A11yAction::Press {
                    selector: selector("button", Some("Go")),
                }),
                "{keyword}"
            );
        }
    }

    #[test]
    fn parses_a_type_step_with_value_and_target() {
        assert_eq!(
            A11yAction::parse("type \"hello world\" into textbox[name=\"Email\"]"),
            Some(A11yAction::Type {
                selector: selector("textbox", Some("Email")),
                value: "hello world".to_string(),
            })
        );
        // An unquoted single-word value also parses.
        assert_eq!(
            A11yAction::parse("fill bob into textbox[name=\"User\"]"),
            Some(A11yAction::Type {
                selector: selector("textbox", Some("User")),
                value: "bob".to_string(),
            })
        );
    }

    #[test]
    fn rejects_a_step_that_is_not_a_recognized_action() {
        // No action keyword, and no selector to bind.
        assert_eq!(A11yAction::parse("the page looks right"), None);
        // A press with no selector.
        assert_eq!(A11yAction::parse("press the shiny button"), None);
        // A type with no `into <selector>`.
        assert_eq!(A11yAction::parse("type hello"), None);
    }

    #[test]
    fn rejects_a_press_with_trailing_scope_or_garbage() {
        // The drive dialect is a single bare selector: a trailing `within` scope
        // (only the observe-side locator honors it) or stray tokens must NOT be
        // silently dropped and pressed against the wrong control.
        assert_eq!(
            A11yAction::parse("press button[name=\"Go\"] within form[name=\"Login\"]"),
            None
        );
        assert_eq!(A11yAction::parse("press button[name=\"Go\"] now"), None);
    }

    #[test]
    fn step_resolves_mechanically_only_for_recognized_or_empty_steps() {
        assert!(step_resolves_mechanically("press button[name=\"Go\"]"));
        assert!(step_resolves_mechanically("   "));
        assert!(!step_resolves_mechanically("do something clever"));
    }
}
