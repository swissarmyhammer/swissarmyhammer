//! Capstone end-to-end test for the **committed example plugin suite** —
//! discovery and layer stacking across every bundle the crate ships.
//!
//! The individual `*_e2e.rs` tests each drive one committed example through the
//! real plugin platform: `kanban_tasks_e2e.rs` proves `_meta` path dispatch,
//! `file_notes_e2e.rs` proves a filesystem effect, `cli_echo_e2e.rs` proves the
//! `{ cli }` transport, `multi_module_e2e.rs` proves sibling-module imports.
//! Each stages its bundle into a single project layer. This file is the
//! capstone: it proves the committed examples are *discovered and loaded
//! through the real layer-stacking machinery* — every layer source, every
//! bundle, loaded EXACTLY as committed.
//!
//! # Why two tests, not one
//!
//! A `{ rust }` module is deliberately **single-activation**:
//! `activate_rust_module` (`src/host.rs`) `remove`s the module from the host's
//! available-modules table, and its doc comment states activation is one-shot.
//! `kanban-tasks`, `multi-module`, and `cli-echo` all consume the *same*
//! `{ rust: "kanban" }` module, so three of them cannot co-load in one host.
//! The capstone is therefore honestly split:
//!
//! - [`committed_examples_coload_across_layers`] co-loads the two bundles that
//!   genuinely coexist — distinct modules, distinct server names — in one
//!   `discover_and_load_all`, then unloads both.
//! - [`each_committed_example_loads_from_its_layer`] loads each remaining
//!   committed example individually, with a fresh host per example, each staged
//!   into a different layer so discovery is exercised from every layer source.
//!
//! No bundle's `{ rust }` id or server name is rewritten — every bundle is
//! loaded exactly as committed. The one allowed substitution is the `cli-echo`
//! `__CLI_ECHO_COMMAND__` token, which carries the fixture binary path; that is
//! the bundle's own placeholder for a host-supplied value, not a scope mutation.
//!
//! # Isolation
//!
//! Both tests own their own [`tempfile::TempDir`] layer roots and fresh
//! [`PluginHost`]s; nothing is `static` and no temp dir is reused. Both run
//! under [`CurrentDirGuard`] and are `#[serial_test::serial]`: the `file-notes`
//! bundle writes through the process CWD — global mutable state — so a
//! CWD-touching test must be temp-CWD isolated and never race another. Every
//! cross-thread interaction is bounded by [`support::TIMEOUT`] so a wedged
//! isolate fails the test fast instead of hanging CI.

mod support;

use std::path::{Path, PathBuf};

use serde_json::json;
use swissarmyhammer_common::test_utils::CurrentDirGuard;
use swissarmyhammer_directory::{FileSource, SwissarmyhammerConfig};
use swissarmyhammer_plugin::{
    discover_plugins, CallerId, Error, LayerRoot, PluginHost, PLUGINS_SUBDIR,
};

/// The committed builtin-layer bundle co-loaded in [`committed_examples_coload_across_layers`].
///
/// It lives at the repository root under `builtin/plugins/`, not in the crate's
/// `examples/plugins/` tree, so [`stage_repo_builtin_probe`] resolves it rather
/// than [`support::stage_example`]. Its `load()` registers `{ rust: "kanban" }`
/// under the canonical server name `kanban` (see [`PROBE_SERVER`]); this const
/// is the bundle's directory id, which is distinct from that server name.
const BUILTIN_PROBE: &str = "kanban-builtin-probe";

/// The server name the `kanban-builtin-probe` bundle registers — it is the
/// `register()` name in the bundle's `index.ts`. The probe registers the
/// host's `{ rust: "kanban" }` module under the *canonical* server name
/// `"kanban"` (not its directory name), so it shares the one single-activation
/// in-process module with the kanban command plugins instead of starving them.
/// The bundle's identity stays its directory name ([`BUILTIN_PROBE`]); only the
/// server it exposes is the canonical `"kanban"`.
const PROBE_SERVER: &str = "kanban";

