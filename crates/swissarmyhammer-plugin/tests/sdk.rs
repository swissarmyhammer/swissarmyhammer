//! Integration tests for the `@swissarmyhammer/plugin` TypeScript SDK.
//!
//! These tests load a real plugin bundle into a real V8 isolate and exercise
//! the SDK's generic dispatch Proxy end to end. A recording [`HostDispatcher`]
//! stands in for the host: it answers `tools/list` with a canned `_meta` tree
//! and records every `tools/call` so the test can assert the exact wire-call
//! shape the transport produced.
//!
//! The point of these tests is the **wire-call shape** — the tool name and the
//! arguments map the SDK hands to the host — for both an operation-tool path
//! call and a flat-tool call, plus the `UnknownOperation` / `UnknownServer`
//! failure modes and the `RESERVED`-name rule.
//!
//! Every runtime interaction is wrapped in a timeout so a wedged isolate fails
//! the test fast instead of hanging CI.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use swissarmyhammer_plugin::{HostDispatcher, PluginLifecycle, PluginRuntime, RuntimeConfig};

/// A generous upper bound on any single runtime interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// One `tools/call` the SDK dispatched, captured for assertion.
#[derive(Debug, Clone, PartialEq)]
struct RecordedCall {
    /// The MCP tool name addressed by the call.
    tool: String,
    /// The `tools/call` arguments map the SDK produced.
    arguments: Value,
}

/// A [`HostDispatcher`] that serves a canned tool list and records calls.
///
/// `tools/list` for the `srv` server returns one operation tool (`kanban`)
/// and one flat tool (`current`); every other server is unknown. Each
/// `tools/call` is appended to `calls` so a test can assert the wire shape.
#[derive(Debug, Default)]
struct RecordingDispatcher {
    /// Every `tools/call` the SDK has dispatched, in order.
    calls: Mutex<Vec<RecordedCall>>,
}

impl RecordingDispatcher {
    /// Returns the recorded `tools/call`s so far.
    fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().expect("calls mutex").clone()
    }

    /// The canned `tools/list` payload for the `srv` server.
    ///
    /// `kanban` is an operation tool — it carries an
    /// `io.swissarmyhammer/operations` `_meta` tree with a `task` noun whose
    /// `add` verb maps to the `"add task"` op string. `current` is a flat tool
    /// — no operations `_meta`, so it dispatches verbatim.
    fn srv_tools() -> Value {
        json!({
            "tools": [
                {
                    "name": "kanban",
                    "description": "Kanban board operations",
                    "inputSchema": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": { "op": { "type": "string" } }
                    },
                    "_meta": {
                        "io.swissarmyhammer/operations": {
                            "task": {
                                "add": {
                                    "op": "add task",
                                    "description": "Create a new task",
                                    "parameters": {
                                        "title": { "type": "string", "required": true }
                                    }
                                },
                                "move": {
                                    "op": "move task",
                                    "description": "Move a task to a column",
                                    "parameters": {
                                        "id": { "type": "string", "required": true }
                                    }
                                }
                            }
                        }
                    }
                },
                {
                    "name": "current",
                    "description": "Current weather",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "city": { "type": "string" } }
                    }
                }
            ]
        })
    }
}

impl HostDispatcher for RecordingDispatcher {
    fn dispatch(&self, payload: Value) -> Result<Value, String> {
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "bridge payload missing 'kind'".to_string())?;
        match kind {
            "toolsList" => {
                let server = payload.get("server").and_then(Value::as_str).unwrap_or("");
                if server == "srv" {
                    Ok(Self::srv_tools())
                } else {
                    Err(format!("unknown server '{server}'"))
                }
            }
            "toolsCall" => {
                let server = payload.get("server").and_then(Value::as_str).unwrap_or("");
                if server != "srv" {
                    return Err(format!("unknown server '{server}'"));
                }
                let tool = payload
                    .get("tool")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "toolsCall payload missing 'tool'".to_string())?
                    .to_string();
                let arguments = payload.get("arguments").cloned().unwrap_or(json!({}));
                self.calls
                    .lock()
                    .expect("calls mutex")
                    .push(RecordedCall { tool, arguments });
                Ok(json!({ "ok": true }))
            }
            other => Err(format!("unsupported bridge kind '{other}'")),
        }
    }
}

