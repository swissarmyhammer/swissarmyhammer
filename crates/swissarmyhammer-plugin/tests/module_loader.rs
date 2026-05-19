//! Integration tests for the plugin module loader.
//!
//! These tests exercise [`PluginRuntime::call_plugin_lifecycle`] against *real*
//! plugin bundles laid out in temporary directories. They cover the three
//! import kinds the loader must handle:
//!
//! - **Relative imports** load and transpile a sibling module on disk, and an
//!   import that escapes the plugin's bundle directory is rejected.
//! - **Bare imports** (`lodash`) fail with a clear, host-specific error.
//! - **`@swissarmyhammer/*` imports** resolve to host-provided virtual modules.
//!
//! Every runtime interaction is wrapped in a timeout so a wedged isolate fails
//! the test fast instead of hanging CI.

use std::time::Duration;

use swissarmyhammer_plugin::{PluginRuntime, RuntimeConfig};

/// A generous upper bound on any single runtime interaction.
///
/// The runtime already bounds its own command waits, but wrapping the test
/// awaits as well guarantees the test process itself cannot hang.
const TIMEOUT: Duration = Duration::from_secs(20);

/// A relative import of a sibling module loads, transpiles, and is usable.
///
/// The plugin entry imports `./helper.ts`; the helper exports a function whose
/// result the entry's `activate` export returns. Observing that result proves
/// the helper was resolved, read from disk, transpiled, and linked.
#[tokio::test]
async fn relative_import_inside_bundle_loads_and_runs() {
    let bundle = tempfile::TempDir::new().expect("temp dir should be created");

    std::fs::write(
        bundle.path().join("helper.ts"),
        "export function answer(): number { const n: number = 42; return n; }",
    )
    .expect("helper.ts should be written");
    std::fs::write(
        bundle.path().join("index.ts"),
        "import { answer } from './helper.ts';\n\
         export function activate(): number { return answer(); }",
    )
    .expect("index.ts should be written");

    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "index.ts", "activate"),
    )
    .await
    .expect("loading a multi-file plugin should not hang")
    .expect("a relative import inside the bundle should resolve and run");

    assert_eq!(
        result,
        serde_json::json!(42),
        "the helper's export should be reachable from the entry module"
    );
}

/// A relative import escaping the bundle directory is rejected.
///
/// The plugin entry imports `../outside.ts`, a file that lives *above* the
/// bundle root. The loader's canonicalize-and-contain check must reject it, so
/// the load fails rather than reading a file outside the plugin's sandbox.
#[tokio::test]
async fn relative_import_escaping_bundle_is_rejected() {
    let root = tempfile::TempDir::new().expect("temp dir should be created");
    let bundle = root.path().join("bundle");
    std::fs::create_dir_all(&bundle).expect("bundle dir should be created");

    // `outside.ts` sits in the parent of the bundle directory — out of bounds.
    std::fs::write(
        root.path().join("outside.ts"),
        "export const secret: string = 'leaked';",
    )
    .expect("outside.ts should be written");
    std::fs::write(
        bundle.join("index.ts"),
        "import { secret } from '../outside.ts';\n\
         export function activate(): string { return secret; }",
    )
    .expect("index.ts should be written");

    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(&bundle, "index.ts", "activate"),
    )
    .await
    .expect("a rejected import should not hang");

    assert!(
        result.is_err(),
        "an import escaping the bundle directory must be rejected, got: {result:?}"
    );
}

/// A bare import (`lodash`) fails with a clear, host-specific error.
///
/// The host is not an npm client: a plugin author must bundle npm dependencies
/// themselves. A bare specifier must therefore fail loudly — not panic, not
/// silently resolve to an empty module.
#[tokio::test]
async fn bare_import_fails_with_clear_error() {
    let bundle = tempfile::TempDir::new().expect("temp dir should be created");

    // The imported binding is used as a value, so the transpiler keeps the
    // import statement — the loader, not type erasure, must reject `lodash`.
    std::fs::write(
        bundle.path().join("index.ts"),
        "import { merge } from 'lodash';\n\
         export function activate(): unknown { return merge({}, {}); }",
    )
    .expect("index.ts should be written");

    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "index.ts", "activate"),
    )
    .await
    .expect("a rejected bare import should not hang");

    let error = result.expect_err("a bare import must not resolve");
    let message = error.to_string();
    assert!(
        message.contains("lodash") && message.contains("bundle"),
        "the bare-import error should name the specifier and explain bundling, got: {message}"
    );
}

/// An `@swissarmyhammer/plugin` import resolves to the in-memory virtual module.
///
/// The SDK module is served from host memory, not from disk. Importing it must
/// succeed — proving the virtual-module resolution and load plumbing works even
/// before the real SDK contents land.
#[tokio::test]
async fn swissarmyhammer_plugin_import_resolves_to_virtual_module() {
    let bundle = tempfile::TempDir::new().expect("temp dir should be created");

    std::fs::write(
        bundle.path().join("index.ts"),
        "import * as sdk from '@swissarmyhammer/plugin';\n\
         export function activate(): string { return typeof sdk; }",
    )
    .expect("index.ts should be written");

    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "index.ts", "activate"),
    )
    .await
    .expect("importing the SDK virtual module should not hang")
    .expect("an @swissarmyhammer/plugin import should resolve to the virtual module");

    assert_eq!(
        result,
        serde_json::json!("object"),
        "the SDK virtual module's namespace should be importable"
    );
}

/// An `@swissarmyhammer/app` import resolves to its no-op virtual stub.
///
/// `@swissarmyhammer/app` is generated app-binding code; codegen is a separate
/// task. Here it must still resolve to a valid (no-op) virtual module so a
/// plugin that imports it loads cleanly.
#[tokio::test]
async fn swissarmyhammer_app_import_resolves_to_virtual_module() {
    let bundle = tempfile::TempDir::new().expect("temp dir should be created");

    std::fs::write(
        bundle.path().join("index.ts"),
        "import * as app from '@swissarmyhammer/app';\n\
         export function activate(): string { return typeof app; }",
    )
    .expect("index.ts should be written");

    let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

    let result = tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle.path(), "index.ts", "activate"),
    )
    .await
    .expect("importing the app virtual module should not hang")
    .expect("an @swissarmyhammer/app import should resolve to the virtual module");

    assert_eq!(
        result,
        serde_json::json!("object"),
        "the app virtual module's namespace should be importable"
    );
}
