//! JavaScript expression engine for SwissArmyHammer
//!
//! This crate provides process-global JavaScript state management using rquickjs
//! (QuickJS-NG). It replaces the CEL expression engine with a full JavaScript
//! runtime.
//!
//! # Architecture
//!
//! - **Dedicated Worker Thread**: A single OS thread owns the rquickjs Runtime+Context
//! - **Channel Communication**: Other threads/tasks send requests via mpsc channels
//! - **Process-Global State**: Single Runtime shared by all components
//! - **In-Memory Only**: No persistence, state is lost when process terminates
//! - **Auto-Capture**: After `set()`, new/modified JS globals are captured back into tracked context
//!
//! # Example
//!
//! ```rust,no_run
//! use swissarmyhammer_js::JsState;
//!
//! # async fn example() {
//! let state = JsState::global();
//!
//! // Set a variable (evaluates JS expression, stores result)
//! let result = state.set("x", "10 + 5").await;
//! assert!(result.is_ok());
//!
//! // Get/evaluate an expression
//! let result = state.get("x * 2").await;
//! assert!(result.is_ok());
//! # }
//! ```

pub mod bridge;
pub mod context;
pub mod error;
pub mod expression;
pub mod processor;
pub mod schema;

pub use context::JsContext;
pub use error::JsError;
pub use processor::JsOperationProcessor;