/// The server name the committed `file-notes` example registers `{ rust: "files" }`
/// under. It must match the `register` call in the bundle's `index.ts`.
const FILE_NOTES_SERVER: &str = "fs";

/// The bundle-relative path of the first note the `file-notes` example writes.
/// It must match the path hard-coded in the bundle's `index.ts`.
const HELLO_NOTE: &str = "notes/hello.txt";

/// The bundle-relative path of the echo note the `file-notes` example writes.
/// It must match the path hard-coded in the bundle's `index.ts`.
const ECHO_NOTE: &str = "notes/echo.txt";

/// The exact body the `file-notes` example writes into both note files. It must
/// match the constant in the bundle's `index.ts`.
const NOTE_BODY: &str = "a note round-tripped through the in-process files tool";

/// The placeholder token the committed `cli-echo` `index.ts` carries where the
/// CLI command belongs. [`support::stage_example_with`] rewrites it in the
/// staged copy with the real fixture binary path; it must match the token
/// spelled in the committed bundle exactly.
const CLI_COMMAND_TOKEN: &str = "__CLI_ECHO_COMMAND__";

/// The server name the committed `cli-echo` example registers its `{ cli }`
/// source under, and the flat tool that source exposes.
const ECHO_SERVER: &str = "echo";

/// The two task titles the committed `kanban-tasks` example adds. They must
/// match the titles hard-coded in that bundle's `index.ts`.
const KANBAN_TASKS_TITLES: [&str; 2] = ["Draft the plugin proposal", "Review the plugin proposal"];

/// The normalized task title the committed `multi-module` example's imported
/// `board-helpers.ts` helper produces. It must match the bundle's source.
const MULTI_MODULE_TITLE: &str = "Ship the multi-module example";

/// Resolves the repository root from the crate's manifest directory.
///
/// The crate lives at `<repo>/crates/swissarmyhammer-plugin`, so the repository
/// root is two directories up. The committed `builtin/plugins/` bundle tree
/// lives at the repository root, outside this crate's `examples/plugins/` tree.
///
/// # Returns
///
/// The absolute path to the repository root.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate manifest dir must have a repository root two levels up")
        .to_path_buf()
}

/// Recursively copies the directory tree at `source` into `destination`.
///
/// Mirrors `support`'s private staging copy: it creates `destination` and every
/// nested directory, then copies each file verbatim. Used by
/// [`stage_repo_builtin_probe`] to lay the repo-root builtin bundle — which is
/// not under `examples/plugins/`, so the `support` stagers cannot reach it —
/// into a temp builtin layer root.
///
/// # Parameters
///
/// - `source` — the directory tree to copy from.
/// - `destination` — the directory tree to create and copy into.
///
/// # Panics
///
/// Panics if any directory creation, directory read, or file copy fails — a
/// staging failure is a test setup error, not a condition under test.
fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap_or_else(|error| {
        panic!(
            "staging directory {} should be created: {error}",
            destination.display(),
        )
    });
    for entry in std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("bundle {} should be readable: {error}", source.display()))
    {
        let entry = entry.expect("a directory entry should be readable");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!(
                    "bundle file {} should copy to {}: {error}",
                    from.display(),
                    to.display(),
                )
            });
        }
    }
}

