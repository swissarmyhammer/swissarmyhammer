//! Point-in-time plugin discovery across stacked layer roots.
//!
//! Plugins live on disk under the `plugins/` subdirectory of whichever layer
//! they ship in — builtin, user, or project — exactly the way skills, prompts,
//! and every other user-editable resource stack. This module scans those
//! layers once and resolves, for each plugin id — the bundle directory name —
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

use crate::error::{Error, Result};

/// The entry-module filenames a plugin bundle may use, in precedence order.
///
/// A TypeScript-only bundle's entry module is found by convention. `index.ts`
/// is preferred — the plugin is authored in TypeScript — and `index.js` is the
/// fallback for a bundle that ships pre-compiled JavaScript. The first of
/// these that exists in a bundle directory is its entry.
const INDEX_ENTRY_FILES: [&str; 2] = ["index.ts", "index.js"];

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

/// A plugin resolved by discovery: its identity, entry module, directory, and
/// layer.
///
/// One `DiscoveredPlugin` is the *active* copy of a plugin id — the
/// highest-precedence copy found across all scanned layers. Lower-precedence
/// copies of the same id are shadowed and do not appear in discovery output.
///
/// A bundle is a directory containing an `index.ts` (preferred) or `index.js`
/// entry module; its identity is the bundle directory name.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// The plugin's identity, authoritative across layers.
    ///
    /// The bundle directory name. Discovery keys plugins by this value, so the
    /// precedence rules pick a single winner per identity.
    pub id: String,

    /// The active copy's entry module — an absolute path proven to be contained
    /// within the bundle [`directory`](Self::directory).
    ///
    /// The bundle's `index.ts` (or `index.js`), canonicalized and
    /// containment-checked.
    pub entry: PathBuf,

    /// The active copy's bundle directory — the directory containing its
    /// entry module.
    pub directory: PathBuf,

    /// The layer the active copy was found in.
    pub source: FileSource,
}

