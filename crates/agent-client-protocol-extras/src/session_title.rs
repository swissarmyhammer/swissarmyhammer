//! Agent-neutral session-title derivation.
//!
//! Sessions are keyed by ULID. A wall of ULIDs is not a browsable
//! `session/list`, so each agent gives a session a short human-readable title
//! after its first meaningful exchange. The title is carried on
//! [`SessionRecord::title`](crate::SessionRecord) and announced live with the
//! built-in [`SessionUpdate::SessionInfoUpdate`](agent_client_protocol::schema::SessionUpdate)
//! notification — there is no custom `ext` method for titles.
//!
//! # Shared trigger / emission contract
//!
//! Both ACP agents implement the *same* title lifecycle; only the generation
//! source differs (see below). The contract is:
//!
//! 1. **Trigger** — generate the title after the first meaningful exchange:
//!    the session has at least one user message *and* at least one agent
//!    response. Earlier turns (e.g. an empty or cancelled first prompt) do not
//!    qualify.
//! 2. **Generate off the turn's critical path** — title generation must never
//!    block the `session/prompt` response. It runs asynchronously; the
//!    notification is emitted when the title is ready.
//! 3. **On generation or change** — update
//!    [`SessionRecord::title`](crate::SessionRecord) and
//!    [`SessionRecord::updated_at`](crate::SessionRecord), persist via
//!    [`SessionStore`](crate::SessionStore), and emit exactly one
//!    [`SessionInfoUpdate`](agent_client_protocol::schema::SessionInfoUpdate)
//!    notification carrying the new title and timestamp.
//! 4. **Generate once** — a session keeps its first generated title for the
//!    rest of its life; later turns only bump `updated_at`.
//!
//! # Per-agent generation source
//!
//! - **claude-agent** derives the title from the first user prompt (the claude
//!   CLI exposes no session summary to borrow).
//! - **llama-agent** asks its own model for a short title and falls back to the
//!   first-user-message heuristic when a model call is unavailable.
//!
//! Both fall through to [`title_from_first_user_message`], which is the single
//! shared implementation of the heuristic so the fallback is identical.

/// Maximum length, in characters, of a session title.
///
/// Long opening prompts and verbose model output are truncated to this many
/// characters so `session/list` stays compact and scannable.
pub const SESSION_TITLE_MAX_CHARS: usize = 80;

/// Derive a session title from the first user message text.
///
/// This is the shared heuristic fallback: it collapses internal whitespace,
/// trims, and truncates to [`SESSION_TITLE_MAX_CHARS`] characters. It is used
/// directly by claude-agent and as the fallback by llama-agent when a model
/// title call is unavailable.
///
/// Returns `None` when `text` has no non-whitespace content — a session with
/// no user-message text yet has no derivable title.
///
/// # Parameters
///
/// * `text` - The raw text of the first user message.
#[must_use]
pub fn title_from_first_user_message(text: &str) -> Option<String> {
    normalize_title(text)
}

/// Normalize an arbitrary candidate string into a session title.
///
/// Collapses runs of whitespace to single spaces, trims the ends, and
/// truncates to [`SESSION_TITLE_MAX_CHARS`] characters. Returns `None` when the
/// candidate is empty after normalization.
///
/// This is applied to *every* title source — the first user message and a
/// model-generated title alike — so a title from any source obeys the same
/// length and whitespace rules.
///
/// # Parameters
///
/// * `candidate` - The raw title text from any source.
#[must_use]
pub fn normalize_title(candidate: &str) -> Option<String> {
    let collapsed = candidate.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(collapsed.chars().take(SESSION_TITLE_MAX_CHARS).collect())
}

/// Instruction used to ask a model for a short session title.
///
/// Kept here so every agent that generates titles with a model call phrases
/// the request identically.
pub const TITLE_GENERATION_INSTRUCTION: &str =
    "Summarize this conversation as a short title of at most 6 words. \
     Reply with only the title, no quotes and no punctuation at the end.";

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain prompt becomes its own trimmed title.
    #[test]
    fn plain_prompt_is_its_own_title() {
        assert_eq!(
            title_from_first_user_message("Add user authentication"),
            Some("Add user authentication".to_string())
        );
    }

    /// Surrounding and internal whitespace is collapsed.
    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(
            title_from_first_user_message("  fix   the\n\tlogin   bug  "),
            Some("fix the login bug".to_string())
        );
    }

    /// A whitespace-only message yields no title.
    #[test]
    fn blank_message_has_no_title() {
        assert!(title_from_first_user_message("   \n\t ").is_none());
        assert!(title_from_first_user_message("").is_none());
    }

    /// Long input is truncated to the character cap.
    #[test]
    fn long_input_is_truncated() {
        let long = "x".repeat(SESSION_TITLE_MAX_CHARS + 50);
        let title = title_from_first_user_message(&long).unwrap();
        assert_eq!(title.chars().count(), SESSION_TITLE_MAX_CHARS);
    }

    /// Truncation counts characters, not bytes, so multi-byte input is not
    /// split mid-character.
    #[test]
    fn truncation_is_char_aware() {
        let long = "é".repeat(SESSION_TITLE_MAX_CHARS + 10);
        let title = normalize_title(&long).unwrap();
        assert_eq!(title.chars().count(), SESSION_TITLE_MAX_CHARS);
    }
}
