//! Point-in-time plugin discovery across stacked layer roots.
//!
//! Plugins live on disk under the `plugins/` subdirectory of whichever layer
//! they ship in — builtin, user, or project — exactly the way skills, prompts,
//! and every other user-editable resource stack. This module scans those
//! layers once and resolves, for each plugin [`id`](crate::manifest::Manifest::id),
//! the single highest-precedence copy.
//!
//! # Host-agnostic by construction
//!
//! Discovery is generic over [`C: DirectoryConfig`](DirectoryConfig). The only
//! literal the platform bakes in is the `plugins/` subdirectory name; the
//! directory *config* — which decides where a host's user and project layers
//! live — is the host's choice, supplied as `C`. The platform hardcodes no
//! `.sah`-specific path: `SwissarmyhammerConfig` is one config a host may use,
//! and a different host (the kanban app) supplies its own.
//!
//! `swissarmyhammer-directory`'s [`VirtualFileSystem<C>`] resolves which of the
//! supplied layer roots actually exist on disk, in precedence order; discovery
//! then walks each resolved `plugins/` directory for plugin bundles.
//!
//! # Precedence
//!
//! Project shadows user shadows builtin. When one `id` appears in more than one
//! layer, the highest-precedence copy is the active one and the rest are
//! shadowed. Reacting to a layer appearing or disappearing — re-emerging a
//! shadowed copy — is hot reload, a separate concern; this module is the
//! point-in-time scan that picks the current winner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use swissarmyhammer_directory::{DirectoryConfig, FileSource, VirtualFileSystem};

use crate::error::Result;
use crate::manifest::{Manifest, MANIFEST_FILE};

/// The subdirectory, under each layer root, that holds plugin bundles.
///
/// A layer root contributes plugins from `<layer_root>/plugins/`; each
/// immediate subdirectory of that is one plugin bundle. This is the single
/// path literal the platform bakes in — everything else about *where* a layer
/// lives is the host's [`DirectoryConfig`] choice.
pub const PLUGINS_SUBDIR: &str = "plugins";

/// One plugin layer: a root directory paired with its precedence source.
///
/// The host supplies its layer roots as a precedence-ordered list of these —
/// builtin lowest, then user, then project. The `root` is the layer's *base*
/// directory; plugin bundles are found under its [`PLUGINS_SUBDIR`].
#[derive(Debug, Clone)]
pub struct LayerRoot {
    /// The layer's base directory. Plugin bundles live under `root/plugins/`.
    pub root: PathBuf,

    /// The layer's precedence source — [`Builtin`](FileSource::Builtin) lowest,
    /// then [`User`](FileSource::User), then [`Local`](FileSource::Local) for
    /// the project layer.
    pub source: FileSource,
}

impl LayerRoot {
    /// Builds a layer root from a base directory and its precedence source.
    pub fn new(root: impl Into<PathBuf>, source: FileSource) -> Self {
        Self {
            root: root.into(),
            source,
        }
    }
}

/// A plugin resolved by discovery: its manifest, its directory, and its layer.
///
/// One `DiscoveredPlugin` is the *active* copy of a plugin id — the
/// highest-precedence copy found across all scanned layers. Lower-precedence
/// copies of the same id are shadowed and do not appear in discovery output.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// The parsed `plugin.json` of the active copy.
    pub manifest: Manifest,

    /// The active copy's bundle directory — the directory containing its
    /// `plugin.json` and its entry module.
    pub directory: PathBuf,

    /// The layer the active copy was found in.
    pub source: FileSource,
}

impl DiscoveredPlugin {
    /// The plugin's identity — the manifest `id`, authoritative across layers.
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
}

