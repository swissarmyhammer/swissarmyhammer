//! End-to-end integration test for the **CLI (stdio subprocess) transport**,
//! driven through a real plugin.
//!
//! This is the capability-level companion to `cli_server.rs`. Where
//! `cli_server.rs` exercises the [`CliServer`] type directly, this test proves
//! the same transport works when a *real plugin* registers it: a discovered
//! probe plugin's `load()` does `this.register("cli", { cli: [...] })`, the
//! host spawns the named subprocess, and the plugin then issues a `tools/call`
//! that crosses the entire pipeline — SDK dispatch Proxy, host dispatcher,
//! `CliServer` stdio transport, the child process, and back.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, real registered servers, and an effect observed on disk that can
//! only happen if every stage works.
//!
//! # The two registered servers
//!
//! The probe plugin registers two servers and uses both:
//!
//! - `cli` — the transport under test. Its source is `{ cli: [<fixture>] }`
//!   pointing at the crate's `cli_server_fixture` binary, a genuine `rmcp`
//!   stdio MCP server exposing a flat `echo` tool. The host spawns it as a
//!   child process and connects a [`CliServer`] to its stdio.
//! - `fs` — the real in-process `files` tool, reached exactly as
//!   `files_dispatch_e2e.rs` reaches it. It is the *observation channel*: the
//!   plugin writes the echoed payload to disk through it, and the test reads
//!   the file back.
//!
//! # What a passing run proves
//!
//! The probe plugin's `load()` calls `echo` on the `cli` server with a known
//! message, extracts the echoed text from the `tools/call` result, and writes
//! that text into a probe file via the real `files` tool. The test asserts the
//! probe file holds exactly the echoed payload.
//!
//! That single assertion proves the round trip: the file can only carry the
//! echoed payload if a `tools/call` reached the fixture subprocess over stdio
//! and its return value crossed back through the dispatcher into the isolate.
//! If the CLI transport is broken at any stage, the plugin's `echo` call
//! throws, `load()` fails, and discovery returns an error before any file is
//! written — the test fails.
//!
//! [`CliServer`]: swissarmyhammer_plugin::CliServer

use std::path::Path;
use std::time::Duration;

use swissarmyhammer_directory::SwissarmyhammerConfig;
use swissarmyhammer_plugin::PluginHost;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::McpServer;

/// A generous upper bound on any single host or subprocess interaction.
///
/// Building the MCP server stands up the full in-process tool registry and the
/// host spawns a real child process, so the bound is wider than a bare isolate
/// test would need. Every cross-thread await is wrapped in it so a wedged
/// subprocess fails the test fast instead of hanging CI.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The probe file the plugin writes the echoed payload into — proof a
/// `tools/call` round-tripped over the subprocess's stdio.
const PROBE_FILE: &str = "cli_echo_probe.txt";

/// The exact message the plugin sends to the fixture's `echo` tool and expects
/// echoed verbatim back over stdio.
const ECHO_PAYLOAD: &str = "e2e payload routed through the CLI stdio transport";

/// Builds a real MCP server against an isolated temp working directory.
///
/// The temp `work_dir` keeps the server's bootstrap from walking the real
/// monorepo. `agent_mode` is `true` so the unified `files` tool is registered
/// and reachable for exposure as the test's observation channel.
async fn build_mcp_server(work_dir: &Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None, true)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// Encodes `value` as a JSON/TypeScript string literal, quotes included.
///
/// Used to interpolate the fixture binary path and the probe file path into
/// the generated entry module; `serde_json` handles escaping so an unusual
/// path still produces valid source.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes to JSON")
}

