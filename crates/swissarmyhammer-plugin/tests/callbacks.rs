//! Integration tests for the callback primitive.
//!
//! Functions cannot cross the host/plugin boundary directly. The SDK marshals
//! them by reference: a function in a callback-bearing dispatch payload is
//! swapped for a `{ "$callback": "cb_xxxx" }` marker and stored in an
//! isolate-local table. The host invokes a stored callback by sending a
//! `notifications/callbacks/invoke` notification *into* the isolate; the SDK
//! runs the stored function and — when a return value is expected — the result
//! flows back.
//!
//! These tests drive a real V8 isolate end to end: a probe plugin passes
//! functions across the callback seam, a recording [`HostDispatcher`] captures
//! the `$callback` markers the SDK produced, and the test then triggers
//! [`PluginRuntime::invoke_callback`] and asserts the function actually ran and
//! its return value arrived back.
//!
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use swissarmyhammer_plugin::{HostDispatcher, PluginRuntime, RuntimeConfig};

/// A generous upper bound on any single runtime interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// A [`HostDispatcher`] that records the `$callback` markers a plugin sends.
///
/// It answers the `callbackDispatch` envelope — the callback-bearing transport
/// path — by capturing the marshalled payload. Any `$callback` marker the SDK
/// substituted for a function is therefore observable to the test. A
/// `toolsCall` is answered with a canned success so the test can prove tool
/// payloads are *not* scanned for functions.
#[derive(Debug, Default)]
struct MarkerRecorder {
    /// Every callback-bearing payload the SDK dispatched, in order.
    payloads: Mutex<Vec<Value>>,
    /// Every `toolsCall` arguments map the SDK dispatched, in order.
    tool_calls: Mutex<Vec<Value>>,
}

impl MarkerRecorder {
    /// Returns the callback-bearing payloads captured so far.
    fn payloads(&self) -> Vec<Value> {
        self.payloads.lock().expect("payloads mutex").clone()
    }

    /// Returns the `toolsCall` arguments maps captured so far.
    fn tool_calls(&self) -> Vec<Value> {
        self.tool_calls.lock().expect("tool_calls mutex").clone()
    }
}

impl HostDispatcher for MarkerRecorder {
    fn dispatch(&self, payload: Value) -> Result<Value, String> {
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "bridge payload missing 'kind'".to_string())?;
        match kind {
            "toolsList" => {
                // `srv` exposes one flat tool, `echo`, so a `this.srv.echo(...)`
                // call resolves and produces a plain `toolsCall`.
                let server = payload.get("server").and_then(Value::as_str).unwrap_or("");
                if server == "srv" {
                    Ok(json!({ "tools": [{ "name": "echo" }] }))
                } else {
                    Err(format!("unknown server '{server}'"))
                }
            }
            "callbackDispatch" => {
                let inner = payload.get("payload").cloned().unwrap_or(Value::Null);
                self.payloads.lock().expect("payloads mutex").push(inner);
                Ok(json!({ "ok": true }))
            }
            "toolsCall" => {
                let arguments = payload.get("arguments").cloned().unwrap_or(json!({}));
                self.tool_calls
                    .lock()
                    .expect("tool_calls mutex")
                    .push(arguments);
                Ok(json!({ "ok": true }))
            }
            other => Err(format!("unsupported bridge kind '{other}'")),
        }
    }
}

/// Builds a [`RuntimeConfig`] whose host bridge is the given dispatcher.
fn config_with(dispatcher: Arc<dyn HostDispatcher>) -> RuntimeConfig {
    RuntimeConfig {
        dispatcher: Some(dispatcher),
        ..Default::default()
    }
}

/// Writes a one-file plugin bundle whose `load` export runs `body`.
///
/// The entry imports the SDK, declares a `Plugin` subclass whose `load`
/// contains `body`, and exports a `load` lifecycle function that constructs the
/// subclass — wrapped in the SDK's plugin Proxy — and awaits its `load`.
fn write_plugin(dir: &std::path::Path, body: &str) {
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         class P extends Plugin {{\n\
           async load(): Promise<void> {{\n{body}\n}}\n\
         }}\n\
         export async function load(): Promise<unknown> {{\n\
           const p = makePluginThis(new P()) as P;\n\
           await p.load();\n\
           return null;\n\
         }}\n"
    );
    std::fs::write(dir.join("entry.ts"), entry).expect("entry.ts should be written");
}

