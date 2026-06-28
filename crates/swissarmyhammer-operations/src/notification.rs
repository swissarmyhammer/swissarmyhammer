//! The Notification trait — static metadata for an event a service emits.

use crate::ParamMeta;

/// Metadata about a notification a service emits — the sibling of [`Operation`]
/// for the event side.
///
/// Where an [`Operation`] declares an invocable `verb noun`, a `Notification`
/// declares an event a plugin can subscribe to: its full wire `method`, the
/// short `event` name the SDK's `this.<server>.on(event, …)` resolves against,
/// a description, and the params schema (the struct fields). Decorating a
/// service's notifications with `#[notification]` and listing them in
/// `operation_tool!`'s `notifications:` field folds them into the tool's
/// `io.swissarmyhammer/notifications` `_meta`, so the available events become a
/// declared, discoverable vocabulary — exactly as operations are.
///
/// Like [`Operation`], methods take `&self` to keep the trait object-safe, but
/// implementations return static values.
///
/// [`Operation`]: crate::Operation
pub trait Notification: Send + Sync {
    /// The full MCP notification method, e.g. `"notifications/commands/executed"`.
    fn method(&self) -> &'static str;

    /// The short event name a plugin subscribes to (e.g. `"executed"`).
    ///
    /// Defaults to the last `/`-separated segment of [`method`](Self::method),
    /// so `"notifications/commands/executed"` ⇒ `"executed"`. The
    /// `#[notification]` macro overrides this when an explicit `event = "…"` is
    /// given.
    fn event(&self) -> &'static str {
        let method = self.method();
        match method.rsplit_once('/') {
            Some((_, last)) => last,
            None => method,
        }
    }

    /// Human-readable description.
    fn description(&self) -> &'static str;

    /// Parameter metadata describing the notification's `params` payload.
    ///
    /// Default returns empty; the `#[notification]` macro overrides this from
    /// the decorated struct's fields, the same way `#[operation]` does.
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CommandsExecuted;

    impl Notification for CommandsExecuted {
        fn method(&self) -> &'static str {
            "notifications/commands/executed"
        }
        fn description(&self) -> &'static str {
            "A command executed."
        }
    }

    struct ExplicitEvent;

    impl Notification for ExplicitEvent {
        fn method(&self) -> &'static str {
            "notifications/store/undo_changed"
        }
        fn event(&self) -> &'static str {
            "undo"
        }
        fn description(&self) -> &'static str {
            "Undo stack changed."
        }
    }

    #[test]
    fn event_defaults_to_last_method_segment() {
        assert_eq!(CommandsExecuted.event(), "executed");
    }

    #[test]
    fn event_can_be_overridden() {
        assert_eq!(ExplicitEvent.event(), "undo");
    }

    #[test]
    fn parameters_default_empty() {
        assert!(CommandsExecuted.parameters().is_empty());
    }
}
