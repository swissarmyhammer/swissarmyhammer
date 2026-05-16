//! JavaScript expression engine for SwissArmyHammer
//!
//! This crate provides process-global JavaScript state management using
//! `deno_core` (V8). It replaces the CEL expression engine with a full
//! JavaScript runtime.
//!
//! # Architecture
//!
//! - **Dedicated Worker Thread**: A single OS thread owns the `deno_core::JsRuntime`
//! - **Channel Communication**: Other threads/tasks send requests via mpsc channels
//! - **Process-Global State**: Single runtime shared by all components
//! - **In-Memory Only**: No persistence, state is lost when process terminates
//! - **Auto-Capture**: After `set()`, new/modified JS globals are captured back into tracked context
//!
//! The worker-thread + channel model is mandatory: `deno_core::JsRuntime` wraps
//! a V8 isolate and is `!Send`, so it must be owned by exactly one thread and
//! driven via message passing.
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
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;
use tokio::sync::oneshot;

use deno_core::v8;
use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions,
};
use deno_error::JsErrorBox;

/// Initial V8 heap size for the JS isolate (1 MB).
const HEAP_INITIAL_BYTES: usize = 1024 * 1024;

/// Maximum V8 heap size for the JS isolate (10 MB).
///
/// Mirrors the prior rquickjs `set_memory_limit(10 MB)`. V8 enforces this as a
/// hard ceiling; allocations beyond it abort the offending script.
const HEAP_MAX_BYTES: usize = 10 * 1024 * 1024;

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

/// Module loader that sandboxes ES module imports to a base directory.
///
/// `deno_core` fixes the [`ModuleLoader`] at runtime-construction time, so this
/// loader holds the sandbox base behind a [`RefCell`]: `SetModuleBase` updates
/// the cell in place rather than swapping the loader. Resolution resolves each
/// specifier — relative specifiers against the importing module's directory,
/// the entry module against the configured base — canonicalizes the result,
/// and rejects any path that escapes the base directory (absolute paths and
/// `..` traversal). There is no `file://` fast-path: a `file://` specifier is
/// containment-checked like every other path.
struct SandboxedModuleLoader {
    /// The configured sandbox base directory, or `None` until set.
    ///
    /// `deno_core` calls the loader only from the single worker thread, so a
    /// plain `RefCell` (no locking) is sufficient and `!Sync` is acceptable.
    base: RefCell<Option<PathBuf>>,
}

impl SandboxedModuleLoader {
    /// Create a loader with no configured base directory.
    ///
    /// Until [`SandboxedModuleLoader::set_base`] is called, every import is
    /// rejected because there is no directory to resolve specifiers against.
    fn new() -> Self {
        Self {
            base: RefCell::new(None),
        }
    }

    /// Update the sandbox base directory for subsequent imports.
    fn set_base(&self, path: PathBuf) {
        *self.base.borrow_mut() = Some(path);
    }