/// Stages the committed repo-root `builtin/plugins/kanban-builtin-probe` bundle
/// into a temp builtin layer root.
///
/// The probe bundle is the real builtin plugin the kanban desktop app ships; it
/// lives at `<repo>/builtin/plugins/kanban-builtin-probe`, outside this crate's
/// `examples/plugins/` tree, so [`support::stage_example`] — which resolves
/// against `examples/plugins/` — cannot reach it. This stager copies the
/// committed bundle, untouched, into `<layer_root>/plugins/kanban-builtin-probe/`
/// so a [`PluginHost`] builtin layer pointed at `layer_root` discovers it. The
/// committed bundle stays read-only; only the temp copy is touched.
///
/// # Parameters
///
/// - `layer_root` — the temp builtin layer root to stage the bundle beneath.
///
/// # Panics
///
/// Panics if the committed probe bundle does not exist or any copy fails.
fn stage_repo_builtin_probe(layer_root: &Path) {
    let source = repo_root().join("builtin/plugins").join(BUILTIN_PROBE);
    assert!(
        source.is_dir(),
        "the committed builtin probe bundle must exist at {}",
        source.display(),
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join(BUILTIN_PROBE);
    copy_dir_recursive(&source, &destination);
}

/// Asserts the layer-by-layer scan resolves each `(id, FileSource)` pair.
///
/// Runs the platform's point-in-time [`discover_plugins`] scan over `layers` —
/// the very scan [`PluginHost::discover_and_load_all`] runs internally — and
/// asserts the discovered set is exactly `expected`, matched on `id`
/// and resolved [`FileSource`]. This is the honest read on "each plugin was
/// discovered from the layer it was staged in": the `FileSource` on a
/// [`DiscoveredPlugin`](swissarmyhammer_plugin::DiscoveredPlugin) is the layer
/// the scan resolved it from.
///
/// # Parameters
///
/// - `layers` — the discovery layers, lowest precedence first.
/// - `expected` — the `(id, FileSource)` pairs every layer must yield.
///
/// # Panics
///
/// Panics if discovery errors or the discovered `(id, source)` set differs from
/// `expected`.
fn assert_discovered_sources(layers: &[LayerRoot], expected: &[(&str, FileSource)]) {
    let discovered = discover_plugins::<SwissarmyhammerConfig>(layers)
        .expect("discovery over the staged layers should succeed");

    // `FileSource` is not `Ord`, so the `(id, source)` pairs are sorted by the
    // `id` alone — which is unique per discovered plugin — to compare
    // the two sets order-independently.
    let mut found: Vec<(String, FileSource)> = discovered
        .iter()
        .map(|plugin| (plugin.id().to_string(), plugin.source.clone()))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));

    let mut want: Vec<(String, FileSource)> = expected
        .iter()
        .map(|(id, source)| (id.to_string(), source.clone()))
        .collect();
    want.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(
        found, want,
        "discovery must resolve each staged bundle with its layer's FileSource",
    );
}