impl DiscoveredPlugin {
    /// The plugin's identity — authoritative across layers.
    ///
    /// The bundle directory name.
    pub fn id(&self) -> &str {
        &self.id
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
/// Returns [`Error::BundleError`](crate::Error::BundleError) for the first
/// plugin directory whose entry module cannot be resolved — for example a
/// symlinked `index.ts` that escapes the bundle.
pub fn discover_plugins<C: DirectoryConfig>(layers: &[LayerRoot]) -> Result<Vec<DiscoveredPlugin>> {
    // Resolve, in precedence order, the `plugins/` directories that exist.
    let plugin_dirs = resolve_plugin_dirs::<C>(layers);

    // Walk each layer's `plugins/` directory; a later (higher-precedence) layer
    // overwrites an earlier copy of the same id.
    let mut by_id: HashMap<String, DiscoveredPlugin> = HashMap::new();
    for (plugins_dir, source) in plugin_dirs {
        for plugin in scan_layer(&plugins_dir, source)? {
            by_id.insert(plugin.id.clone(), plugin);
        }
    }

    let mut discovered: Vec<DiscoveredPlugin> = by_id.into_values().collect();
    discovered.sort_by(|a, b| a.id.cmp(&b.id));
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
/// Each immediate subdirectory is one plugin bundle when it carries an
/// `index.ts` or `index.js` entry module. A subdirectory with neither is not a
/// plugin and is skipped — the directory may hold unrelated content — but the
/// skip is logged at debug level so a misconfigured bundle (one meant to be a
/// plugin but lacking an `index.{ts,js}`) is observable rather than invisible.
///
/// # Errors
///
/// Returns [`Error::BundleError`](crate::Error::BundleError) for the first
/// bundle whose entry module — `index.{ts,js}` — cannot be resolved within the
/// bundle directory.
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
        if let Some(plugin) = scan_bundle(&directory, &source)? {
            found.push(plugin);
        }
    }
    Ok(found)
}

/// Resolves a single `plugins/` subdirectory into a [`DiscoveredPlugin`], or
/// `None` when the directory is not a plugin bundle.
///
/// A directory with an `index.ts` (or `index.js`) is a plugin bundle: its
/// identity is the directory name and the `index.{ts,js}` is its entry. A
/// directory with neither is not a plugin; it is skipped, and the skip is
/// logged so a misconfigured bundle is diagnosable.
///
/// # Errors
///
/// Returns [`Error::BundleError`](crate::Error::BundleError) when the resolved
/// entry module escapes the bundle directory.
fn scan_bundle(directory: &Path, source: &FileSource) -> Result<Option<DiscoveredPlugin>> {
    if let Some(entry) = resolve_index_entry(directory)? {
        // A bundle's identity is its directory name; a directory the
        // filesystem yielded always has a final component.
        let id = directory
            .file_name()
            .expect("a directory entry has a file name")
            .to_string_lossy()
            .into_owned();
        return Ok(Some(DiscoveredPlugin {
            id,
            entry,
            directory: directory.to_path_buf(),
            source: source.clone(),
        }));
    }

    // No index entry: not a plugin bundle. Log the skip so a directory meant
    // to be a plugin but missing one is diagnosable.
    tracing::debug!(
        directory = %directory.display(),
        index_entries = ?INDEX_ENTRY_FILES,
        "skipping a plugins/ subdirectory with no index entry; not a plugin bundle"
    );
    Ok(None)
}

/// Resolves a TypeScript-only bundle's entry module — `index.ts`, else
/// `index.js` — to a path proven to be contained within the bundle directory.
///
/// The `index.{ts,js}` filenames are a fixed convention, not plugin-authored
/// content, so the path itself cannot traverse out of the bundle. But the file
/// on disk may be a *symlink* pointing outside the bundle, so this
/// canonicalizes both the bundle root and the entry path — collapsing
/// symlinks — and rejects any entry that resolves outside the bundle.
///
/// Shared with [`PluginHost::load`](crate::PluginHost::load), which resolves a
/// by-path bundle's entry the same way discovery does, so a bundle loaded
/// directly and a bundle loaded through a discovery scan find their `index.ts`
/// identically.
///
/// # Returns
///
/// `Some` with the canonicalized, bundle-contained absolute entry path when an
/// `index.ts` or `index.js` exists, `None` when the bundle has neither.
///
/// # Errors
///
/// Returns [`Error::BundleError`](crate::Error::BundleError) when the bundle
/// directory or the index file cannot be canonicalized, or when the
/// canonicalized index entry escapes the canonicalized bundle root.
pub(crate) fn resolve_index_entry(directory: &Path) -> Result<Option<PathBuf>> {
    let Some(index_file) = INDEX_ENTRY_FILES
        .iter()
        .find(|name| directory.join(name).is_file())
    else {
        return Ok(None);
    };

    // Canonicalize the bundle root so the containment check compares
    // like-for-like: both sides have their symlinks and `..` collapsed.
    let bundle_root = directory.canonicalize().map_err(|error| {
        Error::BundleError(format!(
            "plugin at {}: cannot resolve bundle directory: {error}",
            directory.display(),
        ))
    })?;

    let canonical_entry = bundle_root
        .join(index_file)
        .canonicalize()
        .map_err(|error| {
            Error::BundleError(format!(
                "plugin at {}: cannot resolve entry '{index_file}': {error}",
                directory.display(),
            ))
        })?;

    // A symlinked `index.{ts,js}` could otherwise point outside the bundle;
    // reject an entry that escapes.
    if !canonical_entry.starts_with(&bundle_root) {
        return Err(Error::BundleError(format!(
            "plugin at {}: entry '{index_file}' escapes the plugin bundle directory",
            directory.display(),
        )));
    }

    Ok(Some(canonical_entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_directory::SwissarmyhammerConfig;
    use tempfile::TempDir;

    /// Writes a TS-only plugin bundle — just an `index.ts` entry — into
    /// `layer_root/plugins/<dir_name>/`.
    fn write_bundle(layer_root: &Path, dir_name: &str, entry_file: &str) {
        let plugin_dir = layer_root.join(PLUGINS_SUBDIR).join(dir_name);
        std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
        std::fs::write(plugin_dir.join(entry_file), "export function load() {}")
            .expect("index entry");
    }

    /// A single plugin in one layer is discovered, keyed by its directory name.
    #[test]
    fn discovers_a_plugin_in_one_layer() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "weather", "index.ts");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].id(),
            "weather",
            "identity is the bundle directory name"
        );
        assert_eq!(discovered[0].source, FileSource::Local);
    }

    /// When one id exists in two layers, the higher-precedence copy wins and
    /// the shadowed copy does not appear.
    #[test]
    fn higher_precedence_layer_shadows_a_lower_one() {
        let user = TempDir::new().expect("user temp dir");
        let project = TempDir::new().expect("project temp dir");
        write_bundle(user.path(), "shared", "index.ts");
        write_bundle(project.path(), "shared", "index.ts");

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
    }

    /// A subdirectory of `plugins/` with no `index.{ts,js}` entry is not a
    /// plugin and is skipped without error.
    #[test]
    fn a_directory_without_an_index_entry_is_skipped() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "real", "index.ts");
        // A bare directory with no index entry.
        std::fs::create_dir_all(project.path().join(PLUGINS_SUBDIR).join("not-a-plugin"))
            .expect("non-plugin dir");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");
        assert_eq!(
            discovered.len(),
            1,
            "only the bundle with an index entry counts"
        );
        assert_eq!(discovered[0].id(), "real");
    }

    /// A `plugins/<dir>/` directory carrying an `index.ts` is discovered:
    /// its `id` is the bundle directory name and its entry is the `index.ts`.
    #[test]
    fn an_index_ts_bundle_is_discovered_with_dir_name_id() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "ts-only", "index.ts");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].id(),
            "ts-only",
            "a plugin's identity is its bundle directory name"
        );
        assert!(
            discovered[0].entry.ends_with("index.ts"),
            "the resolved entry must be the bundle's index.ts, got: {}",
            discovered[0].entry.display()
        );
        assert!(
            discovered[0].entry.is_file(),
            "the resolved entry must be the real index.ts file"
        );
    }

    /// When a bundle has no `index.ts`, its `index.js` is used as the entry
    /// instead.
    #[test]
    fn a_bundle_falls_back_to_index_js() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "js-only", "index.js");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].id(), "js-only");
        assert!(
            discovered[0].entry.ends_with("index.js"),
            "index.js must be the entry when no index.ts is present, got: {}",
            discovered[0].entry.display()
        );
    }

    /// When a bundle carries both `index.ts` and `index.js`, the `index.ts`
    /// is preferred.
    #[test]
    fn index_ts_is_preferred_over_index_js() {
        let project = TempDir::new().expect("temp dir");
        write_bundle(project.path(), "both", "index.ts");
        write_bundle(project.path(), "both", "index.js");

        let layers = vec![LayerRoot::new(project.path(), FileSource::Local)];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(discovered.len(), 1);
        assert!(
            discovered[0].entry.ends_with("index.ts"),
            "index.ts must be preferred over index.js, got: {}",
            discovered[0].entry.display()
        );
    }

    /// A bundle in one layer is shadowed by a higher-precedence copy with the
    /// same directory name in another layer — its identity is the directory
    /// name, so layer stacking keys off that.
    #[test]
    fn a_bundle_shadows_across_layers_by_dir_name() {
        let user = TempDir::new().expect("user temp dir");
        let project = TempDir::new().expect("project temp dir");
        write_bundle(user.path(), "shared", "index.ts");
        write_bundle(project.path(), "shared", "index.ts");

        let layers = vec![
            LayerRoot::new(user.path(), FileSource::User),
            LayerRoot::new(project.path(), FileSource::Local),
        ];
        let discovered =
            discover_plugins::<SwissarmyhammerConfig>(&layers).expect("discovery should succeed");

        assert_eq!(
            discovered.len(),
            1,
            "an id shared across layers resolves to one copy"
        );
        assert_eq!(discovered[0].id(), "shared");
        assert_eq!(
            discovered[0].source,
            FileSource::Local,
            "the project copy must shadow the user copy"
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