    /// Resolve a specifier to a sandboxed `file://` URL.
    ///
    /// Every specifier — bare, relative, or an explicit `file://` URL — is
    /// resolved to a concrete path, canonicalized to collapse `..` segments and
    /// symlinks, and verified to be contained within the canonicalized base
    /// directory. There is no fast-path: a `file://` specifier is accepted only
    /// if it resolves inside the sandbox, exactly like every other path.
    ///
    /// Relative specifiers resolve against the importing module's directory
    /// (derived from `referrer`) when the referrer is a `file://` URL inside
    /// the sandbox; the configured base is used only for the entry module,
    /// where no in-sandbox referrer is available. This matches standard ES
    /// module resolution semantics for multi-directory module trees.
    fn resolve_sandboxed(
        &self,
        specifier: &str,
        referrer: &str,
    ) -> Result<ModuleSpecifier, String> {
        let base = self
            .base
            .borrow()
            .clone()
            .ok_or_else(|| "module base directory is not configured".to_string())?;
        let base_canonical = base
            .canonicalize()
            .map_err(|e| format!("cannot resolve module base: {e}"))?;

        // Reject absolute paths and obvious traversal markers up front.
        if specifier.starts_with('/') || specifier.starts_with('\\') {
            return Err(format!("absolute import path rejected: {specifier}"));
        }

        // Determine the directory the specifier resolves against. A `file://`
        // specifier is already an absolute path, so it has no resolution base;
        // otherwise resolve relative to the importing module's directory when
        // the referrer is an in-sandbox `file://` URL, falling back to the
        // configured base for the entry module.
        let requested = if specifier.starts_with("file://") {
            ModuleSpecifier::parse(specifier)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .ok_or_else(|| format!("invalid file:// import URL: {specifier}"))?
        } else {
            let resolution_dir = referrer_directory(referrer).unwrap_or_else(|| base.clone());
            resolution_dir.join(specifier)
        };

        // Canonicalize to collapse any `..` segments and resolve symlinks.
        let canonical = requested
            .canonicalize()
            .map_err(|e| format!("cannot resolve import '{specifier}': {e}"))?;

        // Verify the resolved path stays inside the sandbox.
        if !canonical.starts_with(&base_canonical) {
            return Err(format!(
                "import '{specifier}' escapes the sandbox base directory"
            ));
        }

        ModuleSpecifier::from_file_path(&canonical)
            .map_err(|_| format!("cannot build file URL for import '{specifier}'"))
    }
}

/// Extract the parent directory of an importing module from its referrer URL.
///
/// Returns the directory of the referring module when `referrer` is a parseable
/// `file://` URL pointing at a file; returns `None` for non-`file://` referrers
/// (e.g. the synthetic `<eval>` referrer used for top-level imports), so the
/// caller falls back to the configured sandbox base for the entry module.
fn referrer_directory(referrer: &str) -> Option<PathBuf> {
    if !referrer.starts_with("file://") {
        return None;
    }
    let path = ModuleSpecifier::parse(referrer).ok()?.to_file_path().ok()?;
    path.parent().map(PathBuf::from)
}

impl ModuleLoader for SandboxedModuleLoader {
    /// Resolve an import specifier, enforcing the base-directory sandbox.
    ///
    /// Every specifier — bare, relative, or an explicit `file://` URL — is
    /// routed through the same base-directory containment check: there is no
    /// escape hatch. Relative specifiers resolve against the importing module's
    /// directory (from `referrer`) when one is available, falling back to the
    /// configured base for the entry module. Any failure is surfaced as a
    /// `JsErrorBox`, which V8 reports to the importing script as a rejected
    /// promise.
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, deno_core::error::ModuleLoaderError> {
        self.resolve_sandboxed(specifier, referrer)
            .map_err(JsErrorBox::generic)
    }