/// Reads the single `$callback` id at `payload[field]`.
fn callback_id(payload: &Value, field: &str) -> String {
    payload
        .get(field)
        .and_then(|marker| marker.get("$callback"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected a $callback marker at '{field}', got {payload}"))
        .to_string()
}

/// A function in a callback-bearing payload is replaced with a `$callback`
/// marker, and the host can invoke that marker to run the stored function and
/// get its return value back.
#[tokio::test]
async fn host_invokes_a_callback_and_receives_its_return_value() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // The plugin hands the host a function in a callback-bearing payload. The
    // function doubles its argument — a return value the test can observe.
    write_plugin(
        bundle.path(),
        "this.__transport.callbackDispatch({ handler: (n: number) => n * 2 });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    // The host saw a `$callback` marker, not the function itself.
    let payloads = dispatcher.payloads();
    assert_eq!(payloads.len(), 1, "exactly one callback dispatch expected");
    let id = callback_id(&payloads[0], "handler");
    assert!(
        id.starts_with("cb_"),
        "a callback marker id must be `cb_`-prefixed, got '{id}'"
    );

    // The host invokes the stored function by id; its return value flows back.
    let result = tokio::time::timeout(TIMEOUT, runtime.invoke_callback(&id, json!([21])))
        .await
        .expect("invoking the callback should not hang")
        .expect("invoking a stored callback should succeed");
    assert_eq!(
        result,
        json!(42),
        "the host must receive the callback's return value"
    );
}

/// A callback that returns nothing and a callback that returns a value both
/// behave correctly: the void callback still runs (an observable side effect)
/// and the value callback's result still flows back.
#[tokio::test]
async fn void_and_value_callbacks_both_behave_correctly() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // `record` returns nothing but writes a module-global the test can read
    // back; `compute` returns a value.
    write_plugin(
        bundle.path(),
        "this.__transport.callbackDispatch({\n\
           record: (msg: string): void => { (globalThis as Record<string, unknown>).__seen = msg; },\n\
           compute: (a: number, b: number): number => a + b,\n\
         });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let payloads = dispatcher.payloads();
    assert_eq!(payloads.len(), 1, "exactly one callback dispatch expected");
    let record_id = callback_id(&payloads[0], "record");
    let compute_id = callback_id(&payloads[0], "compute");
    assert_ne!(
        record_id, compute_id,
        "each function must get a distinct callback id"
    );

    // The void callback runs: invoking it returns JSON null, and the side
    // effect it performed is observable in the isolate afterward.
    let void_result = tokio::time::timeout(
        TIMEOUT,
        runtime.invoke_callback(&record_id, json!(["hello from host"])),
    )
    .await
    .expect("invoking the void callback should not hang")
    .expect("invoking a void callback should succeed");
    assert_eq!(
        void_result,
        Value::Null,
        "a callback with no return value must yield JSON null"
    );
    let side_effect = runtime
        .eval("globalThis.__seen")
        .await
        .expect("reading the side effect should succeed");
    assert_eq!(
        side_effect,
        json!("hello from host"),
        "the void callback must have run its side effect inside the isolate"
    );

    // The value callback's result still flows back.
    let value_result =
        tokio::time::timeout(TIMEOUT, runtime.invoke_callback(&compute_id, json!([3, 4])))
            .await
            .expect("invoking the value callback should not hang")
            .expect("invoking a value callback should succeed");
    assert_eq!(
        value_result,
        json!(7),
        "the value callback's return value must flow back to the host"
    );
}

/// An `async` callback is awaited before its result flows back.
#[tokio::test]
async fn an_async_callback_is_awaited() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "this.__transport.callbackDispatch({\n\
           later: async (n: number): Promise<number> => {\n\
             await Promise.resolve();\n\
             return n + 100;\n\
           },\n\
         });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let id = callback_id(&dispatcher.payloads()[0], "later");
    let result = tokio::time::timeout(TIMEOUT, runtime.invoke_callback(&id, json!([5])))
        .await
        .expect("invoking the async callback should not hang")
        .expect("invoking an async callback should succeed");
    assert_eq!(
        result,
        json!(105),
        "an async callback's resolved value must flow back to the host"
    );
}

/// Tool-call payloads are unaffected: a function never reaches a `toolsCall`,
/// because the callback machinery only scans callback-bearing paths.
///
/// `callbackDispatch` carries a function and produces a `$callback` marker;
/// the `toolsCall` issued right after carries only plain JSON and is delivered
/// verbatim with no `$callback` key anywhere in it.
#[tokio::test]
async fn tool_call_payloads_are_not_scanned_for_callbacks() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // First a callback-bearing dispatch (a function → marker), then a plain
    // `toolsCall` carrying ordinary JSON arguments.
    write_plugin(
        bundle.path(),
        "this.__transport.callbackDispatch({ handler: () => 1 });\n\
         await this.srv.echo({ message: 'plain args' });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    // The callback path produced a marker.
    assert_eq!(
        dispatcher.payloads().len(),
        1,
        "the callback-bearing dispatch should have been recorded"
    );

    // The tool call carried plain arguments verbatim — no `$callback` key.
    let tool_calls = dispatcher.tool_calls();
    assert_eq!(tool_calls.len(), 1, "exactly one tools/call expected");
    assert_eq!(
        tool_calls[0],
        json!({ "message": "plain args" }),
        "a tools/call payload must cross verbatim, with no callback machinery"
    );
    assert!(
        !rendered(&tool_calls[0]).contains("$callback"),
        "no `$callback` marker may appear in a tools/call payload, got {}",
        rendered(&tool_calls[0])
    );
}

/// Renders a value to a string for substring assertions.
fn rendered(value: &Value) -> String {
    serde_json::to_string(value).expect("a JSON value is serializable")
}

/// A cyclic plain object in a `callbackDispatch` payload does not overflow the
/// V8 stack: `marshalCallbacks` detects the cycle and throws, so the marshalling
/// walk terminates fast and the isolate stays alive.
///
/// A cyclic object cannot cross the host boundary — it has no JSON encoding —
/// so the dispatch is *expected* to fail. What matters is *how*: marshalling
/// throws a `TypeError` on the first back-edge, the plugin's `load` rejects
/// cleanly, and the isolate is intact. Before the cycle guard, `marshalCallbacks`
/// recursed into `a.self.self.…` until the V8 stack overflowed. The test proves
/// the runtime survives by evaluating an expression in the same isolate after.
#[tokio::test]
async fn a_cyclic_callback_payload_does_not_overflow_the_isolate() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // `a.self = a` makes a cycle through a plain object — exactly the shape
    // that drove `marshalCallbacks` into unbounded recursion before the guard.
    write_plugin(
        bundle.path(),
        "const a: Record<string, unknown> = {};\n\
         a.self = a;\n\
         this.__transport.callbackDispatch({ a });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let load = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("the cyclic dispatch must not hang — the cycle guard must terminate the walk");

    // A cyclic payload cannot be marshalled, so the dispatch fails — cleanly.
    assert!(
        load.is_err(),
        "a cyclic callback payload cannot cross the boundary and must fail, got {load:?}"
    );

    // The isolate survived the cycle: it did not stack-overflow. Proven by the
    // runtime still answering an eval.
    let alive = tokio::time::timeout(TIMEOUT, runtime.eval("1 + 1"))
        .await
        .expect("the runtime must still respond after a cyclic payload")
        .expect("evaluating in the isolate after a cyclic payload should succeed");
    assert_eq!(
        alive,
        json!(2),
        "the isolate must remain usable after a cyclic callback payload"
    );
}

/// A sub-object reached by two acyclic paths (a diamond) is marshalled exactly
/// once, and the function inside that shared subtree is still caught.
///
/// The payload references one `shared` object — which holds a function — under
/// two keys, `left` and `right`. The marshalling guard records each container
/// the first time it is walked and reuses the marshalled result on the second
/// path, so `shared.fn` becomes a `$callback` marker on both paths and the two
/// markers carry the *same* id. This proves the guard suppresses only the
/// redundant re-walk, never the marshalling of a function in a shared subtree.
#[tokio::test]
async fn a_shared_subtree_is_marshalled_once_and_its_function_is_caught() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // `shared` is referenced twice; it carries a function the host must see as
    // a `$callback` marker on both paths.
    write_plugin(
        bundle.path(),
        "const shared = { fn: (n: number): number => n * 3 };\n\
         this.__transport.callbackDispatch({ left: shared, right: shared });",
    );

    let dispatcher = Arc::new(MarkerRecorder::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("a diamond-shaped (acyclic) payload should dispatch cleanly");

    let payloads = dispatcher.payloads();
    assert_eq!(payloads.len(), 1, "exactly one callback dispatch expected");

    // Both paths carry a `$callback` marker for the one shared function.
    let left_id = callback_id(
        payloads[0]
            .get("left")
            .expect("the marshalled payload must keep the `left` key"),
        "fn",
    );
    let right_id = callback_id(
        payloads[0]
            .get("right")
            .expect("the marshalled payload must keep the `right` key"),
        "fn",
    );
    assert_eq!(
        left_id, right_id,
        "a shared subtree must be marshalled once, so both paths share one id"
    );

    // The single marshalled function is live: invoking its id runs it.
    let result = tokio::time::timeout(TIMEOUT, runtime.invoke_callback(&left_id, json!([7])))
        .await
        .expect("invoking the shared callback should not hang")
        .expect("the function inside the shared subtree must have been registered");
    assert_eq!(
        result,
        json!(21),
        "the shared subtree's function must be the stored callback"
    );
}

/// Invoking a callback id the isolate never registered fails cleanly rather
/// than hanging or panicking.
#[tokio::test]
async fn invoking_an_unknown_callback_id_fails() {
    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime starts");
    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.invoke_callback("cb_does_not_exist", json!([])),
    )
    .await
    .expect("invoking an unknown callback should not hang");
    assert!(
        result.is_err(),
        "invoking an unregistered callback id must fail, got {result:?}"
    );
}
