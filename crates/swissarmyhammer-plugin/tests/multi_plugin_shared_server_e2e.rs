//! End-to-end integration test for **idempotent server registration**: two
//! plugins that both register the same `(name, source)` share ONE underlying
//! MCP server, and that server stays live until the LAST plugin unloads.
//!
//! # What this test proves
//!
//! Two plugins that both depend on the same external MCP server — a community
//! CLI tool, a project-local data server, anything backed by a `{ cli }` or
//! `{ url }` source — must both succeed at `register(name, source)` without
//! coordination. The committed bundles
//! [`examples/plugins/shared-cli-a/`](../../examples/plugins/shared-cli-a/) and
//! [`examples/plugins/shared-cli-b/`](../../examples/plugins/shared-cli-b/)
//! each register the same name (`"shared-cli"`) against the SAME `{ cli }`
//! source (the crate's `cli_server_fixture` binary, identical literal command
//! and no overrides). The platform recognizes the duplicate source and merges
//! the two calls into one registration with refcount=2.
//!
//! # The four assertions
//!
//! 1. Loading bundle A and then bundle B both succeed; neither's `load()`
//!    sees `ServerNameTaken`.
//! 2. After both bundles are loaded, an external `tools/call` to
//!    `shared-cli.echo` is answered by the spawned subprocess — proving the
//!    shared registration is live and routable.
//! 3. After unloading bundle A, the same `tools/call` still succeeds: the
//!    underlying subprocess is alive because bundle B still holds the
//!    registration.
//! 4. After unloading bundle B, the call fails with `ServerUnavailable` or
//!    `UnknownServer` (the name is no longer live) — the registration was
//!    torn down with the last holder.

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use swissarmyhammer_plugin::{CallerId, Error, PluginHost, PLUGINS_SUBDIR};

/// The example bundle directory name for the first holder of the shared CLI
/// source.
const SHARED_CLI_A: &str = "shared-cli-a";

/// The example bundle directory name for the second holder of the shared CLI
/// source.
const SHARED_CLI_B: &str = "shared-cli-b";

/// The shared registered server name both bundles target.
///
/// Held as a constant so the test, the bundles, and any later cross-check all
/// agree on the literal string.
const SHARED_SERVER_NAME: &str = "shared-cli";

/// The placeholder token both committed bundles carry in their `{ cli }`
/// source — rewritten in the staged copies with the real fixture binary path.
///
/// Must match the literal token in both bundles' `index.ts`.
const CLI_COMMAND_TOKEN: &str = "__CLI_ECHO_COMMAND__";

/// The probe message the test sends through the shared subprocess's `echo`
/// tool. Held here so each assertion uses the same literal.
const PROBE_MESSAGE: &str = "shared-cli subprocess is live";

/// The resolved path to a staged example bundle's directory.
fn staged_bundle_path(layer_root: &Path, name: &str) -> PathBuf {
    layer_root.join(PLUGINS_SUBDIR).join(name)
}

/// Extracts the echoed text from an `echo` `tools/call` result.
fn echoed_text(result: &Value) -> String {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .expect("an `echo` tool result must carry text content");
    text.to_string()
}