    /// Load a previously resolved module from disk.
    ///
    /// The specifier has already passed [`SandboxedModuleLoader::resolve`], so
    /// it is guaranteed to be a `file://` URL inside the sandbox. The file is
    /// read synchronously and returned as a JavaScript [`ModuleSource`].
    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let path = match module_specifier.to_file_path() {
            Ok(p) => p,
            Err(()) => {
                return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(
                    "only file:// module URLs are supported",
                )));
            }
        };

        let code = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
                    "failed to read module {}: {e}",
                    path.display()
                ))));
            }
        };

        ModuleLoadResponse::Sync(Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleSourceCode::String(code.into()),
            module_specifier,
            None,
        )))
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

    /// The worker loop that owns the `deno_core::JsRuntime`.
    ///
    /// V8 is single-threaded and `JsRuntime` is `!Send`, so the runtime is
    /// created and used entirely within this thread. A current-thread Tokio
    /// runtime drives the V8 event loop (Promise jobs, dynamic imports).
    fn worker_loop(rx: std::sync::mpsc::Receiver<JsRequest>) {
        // The sandbox loader is shared with the runtime via `Rc`; `SetModuleBase`
        // mutates it in place rather than rebuilding the runtime.
        let module_loader = Rc::new(SandboxedModuleLoader::new());

        // Cap the V8 heap to mirror the prior rquickjs memory limit. V8 has no
        // embedder-facing stack-size knob equivalent to rquickjs'
        // `set_max_stack_size`; it manages its own call-stack guard internally.
        let create_params =
            v8::CreateParams::default().heap_limits(HEAP_INITIAL_BYTES, HEAP_MAX_BYTES);

        let mut runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(module_loader.clone()),
            create_params: Some(create_params),
            ..Default::default()
        });

        // A current-thread Tokio runtime to drive `run_event_loop`. Pending
        // Promise jobs and dynamic `import()` calls only progress while the
        // event loop is polled.
        let tokio_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build worker Tokio runtime");

        // Tracked variables (mirrored from JS globals)
        let mut variables: HashMap<String, serde_json::Value> = HashMap::new();

        // Inject the `env` and `process.env` globals before serving requests.
        inject_env(&mut runtime);

        // Process requests until all senders are dropped
        while let Ok(request) = rx.recv() {
            match request {
                JsRequest::Set {
                    name,
                    expression,
                    reply,
                } => {
                    let result = eval_and_store(&mut runtime, &tokio_rt, &name, &expression);

                    if let Ok(ref json_result) = result {
                        variables.insert(name, json_result.clone());

                        // Auto-capture: scan globals for new/modified user
                        // variables (e.g. side effects of the evaluated code).
                        for (k, v) in capture_user_globals(&mut runtime) {
                            variables.insert(k, v);
                        }
                    }

                    let _ = reply.send(result);
                }

                JsRequest::Get { expression, reply } => {
                    let result = eval_expression(&mut runtime, &tokio_rt, &expression);
                    let _ = reply.send(result);
                }

                JsRequest::GetAllVariables { reply } => {
                    let _ = reply.send(Ok(variables.clone()));
                }

                JsRequest::SetModuleBase { path, reply } => {
                    module_loader.set_base(path);
                    let _ = reply.send(Ok(()));
                }
            }
        }

        tracing::debug!("JS worker thread shutting down");
    }
}

/// Inject the `env` and `process.env` globals into the runtime.
///
/// Both globals expose the process environment as plain JS objects. The
/// environment map is serialized to a JSON string and reconstructed via
/// `JSON.parse`, so arbitrary key/value content is escaped safely and cannot
/// inject JavaScript syntax.
fn inject_env(runtime: &mut JsRuntime) {
    let env_map: serde_json::Map<String, serde_json::Value> = std::env::vars()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    let env_json = serde_json::to_string(&serde_json::Value::Object(env_map))
        .unwrap_or_else(|_| "{}".to_string());

    // `env_json` is itself JSON-encoded into a JS string literal so the data is
    // always inside a string and never parsed as code.
    let env_literal = serde_json::to_string(&env_json).unwrap_or_else(|_| "\"{}\"".to_string());

    let script = format!(
        "globalThis.env = JSON.parse({env_literal}); \
         globalThis.process = {{ env: JSON.parse({env_literal}) }};"
    );

    if let Err(e) = runtime.execute_script("<inject-env>", script) {
        tracing::warn!("failed to inject env globals: {e}");
    }
}

/// Drain pending Promise jobs / microtasks / dynamic imports.
///
/// `deno_core` advances Promise resolution and dynamic `import()` only while
/// the event loop is polled. Running it to completion here matches the prior
/// rquickjs `execute_pending_job` draining behavior.
fn drain_event_loop(runtime: &mut JsRuntime, tokio_rt: &tokio::runtime::Runtime) {
    if let Err(e) = tokio_rt.block_on(runtime.run_event_loop(Default::default())) {
        tracing::warn!("error draining JS event loop: {e}");
    }
}