/// Co-loads the two committed bundles that genuinely coexist across two layers,
/// asserts each loaded from its own layer source, then unloads both.
///
/// `kanban-builtin-probe` (`{ rust: "kanban" }`, server `kanban`) and
/// `file-notes` (`{ rust: "files" }`, server `fs`) consume distinct modules and
/// register distinct server names, so unlike the three `kanban`-consuming
/// examples they can load into one host at once. This single test stitches the
/// layer-stacking capability together against committed bundles:
///
/// - the committed probe is staged into a temp **builtin** layer root and the
///   committed `file-notes` example into a temp **project** layer root, each
///   loaded EXACTLY as committed — no `{ rust }` id or server name rewritten;
/// - [`discover_plugins`] resolves both, and the assertion pins each plugin's
///   `FileSource` to the layer it was staged in — `Builtin` for the probe,
///   `Local` for `file-notes`;
/// - one `discover_and_load_all` loads both bundles through the real stacked
///   layers; each plugin's observable effect is then asserted — the probe
///   drives a real `kanban` `init board` (creating a `.kanban` board on disk)
///   through its registered server, and `file-notes` writes both note files;
/// - both plugins are unloaded, and each server they registered is asserted
///   gone from the live registry — a routed call now fails `ServerUnavailable`.
///
/// Both Rust modules the bundles consume — `kanban` and `files` — come from one
/// [`support::build_mcp_server`] in-process tool set: the agent-mode server
/// registers both, so a single `expose_tools_to_plugin_host` call backs both
/// bundles. The server's `kanban` tool resolves its board against the server's
/// `work_dir`, so the probe's `init board` lands a `.kanban` board there.
#[tokio::test]
#[serial_test::serial]
async fn committed_examples_coload_across_layers() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let builtin_root = tempfile::TempDir::new().expect("builtin layer root temp");
    let project_root = tempfile::TempDir::new().expect("project layer root temp");
    let cwd_dir = tempfile::TempDir::new().expect("process cwd temp");

    // The `file-notes` bundle writes against the process CWD; pin it to a temp
    // dir for the whole test so its note files land there and the real source
    // tree is never written to. The guard restores the CWD on drop, even on
    // panic; `#[serial]` keeps it from racing another CWD-touching test.
    let _cwd_guard = CurrentDirGuard::new(cwd_dir.path())
        .expect("pinning the process CWD to the temp dir should succeed");

    // Stage the two bundles EXACTLY as committed — the repo-root builtin probe
    // into the builtin layer, the `file-notes` example into the project layer.
    // No `{ rust }` id or server name is rewritten.
    stage_repo_builtin_probe(builtin_root.path());
    support::stage_example("file-notes", project_root.path());

    // The host's discovery layers, lowest precedence first: builtin then user
    // then project. The user layer stays empty — it is a real layer in the
    // stack the two staged bundles load around.
    let host = PluginHost::for_tests_with_builtin(
        builtin_root.path().to_path_buf(),
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Assert the point-in-time scan resolves each bundle from its own layer:
    // the probe from the builtin layer, `file-notes` from the project layer.
    // This is the same scan `discover_and_load_all` runs internally.
    assert_discovered_sources(
        &[
            LayerRoot::new(builtin_root.path(), FileSource::Builtin),
            LayerRoot::new(work_dir.path(), FileSource::User),
            LayerRoot::new(project_root.path(), FileSource::Local),
        ],
        &[
            (BUILTIN_PROBE, FileSource::Builtin),
            ("file-notes", FileSource::Local),
        ],
    );

    // Expose the in-process tool set the two bundles consume: the agent-mode
    // server registers BOTH the `kanban` module (the probe registers
    // `{ rust: "kanban" }`) and the `files` module (`file-notes` registers
    // `{ rust: "files" }`), so one `expose_tools_to_plugin_host` backs both.
    let server = support::build_mcp_server(work_dir.path()).await;
    tokio::time::timeout(support::TIMEOUT, server.expose_tools_to_plugin_host(&host))
        .await
        .expect("exposing the in-process tools should not hang")
        .expect("exposing the in-process tools should succeed");

    // One discovery pass loads BOTH bundles through the real stacked layers:
    // the probe from the builtin layer and `file-notes` from the project layer.
    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("co-loading the probe and file-notes across layers should succeed");
    assert_eq!(
        loaded.len(),
        2,
        "both committed bundles must load in one discovery pass, got {loaded:?}",
    );

    // Effect 1 — the builtin probe is live. It registered `{ rust: "kanban" }`
    // under the canonical server name `kanban`; routing a real `kanban`
    // `init board` call through that server creates a `.kanban` board on disk —
    // an effect only a discovered-and-loaded probe whose server is routable can
    // produce. The `kanban` tool resolves its board against the server's
    // `work_dir`.
    let probe_result = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            PROBE_SERVER,
            support::KANBAN_MODULE_ID,
            json!({ "op": "init board", "name": "Builtin Probe Board" }),
        ),
    )
    .await
    .expect("a kanban call through the probe server should not hang")
    .expect("the builtin probe's kanban server should be live and routable");
    let probe_rendered =
        serde_json::to_string(&probe_result).expect("a kanban result is serializable");
    assert!(
        probe_rendered.contains("Builtin Probe Board"),
        "the builtin probe must drive a real kanban effect, got {probe_rendered}",
    );
    assert!(
        work_dir.path().join(".kanban").is_dir(),
        "the builtin probe's kanban call must have created a real .kanban board",
    );

    // Effect 2 — the project-layer `file-notes` bundle ran. It wrote both note
    // files against relative paths resolved at the pinned process CWD; both
    // existing with the written body proves its `load()` ran end to end.
    let hello_path = cwd_dir.path().join(HELLO_NOTE);
    assert_eq!(
        std::fs::read_to_string(&hello_path).unwrap_or_else(|error| panic!(
            "the file-notes hello note must exist at {}: {error}",
            hello_path.display()
        )),
        NOTE_BODY,
        "the file-notes hello note must hold the body the plugin wrote",
    );
    let echo_path = cwd_dir.path().join(ECHO_NOTE);
    assert_eq!(
        std::fs::read_to_string(&echo_path).unwrap_or_else(|error| panic!(
            "the file-notes echo note must exist at {}: {error}",
            echo_path.display()
        )),
        NOTE_BODY,
        "the file-notes echo note must hold the read-back body",
    );

    // Unload both plugins. The host runs each plugin's optional `unload()` hook
    // and then disposes every registration it made.
    for plugin_id in &loaded {
        tokio::time::timeout(support::TIMEOUT, host.unload(plugin_id))
            .await
            .expect("unloading a co-loaded plugin should not hang")
            .expect("unloading a co-loaded plugin should succeed");
    }

    // After unload, neither plugin's server remains in the live registry — a
    // routed call to each fails as the registry's disposed-server tombstone.
    for server_name in [PROBE_SERVER, FILE_NOTES_SERVER] {
        let error = tokio::time::timeout(
            support::TIMEOUT,
            host.call(CallerId::HostInternal, server_name, "noop", json!({})),
        )
        .await
        .expect("a routed call after unload should not hang")
        .expect_err("a disposed plugin's server must no longer be routable");
        assert!(
            matches!(error, Error::ServerUnavailable | Error::UnknownServer),
            "server '{server_name}' must be gone from the registry after unload, got {error:?}",
        );
    }
}