/// Build a [`RuntimeConfig`] whose host bridge is the given dispatcher.
fn config_with(dispatcher: Arc<dyn HostDispatcher>) -> RuntimeConfig {
    RuntimeConfig {
        dispatcher: Some(dispatcher),
        ..Default::default()
    }
}

/// Write a one-file plugin bundle whose default-class `load()` runs `body`.
///
/// The entry imports the SDK and default-exports a `Plugin` subclass whose
/// `load()` contains `body`. The host instantiates the default export, wraps it
/// with the SDK's plugin Proxy, and runs its `load()`. The hook returns
/// `globalThis.__result ?? null` so a test that records a value on
/// `globalThis.__result` observes it as the lifecycle call's return value.
fn write_plugin(dir: &std::path::Path, body: &str) {
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         export default class P extends Plugin {{\n\
           async load(): Promise<unknown> {{\n{body}\n\
             return globalThis.__result ?? null;\n\
           }}\n\
         }}\n"
    );
    std::fs::write(dir.join("entry.ts"), entry).expect("entry.ts should be written");
}

/// A path-form operation call compiles to `tools/call(tool, {op, ...args})`.
///
/// `this.srv.kanban.task.add({title})` must reach the host as a `tools/call`
/// on the `kanban` tool with `{ op: "add task", title }` — the `op` string
/// looked up from the operation tool's `_meta` tree.
#[tokio::test]
async fn operation_path_call_produces_op_wire_shape() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "await this.srv.kanban.task.add({ title: 'Fix login bug' });",
    );

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let calls = dispatcher.calls();
    assert_eq!(calls.len(), 1, "exactly one tools/call expected");
    assert_eq!(
        calls[0],
        RecordedCall {
            tool: "kanban".to_string(),
            arguments: json!({ "op": "add task", "title": "Fix login bug" }),
        },
        "a path-form operation call must dispatch as tools/call(tool, {{op, ...args}})"
    );
}

/// A flat-tool call compiles to `tools/call(tool, args)` verbatim.
///
/// `this.srv.current({city})` reaches the host as a `tools/call` on the
/// `current` tool with the arguments passed straight through — no `op` key,
/// because `current` has no operations `_meta`.
#[tokio::test]
async fn flat_tool_call_produces_verbatim_wire_shape() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(bundle.path(), "await this.srv.current({ city: 'Austin' });");

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let calls = dispatcher.calls();
    assert_eq!(calls.len(), 1, "exactly one tools/call expected");
    assert_eq!(
        calls[0],
        RecordedCall {
            tool: "current".to_string(),
            arguments: json!({ "city": "Austin" }),
        },
        "a flat-tool call must dispatch as tools/call(tool, args) with no op key"
    );
}

/// The direct form — `op` already in args — passes through unchanged.
#[tokio::test]
async fn operation_direct_form_passes_op_through() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "await this.srv.kanban({ op: 'move task', id: 't_12' });",
    );

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let calls = dispatcher.calls();
    assert_eq!(calls.len(), 1, "exactly one tools/call expected");
    assert_eq!(
        calls[0],
        RecordedCall {
            tool: "kanban".to_string(),
            arguments: json!({ "op": "move task", "id": "t_12" }),
        },
        "the direct form must pass {{op, ...}} straight through"
    );
}

/// An unknown verb path raises `UnknownOperation`, and the error lists the
/// valid verbs for that noun straight from `_meta`.
#[tokio::test]
async fn unknown_verb_raises_unknown_operation_listing_verbs() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // `task.delete` is not in the canned `_meta` tree — only `add` and `move`.
    write_plugin(
        bundle.path(),
        "try {\n\
           await this.srv.kanban.task.delete({ id: 't_9' });\n\
           globalThis.__result = 'no-error';\n\
         } catch (e) {\n\
           globalThis.__result = { name: (e as Error).name, message: (e as Error).message };\n\
         }",
    );

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let name = result.get("name").and_then(Value::as_str).unwrap_or("");
    let message = result.get("message").and_then(Value::as_str).unwrap_or("");
    assert_eq!(
        name, "UnknownOperation",
        "an unknown verb must raise UnknownOperation, got result: {result}"
    );
    assert!(
        message.contains("add") && message.contains("move"),
        "the UnknownOperation message must list the valid verbs, got: {message}"
    );
    assert!(
        dispatcher.calls().is_empty(),
        "an unknown verb must not produce a tools/call"
    );
}

