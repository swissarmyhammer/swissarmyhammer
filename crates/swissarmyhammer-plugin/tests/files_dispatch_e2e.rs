//! Reference end-to-end integration test for the plugin platform.
//!
//! This is the canonical example every other capability `*_e2e.rs` test
//! follows. It exercises the **whole** plugin pipeline through the real `files`
//! MCP server — manifest-less bundle discovery, TypeScript transpile, a fresh
//! V8 isolate, the SDK dispatch Proxy, the host dispatcher, operation-tool `op`
//! dispatch, and return-value marshalling — and verifies the one effect that
//! can only happen if every stage works: real files land on disk.
//!
//! # Why `files` is the target
//!
//! The `files` tool is a genuine in-process MCP operation tool with observable
//! state — the filesystem. Verification is therefore unambiguous: "did the
//! files appear on disk with the expected contents." There is no mock, no
//! fixture, and no hand-built registry anywhere in this test. The real
//! [`FilesTool`] is obtained through the same `ToolModuleServer` exposure path
//! the production MCP bootstrap uses ([`McpServer::expose_tools_to_plugin_host`])
//! and handed to a real [`PluginHost`].
//!
//! # What a passing run proves
//!
//! The probe plugin's `load()` does three things through the registered `files`
//! server and nothing else:
//!
//! 1. writes a first probe file,
//! 2. reads that file back,
//! 3. writes the read-back content into a second probe file.
//!
//! Asserting **both** files exist with the expected contents proves the whole
//! round trip: the first file proves an `op` dispatch reached the real `files`
//! handler; the second proves the handler's return value crossed back through
//! the dispatcher into the isolate and was usable by plugin code. If any
//! pipeline stage is broken — bundle discovery, transpile, isolate creation,
//! server lookup, `op` dispatch, or return-value marshalling — at least one
//! assertion fails.
//!
//! # Isolation
//!
//! Each test owns its own [`tempfile::TempDir`] roots and a fresh
//! [`PluginHost`]; nothing is `static` and no temp dir is reused. Every
//! cross-thread interaction is bounded by a timeout so a wedged isolate fails
//! the test fast instead of hanging CI.

use std::path::Path;
use std::time::Duration;

use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::PluginHost;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::McpServer;

/// A generous upper bound on any single host or server interaction.
///
/// Building the MCP server stands up the full in-process tool registry, so the
/// bound is wider than a bare isolate test would need.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The first probe file the plugin writes — proof an `op` dispatch reached the
/// real `files` write handler.
const FIRST_PROBE_FILE: &str = "first_probe.txt";

/// The second probe file the plugin writes — proof the `read file` return value
/// crossed back through the dispatcher into the isolate.
const SECOND_PROBE_FILE: &str = "second_probe.txt";

/// The exact payload the plugin writes into the first probe file and then reads
/// back. It is asserted verbatim in both files.
const PROBE_PAYLOAD: &str = "e2e payload routed through the real files tool";

