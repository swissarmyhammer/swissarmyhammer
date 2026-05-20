//! End-to-end integration test for the **server-name collision** policy:
//! once a name is registered the first registrant holds it, and a later
//! `register` of the same name fails with `ServerNameTaken` — visible from
//! the plugin side as a synchronously-thrown JS error, leaves the first
//! registration untouched, and is reversible by unloading the first plugin.
//!
//! # What this test proves
//!
//! The spec requires the MCP server registry to enforce a single global
//! namespace with first-registration-wins semantics — no override, no silent
//! displacement. Until this test, that policy was only asserted at the
//! `ServerRegistry` unit level; nothing exercised the whole stack from a
//! plugin author's perspective. This file fills that gap by driving two
//! committed example bundles —
//! [`examples/plugins/collide-probe-a/`](../../examples/plugins/collide-probe-a/)
//! and [`examples/plugins/collide-probe-b/`](../../examples/plugins/collide-probe-b/) —
//! through the real plugin platform and observing the policy end to end.
//!
//! Each bundle is a real, committed `index.ts` shaped exactly like the other
//! example bundles in this crate — a `Plugin` subclass with the usual
//! `readonly name`/`version`/`description` props and a `load()` that calls
//! `register(...)`. Bundle A claims the shared name first; bundle B's `load()`
//! tries to claim the same name and fails.
//!
//! # The four assertions
//!
//! 1. Bundle A's `register("collide-probe", ...)` succeeds.
//! 2. Bundle B's `load()` fails because its synchronous `register` call sees
//!    the platform's `ServerNameTaken` error as a thrown JS error — the host's
//!    `load()` surfaces it as an `Err`, and the error message carries the
//!    `Display` form of `Error::ServerNameTaken`, naming `"collide-probe"`.
//! 3. Bundle A's registered server remains live and callable through a
//!    `tools/call` after the collision — a failed second load does not poison
//!    the registry.
//! 4. After unloading bundle A, loading bundle B fresh succeeds — the name's
//!    tombstone is reusable.
//!
//! # Why two distinct `{ rust }` ids back one registered name
//!
//! The most natural reading of the test — "have both bundles do
//! `this.register("collide-probe", { rust: "<that-id>" })` with the same
//! `{ rust }` id" — does not work against the real platform. An in-process
//! `{ rust }` source is *single-activation*: the host's
//! [`PluginHost::activate_rust_module`] moves the module out of the
//! available-modules table on first activation. A second `{ rust: "<same-id>" }`
//! from another bundle resolves to `UnknownServer` rather than ever reaching
//! the registry's name-uniqueness check, so the test would observe the wrong
//! error.
//!
//! To exercise `ServerNameTaken` honestly, each `collide-probe-*` bundle
//! activates its OWN `{ rust }` module — bundle A uses `collide-probe-a-mod`,
//! bundle B uses `collide-probe-b-mod` — while both register under the SAME
//! name (`"collide-probe"`). The collision the registry enforces is on the
//! registered name; that is what this test exercises. The bundle source files
//! and their READMEs document the rationale at the plugin-author layer.
//!
//! # Why the test loads bundles with `host.load(path)` per bundle
//!
//! [`PluginHost::discover_and_load_all`] is **atomic**: if any discovered
//! plugin fails to load, every plugin the same call already loaded is rolled
//! back. Loading both bundles through one discovery scan would therefore lose
//! bundle A's registration when bundle B's load fails, defeating assertion 3.
//! So this test stages both bundles into a project layer with the shared
//! `support::stage_example` helper but loads each through `host.load(<bundle>)`
//! directly — one load per bundle, isolating their fates.

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use swissarmyhammer_plugin::{CallerId, Error, PluginHost, PLUGINS_SUBDIR};

use support::{COLLIDE_PROBE_B_MODULE_ID, COLLIDE_PROBE_SERVER_NAME};

/// The example bundle directory name for the winning side of the collision.
const COLLIDE_PROBE_A: &str = "collide-probe-a";

/// The example bundle directory name for the losing side of the collision.
const COLLIDE_PROBE_B: &str = "collide-probe-b";

/// The probe message bundle A sends through its registered server's `echo`
/// tool. Held here so the test asserts on the same literal the bundle uses.
/// Must match `PROBE_MESSAGE` in `examples/plugins/collide-probe-a/index.ts`.
const COLLIDE_PROBE_A_MESSAGE: &str = "collide-probe-a is live";

/// The fresh probe message the post-unload bundle-B path uses. Bundle B does
/// not itself drive its registered server — its successful-fresh-load path
/// just leaves the server registered — so the test issues this probe directly
/// from the host side to prove the fresh registration is callable.
const COLLIDE_PROBE_B_MESSAGE: &str = "collide-probe-b is live after unload";

/// The resolved path to a staged example bundle's directory.
///
/// `support::stage_example` lays the committed bundle into
/// `<layer_root>/plugins/<name>/`; the host loads it with that path.
fn staged_bundle_path(layer_root: &Path, name: &str) -> PathBuf {
    layer_root.join(PLUGINS_SUBDIR).join(name)
}

/// Extracts the echoed text from a `tools/call` result on the probe server.
///
/// The probe `echo` tool returns its `message` argument verbatim inside a
/// `CallToolResult` shape; the test walks that shape to assert the exact
/// echoed string.
///
/// # Panics
///
/// Panics if the result is not the expected `CallToolResult` content shape —
/// a malformed result would be a wiring error in the platform, not a condition
/// under test.
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