/// Scans the supplied layer roots and resolves the active copy of every plugin.
///
/// The `layers` are scanned lowest-precedence first; a later layer's copy of an
/// `id` shadows an earlier layer's copy. The returned vector holds exactly one
/// [`DiscoveredPlugin`] per distinct `id` — the winning copy — and is sorted by
/// `id` so the result is deterministic regardless of filesystem iteration
/// order.
///
/// # Type Parameters
///
/// - `C` — the host's [`DirectoryConfig`]. It parameterizes the
///   [`VirtualFileSystem`] used to resolve which layer directories exist, so
///   the platform stays host-agnostic: the config, not a hardcoded path, names
///   the host's directories.
///
/// # Parameters
///
/// - `layers` — the plugin layers in precedence order, lowest first. A layer
///   whose `plugins/` directory does not exist contributes nothing.
///
/// # Errors
///
/// Returns [`Error::Manifest`](crate::Error::Manifest) for the first plugin
/// directory whose `plugin.json` is unreadable, malformed, or missing a
/// required field. A broken manifest fails discovery loudly rather than being
/// silently skipped.
pub fn discover_plugins<C: DirectoryConfig>(layers: &[LayerRoot]) -> Result<Vec<DiscoveredPlugin>> {
    // Resolve, in precedence order, the `plugins/` directories that exist.
    let plugin_dirs = resolve_plugin_dirs::<C>(layers);

    // Walk each layer's `plugins/` directory; a later (higher-precedence) layer
    // overwrites an earlier copy of the same id.
    let mut by_id: HashMap<String, DiscoveredPlugin> = HashMap::new();
    for (plugins_dir, source) in plugin_dirs {
        for plugin in scan_layer(&plugins_dir, source)? {
            by_id.insert(plugin.manifest.id.clone(), plugin);
        }
    }

    let mut discovered: Vec<DiscoveredPlugin> = by_id.into_values().collect();
    discovered.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(discovered)
}

/// Resolves, in precedence order, the `plugins/` directories that exist on disk.
///
/// A [`VirtualFileSystem<C>`] scoped to [`PLUGINS_SUBDIR`] does the resolution:
/// each layer root is added as a search path tagged with its [`FileSource`],
/// and `get_search_paths` returns only the directories that exist, preserving
/// the supplied precedence order. Using the `swissarmyhammer-directory`
/// machinery keeps discovery generic over `C` rather than re-implementing layer
/// resolution.
fn resolve_plugin_dirs<C: DirectoryConfig>(layers: &[LayerRoot]) -> Vec<(PathBuf, FileSource)> {
    let mut vfs = VirtualFileSystem::<C>::new(PLUGINS_SUBDIR);
    for layer in layers {
        // The search path is the layer's `plugins/` directory itself; the VFS
        // reports back exactly the paths that exist, in insertion order.
        vfs.add_search_path(layer.root.join(PLUGINS_SUBDIR), layer.source.clone());
    }
    vfs.get_search_paths()
        .into_iter()
        .map(|search_path| (search_path.path, search_path.source))
        .collect()
}