#[tokio::test]
async fn two_plugins_share_one_cli_server_and_refcount_through_unload() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");

    // Cargo sets `CARGO_BIN_EXE_<name>` for every binary target when it builds
    // the crate's integration tests, so this always points at the freshly
    // built fixture stdio MCP server.
    let fixture_binary = env!("CARGO_BIN_EXE_cli_server_fixture");

    // Stage BOTH committed bundles into the project layer, rewriting their
    // shared placeholder CLI-command token with the SAME real fixture path.
    // Because the two bundles register the same `(name, source)` literal
    // string, they form a structurally-equal duplicate registration.
    support::stage_example_with(
        SHARED_CLI_A,
        project_root.path(),
        &[(CLI_COMMAND_TOKEN, fixture_binary)],
    );
    support::stage_example_with(
        SHARED_CLI_B,
        project_root.path(),
        &[(CLI_COMMAND_TOKEN, fixture_binary)],
    );

    let bundle_a_path = staged_bundle_path(project_root.path(), SHARED_CLI_A);
    let bundle_b_path = staged_bundle_path(project_root.path(), SHARED_CLI_B);

    // A fresh host. Each bundle is loaded by its absolute path through
    // `host.load(<path>)` so per-bundle outcomes are observable independently
    // — discovery would atomically roll back, masking the per-bundle effect.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // ── Assertion 1 ────────────────────────────────────────────────────────
    // Both bundles' `register("shared-cli", { cli: [<fixture>] })` calls
    // succeed. The first claims the name and spawns the subprocess; the
    // second recognizes the same source and joins it (refcount bumps to 2).
    let plugin_a_id = tokio::time::timeout(support::TIMEOUT, host.load(&bundle_a_path))
        .await
        .expect("loading bundle A should not hang")
        .expect("bundle A's load should succeed — it claims a fresh name");

    let plugin_b_id = tokio::time::timeout(support::TIMEOUT, host.load(&bundle_b_path))
        .await
        .expect("loading bundle B should not hang")
        .expect(
            "bundle B's load should succeed — same-(name, source) is idempotent, \
             not a ServerNameTaken collision",
        );

    // ── Assertion 2 ────────────────────────────────────────────────────────
    // The shared registration is live and routes a real `tools/call` over the
    // spawned subprocess's stdio. The host dispatches the call by the shared
    // name, exactly as a tool consumer would.
    let echo_result = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            SHARED_SERVER_NAME,
            "echo",
            json!({ "message": PROBE_MESSAGE }),
        ),
    )
    .await
    .expect("the shared echo dispatch should not hang")
    .expect("the shared CLI subprocess must answer through its registered name");
    assert_eq!(
        echoed_text(&echo_result),
        PROBE_MESSAGE,
        "the shared subprocess's echo must round-trip its argument verbatim"
    );

    // ── Assertion 3 ────────────────────────────────────────────────────────
    // Unloading bundle A decrements the refcount but the registration stays
    // live because bundle B still holds it. The same `tools/call` keeps
    // succeeding — proof the subprocess was NOT torn down with the first
    // holder's unload.
    tokio::time::timeout(support::TIMEOUT, host.unload(&plugin_a_id))
        .await
        .expect("unloading bundle A should not hang")
        .expect("unloading bundle A should succeed");

    let echo_after_a = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            SHARED_SERVER_NAME,
            "echo",
            json!({ "message": PROBE_MESSAGE }),
        ),
    )
    .await
    .expect("the post-unload-A echo dispatch should not hang")
    .expect(
        "the shared CLI subprocess must stay live after bundle A unloads — \
         bundle B still holds the registration",
    );
    assert_eq!(
        echoed_text(&echo_after_a),
        PROBE_MESSAGE,
        "the still-live subprocess must keep echoing arguments verbatim"
    );

    // ── Assertion 4 ────────────────────────────────────────────────────────
    // Unloading bundle B drops the last holder: refcount hits zero, the
    // registration is torn down, the subprocess is killed. A subsequent call
    // by the shared name now fails — the test does not pin the exact error
    // variant (the registry may report Disposed/ServerUnavailable, or the
    // dispatcher may surface UnknownServer for a fully gone name), but it
    // must NOT silently succeed.
    tokio::time::timeout(support::TIMEOUT, host.unload(&plugin_b_id))
        .await
        .expect("unloading bundle B should not hang")
        .expect("unloading bundle B should succeed");

    let echo_after_b = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            SHARED_SERVER_NAME,
            "echo",
            json!({ "message": PROBE_MESSAGE }),
        ),
    )
    .await
    .expect("the post-unload-B echo dispatch should not hang");
    let err = echo_after_b.expect_err(
        "after the last holder unloads the shared subprocess must be torn down — \
         the call must NOT silently succeed",
    );
    assert!(
        matches!(err, Error::ServerUnavailable | Error::UnknownServer),
        "the call after the last holder unloaded should report Unavailable/Unknown, got {err:?}"
    );
}