/// Loads each remaining committed example from its own layer with a fresh host,
/// exercising discovery from the user, project, and builtin layer sources.
///
/// The three examples here — `kanban-tasks`, `multi-module`, `cli-echo` — all
/// consume the single-activation `{ rust: "kanban" }` module, so they cannot
/// co-load; each is loaded into its **own fresh [`PluginHost`]**. Each is staged
/// into a *different* layer so this one test exercises discovery from every
/// layer source:
///
/// - `kanban-tasks` from the **user** layer;
/// - `multi-module` from the **project** layer;
/// - `cli-echo` from the **builtin** layer.
///
/// For each, the point-in-time scan is asserted to resolve the bundle with the
/// expected [`FileSource`], `discover_and_load_all` is asserted to load exactly
/// it, and its observable effect is asserted — the two `kanban-tasks` tasks and
/// the one normalized `multi-module` task on their boards, and a live `echo`
/// round-trip for `cli-echo`. Every bundle is loaded EXACTLY as committed; the
/// only substitution is the `cli-echo` `__CLI_ECHO_COMMAND__` fixture-path
/// token, the bundle's own placeholder for a host-supplied binary path.
#[tokio::test]
#[serial_test::serial]
async fn each_committed_example_loads_from_its_layer() {
    // `cli-echo` writes nothing through the process CWD, but `file-notes`'
    // sibling test does; pinning the CWD here keeps this `#[serial]` test from
    // leaving the process CWD anywhere surprising for whatever runs next.
    let cwd_dir = tempfile::TempDir::new().expect("process cwd temp");
    let _cwd_guard = CurrentDirGuard::new(cwd_dir.path())
        .expect("pinning the process CWD to the temp dir should succeed");

    load_kanban_tasks_from_user_layer().await;
    load_multi_module_from_project_layer().await;
    load_cli_echo_from_builtin_layer().await;
}

