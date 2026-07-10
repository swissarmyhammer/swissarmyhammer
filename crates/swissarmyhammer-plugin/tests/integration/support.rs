//! Shared test-support for the SDK-helper integration tests.
//!
//! The helpers here cover the boilerplate every test in this suite repeats:
//! standing up a [`PluginHost`] paired with a bootstrapped command service,
//! staging committed example bundles into a project-layer plugin root, and
//! reading the command registry's listing back to assert what each plugin
//! actually registered.

#![allow(dead_code)] // shared by multiple submodules

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginHost, PLUGINS_SUBDIR};
use tempfile::TempDir;

/// A generous upper bound on any single host interaction. Mirrors the
/// `TIMEOUT` const the reference end-to-end tests use.
pub const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The directory holding the committed example plugin bundles.
///
/// Resolves to `<CARGO_MANIFEST_DIR>/examples/plugins`, the home for every
/// example bundle this crate ships. A bundle lives in a `<name>/` subdirectory
/// of this root and carries a real `index.ts` entry module.
pub fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plugins")
}

/// Stage a committed example bundle into a project-layer plugin root.
///
/// Recursively copies the committed bundle
/// `examples_root()/<name>/` into `<project_root>/plugins/<name>/`, so the
/// staged copy is discoverable from a host whose project layer is pointed at
/// `project_root`. The committed source stays read-only — only the temp copy
/// is touched.
///
/// Returns the staged bundle's absolute path so callers can `host.load(...)`
/// it directly, bypassing layered discovery when per-bundle outcomes need to
/// be observed independently.
pub fn stage_example(name: &str, project_root: &Path) -> PathBuf {
    let source = examples_root().join(name);
    assert!(
        source.is_dir(),
        "example bundle '{name}' must exist at {} to be staged",
        source.display(),
    );
    let destination = project_root.join(PLUGINS_SUBDIR).join(name);
    copy_dir_recursive(&source, &destination);
    destination
}

/// Recursively copy the directory tree at `source` to `destination`.
fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap_or_else(|error| {
        panic!(
            "staging directory {} should be created: {error}",
            destination.display(),
        )
    });
    let entries = std::fs::read_dir(source).unwrap_or_else(|error| {
        panic!(
            "example bundle {} should be readable: {error}",
            source.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("a directory entry should be readable");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!(
                    "example file {} should copy to {}: {error}",
                    from.display(),
                    to.display(),
                )
            });
        }
    }
}

/// One-stop scaffolding for an SDK-helper integration test.
///
/// Owns the temp roots (kept alive for the test's duration), the host built
/// against them, and the bootstrapped command-service handle. Tests drive the
/// service through the host's `commands` server activated by a probe plugin's
/// `ensureServices` call, and read the registry back through the shared
/// `service` handle directly.
pub struct BootstrappedHost {
    /// The temp user-layer root; kept alive so its directory survives the
    /// test.
    pub _user_root: TempDir,
    /// The temp project-layer root; kept alive for the same reason.
    pub _project_root: TempDir,
    /// The live plugin host the bootstrap wired into.
    pub host: PluginHost,
    /// Shared handle to the bootstrapped command service.
    pub service: Arc<CommandService>,
}

impl BootstrappedHost {
    /// Build a fresh host with a project-layer root, install the commands
    /// module, and return all four handles wrapped.
    pub async fn new() -> Self {
        let user_root = TempDir::new().expect("user root temp dir");
        let project_root = TempDir::new().expect("project root temp dir");
        let host = PluginHost::for_tests(
            user_root.path().to_path_buf(),
            Some(project_root.path().to_path_buf()),
        );
        let service = install_commands_module(&host)
            .await
            .expect("install_commands_module must succeed");
        Self {
            _user_root: user_root,
            _project_root: project_root,
            host,
            service,
        }
    }

    /// The temp project-layer root, for staging bundles into.
    pub fn project_root(&self) -> &Path {
        self._project_root.path()
    }
}

/// Snapshot every active command id from `service`'s registry, sorted
/// alphabetically for stable assertions.
///
/// Reads the command service's [`CommandRegistry`] directly through the
/// service handle's `with_registry` accessor — independent of whether the
/// host's `commands` server is currently activated. This matters at
/// post-unload assertion time: the last plugin unloading drops the
/// refcounted hold on the `commands` registration, so a `host.call(...)`
/// against `commands` afterward fails with [`Error::ServerUnavailable`],
/// while the service itself (held alive by the test's [`Arc`]) still
/// reflects the registry's post-purge state.
///
/// [`CommandRegistry`]: swissarmyhammer_command_service::CommandRegistry
/// [`Error::ServerUnavailable`]: swissarmyhammer_plugin::Error::ServerUnavailable
pub fn list_command_ids(service: &CommandService) -> Vec<String> {
    let mut ids: Vec<String> = service.with_registry(|registry| {
        registry
            .list()
            .into_iter()
            .map(|entry| entry.registration.id.clone())
            .collect()
    });
    ids.sort();
    ids
}

/// Call the `list command` operation on the host's `commands` server.
///
/// Used by the smoke check that exercises the production path — the same
/// route a host caller would use to drive `list command` through the
/// activated server. The call is attributed to [`CallerId::HostInternal`]
/// so the listing is unfiltered.
///
/// Returns the raw `CallToolResult`-shaped JSON the host's `call` produces,
/// or the host error when the `commands` server is not (or no longer) live.
pub async fn list_via_host(host: &PluginHost) -> swissarmyhammer_plugin::Result<Value> {
    host.call(
        CallerId::HostInternal,
        "commands",
        "command",
        serde_json::json!({ "op": "list command" }),
    )
    .await
}