/// An unknown server raises `UnknownServer` at dispatch time.
#[tokio::test]
async fn unknown_server_raises_unknown_server() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "try {\n\
           await this.nope.whatever({});\n\
           globalThis.__result = 'no-error';\n\
         } catch (e) {\n\
           globalThis.__result = { name: (e as Error).name };\n\
         }",
    );

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    assert_eq!(
        result.get("name").and_then(Value::as_str),
        Some("UnknownServer"),
        "a call to an unregistered server must raise UnknownServer, got: {result}"
    );
}

/// A `Plugin` subclass that sets `name`/`version`/`description` transpiles
/// cleanly and the properties are readable on the constructed instance.
///
/// `name`, `version`, and `description` are author-facing descriptive metadata
/// on the `Plugin` base class. A subclass overrides them as plain field
/// initializers; this test confirms such a subclass transpiles, constructs, and
/// exposes the overridden values — and that an instance keeps the base
/// defaults when it does not.
#[tokio::test]
async fn plugin_subclass_exposes_metadata_props() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // Two subclasses: `Named` overrides every metadata field, `Bare` overrides
    // none and so must inherit the base defaults.
    let entry = "import { Plugin } from '@swissarmyhammer/plugin';\n\
         class Named extends Plugin {\n\
           readonly name = 'my-plugin';\n\
           readonly version = '1.2.3';\n\
           readonly description = 'a worked example plugin';\n\
         }\n\
         class Bare extends Plugin {}\n\
         export default class MetaProbe extends Plugin {\n\
           async load(): Promise<unknown> {\n\
             const named = new Named();\n\
             const bare = new Bare();\n\
             return {\n\
               namedName: named.name,\n\
               namedVersion: named.version,\n\
               namedDescription: named.description,\n\
               bareName: bare.name,\n\
               bareVersion: bare.version,\n\
               bareDescription: bare.description,\n\
             };\n\
           }\n\
         }\n";
    std::fs::write(bundle.path().join("entry.ts"), entry).expect("entry.ts should be written");

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    assert_eq!(
        result.get("namedName").and_then(Value::as_str),
        Some("my-plugin"),
        "a subclass-set `name` must be readable on the instance, got: {result}"
    );
    assert_eq!(
        result.get("namedVersion").and_then(Value::as_str),
        Some("1.2.3"),
        "a subclass-set `version` must be readable on the instance, got: {result}"
    );
    assert_eq!(
        result.get("namedDescription").and_then(Value::as_str),
        Some("a worked example plugin"),
        "a subclass-set `description` must be readable on the instance, got: {result}"
    );
    assert_eq!(
        result.get("bareName").and_then(Value::as_str),
        Some("unnamed plugin"),
        "a subclass that omits `name` must inherit the base default, got: {result}"
    );
    assert_eq!(
        result.get("bareVersion").and_then(Value::as_str),
        Some("0.0.0"),
        "a subclass that omits `version` must inherit the base default, got: {result}"
    );
    assert_eq!(
        result.get("bareDescription").and_then(Value::as_str),
        Some(""),
        "a subclass that omits `description` must inherit the base default, got: {result}"
    );
}

/// `RESERVED` names are not treated as path segments.
///
/// Accessing `this.srv.kanban.on` must yield the reserved-name handler, not a
/// dispatcher extending the path with `on` — so calling through it does not
/// produce a `tools/call` on a phantom `on` tool/noun/verb.
#[tokio::test]
async fn reserved_names_are_not_path_segments() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    write_plugin(
        bundle.path(),
        "const onHandler = this.srv.kanban.on;\n\
         globalThis.__result = { onType: typeof onHandler };",
    );

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    assert_eq!(
        result.get("onType").and_then(Value::as_str),
        Some("function"),
        "a RESERVED name must resolve to the reserved handler, got: {result}"
    );
    assert!(
        dispatcher.calls().is_empty(),
        "accessing a RESERVED name must not produce a tools/call"
    );
}