/// Writes the probe plugin bundle — a TypeScript-only `index.ts` entry —
/// into `<project_root>/plugins/probe/`.
///
/// The bundle's identity is the bundle directory name (`probe`) and its entry
/// module is the conventional `index.ts`.
///
/// The entry module's `load()`:
///
/// 1. registers `cli` as a `{ cli: [<fixture>] }` source — the host spawns the
///    fixture subprocess and connects a [`CliServer`] to its stdio;
/// 2. registers `fs` as the host-exposed real `files` tool;
/// 3. calls `echo` on the `cli` server, crossing a real `tools/call` over
///    stdio to the subprocess and back;
/// 4. extracts the echoed text from the result and writes it into the probe
///    file through the real `files` tool.
///
/// `fixture_binary` and `probe_path` are absolute literals the test owns
/// (it owns the temp dirs); the calls themselves still cross the entire real
/// pipeline.
fn write_probe_plugin(project_root: &Path, fixture_binary: &str, probe_path: &Path) {
    let plugin_dir = project_root
        .join(swissarmyhammer_plugin::PLUGINS_SUBDIR)
        .join("probe");
    std::fs::create_dir_all(&plugin_dir).expect("probe plugin directory should be created");

    // The entry module. `load()` registers the CLI subprocess server and the
    // real `files` tool, calls `echo` over stdio, and writes the echoed text
    // to disk. The `indexOf` check makes the plugin fail loudly if the
    // `tools/call` return value is broken, rather than writing an empty file.
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         \n\
         /** Extracts the echoed text from an `echo` `tools/call` result. */\n\
         function echoedText(result: unknown): string {{\n\
         \x20 const content = (result as {{ content?: Array<{{ text?: string }}> }}).content;\n\
         \x20 if (content === undefined || content.length === 0) {{\n\
         \x20   throw new Error('echo result carried no content');\n\
         \x20 }}\n\
         \x20 const text = content[0].text;\n\
         \x20 if (typeof text !== 'string') {{\n\
         \x20   throw new Error('echo content[0].text was not a string');\n\
         \x20 }}\n\
         \x20 return text;\n\
         }}\n\
         \n\
         export default class ProbePlugin extends Plugin {{\n\
         \x20 async load(): Promise<void> {{\n\
         \x20   // The transport under test: a stdio MCP server subprocess.\n\
         \x20   this.register('cli', {{ cli: [{fixture}] }});\n\
         \x20   // The observation channel: the host-exposed real `files` tool.\n\
         \x20   this.register('fs', {{ rust: 'files' }});\n\
         \n\
         \x20   // Call `echo` over stdio; the result crosses the dispatcher.\n\
         \x20   const echoResult = await this.cli.echo({{ message: {payload} }});\n\
         \x20   const echoed = echoedText(echoResult);\n\
         \x20   if (echoed.indexOf({payload}) < 0) {{\n\
         \x20     throw new Error('echo did not return the sent payload');\n\
         \x20   }}\n\
         \n\
         \x20   // Write the echoed text to disk so the test can observe it.\n\
         \x20   await this.fs.files({{\n\
         \x20     op: 'write file',\n\
         \x20     file_path: {probe},\n\
         \x20     content: echoed,\n\
         \x20   }});\n\
         \x20 }}\n\
         }}\n",
        fixture = json_string(fixture_binary),
        probe = json_string(&probe_path.to_string_lossy()),
        payload = json_string(ECHO_PAYLOAD),
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("probe index.ts should be written");
}

/// A discovered probe plugin registers a `{ cli }` source and proves a
/// `tools/call` round-trips over the spawned subprocess's stdio.
///
/// The plugin's `load()` registers the `cli_server_fixture` binary as a CLI
/// source, calls its `echo` tool, and writes the echoed payload to disk
/// through the real `files` tool. The file landing with exactly the echoed
/// payload can only happen if the host spawned the subprocess and a
/// `tools/call` crossed its stdio in both directions — proving the CLI
/// transport works end to end through a real plugin.
#[tokio::test]
async fn discovered_plugin_round_trips_a_tools_call_over_the_cli_subprocess() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");

    // Cargo sets `CARGO_BIN_EXE_<name>` for every binary target when it builds
    // the crate's integration tests, so this always points at the freshly
    // built fixture stdio MCP server.
    let fixture_binary = env!("CARGO_BIN_EXE_cli_server_fixture");
    let probe_path = output_dir.path().join(PROBE_FILE);

    // The probe bundle is laid out under the project layer's `plugins/` dir,
    // where discovery will find it.
    write_probe_plugin(project_root.path(), fixture_binary, &probe_path);

    // The real in-process tool set, including the unified `files` tool.
    let server = build_mcp_server(work_dir.path()).await;

    // A fresh host, with the project layer pointed at the temp plugin root.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose every in-process tool — `files` among them — as an addressable
    // Rust module. No module is live until a plugin activates it.
    tokio::time::timeout(TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing the in-process tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // Trigger discovery: the host scans the project layer, transpiles the
    // probe's `index.ts`, creates a fresh isolate, and runs `load` — which
    // spawns the fixture subprocess and drives the `echo` call over stdio.
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

    // The single assertion: the probe file holds exactly the echoed payload.
    // This can only be true if a `tools/call` carrying `echo` reached the
    // fixture subprocess over stdio and its return value crossed back through
    // the dispatcher into the isolate.
    let content = std::fs::read_to_string(&probe_path).unwrap_or_else(|error| {
        panic!(
            "the probe file must exist at {} — the echo tools/call did not \
             round-trip over the CLI subprocess: {error}",
            probe_path.display()
        )
    });
    assert_eq!(
        content, ECHO_PAYLOAD,
        "the probe file must hold the payload echoed back over stdio — proving \
         the CLI transport round-tripped a tools/call through a real plugin"
    );
}