/// Scans one layer's `plugins/` directory for plugin bundles.
///
/// Each immediate subdirectory that contains a [`MANIFEST_FILE`] is one plugin
/// bundle; its manifest is parsed and a [`DiscoveredPlugin`] produced. A
/// subdirectory without a manifest is not a plugin and is skipped — the
/// directory may hold unrelated content — but the skip is logged at debug
/// level so a misconfigured bundle (one that looks like a plugin but has a
/// missing or misnamed `plugin.json`) is observable rather than invisible.
///
/// # Errors
///
/// Returns [`Error::Manifest`](crate::Error::Manifest) for the first bundle
/// whose `plugin.json` is present but unreadable or invalid.
fn scan_layer(plugins_dir: &Path, source: FileSource) -> Result<Vec<DiscoveredPlugin>> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(plugins_dir) else {
        // A `plugins/` directory that vanished between resolution and scan
        // contributes nothing rather than failing the whole discovery.
        return Ok(found);
    };

    for entry in entries.flatten() {
        let directory = entry.path();
        if !directory.is_dir() {
            continue;
        }
        // A subdirectory without a manifest is not a plugin bundle. Log the
        // skip so a directory that was meant to be a plugin but lacks a
        // (correctly named) `plugin.json` is diagnosable.
        if !directory.join(MANIFEST_FILE).is_file() {
            tracing::debug!(
                directory = %directory.display(),
                manifest = MANIFEST_FILE,
                "skipping a plugins/ subdirectory with no manifest; not a plugin bundle"
            );
            continue;
        }
        let manifest = Manifest::load(&directory)?;
        found.push(DiscoveredPlugin {
            manifest,
            directory,
            source: source.clone(),
        });
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_directory::SwissarmyhammerConfig;
    use tempfile::TempDir;

    /// Writes a plugin bundle — a `plugin.json` and an empty entry file — into
    /// `layer_root/plugins/<dir_name>/`.
    fn write_bundle(layer_root: &Path, dir_name: &str, id: &str, provides: &[&str]) {
        let plugin_dir = layer_root.join(PLUGINS_SUBDIR).join(dir_name);
        std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
        let provides_json = serde_json::to_string(provides).expect("provides serializes");
        let manifest = format!(
            "{{\"id\":\"{id}\",\"name\":\"{id}\",\"version\":\"1.0.0\",\
             \"entry\":\"entry.ts\",\"provides\":{provides_json}}}"
        );
        std::fs::write(plugin_dir.join(MANIFEST_FILE), manifest).expect("manifest");
        std::fs::write(plugin_dir.join("entry.ts"), "export function load() {}").expect("entry");
    }

    /// A single plugin in one layer is discovered, keyed by its manifest id.
    #[test]
    fn discovers_a_plugin_in_one_layer() {
        let project = TempDir::new().expect("temp dir");
        // The disk directory name differs from the manifest id.
        write_bundle(project.path(), "weather-dir", "weather", &["weather"]);

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].id(),
            "weather",
            "identity follows the manifest"
        );
        assert_eq!(discovered[0].source, FileSource::Local);
    }

    /// When one id exists in two layers, the higher-precedence copy wins and
    /// the shadowed copy does not appear.
    #[test]
    fn higher_precedence_layer_shadows_a_lower_one() {
        let user = TempDir::new().expect("user temp dir");
        let project = TempDir::new().expect("project temp dir");
        write_bundle(user.path(), "shared", "shared", &["from-user"]);
        write_bundle(project.path(), "shared", "shared", &["from-project"]);

        // User is lower precedence than project.
        let layers = vec![
            LayerRoot::new(user.path(), FileSource::User),
            LayerRoot::new(project.path(), FileSource::Local),
        ];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1, "a shadowed id resolves to one copy");
        assert_eq!(
            discovered[0].source,
            FileSource::Local,
            "the project copy must shadow the user copy"
        );
        assert_eq!(
            discovered[0].manifest.provides,
            vec!["from-project".to_string()],
            "the active copy must be the project layer's manifest"
        );
    }

    /// A subdirectory of `plugins/` without a `plugin.json` is not a plugin and
    /// is skipped without error.
    #[test]
    fn a_directory_without_a_manifest_is_skipped() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "real", "real", &["real"]);
        // A bare directory with no manifest.
        std::fs::create_dir_all(project.path().join(PLUGINS_SUBDIR).join("not-a-plugin"))
            .expect("non-plugin dir");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");
        assert_eq!(
            discovered.len(),
            1,
            "only the bundle with a manifest counts"
        );
        assert_eq!(discovered[0].id(), "real");
    }

    /// A bundle whose `plugin.json` is invalid fails discovery loudly.
    #[test]
    fn an_invalid_manifest_fails_discovery() {
        let project = TempDir::new().expect("temp dir");
        let plugin_dir = project.path().join(PLUGINS_SUBDIR).join("broken");
        std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
        std::fs::write(plugin_dir.join(MANIFEST_FILE), "{ not json")
            .expect("write broken manifest");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let error = discover_plugins::<SwissarmyhammerConfig>(&layers)
            .expect_err("an invalid manifest must fail discovery");
        assert!(
            matches!(error, crate::Error::Manifest(_)),
            "a broken manifest must surface as Error::Manifest, got: {error:?}"
        );
    }

    /// A layer whose `plugins/` directory does not exist contributes nothing
    /// rather than failing.
    #[test]
    fn a_missing_plugins_directory_contributes_nothing() {
        let empty = TempDir::new().expect("temp dir");
        let layers = vec![LayerRoot::new(empty.path(), FileSource::User)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");
        assert!(
            discovered.is_empty(),
            "a layer with no plugins/ directory yields nothing"
        );
    }
}