// Re-export operations framework traits
pub use swissarmyhammer_operations::{Execute, Operation, OperationProcessor};

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Request types sent to the JS worker thread
enum JsRequest {
    Set {
        name: String,
        expression: String,
        reply: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    Get {
        expression: String,
        reply: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    GetAllVariables {
        reply: oneshot::Sender<Result<HashMap<String, serde_json::Value>, String>>,
    },
    SetModuleBase {
        path: PathBuf,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

/// Module resolver that sandboxes imports to a base directory.
///
/// Resolves module specifiers (e.g. `"helpers/text.js"`) relative to a
/// configured base path. Rejects any resolution that would escape the
/// base directory via `..` traversal.
struct SandboxedResolver {
    base: PathBuf,
}

impl SandboxedResolver {
    fn new(base: PathBuf) -> Self {
        Self { base }
    }
}

impl rquickjs::loader::Resolver for SandboxedResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &rquickjs::Ctx<'js>,
        _base: &str,
        name: &str,
    ) -> rquickjs::Result<String> {
        // Reject absolute paths and obvious traversal
        if name.starts_with('/') || name.starts_with('\\') {
            return Err(rquickjs::Error::new_resolving(_base, name));
        }

        let requested = self.base.join(name);

        // Canonicalize to resolve any .. or symlinks
        let canonical = requested
            .canonicalize()
            .map_err(|_| rquickjs::Error::new_resolving(_base, name))?;

        // Verify the resolved path is within the sandbox
        let base_canonical = self
            .base
            .canonicalize()
            .map_err(|_| rquickjs::Error::new_resolving(_base, name))?;

        if !canonical.starts_with(&base_canonical) {
            return Err(rquickjs::Error::new_resolving(_base, name));
        }

        Ok(canonical.to_string_lossy().to_string())
    }
}

/// Handle to the JS worker thread
struct JsWorker {
    sender: std::sync::mpsc::Sender<JsRequest>,
}

impl JsWorker {
    /// Spawn the dedicated JS worker thread and return a handle
    fn spawn() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<JsRequest>();

        std::thread::Builder::new()
            .name("js-runtime".to_string())
            .spawn(move || {
                Self::worker_loop(rx);
            })
            .expect("Failed to spawn JS runtime thread");

        Self { sender: tx }
    }

    /// The worker loop that owns the rquickjs Runtime+Context
    fn worker_loop(rx: std::sync::mpsc::Receiver<JsRequest>) {
        use rquickjs::{CatchResultExt, CaughtError, Context, Object, Runtime};

        // Create runtime with resource limits
        let rt = Runtime::new().expect("Failed to create JS runtime");
        rt.set_memory_limit(10 * 1024 * 1024); // 10 MB
        rt.set_max_stack_size(512 * 1024); // 512 KB

        let ctx = Context::full(&rt).expect("Failed to create JS context");

        /// Drain all pending microtasks/Promise jobs from the runtime.
        fn drain_pending_jobs(rt: &Runtime) {
            loop {
                match rt.execute_pending_job() {
                    Ok(false) => break,
                    Ok(true) => continue,
                    Err(e) => {
                        tracing::warn!("error executing pending JS job: {:?}", e);
                        break;
                    }
                }
            }
        }

        // Tracked variables (mirrored from JS globals)
        let mut variables: HashMap<String, serde_json::Value> = HashMap::new();

        // Inject env vars
        ctx.with(|ctx| {
            let globals = ctx.globals();

            // env object (flat)
            if let Ok(env_obj) = Object::new(ctx.clone()) {
                for (key, value) in std::env::vars() {
                    let _ = env_obj.set(key.as_str(), value.as_str());
                }
                let _ = globals.set("env", env_obj);
            }

            // process.env object (Node-style)
            if let Ok(process_obj) = Object::new(ctx.clone()) {
                if let Ok(process_env_obj) = Object::new(ctx.clone()) {
                    for (key, value) in std::env::vars() {
                        let _ = process_env_obj.set(key.as_str(), value.as_str());
                    }
                    let _ = process_obj.set("env", process_env_obj);
                }
                let _ = globals.set("process", process_obj);
            }
        });

        // Process requests until all senders are dropped
        while let Ok(request) = rx.recv() {
            match request {
                JsRequest::Set {
                    name,
                    expression,
                    reply,
                } => {
                    let result = ctx.with(|ctx| {
                        // Evaluate the expression
                        let eval_result: rquickjs::Value = ctx
                            .eval(expression.as_bytes())
                            .catch(&ctx)
                            .map_err(|e| match e {
                                CaughtError::Exception(ex) => {
                                    format!("JS error: {}", ex)
                                }
                                CaughtError::Value(v) => {
                                    let s: std::result::Result<String, _> = v.get();
                                    format!(
                                        "JS threw: {}",
                                        s.unwrap_or_else(|_| "unknown".to_string())
                                    )
                                }
                                CaughtError::Error(e) => format!("Error: {}", e),
                            })?;

                        // Convert result to JSON
                        let json_result = bridge::js_to_json(&ctx, eval_result.clone())
                            .map_err(|e| e.to_string())?;

                        // Assign the result to the named global
                        let globals = ctx.globals();
                        globals
                            .set(name.as_str(), eval_result)
                            .map_err(|e| format!("Failed to set global '{}': {}", name, e))?;

                        Ok(json_result)
                    });

                    // Drain pending jobs (Promise resolution, microtasks)
                    drain_pending_jobs(&rt);

                    if let Ok(ref json_result) = result {
                        variables.insert(name, json_result.clone());

                        // Auto-capture: scan globals for new/modified user variables
                        let captured: Vec<(String, serde_json::Value)> = ctx.with(|ctx| {
                            let globals = ctx.globals();
                            let mut result = Vec::new();
                            // Use OwnedKey iteration to get property names
                            let key_names: Vec<String> =
                                globals.keys::<String>().flatten().collect();
                            for key in key_names {
                                if bridge::is_builtin(&key) {
                                    continue;
                                }
                                let val: std::result::Result<rquickjs::Value, _> =
                                    globals.get(key.clone());
                                if let Ok(value) = val {
                                    if value.is_function() || value.is_constructor() {
                                        continue;
                                    }
                                    if let Ok(json_val) = bridge::js_to_json(&ctx, value) {
                                        result.push((key, json_val));
                                    }
                                }
                            }
                            result
                        });
                        for (k, v) in captured {
                            variables.insert(k, v);
                        }
                    }

                    let _ = reply.send(result);
                }

                JsRequest::Get { expression, reply } => {
                    let result = ctx.with(|ctx| {
                        let eval_result: rquickjs::Value = ctx
                            .eval(expression.as_bytes())
                            .catch(&ctx)
                            .map_err(|e| match e {
                                CaughtError::Exception(ex) => {
                                    format!("JS error: {}", ex)
                                }
                                CaughtError::Value(v) => {
                                    let s: std::result::Result<String, _> = v.get();
                                    format!(
                                        "JS threw: {}",
                                        s.unwrap_or_else(|_| "unknown".to_string())
                                    )
                                }
                                CaughtError::Error(e) => format!("Error: {}", e),
                            })?;

                        bridge::js_to_json(&ctx, eval_result).map_err(|e| e.to_string())
                    });

                    // Drain pending jobs (Promise resolution, microtasks)
                    drain_pending_jobs(&rt);

                    let _ = reply.send(result);
                }

                JsRequest::GetAllVariables { reply } => {
                    let _ = reply.send(Ok(variables.clone()));
                }

                JsRequest::SetModuleBase { path, reply } => {
                    let resolver = SandboxedResolver::new(path);
                    let loader = rquickjs::loader::ScriptLoader::default();
                    rt.set_loader(resolver, loader);
                    let _ = reply.send(Ok(()));
                }
            }
        }

        tracing::debug!("JS worker thread shutting down");
    }
}

/// Process-global JS worker handle
static GLOBAL_JS_WORKER: Lazy<Mutex<JsWorker>> = Lazy::new(|| Mutex::new(JsWorker::spawn()));

/// JavaScript state manager providing async access to a process-global JS context
///
/// All operations are async because they communicate with the dedicated JS
/// worker thread via channels.
#[derive(Clone)]
pub struct JsState;

impl JsState {
    /// Get the global JS state instance
    pub fn global() -> Self {
        Self
    }