/// Evaluate a JS expression and return its result as JSON.
///
/// The expression is run via `execute_script`; the resulting V8 value is
/// converted to JSON, then the event loop is drained so any Promise side
/// effects settle. Syntax errors and thrown exceptions are returned as `Err`.
fn eval_expression(
    runtime: &mut JsRuntime,
    tokio_rt: &tokio::runtime::Runtime,
    expression: &str,
) -> Result<serde_json::Value, String> {
    let global = runtime
        .execute_script("<eval>", expression.to_string())
        .map_err(|e| format!("JS error: {}", e.exception_message))?;

    let json = {
        deno_core::scope!(scope, runtime);
        let local = v8::Local::new(scope, global);
        bridge::v8_to_json(scope, local).map_err(|e| e.to_string())?
    };

    // Settle Promise jobs / dynamic imports triggered by the expression.
    drain_event_loop(runtime, tokio_rt);

    Ok(json)
}

/// Evaluate a JS expression, store the result as a named global, return JSON.
///
/// This is the engine side of [`JsState::set`]: the expression is evaluated,
/// the raw V8 result is assigned to `globalThis[name]` so later `get` calls can
/// read it, and the JSON form of the result is returned.
fn eval_and_store(
    runtime: &mut JsRuntime,
    tokio_rt: &tokio::runtime::Runtime,
    name: &str,
    expression: &str,
) -> Result<serde_json::Value, String> {
    let global = runtime
        .execute_script("<eval>", expression.to_string())
        .map_err(|e| format!("JS error: {}", e.exception_message))?;

    let json = {
        deno_core::scope!(scope, runtime);
        let local = v8::Local::new(scope, global);

        // Convert before assigning so a conversion failure is reported.
        let json = bridge::v8_to_json(scope, local).map_err(|e| e.to_string())?;

        // Assign the raw evaluated value to `globalThis[name]`.
        let global_obj = scope.get_current_context().global(scope);
        let key = v8::String::new(scope, name)
            .ok_or_else(|| format!("failed to allocate global name '{name}'"))?;
        global_obj.set(scope, key.into(), local);

        json
    };

    // Settle Promise jobs / dynamic imports triggered by the expression.
    drain_event_loop(runtime, tokio_rt);

    Ok(json)
}