/// The SDK's `scopeId` / `targetId` moniker helpers resolve the id half of a
/// `"<entityType>:<id>"` moniker out of a {@link CommandContext}.
///
/// This is genuine runtime unit coverage of the moniker logic the command
/// plugins delegate to the SDK for: it imports `scopeId` / `targetId` from
/// `@swissarmyhammer/plugin` into a real isolate, calls them against crafted
/// contexts, and returns the results so the test can assert each branch:
///
///   * **leaf-last scan** — with two `task:` monikers in the chain, the NEAREST
///     (last) one wins;
///   * **type-prefix match** — the helper returns the id half of the matching
///     type and skips monikers of other types;
///   * **target type mismatch** — `targetId` for a different type than the
///     target moniker carries returns `undefined`;
///   * **missing scope / target** — an empty / absent chain or target returns
///     `undefined`.
#[tokio::test]
async fn scope_and_target_id_resolve_monikers() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    // Import the helpers and probe every branch. `undefined` does not survive
    // JSON, so each undefined-returning case is mapped to the sentinel string
    // "UNDEFINED" before it is recorded on `__result`.
    let entry = "import { Plugin, scopeId, targetId } from '@swissarmyhammer/plugin';\n\
         const u = (v: string | undefined): string => (v === undefined ? 'UNDEFINED' : v);\n\
         export default class MonikerProbe extends Plugin {\n\
           async load(): Promise<unknown> {\n\
             // Leaf-last: nearest (last) task moniker wins over the earlier one,\n\
             // and a board moniker in between is skipped on a `task` lookup.\n\
             const leafLast = scopeId(\n\
               { scope_chain: ['task:first', 'board:01A', 'task:nearest'] },\n\
               'task',\n\
             );\n\
             // Type-prefix match: only the `board` moniker is returned for a\n\
             // `board` lookup even though a `task` moniker is nearer the leaf.\n\
             const prefixMatch = scopeId(\n\
               { scope_chain: ['board:42', 'task:99'] },\n\
               'board',\n\
             );\n\
             // No matching type in scope → undefined.\n\
             const scopeMismatch = scopeId({ scope_chain: ['tag:x'] }, 'task');\n\
             // Absent scope chain → undefined.\n\
             const scopeMissing = scopeId({}, 'task');\n\
             // Target of the matching type → the id half.\n\
             const targetMatch = targetId({ target: 'column:done' }, 'column');\n\
             // Target of a different type → undefined.\n\
             const targetMismatch = targetId({ target: 'task:7' }, 'column');\n\
             // No target → undefined.\n\
             const targetMissing = targetId({}, 'column');\n\
             return {\n\
               leafLast: u(leafLast),\n\
               prefixMatch: u(prefixMatch),\n\
               scopeMismatch: u(scopeMismatch),\n\
               scopeMissing: u(scopeMissing),\n\
               targetMatch: u(targetMatch),\n\
               targetMismatch: u(targetMismatch),\n\
               targetMissing: u(targetMissing),\n\
             };\n\
           }\n\
         }\n";
    std::fs::write(bundle.path().join("entry.ts"), entry).expect("entry.ts should be written");

    let dispatcher = Arc::new(RecordingDispatcher::default());
    let runtime = PluginRuntime::new(config_with(dispatcher.clone())).expect("runtime starts");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "entry.ts", PluginLifecycle::Load),
    )
    .await
    .expect("loading the plugin should not hang")
    .expect("the plugin's load should succeed");

    let field = |key: &str| result.get(key).and_then(Value::as_str).unwrap_or("");
    assert_eq!(
        field("leafLast"),
        "nearest",
        "scopeId must scan leaf-last so the nearest matching moniker wins, got: {result}"
    );
    assert_eq!(
        field("prefixMatch"),
        "42",
        "scopeId must return the id half of the type-matching moniker, got: {result}"
    );
    assert_eq!(
        field("scopeMismatch"),
        "UNDEFINED",
        "scopeId must return undefined when no moniker of the type is in scope, got: {result}"
    );
    assert_eq!(
        field("scopeMissing"),
        "UNDEFINED",
        "scopeId must return undefined for an absent scope chain, got: {result}"
    );
    assert_eq!(
        field("targetMatch"),
        "done",
        "targetId must return the id half of a same-type target moniker, got: {result}"
    );
    assert_eq!(
        field("targetMismatch"),
        "UNDEFINED",
        "targetId must return undefined when the target is a different type, got: {result}"
    );
    assert_eq!(
        field("targetMissing"),
        "UNDEFINED",
        "targetId must return undefined when there is no target, got: {result}"
    );
}