/// Builds a real MCP server against an isolated temp working directory.
///
/// The temp `work_dir` keeps the server's bootstrap from walking the real
/// monorepo. `agent_mode` is `true` so the full in-process tool set — including
/// the unified `files` tool, an agent tool — is registered, which is what makes
/// the real [`FilesTool`] reachable for exposure.
async fn build_mcp_server(work_dir: &Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None, true)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// Writes the probe plugin bundle — a manifest-less, TypeScript-only
/// `index.ts` entry — into `<project_root>/plugins/probe/`.
///
/// The bundle is what discovery scans for and the host loads:
///
/// - There is no `plugin.json`: the bundle is a manifest-less, TS-only bundle.
///   Its identity is the bundle directory name (`probe`) and its entry module
///   is the conventional `index.ts`.
/// - The entry imports the SDK, subclasses [`Plugin`], and in `load()` registers
///   the host-exposed `files` Rust module under the name `fs`, then drives three
///   real `files` operations through the SDK dispatch Proxy:
///   `write file` → `read file` → `write file`.
///
/// The probe file paths are absolute literals interpolated into the TypeScript
/// source: the test owns the temp directory, so it owns the paths the plugin
/// writes to — exactly as a plugin author would hard-code or compute a path.
/// The calls themselves still cross the entire real pipeline; only the path
/// strings are test-controlled.
fn write_probe_plugin(project_root: &Path, output_dir: &Path) {
    let plugin_dir = project_root
        .join(swissarmyhammer_plugin::PLUGINS_SUBDIR)
        .join("probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // Absolute paths the plugin writes to. `to_string_lossy` is fine here:
    // `tempfile` hands back valid UTF-8 paths on every supported platform.
    let first_path = output_dir.join(FIRST_PROBE_FILE);
    let second_path = output_dir.join(SECOND_PROBE_FILE);

    // The entry module. `load()` uses ONLY the registered `files` server:
    //
    //   1. `write file`  — lands the first probe file via `op` dispatch.
    //   2. `read file`   — reads it back; the result crosses the dispatcher
    //                      back into the isolate as a `CallToolResult` JSON
    //                      shape (`{ content: [{ text, ... }], ... }`).
    //   3. `write file`  — writes the read-back text into the second file.
    //
    // A non-trivial check on the read-back value (it must contain the payload)
    // makes the plugin fail loudly if return-value marshalling is broken,
    // rather than silently writing an empty second file.
    let entry = format!(
        "import {{ Plugin, makePluginThis }} from '@swissarmyhammer/plugin';\n\
         \n\
         /** Extracts the file text from a `files` `read file` result. */\n\
         function readBackText(result: unknown): string {{\n\
         \x20 const content = (result as {{ content?: Array<{{ text?: string }}> }}).content;\n\
         \x20 if (content === undefined || content.length === 0) {{\n\
         \x20   throw new Error('read file result carried no content');\n\
         \x20 }}\n\
         \x20 const text = content[0].text;\n\
         \x20 if (typeof text !== 'string') {{\n\
         \x20   throw new Error('read file content[0].text was not a string');\n\
         \x20 }}\n\
         \x20 return text;\n\
         }}\n\
         \n\
         class ProbePlugin extends Plugin {{\n\
         \x20 async load(): Promise<void> {{\n\
         \x20   // Activate the host-exposed real `files` tool under the name `fs`.\n\
         \x20   this.register('fs', {{ rust: 'files' }});\n\
         \n\
         \x20   // (1) write the first probe file through the real `op` dispatch.\n\
         \x20   await this.fs.files({{\n\
         \x20     op: 'write file',\n\
         \x20     file_path: {first_path},\n\
         \x20     content: {payload},\n\
         \x20   }});\n\
         \n\
         \x20   // (2) read it back; the return value crosses the dispatcher.\n\
         \x20   const readResult = await this.fs.files({{\n\
         \x20     op: 'read file',\n\
         \x20     path: {first_path},\n\
         \x20   }});\n\
         \x20   const readBack = readBackText(readResult);\n\
         \x20   if (readBack.indexOf({payload}) < 0) {{\n\
         \x20     throw new Error('read file did not return the written payload');\n\
         \x20   }}\n\
         \n\
         \x20   // (3) write the read-back content into the second probe file.\n\
         \x20   await this.fs.files({{\n\
         \x20     op: 'write file',\n\
         \x20     file_path: {second_path},\n\
         \x20     content: readBack,\n\
         \x20   }});\n\
         \x20 }}\n\
         }}\n\
         \n\
         export async function load(): Promise<unknown> {{\n\
         \x20 const p = makePluginThis(new ProbePlugin()) as ProbePlugin;\n\
         \x20 await p.load();\n\
         \x20 return null;\n\
         }}\n",
        first_path = json_string(&first_path.to_string_lossy()),
        second_path = json_string(&second_path.to_string_lossy()),
        payload = json_string(PROBE_PAYLOAD),
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts should be written");
}

/// Encodes `value` as a JSON/TypeScript string literal, quotes included.
///
/// Used to interpolate filesystem paths into the generated entry module;
/// `serde_json` handles escaping of backslashes and quotes so a Windows path or
/// a path with unusual characters still produces valid source.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes to JSON")
}

/// The reference end-to-end test: a discovered probe plugin drives the real
/// `files` tool through the whole pipeline and lands two real files on disk.
///
/// This single test stitches every pipeline stage together:
///
/// - the real [`FilesTool`] is built by the MCP server bootstrap and exposed to
///   the host with [`McpServer::expose_tools_to_plugin_host`] — no mock;
/// - the probe bundle (a manifest-less `index.ts`) is discovered from the
///   project layer and loaded through `discover_and_load_all`, which transpiles
///   the TypeScript, creates a fresh V8 isolate, and runs the exported `load`;
/// - inside the isolate the SDK dispatch Proxy turns `this.fs.files({ op, … })`
///   into a real `tools/call` routed by the host dispatcher into the live
///   `files` handler.
///
/// Both assertions observe the filesystem — the only honest verification for an
/// in-process server whose state *is* the filesystem.
#[tokio::test]
async fn discovered_plugin_drives_the_real_files_tool_end_to_end() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");

    // The probe bundle is laid out under the project layer's `plugins/` dir,
    // where discovery will find it.
    write_probe_plugin(project_root.path(), output_dir.path());

    // The real in-process tool set, including the unified `files` tool.
    let server = build_mcp_server(work_dir.path()).await;

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose every in-process tool — `files` among them — as an addressable
    // Rust module. This is the production exposure path: each tool is wrapped
    // in a `ToolModuleServer` and recorded in the host's available-modules
    // table. No module is live until a plugin activates it.
    tokio::time::timeout(TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing the in-process tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // Trigger discovery: the host scans the project layer, resolves the
    // manifest-less probe's `index.ts` entry, transpiles it, creates a fresh
    // isolate, and runs the exported `load` — whose body performs the three
    // real `files` calls.
    let loaded = tokio::time::timeout(
        TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("discovering and loading the probe plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one probe plugin should be discovered and loaded"
    );

    // Assertion 1 — the first probe file exists with the written payload.
    // This can only be true if a `tools/call` carrying `op: "write file"`
    // reached the real `files` handler through the dispatcher.
    let first_path = output_dir.path().join(FIRST_PROBE_FILE);
    let first_content = std::fs::read_to_string(&first_path).unwrap_or_else(|error| {
        panic!(
            "the first probe file must exist at {} — op dispatch into the real \
             files handler did not land it: {error}",
            first_path.display()
        )
    });
    assert_eq!(
        first_content, PROBE_PAYLOAD,
        "the first probe file must hold exactly the payload the plugin wrote"
    );

    // Assertion 2 — the second probe file exists holding the read-back content.
    // This can only be true if the `read file` return value crossed back
    // through the dispatcher into the isolate and was usable by plugin code:
    // the plugin wrote the *read-back* string, not a constant.
    let second_path = output_dir.path().join(SECOND_PROBE_FILE);
    let second_content = std::fs::read_to_string(&second_path).unwrap_or_else(|error| {
        panic!(
            "the second probe file must exist at {} — the read-file return value \
             did not cross back into the isolate: {error}",
            second_path.display()
        )
    });
    assert_eq!(
        second_content, PROBE_PAYLOAD,
        "the second probe file must hold the content the plugin read back from \
         the first file — proving the dispatcher's return value round-tripped"
    );
}
