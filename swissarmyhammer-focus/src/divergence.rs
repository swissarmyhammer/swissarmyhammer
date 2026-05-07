//! Snapshot/registry divergence diagnostic — debug-only dual-walk harness
//! that pins the snapshot path and the legacy registry path to the same
//! result for every focus-mutating IPC.
//!
//! The kanban-app spatial-nav adapters run both paths through
//! [`compare_paths`] in debug builds: when the two results disagree, a
//! single `tracing::warn!` records `op`, `snapshot`, and `registry` so the
//! divergence can be triaged from the dev log. Release builds skip the
//! comparison entirely — the registry closure is never invoked and the
//! snapshot result is returned directly, so the helper compiles to a
//! plain forward of the snapshot path with no double-walk cost.
//!
//! Consolidating the comparison here keeps the warn shape identical
//! across `spatial_navigate`, `spatial_focus`, and `spatial_focus_lost`,
//! so log scrapers and soak-test assertions can match a single field
//! schema regardless of which IPC fired.

use std::fmt::Debug;

/// Run both paths in debug builds, warn on divergence, return the
/// snapshot result. In release builds, run only the snapshot path.
///
/// `op` distinguishes the call site in the captured warn event
/// (`spatial_navigate.divergence`, `spatial_focus.divergence`,
/// `spatial_focus_lost.divergence`). `snapshot_path` is the path that
/// will be authoritative after cutover; `registry_path` is the legacy
/// kernel-side replica used only for the dual-walk parity check.
#[cfg(debug_assertions)]
pub fn compare_paths<R, F1, F2>(op: &str, snapshot_path: F1, registry_path: F2) -> R
where
    R: PartialEq + Debug,
    F1: FnOnce() -> R,
    F2: FnOnce() -> R,
{
    let snapshot_result = snapshot_path();
    let registry_result = registry_path();
    if snapshot_result != registry_result {
        tracing::warn!(
            op = %op,
            snapshot = ?snapshot_result,
            registry = ?registry_result,
            "spatial-nav snapshot/registry divergence",
        );
    }
    snapshot_result
}

/// Release-build counterpart of [`compare_paths`]. Runs `snapshot_path`
/// once, drops `registry_path` unevaluated, and returns the snapshot
/// result. The closure indirection optimizes away under `--release`, so
/// the helper compiles to a direct forward.
#[cfg(not(debug_assertions))]
pub fn compare_paths<R, F1, F2>(_op: &str, snapshot_path: F1, _registry_path: F2) -> R
where
    R: PartialEq + Debug,
    F1: FnOnce() -> R,
    F2: FnOnce() -> R,
{
    snapshot_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use tracing::{
        field::{Field, Visit},
        span::Attributes,
        Event, Id, Level, Subscriber,
    };
    use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

    #[derive(Debug, Default, Clone)]
    struct CapturedEvent {
        level: Option<Level>,
        message: String,
        fields: HashMap<String, String>,
    }

    struct FieldVisitor<'a> {
        fields: &'a mut HashMap<String, String>,
        message: &'a mut String,
    }

    impl<'a> Visit for FieldVisitor<'a> {
        fn record_str(&mut self, field: &Field, value: &str) {
            if field.name() == "message" {
                self.message.push_str(value);
            } else {
                self.fields
                    .insert(field.name().to_string(), value.to_string());
            }
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.message.push_str(&format!("{value:?}"));
            } else {
                self.fields
                    .insert(field.name().to_string(), format!("{value:?}"));
            }
        }
    }

    struct CapturingLayer {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    impl<S> Layer<S> for CapturingLayer
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let level = *event.metadata().level();
            if level > Level::WARN {
                return;
            }
            let mut captured = CapturedEvent {
                level: Some(level),
                ..CapturedEvent::default()
            };
            let mut visitor = FieldVisitor {
                fields: &mut captured.fields,
                message: &mut captured.message,
            };
            event.record(&mut visitor);
            self.events.lock().unwrap().push(captured);
        }
        fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
    }

    fn run_capturing<F: FnOnce()>(f: F) -> Vec<CapturedEvent> {
        let events = Arc::new(Mutex::new(Vec::<CapturedEvent>::new()));
        let layer = CapturingLayer {
            events: events.clone(),
        };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, f);
        let captured = events.lock().unwrap().clone();
        captured
    }

    /// When both paths return the same value, no warn fires and the
    /// snapshot result is forwarded.
    #[cfg(debug_assertions)]
    #[test]
    fn matching_results_emit_no_warn() {
        let events = Arc::new(Mutex::new(Vec::<CapturedEvent>::new()));
        let layer = CapturingLayer {
            events: events.clone(),
        };
        let subscriber = tracing_subscriber::registry().with(layer);
        let returned = tracing::subscriber::with_default(subscriber, || {
            compare_paths("test.op", || Some(42_u32), || Some(42_u32))
        });
        let captured = events.lock().unwrap().clone();

        assert_eq!(returned, Some(42));
        let divergence: Vec<_> = captured
            .iter()
            .filter(|e| e.fields.get("op").map(|v| v == "test.op").unwrap_or(false))
            .collect();
        assert!(
            divergence.is_empty(),
            "matching paths must not emit a divergence warn, got {captured:?}",
        );
    }

    /// When the two paths return different values, a single WARN fires
    /// with `op`, `snapshot`, and `registry` fields and the message
    /// `"spatial-nav snapshot/registry divergence"`. The returned value
    /// is the snapshot result.
    #[cfg(debug_assertions)]
    #[test]
    fn diverging_results_emit_warn_with_op_snapshot_registry() {
        let mut returned = 0;
        let captured = run_capturing(|| {
            returned = compare_paths("test.divergent", || 7_u32, || 13_u32);
        });

        assert_eq!(returned, 7, "must return the snapshot result");
        let divergence: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.fields
                    .get("op")
                    .map(|v| v == "test.divergent")
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(
            divergence.len(),
            1,
            "expected exactly one divergence warn, got {captured:?}",
        );
        let event = divergence[0];
        assert_eq!(event.level, Some(Level::WARN));
        assert_eq!(event.fields.get("snapshot").map(String::as_str), Some("7"),);
        assert_eq!(event.fields.get("registry").map(String::as_str), Some("13"),);
        assert!(
            event
                .message
                .contains("spatial-nav snapshot/registry divergence"),
            "warn message must identify the divergence; got {:?}",
            event.message,
        );
    }

    /// Release-build counterpart compiles. Build-config gating is
    /// asserted by the cfg-attribute on the function itself.
    #[test]
    fn compare_paths_compiles_in_both_configs() {
        let result = compare_paths("test.smoke", || 1_u32, || 1_u32);
        assert_eq!(result, 1);
    }
}