/// Loads the committed `kanban-tasks` example from a temp **user** layer.
///
/// Stages the bundle EXACTLY as committed into a user layer root, asserts the
/// point-in-time scan resolves it as [`FileSource::User`], loads it through
/// `discover_and_load_all`, and asserts the board carries exactly the two tasks
/// the bundle's `index.ts` adds through the `_meta` path form — the observable
/// proof its `load()` ran.
async fn load_kanban_tasks_from_user_layer() {
    // Per-example isolation: a fresh host and fresh roots.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // The user layer IS the host's `work_dir` root: `for_tests` takes the user
    // root directly, so staging into `work_dir` puts the bundle in the user
    // layer's `plugins/` directory.
    support::stage_example("kanban-tasks", work_dir.path());

    let host = PluginHost::for_tests(work_dir.path().to_path_buf(), None);

    assert_discovered_sources(
        &[LayerRoot::new(work_dir.path(), FileSource::User)],
        &[("kanban-tasks", FileSource::User)],
    );

    // Expose the `kanban` tool the bundle consumes and seed an initialized
    // board — the `add task` op the bundle runs requires one.
    let kanban = support::expose_kanban_module(board_dir.path()).await;
    kanban
        .call(json!({ "op": "init board", "name": "kanban-tasks user-layer board" }))
        .await
        .expect("initializing the temp board should succeed");
    tokio::time::timeout(support::TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing the kanban module should not hang")
        .expect("exposing the kanban module should succeed");

    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("loading kanban-tasks from the user layer should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one kanban-tasks bundle must load, got {loaded:?}",
    );

    // The board holds exactly the two tasks the bundle adds — the observable
    // effect that proves its `load()` ran through the user-layer discovery.
    let listed = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    let titles = support::task_titles(&listed);
    assert_eq!(
        titles.len(),
        2,
        "the kanban-tasks bundle must add exactly two tasks, got {titles:?}",
    );
    for expected in KANBAN_TASKS_TITLES {
        assert!(
            titles.iter().any(|title| title == expected),
            "the board must carry the kanban-tasks task '{expected}', got {titles:?}",
        );
    }
}

/// Loads the committed `multi-module` example from a temp **project** layer.
///
/// Stages the multi-file bundle EXACTLY as committed into a project layer root,
/// asserts the point-in-time scan resolves it as [`FileSource::Local`], loads it
/// through `discover_and_load_all`, and asserts the board carries exactly the
/// one normalized task the bundle's imported `board-helpers.ts` helper adds —
/// the observable proof the relative sibling import resolved and its code ran.
async fn load_multi_module_from_project_layer() {
    // Per-example isolation: a fresh host and fresh roots.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project layer root temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // `stage_example` copies the whole bundle directory recursively, so the
    // sibling `board-helpers.ts` module lands alongside `index.ts`.
    support::stage_example("multi-module", project_root.path());

    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    assert_discovered_sources(
        &[
            LayerRoot::new(work_dir.path(), FileSource::User),
            LayerRoot::new(project_root.path(), FileSource::Local),
        ],
        &[("multi-module", FileSource::Local)],
    );

    let kanban = support::expose_kanban_module(board_dir.path()).await;
    kanban
        .call(json!({ "op": "init board", "name": "multi-module project-layer board" }))
        .await
        .expect("initializing the temp board should succeed");
    tokio::time::timeout(support::TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing the kanban module should not hang")
        .expect("exposing the kanban module should succeed");

    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("loading multi-module from the project layer should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one multi-module bundle must load, got {loaded:?}",
    );

    // The board holds exactly the one task the imported helper produced — the
    // observable effect that proves the relative sibling import resolved.
    let listed = kanban
        .call(json!({ "op": "list tasks" }))
        .await
        .expect("listing tasks on the temp board should succeed");
    assert_eq!(
        support::task_titles(&listed),
        vec![MULTI_MODULE_TITLE.to_string()],
        "the multi-module bundle's imported helper must have added exactly its one task",
    );
}

/// Loads the committed `cli-echo` example from a temp **builtin** layer.
///
/// Stages the bundle into a builtin layer root, rewriting only the
/// `__CLI_ECHO_COMMAND__` fixture-path token — the bundle's own placeholder for
/// a host-supplied binary path, not a `{ rust }` id or server-name rewrite.
/// Asserts the point-in-time scan resolves the bundle as [`FileSource::Builtin`],
/// loads it through `discover_and_load_all`, and asserts the registered `echo`
/// server round-trips a real `tools/call` over its stdio subprocess — the
/// observable proof the builtin-layer bundle loaded and its `{ cli }` transport
/// is live.
async fn load_cli_echo_from_builtin_layer() {
    // Per-example isolation: a fresh host and fresh roots.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let builtin_root = tempfile::TempDir::new().expect("builtin layer root temp");
    let board_dir = tempfile::TempDir::new().expect("kanban board temp");

    // Cargo sets `CARGO_BIN_EXE_<name>` for every binary target, so this always
    // points at the freshly built fixture stdio MCP server.
    let fixture_binary = env!("CARGO_BIN_EXE_cli_server_fixture");

    // Stage the bundle EXACTLY as committed, substituting only the fixture-path
    // token — the bundle's own placeholder for the host-supplied command path.
    support::stage_example_with(
        "cli-echo",
        builtin_root.path(),
        &[(CLI_COMMAND_TOKEN, fixture_binary)],
    );

    let host = PluginHost::for_tests_with_builtin(
        builtin_root.path().to_path_buf(),
        work_dir.path().to_path_buf(),
        None,
    );

    assert_discovered_sources(
        &[
            LayerRoot::new(builtin_root.path(), FileSource::Builtin),
            LayerRoot::new(work_dir.path(), FileSource::User),
        ],
        &[("cli-echo", FileSource::Builtin)],
    );

    // `cli-echo` registers `{ rust: "kanban" }` as `board` (its `unload()` uses
    // it) alongside the `{ cli }` `echo` server, so the `kanban` module must be
    // exposed for `load()` to succeed.
    let kanban = support::expose_kanban_module(board_dir.path()).await;
    kanban
        .call(json!({ "op": "init board", "name": "cli-echo builtin-layer board" }))
        .await
        .expect("initializing the temp board should succeed");
    tokio::time::timeout(support::TIMEOUT, kanban.expose_to(&host))
        .await
        .expect("exposing the kanban module should not hang")
        .expect("exposing the kanban module should succeed");

    let loaded = tokio::time::timeout(
        support::TIMEOUT,
        host.discover_and_load_all::<SwissarmyhammerConfig>(),
    )
    .await
    .expect("discovery should not hang")
    .expect("loading cli-echo from the builtin layer should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one cli-echo bundle must load, got {loaded:?}",
    );

    // The `echo` server the bundle registered is live: routing a real
    // `tools/call` over its spawned stdio subprocess echoes the payload back —
    // the observable effect that proves the builtin-layer bundle loaded.
    let payload = "payload routed over the builtin-layer cli-echo subprocess";
    let echoed = tokio::time::timeout(
        support::TIMEOUT,
        host.call(
            CallerId::HostInternal,
            ECHO_SERVER,
            ECHO_SERVER,
            json!({ "message": payload }),
        ),
    )
    .await
    .expect("an echo call over the cli subprocess should not hang")
    .expect("the cli-echo bundle's echo server should be live and routable");
    let rendered = serde_json::to_string(&echoed).expect("an echo result is serializable");
    assert!(
        rendered.contains(payload),
        "the echoed payload must round-trip back over stdio, got {rendered}",
    );
}