    /// Send a request to the worker and await the response
    async fn send_request<T>(
        &self,
        make_request: impl FnOnce(oneshot::Sender<Result<T, String>>) -> JsRequest,
    ) -> Result<T, String> {
        let (tx, rx) = oneshot::channel();
        let request = make_request(tx);

        {
            let worker = GLOBAL_JS_WORKER
                .lock()
                .map_err(|e| format!("Worker lock error: {}", e))?;
            worker
                .sender
                .send(request)
                .map_err(|_| "JS worker thread has stopped".to_string())?;
        }

        rx.await
            .map_err(|_| "JS worker did not respond".to_string())?
    }

    /// Evaluate a JS expression and store the result as a named variable.
    ///
    /// After storing the named variable, this scans all JS globals for
    /// new/modified user variables and merges them into the tracked context.
    ///
    /// # Arguments
    ///
    /// * `name` - Variable name to store the result under
    /// * `expression` - JavaScript expression to evaluate
    ///
    /// # Returns
    ///
    /// The evaluated result as a JSON value, or an error string
    pub async fn set(&self, name: &str, expression: &str) -> Result<serde_json::Value, String> {
        let name = name.to_string();
        let expression = expression.to_string();
        self.send_request(|reply| JsRequest::Set {
            name,
            expression,
            reply,
        })
        .await
    }

    /// Evaluate a JS expression in the current context without storing it
    ///
    /// # Arguments
    ///
    /// * `expression` - JavaScript expression to evaluate
    ///
    /// # Returns
    ///
    /// The evaluated result as a JSON value, or an error string
    pub async fn get(&self, expression: &str) -> Result<serde_json::Value, String> {
        let expression = expression.to_string();
        self.send_request(|reply| JsRequest::Get { expression, reply })
            .await
    }

    /// Configure the module base path for ES module imports.
    ///
    /// After calling this, JS code evaluated via `set()` or `get()` can use
    /// dynamic `import()` to load `.js` modules from the specified directory.
    /// Imports are sandboxed — path traversal outside the base directory is rejected.
    pub async fn set_module_base(&self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let (tx, rx) = oneshot::channel();
        let request = JsRequest::SetModuleBase { path, reply: tx };

        {
            let worker = GLOBAL_JS_WORKER
                .lock()
                .map_err(|e| format!("Worker lock error: {}", e))?;
            worker
                .sender
                .send(request)
                .map_err(|_| "JS worker thread has stopped".to_string())?;
        }

        rx.await
            .map_err(|_| "JS worker did not respond".to_string())?
    }

    /// Get all tracked variables as a HashMap
    ///
    /// Used by workflow context stacking to copy global variables
    /// into a fresh evaluation context.
    pub async fn get_all_variables(&self) -> Result<HashMap<String, serde_json::Value>, String> {
        self.send_request(|reply| JsRequest::GetAllVariables { reply })
            .await
    }
}

impl Default for JsState {
    fn default() -> Self {
        Self::global()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let state = JsState::global();

        let result = state.set("test_var", "42").await;
        assert!(result.is_ok(), "set failed: {:?}", result);
        assert_eq!(result.unwrap(), serde_json::json!(42));

        let result = state.get("test_var").await;
        assert!(result.is_ok(), "get failed: {:?}", result);
        assert_eq!(result.unwrap(), serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_expression_evaluation() {
        let state = JsState::global();

        let result = state.set("calc", "10 + 5 * 2").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(20));

        let result = state.get("calc * 2").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(40));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let state = JsState::global();

        // Syntax error
        let result = state.get("2 +").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("error") || err.contains("Error"),
            "Got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_boolean_values() {
        let state = JsState::global();

        let result = state.set("flag_on", "true").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(true));

        let result = state.set("flag_off", "false").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(false));

