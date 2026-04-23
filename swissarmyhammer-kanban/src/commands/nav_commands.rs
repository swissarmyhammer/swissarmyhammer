//! Spatial navigation commands (`nav.up`, `nav.down`, …).
//!
//! These commands are the sole path by which cardinal + edge spatial
//! navigation moves focus.  React dispatches them like any other command
//! (keypress → `createKeyHandler` → `useDispatchCommand` → `dispatch_command`),
//! and the handler delegates to the [`SpatialNavigator`] extension the
//! Tauri binary installs on every `CommandContext`.
//!
//! This replaces the previous in-JS side-channel (`broadcastNavCommand` +
//! `NAV_DIRECTION_MAP` + an `execute:` handler on every nav `CommandDef`)
//! that short-circuited the dispatch pipeline and prevented the
//! `focus-changed` round-trip from reaching the focused-moniker store.
//! With the side-channel removed, every nav keypress now logs a command,
//! runs through Rust, mutates `SpatialState`, and fans out a
//! `focus-changed` event the React store subscribes to.
//!
//! Undoable: no — focus is transient UI state, not user content.

use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_spatial_nav::Direction;

use crate::spatial::SpatialNavigatorExt;

/// Navigate focus in a fixed direction.
///
/// The direction is baked in at construction time so each nav command
/// (`nav.up`, `nav.down`, …) is a distinct `Arc<dyn Command>` but shares
/// the same implementation.  Availability is unconditional: nav keys must
/// resolve to a command even when focus is momentarily `None`, so the
/// `SpatialNavigator` can recover via its first-in-layer fallback.
pub struct NavigateCmd(pub Direction);

#[async_trait]
impl Command for NavigateCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let navigator = ctx.require_extension::<SpatialNavigatorExt>()?;
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let moniker = navigator
            .0
            .navigate(window_label, self.0)
            .await
            .map_err(CommandError::ExecutionFailed)?;
        Ok(match moniker {
            Some(m) => Value::String(m),
            None => Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Test double that captures every navigate call and returns a scripted
    /// moniker. No real `SpatialState` needed — the command layer only cares
    /// about delegation, not beam-test mechanics.
    struct FakeNavigator {
        log: Mutex<Vec<(String, Direction)>>,
        result: Option<String>,
    }

    #[async_trait]
    impl crate::spatial::SpatialNavigator for FakeNavigator {
        async fn navigate(
            &self,
            window_label: &str,
            direction: Direction,
        ) -> Result<Option<String>, String> {
            self.log
                .lock()
                .unwrap()
                .push((window_label.to_string(), direction));
            Ok(self.result.clone())
        }
    }

    fn ctx_with_navigator(
        scope: &[&str],
        nav: Arc<FakeNavigator>,
    ) -> (CommandContext, Arc<FakeNavigator>) {
        let mut ctx = CommandContext::new(
            "nav.down",
            scope.iter().map(|s| s.to_string()).collect(),
            None,
            HashMap::new(),
        );
        let ext = Arc::new(SpatialNavigatorExt(nav.clone()));
        ctx.set_extension(ext);
        (ctx, nav)
    }

    #[tokio::test]
    async fn navigate_cmd_delegates_to_provider() {
        let nav = Arc::new(FakeNavigator {
            log: Mutex::new(vec![]),
            result: Some("task:01XYZ".to_string()),
        });
        let (ctx, nav) = ctx_with_navigator(&["window:main"], nav);

        let cmd = NavigateCmd(Direction::Down);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result, Value::String("task:01XYZ".to_string()));

        let log = nav.log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, "main");
        assert_eq!(log[0].1, Direction::Down);
    }

    #[tokio::test]
    async fn navigate_cmd_uses_window_label_from_scope() {
        let nav = Arc::new(FakeNavigator {
            log: Mutex::new(vec![]),
            result: None,
        });
        let (ctx, nav) = ctx_with_navigator(&["task:01AB", "window:secondary"], nav);

        let cmd = NavigateCmd(Direction::Right);
        cmd.execute(&ctx).await.unwrap();

        let log = nav.log.lock().unwrap();
        assert_eq!(log[0].0, "secondary");
    }

    #[tokio::test]
    async fn navigate_cmd_returns_null_when_navigator_returns_none() {
        let nav = Arc::new(FakeNavigator {
            log: Mutex::new(vec![]),
            result: None,
        });
        let (ctx, _) = ctx_with_navigator(&["window:main"], nav);

        let cmd = NavigateCmd(Direction::Up);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn navigate_cmd_defaults_to_main_window_when_no_window_in_scope() {
        let nav = Arc::new(FakeNavigator {
            log: Mutex::new(vec![]),
            result: None,
        });
        let (ctx, nav) = ctx_with_navigator(&[], nav);

        let cmd = NavigateCmd(Direction::First);
        cmd.execute(&ctx).await.unwrap();

        let log = nav.log.lock().unwrap();
        assert_eq!(log[0].0, "main");
    }

    #[test]
    fn navigate_cmd_always_available() {
        let ctx = CommandContext::new("nav.up", vec![], None, HashMap::new());
        assert!(NavigateCmd(Direction::Up).available(&ctx));
    }
}