/// Scan `globalThis` for user-defined variables and return them as JSON.
///
/// Builtin globals (see [`bridge::JS_BUILTINS`]) and function values are
/// skipped, mirroring the original auto-capture filter. Each remaining global
/// is converted to JSON via [`bridge::v8_to_json`].
fn capture_user_globals(runtime: &mut JsRuntime) -> Vec<(String, serde_json::Value)> {
    deno_core::scope!(scope, runtime);
    let global_obj = scope.get_current_context().global(scope);

    let mut captured = Vec::new();

    // Own enumerable property names of the global object.
    let Some(names) = global_obj.get_own_property_names(scope, Default::default()) else {
        return captured;
    };

    for i in 0..names.length() {
        let Some(key) = names.get_index(scope, i) else {
            continue;
        };
        let key_str = key.to_rust_string_lossy(scope);
        if bridge::is_builtin(&key_str) {
            continue;
        }

        let Some(value) = global_obj.get(scope, key) else {
            continue;
        };
        // Functions are not tracked as variables.
        if value.is_function() {
            continue;
        }
        if let Ok(json) = bridge::v8_to_json(scope, value) {
            captured.push((key_str, json));
        }
    }

    captured
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

    /// Serializes tests that call `set_module_base`.
    ///
    /// `JsState` is a process-global singleton backed by one shared worker
    /// thread, so `set_module_base` mutates state visible to every test. Tests
    /// that configure a module base must therefore not run concurrently with
    /// one another, or one test's sandbox base clobbers another's mid-import.
    /// Each such test holds this lock for its full body.
    static MODULE_BASE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

        // Promise.resolve sets a global via .then — requires event loop draining
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
    async fn test_default_trait() {
        let state = JsState;
        let result = state.get("1 + 1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(2));
    }

    #[tokio::test]
    async fn test_clone_trait() {
        let state = JsState::global();
        let cloned = state.clone();
        let result = cloned.get("2 + 3").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(5));
    }

    #[tokio::test]
    async fn test_thrown_value_not_exception() {
        let state = JsState::global();
        // throw a non-Error value to exercise the thrown-value error path
        let result = state
            .get("(function() { throw 'custom error string'; })()")
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("threw") || err.contains("error") || err.contains("Error"),
            "Expected thrown value error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_set_thrown_value_error() {
        let state = JsState::global();
        // Exercise the thrown-value error path during set
        let result = state
            .set("throw_test", "(function() { throw 'oops'; })()")
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("threw") || err.contains("oops") || err.contains("error"),
            "Expected thrown value error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_set_with_exception() {
        let state = JsState::global();
        // throw an Error object to exercise the exception error path
        let result = state
            .set("exc_test", "throw new Error('test exception')")
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("test exception") || err.contains("JS error"),
            "Expected exception error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_get_with_exception() {
        let state = JsState::global();
        let result = state.get("throw new TypeError('bad type')").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("bad type") || err.contains("JS error"),
            "Expected exception error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_set_syntax_error() {
        let state = JsState::global();
        // Syntax error exercises the error path
        let result = state.set("syn_err", "function {{{").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_undefined_value() {
        let state = JsState::global();
        // undefined in JS converts to null in JSON via bridge
        let result = state.set("undef_test", "undefined").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_set_function_value() {
        let state = JsState::global();
        // Functions are converted to null in JSON bridge
        let result = state.set("fn_test", "(function() {})").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_set_nested_object() {
        let state = JsState::global();
        let result = state.set("nested_obj", "({a: {b: {c: 42}}})").await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["a"]["b"]["c"], 42);
    }

    #[tokio::test]
    async fn test_set_float_value() {
        let state = JsState::global();
        let result = state.set("float_val", "2.72").await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!((val.as_f64().unwrap() - 2.72).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_set_empty_object() {
        let state = JsState::global();
        let result = state.set("empty_obj", "({})").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!({}));
    }

    #[tokio::test]
    async fn test_set_empty_array() {
        let state = JsState::global();
        let result = state.set("empty_arr", "[]").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_get_template_literal() {
        let state = JsState::global();
        let _ = state.set("tpl_name", "'World'").await;
        let result = state.get("`Hello ${tpl_name}`").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("Hello World"));
    }

    #[tokio::test]
    async fn test_auto_capture_skips_functions() {
        let state = JsState::global();
        // Define a function global — it should NOT appear in variables
        let _ = state
            .set(
                "capture_fn_test",
                "(function() { globalThis.myFunc = function() {}; return 1; })()",
            )
            .await;

        let vars = state.get_all_variables().await.unwrap();
        // myFunc should NOT be in the variables because functions are skipped
        assert!(
            !vars.contains_key("myFunc"),
            "Functions should not be auto-captured"
        );
    }

    #[tokio::test]
    async fn test_auto_capture_skips_builtins() {
        let state = JsState::global();
        let _ = state.set("builtin_check", "1").await;

        let vars = state.get_all_variables().await.unwrap();
        // Builtins should never appear in tracked variables
        assert!(!vars.contains_key("Object"));
        assert!(!vars.contains_key("Array"));
        assert!(!vars.contains_key("Math"));
        assert!(!vars.contains_key("JSON"));
        assert!(!vars.contains_key("env"));
        assert!(!vars.contains_key("process"));
    }

    #[tokio::test]
    async fn test_set_overwrites_previous_value() {
        let state = JsState::global();
        let _ = state.set("overwrite_me", "1").await;
        let _ = state.set("overwrite_me", "2").await;

        let result = state.get("overwrite_me").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(2));

        let vars = state.get_all_variables().await.unwrap();
        assert_eq!(vars["overwrite_me"], serde_json::json!(2));
    }

    #[tokio::test]
    async fn test_set_failed_does_not_update_variables() {
        let state = JsState::global();
        // Set a valid value first
        let _ = state.set("fail_track_test", "100").await;

        // Now try to set with invalid JS - should fail
        let result = state
            .set("fail_track_test", "throw new Error('nope')")
            .await;
        assert!(result.is_err());

        // The variables should still have the old value
        let vars = state.get_all_variables().await.unwrap();
        assert_eq!(vars["fail_track_test"], serde_json::json!(100));
    }

    #[tokio::test]
    async fn test_absolute_path_import_rejected() {
        let _guard = MODULE_BASE_TEST_LOCK.lock().await;
        let state = JsState::global();
        let tmp = tempfile::TempDir::new().unwrap();

        // Create a module inside the sandbox
        let sandbox = tmp.path().join("abs_sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::write(sandbox.join("ok.js"), "export const v = 1;").unwrap();

        // Set module base
        let _ = state.set_module_base(&sandbox).await;

        // Attempt to import an absolute path — should be rejected by the loader
        let _ = state
            .set(
                "abs_import_test",
                "(function() { import('/etc/passwd').then(m => { globalThis.abs_ok = true; }).catch(e => { globalThis.abs_err = e.message || 'blocked'; }); return 'tried'; })()",
            )
            .await;

        // The absolute path import should have been blocked
        let err_result = state.get("abs_err").await;
        assert!(
            err_result.is_ok(),
            "abs_err should be set from catch: {:?}",
            err_result
        );
        let ok_result = state.get("abs_ok").await;
        assert!(
            ok_result.is_err(),
            "abs_ok should not exist — absolute import should have been rejected"
        );
    }

    #[tokio::test]
    async fn test_backslash_path_import_rejected() {
        let _guard = MODULE_BASE_TEST_LOCK.lock().await;
        let state = JsState::global();
        let tmp = tempfile::TempDir::new().unwrap();

        let sandbox = tmp.path().join("bs_sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();

        // Create a real file whose name literally begins with a backslash and
        // lives INSIDE the sandbox. Because the file exists and is contained,
        // `canonicalize()` and the containment check would both succeed — so
        // the ONLY thing that can reject this import is the up-front
        // backslash-prefix check in `resolve_sandboxed`. This proves that
        // check, not a downstream `canonicalize()` failure, did the rejecting.
        //
        // The filename is pushed as a single explicit path component so the
        // leading backslash is treated as part of the name, not a separator.
        let mut backslash_file = sandbox.clone();
        backslash_file.push(std::ffi::OsStr::new("\\something.js"));
        std::fs::write(&backslash_file, "export const v = 1;").unwrap();
        assert!(
            backslash_file.exists(),
            "backslash-named file should exist inside the sandbox"
        );

        let _ = state.set_module_base(&sandbox).await;

        // Attempt to import the backslash-prefixed specifier.
        let _ = state
            .set(
                "bs_import_test",
                r#"(function() { import('\\something.js').then(m => { globalThis.bs_ok = true; }).catch(e => { globalThis.bs_err = e.message || 'blocked'; }); return 'tried'; })()"#,
            )
            .await;

        // The import must be rejected, and rejected specifically by the
        // up-front absolute/backslash-prefix check.
        let err_result = state.get("bs_err").await;
        assert!(
            err_result.is_ok(),
            "bs_err should be set from catch: {:?}",
            err_result
        );
        let err_msg = err_result.unwrap();
        let err_text = err_msg.as_str().unwrap_or_default();
        assert!(
            err_text.contains("absolute import path rejected"),
            "rejection must come from the up-front backslash-prefix check, got: {err_text}"
        );

        // The backslash import must not have loaded the (existing, contained) file.
        let ok_result = state.get("bs_ok").await;
        assert!(
            ok_result.is_err(),
            "bs_ok should not exist — backslash specifier must be rejected up front"
        );
    }

    #[tokio::test]
    async fn test_file_url_import_escaping_base_rejected() {
        let _guard = MODULE_BASE_TEST_LOCK.lock().await;
        let state = JsState::global();
        let tmp = tempfile::TempDir::new().unwrap();

        // Create a sandbox directory with a legitimate module.
        let sandbox = tmp.path().join("file_url_sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::write(sandbox.join("ok.js"), "export const v = 1;").unwrap();

        // Create a file OUTSIDE the sandbox that must never be reachable.
        let outside = tmp.path().join("file_url_outside");
        std::fs::create_dir_all(&outside).unwrap();
        let secret = outside.join("secret.js");
        std::fs::write(&secret, "export const leaked = 'secret';").unwrap();

        let _ = state.set_module_base(&sandbox).await;

        // Build an absolute `file://` URL pointing at the outside file and
        // attempt to import it directly. The loader must reject it because the
        // resolved path is not contained within the sandbox base. The URL is
        // JSON-encoded into a JS string literal so the path is passed verbatim.
        let secret_url = ModuleSpecifier::from_file_path(&secret).unwrap();
        let url_literal = serde_json::to_string(secret_url.as_str()).unwrap();
        let import_src = format!(
            "(function() {{ import({url_literal}).then(m => {{ globalThis.file_url_ok = m.leaked; }}).catch(e => {{ globalThis.file_url_err = e.message || 'blocked'; }}); return 'tried'; }})()"
        );
        let _ = state.set("file_url_import_test", &import_src).await;

        // The `file://` import escaping the base must have been blocked.
        let err_result = state.get("file_url_err").await;
        assert!(
            err_result.is_ok(),
            "file_url_err should be set from catch: {:?}",
            err_result
        );
        let ok_result = state.get("file_url_ok").await;
        assert!(
            ok_result.is_err(),
            "file_url_ok should not exist — a file:// URL outside the base must be rejected"
        );
    }

    #[tokio::test]
    async fn test_nested_relative_import_resolves_against_importer() {
        let _guard = MODULE_BASE_TEST_LOCK.lock().await;
        let state = JsState::global();
        let tmp = tempfile::TempDir::new().unwrap();

        // Build a multi-directory module tree inside the sandbox:
        //   <base>/helpers/math.js   imports ./util.js
        //   <base>/helpers/util.js   is the intended sibling
        //   <base>/util.js           is a decoy with a different value
        let base = tmp.path().join("nested_sandbox");
        let helpers = base.join("helpers");
        std::fs::create_dir_all(&helpers).unwrap();
        std::fs::write(
            helpers.join("util.js"),
            "export const TAG = 'helpers-util';",
        )
        .unwrap();
        std::fs::write(base.join("util.js"), "export const TAG = 'base-util';").unwrap();
        std::fs::write(
            helpers.join("math.js"),
            "import { TAG } from './util.js'; export const tag = TAG;",
        )
        .unwrap();

        let result = state.set_module_base(&base).await;
        assert!(result.is_ok(), "set_module_base failed: {:?}", result);

        // Import the entry module; its `./util.js` must resolve to the sibling
        // inside <base>/helpers, NOT the decoy at <base>/util.js.
        let _ = state
            .set(
                "nested_import_test",
                "(function() { import('helpers/math.js').then(m => { globalThis.nested_tag = m.tag; }).catch(e => { globalThis.nested_err = e.message; }); return 'started'; })()",
            )
            .await;

        let err = state.get("nested_err").await;
        assert!(
            err.is_err(),
            "nested_err should not be set — import should succeed: {:?}",
            err
        );
        let tag = state.get("nested_tag").await;
        assert!(tag.is_ok(), "nested_tag should exist: {:?}", tag);
        assert_eq!(
            tag.unwrap(),
            serde_json::json!("helpers-util"),
            "nested relative import must resolve against the importing module's directory"
        );
    }

    #[tokio::test]
    async fn test_json_stringify_returns_none_for_symbol() {
        let state = JsState::global();
        // Symbol() cannot be JSON.stringified — should return null
        let result = state.set("sym_test", "Symbol('test')").await;
        // Symbols stringify to `undefined`, which the bridge maps to null.
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_module_import_and_sandbox() {
        let _guard = MODULE_BASE_TEST_LOCK.lock().await;
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
        assert!(
            result.is_ok(),
            "mod_import_result should exist: {:?}",
            result
        );
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
        assert!(
            ok_result.is_err(),
            "escape_ok should not exist — import should have been rejected"
        );
    }
}