/// Drives the server-name collision policy end to end against two committed
/// example bundles and verifies the four assertions named in the module
/// docstring.
///
/// A single test holds the assertions together because they share state — the
/// host, the shared registered name, the registered server's liveness across
/// the collision, and the post-unload re-registration. Splitting them into
/// separate `#[tokio::test]` functions would force re-staging and re-exposing
/// the same scaffolding several times and would obscure the cause-and-effect
/// chain the policy is about.
#[tokio::test]
async fn server_name_collision_policy_holds_across_two_committed_bundles() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");

    // Stage both committed bundles into the project layer's `plugins/`
    // directory. Each becomes a real bundle on disk; the host loads each
    // through `host.load(<staged-path>)`.
    support::stage_example(COLLIDE_PROBE_A, project_root.path());
    support::stage_example(COLLIDE_PROBE_B, project_root.path());

    let bundle_a_path = staged_bundle_path(project_root.path(), COLLIDE_PROBE_A);
    let bundle_b_path = staged_bundle_path(project_root.path(), COLLIDE_PROBE_B);

    // A fresh host with the project layer pointed at the staged plugin root.
    // No discovery scan is used; each bundle is loaded by its absolute path
    // through `host.load(<path>)` so a failed load does not roll back the
    // earlier successful one.
    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose both `{ rust }` modules the bundles activate. The exposures are
    // one-shot: `register` moves the module out of the available-modules
    // table, so the test must re-expose any module it wants to activate
    // again later.
    support::expose_collide_probe_modules(&host).await;

    // ── Assertion 1 ────────────────────────────────────────────────────────
    // Bundle A's `register("collide-probe", { rust: "collide-probe-a-mod" })`
    // succeeds; the host returns its newly minted plugin id.
    let plugin_a_id = tokio::time::timeout(support::TIMEOUT, host.load(&bundle_a_path))
        .await
        .expect("loading bundle A should not hang")
        .expect("bundle A's load should succeed — it claims a fresh name");

    // ── Assertion 2 ────────────────────────────────────────────────────────
    // Bundle B's `load()` calls `register("collide-probe", ...)` — the
    // synchronous SDK `register` sees the platform's `ServerNameTaken` failure
    // come back from the host bridge as a thrown JS error, which propagates
    // out of the plugin's `load()` and out of the host's `load()` call as an
    // `Err`. The `Err` is the runtime variant: the JS isolate exception
    // carries the host's `Display` of `Error::ServerNameTaken`, which names
    // the shared registered server name.
    let bundle_b_first_load = tokio::time::timeout(support::TIMEOUT, host.load(&bundle_b_path))
        .await
        .expect("loading bundle B should not hang");
    let load_b_error = bundle_b_first_load
        .expect_err("bundle B's load must fail — `collide-probe` is already taken");
    let load_b_message = match &load_b_error {
        Error::Runtime(message) => message.clone(),
        other => panic!(
            "bundle B's load should fail with Error::Runtime carrying the JS exception, \
             got {other:?}"
        ),
    };
    assert!(
        load_b_message.contains(COLLIDE_PROBE_SERVER_NAME),
        "the bridged JS error should name the colliding server '{}', got '{}'",
        COLLIDE_PROBE_SERVER_NAME,
        load_b_message
    );
    assert!(
        load_b_message.contains("already taken"),
        "the bridged JS error should carry the `ServerNameTaken` Display text \
         ('... is already taken'), got '{load_b_message}'"
    );

    // ── Assertion 3 ────────────────────────────────────────────────────────
    // Bundle A's registered server stays live and callable across the
    // collision. The host dispatches a real `tools/call` against the same
    // name bundle B failed to claim, and the call lands on bundle A's
    // in-process probe server — proving the failed second load disposed
    // nothing it should not have.
    let echo_result = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            COLLIDE_PROBE_SERVER_NAME,
            "echo",
            json!({ "message": COLLIDE_PROBE_A_MESSAGE }),
        ),
    )
    .await
    .expect("the echo dispatch should not hang")
    .expect("bundle A's registered server must still answer after the collision");
    assert_eq!(
        echoed_text(&echo_result),
        COLLIDE_PROBE_A_MESSAGE,
        "bundle A's registered echo must round-trip its argument verbatim"
    );

    // ── Assertion 4 ────────────────────────────────────────────────────────
    // Unloading bundle A frees the shared name (with a tombstone the registry
    // clears on re-registration). After re-exposing bundle B's `{ rust }`
    // module — the failed first load above moved it out of the
    // available-modules table — a fresh load of bundle B succeeds. The
    // newly-claimed server answers a real `tools/call`, confirming the
    // registration is genuine and not just a syntactic success.
    tokio::time::timeout(support::TIMEOUT, host.unload(&plugin_a_id))
        .await
        .expect("unloading bundle A should not hang")
        .expect("unloading bundle A should succeed");

    support::expose_collide_probe_module(&host, COLLIDE_PROBE_B_MODULE_ID).await;

    let _plugin_b_id = tokio::time::timeout(support::TIMEOUT, host.load(&bundle_b_path))
        .await
        .expect("the fresh bundle B load should not hang")
        .expect(
            "loading bundle B after bundle A is unloaded must succeed — the freed \
             name is re-registerable",
        );

    let fresh_echo = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            COLLIDE_PROBE_SERVER_NAME,
            "echo",
            json!({ "message": COLLIDE_PROBE_B_MESSAGE }),
        ),
    )
    .await
    .expect("the post-unload echo dispatch should not hang")
    .expect("bundle B's freshly registered server must answer after the unload");
    assert_eq!(
        echoed_text(&fresh_echo),
        COLLIDE_PROBE_B_MESSAGE,
        "bundle B's freshly registered echo must round-trip its argument verbatim"
    );

    // `_plugin_b_id` is intentionally not unloaded here; the host drops
    // cleanly at the end of the test scope, which exercises the implicit
    // teardown path too.
}