        let result = state.get("flag_on && !flag_off").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(true));
    }

    #[tokio::test]
    async fn test_string_values() {
        let state = JsState::global();

        let result = state.set("greeting", "'Hello World'").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("Hello World"));

        let result = state.get("greeting.includes('Hello')").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(true));
    }

    #[tokio::test]
    async fn test_object_values() {
        let state = JsState::global();

        let result = state.set("obj", "({name: 'test', count: 42})").await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["name"], "test");
        assert_eq!(val["count"], 42);
    }

    #[tokio::test]
    async fn test_array_values() {
        let state = JsState::global();

        let result = state.set("arr", "[1, 2, 3]").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!([1, 2, 3]));
    }

    #[tokio::test]
    async fn test_env_vars_accessible() {
        let state = JsState::global();

        // env should be an object
        let result = state.get("typeof env").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("object"));

        // process.env should also work
        let result = state.get("typeof process.env").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("object"));
    }

    #[tokio::test]
    async fn test_auto_capture_globals() {
        let state = JsState::global();

        // Set creates a global and auto-captures
        let _ = state.set("capture_x", "10").await;

        // A script that creates side-effect variables during set
        let _ = state
            .set(
                "capture_y",
                "(function() { globalThis.side_var = 99; return 20; })()",
            )
            .await;

        let vars = state.get_all_variables().await.unwrap();
        assert!(vars.contains_key("capture_x"));
        assert!(vars.contains_key("capture_y"));
        assert!(
            vars.contains_key("side_var"),
            "side_var should have been auto-captured"
        );
        assert_eq!(vars["side_var"], serde_json::json!(99));
    }

    #[tokio::test]
    async fn test_get_all_variables() {
        let state = JsState::global();

        let _ = state.set("var_a", "100").await;
        let _ = state.set("var_b", "true").await;

        let vars = state.get_all_variables().await.unwrap();
        assert!(vars.contains_key("var_a"));
        assert!(vars.contains_key("var_b"));
        assert_eq!(vars["var_a"], serde_json::json!(100));
        assert_eq!(vars["var_b"], serde_json::json!(true));
    }

    #[tokio::test]
    async fn test_null_value() {
        let state = JsState::global();

        let result = state.set("nothing", "null").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_undefined_reference_returns_error() {
        let state = JsState::global();

        let result = state.get("totally_undefined_var_xyz_123").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_promise_resolve_chain() {
        let state = JsState::global();

        // Promise.resolve sets a global via .then — requires job draining
        let _ = state
            .set(
                "promise_test",
                "(function() { Promise.resolve(42).then(v => { globalThis.promise_result = v; }); return 'started'; })()",
            )
            .await;

        let result = state.get("promise_result").await;
        assert!(result.is_ok(), "promise_result should exist: {:?}", result);
        assert_eq!(result.unwrap(), serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_promise_chain_multiple_then() {
        let state = JsState::global();

        let _ = state
            .set(
                "chain_test",
                "(function() { Promise.resolve(10).then(v => v * 2).then(v => { globalThis.chain_result = v; }); return 'ok'; })()",
            )
            .await;

        let result = state.get("chain_result").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(20));
    }

    #[tokio::test]
    async fn test_module_import_and_sandbox() {
        let state = JsState::global();
        let tmp = tempfile::TempDir::new().unwrap();

        // Create a helper module inside the sandbox
        let lib_dir = tmp.path().join("sandbox").join("helpers");
        std::fs::create_dir_all(&lib_dir).unwrap();
        std::fs::write(
            lib_dir.join("math.js"),
            "export function double(x) { return x * 2; }",
        )
        .unwrap();

        // Create a file outside the sandbox
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.js"), "export const x = 'secret';").unwrap();

        // Set module base to the sandbox directory
        let base = tmp.path().join("sandbox");
        let result = state.set_module_base(&base).await;
        assert!(result.is_ok(), "set_module_base failed: {:?}", result);

        // Test 1: Dynamic import from within the sandbox should work
        let _ = state
            .set(
                "mod_import_test",
                "(function() { import('helpers/math.js').then(m => { globalThis.mod_import_result = m.double(21); }).catch(e => { globalThis.mod_import_err = e.message; }); return 'started'; })()",
            )
            .await;

        let result = state.get("mod_import_result").await;
        assert!(result.is_ok(), "mod_import_result should exist: {:?}", result);
        assert_eq!(result.unwrap(), serde_json::json!(42));

        // Test 2: Import from outside the sandbox via path traversal should be rejected
        let _ = state
            .set(
                "escape_test",
                "(function() { import('../outside/secret.js').then(m => { globalThis.escape_ok = m.x; }).catch(e => { globalThis.escape_err = e.message || 'failed'; }); return 'tried'; })()",
            )
            .await;

        let error_result = state.get("escape_err").await;
        assert!(
            error_result.is_ok(),
            "escape_err should be set from catch: {:?}",
            error_result
        );
        // escape_ok should NOT exist (import should have failed)
        let ok_result = state.get("escape_ok").await;
        assert!(ok_result.is_err(), "escape_ok should not exist — import should have been rejected");
    }
}
