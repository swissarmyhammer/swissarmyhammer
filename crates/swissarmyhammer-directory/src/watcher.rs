//! Stack-aware filesystem watcher for managed configuration directories.
//!
//! [`Watcher`] watches a single subdirectory (such as `plugins`) across every
//! *writable* layer that a [`DirectoryConfig`] exposes — the user XDG data
//! layer and the project (git-root) layer — and emits a [`StackedEvent`] each
//! time a top-level entry within that subdirectory changes.
//!
//! The builtin layer is read-only (its files live in the binary, not on disk)
//! and is therefore never watched.
//!
//! # Coalescing
//!
//! Saving a plugin commonly touches several files at once — a manifest plus a
//! handful of source files. The watcher debounces these into a single
//! [`StackedEvent`] for the affected entry rather than one event per file, so
//! consumers reload each entry exactly once per burst.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_watcher::notify::EventKind;
use async_watcher::DebouncedEvent;
use tokio::sync::mpsc;

use crate::async_watch::{top_level_entry, DebouncedDirWatcher, DEFAULT_DEBOUNCE};
use crate::config::DirectoryConfig;
use crate::directory::ManagedDirectory;
use crate::error::Result;
use crate::file_loader::FileSource;

/// How a single layer's copy of a watched entry changed.
///
/// Each variant carries the [`FileSource`] of the layer the change occurred in
/// and the filesystem `path` that triggered it.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerChange {
    /// A new file or directory appeared under the entry.
    Added {
        /// The layer the change occurred in (`User` or `Local`).
        layer: FileSource,
        /// The filesystem path that triggered the change.
        path: PathBuf,
    },
    /// An existing file or directory under the entry was modified.
    Modified {
        /// The layer the change occurred in (`User` or `Local`).
        layer: FileSource,
        /// The filesystem path that triggered the change.
        path: PathBuf,
    },
    /// A file or directory under the entry was removed.
    Removed {
        /// The layer the change occurred in (`User` or `Local`).
        layer: FileSource,
        /// The filesystem path that triggered the change.
        path: PathBuf,
    },
}

impl LayerChange {
    /// The [`FileSource`] layer this change belongs to.
    pub fn layer(&self) -> &FileSource {
        match self {
            LayerChange::Added { layer, .. }
            | LayerChange::Modified { layer, .. }
            | LayerChange::Removed { layer, .. } => layer,
        }
    }
}

/// A debounced, stack-aware change to one entry within a watched subdirectory.
///
/// A `StackedEvent` identifies *which* top-level entry changed (by
/// `subdirectory` and `name`) and *how* it changed in *which* layer (via
/// [`LayerChange`]). The `name` is always the top-level entry within the
/// subdirectory — for example the plugin directory name `weather` — never a
/// raw nested file path.
#[derive(Debug, Clone, PartialEq)]
pub struct StackedEvent {
    /// The watched subdirectory, e.g. `"plugins"`.
    pub subdirectory: String,
    /// The top-level entry within the subdirectory that changed, e.g. `"weather"`.
    pub name: String,
    /// The layer-scoped change that produced this event.
    pub change: LayerChange,
}

/// One writable layer the watcher monitors: a `FileSource` paired with the
/// resolved on-disk root of the watched subdirectory for that layer.
struct WatchedLayer {
    /// The precedence source of this layer (`User` or `Local`).
    source: FileSource,
    /// The watched subdirectory root for this layer (e.g. `<root>/plugins`).
    root: PathBuf,
}

/// A stack-aware filesystem watcher over the writable layers of a config.
///
/// `Watcher<C>` is generic over the [`DirectoryConfig`] `C`, so it watches the
/// `.sah`, `.avp`, or any other configured directory layout without being
/// hardcoded to a specific config. While the returned `Watcher` value is held
/// alive, changes under the watched subdirectory are delivered as
/// [`StackedEvent`]s on the paired receiver; dropping the `Watcher` stops all
/// watching.
pub struct Watcher<C: DirectoryConfig> {
    /// The underlying debounced directory watcher; kept alive to keep watching.
    _inner: DebouncedDirWatcher,
    /// Background task translating debounced batches into [`StackedEvent`]s.
    _translator: tokio::task::JoinHandle<()>,
    /// Phantom marker for the config type.
    _phantom: std::marker::PhantomData<C>,
}

impl<C: DirectoryConfig> Watcher<C> {
    /// Watch `subdirectory` across every writable layer the config exposes.
    ///
    /// The writable layers are the user XDG data layer
    /// (`$XDG_DATA_HOME/{XDG_NAME}/{subdirectory}`) and the project layer at the
    /// git repository root (`{git_root}/{DIR_NAME}/{subdirectory}`). Each layer
    /// root is created if it does not yet exist so it can be watched. The
    /// read-only builtin layer is never watched.
    ///
    /// # Parameters
    ///
    /// * `subdirectory` - The subdirectory to watch within each layer, e.g.
    ///   `"plugins"`.
    ///
    /// # Returns
    ///
    /// The live `Watcher` (which must be kept alive to continue receiving
    /// events) and an `mpsc` receiver of [`StackedEvent`]s.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::DirectoryError`] if a writable layer root cannot be
    /// created, or if the underlying filesystem watcher cannot be started. When
    /// no writable layer can be resolved at all, an error is returned rather
    /// than a silently inert watcher.
    pub async fn watch(subdirectory: &str) -> Result<(Self, mpsc::Receiver<StackedEvent>)> {
        let layers = resolve_writable_layers::<C>(subdirectory)?;
        Self::start(subdirectory, layers).await
    }

    /// Watch `subdirectory` within a single explicit project root.
    ///
    /// This treats `project_root` as a project (`Local`) layer directly,
    /// watching `{project_root}/{subdirectory}`. It mirrors
    /// [`ManagedDirectory::from_custom_root`] and is the deterministic entry
    /// point for tests and for callers that already know the directory to
    /// watch, with no dependence on the ambient git root or XDG environment.
    ///
    /// # Parameters
    ///
    /// * `project_root` - The directory that contains the watched subdirectory.
    /// * `subdirectory` - The subdirectory to watch within `project_root`.
    ///
    /// # Returns
    ///
    /// The live `Watcher` and an `mpsc` receiver of [`StackedEvent`]s.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::DirectoryError`] if the watched root cannot be created
    /// or if the underlying filesystem watcher cannot be started.
    pub async fn watch_in(
        project_root: &Path,
        subdirectory: &str,
    ) -> Result<(Self, mpsc::Receiver<StackedEvent>)> {
        let root = project_root.join(subdirectory);
        std::fs::create_dir_all(&root)
            .map_err(|e| crate::DirectoryError::directory_creation(&root, e))?;
        let layers = vec![WatchedLayer {
            source: FileSource::Local,
            root,
        }];
        Self::start(subdirectory, layers).await
    }

    /// Start watching the given resolved layers for `subdirectory`.
    ///
    /// Spawns the shared debounced watcher over every layer root, then a
    /// translator task that converts each debounced batch into coalesced
    /// [`StackedEvent`]s. Layer roots are canonicalized first so paths reported
    /// by the OS watcher (which on some platforms resolves symlinks, e.g.
    /// macOS `/var` → `/private/var`) match the watched roots when attributing
    /// changes back to a layer.
    async fn start(
        subdirectory: &str,
        mut layers: Vec<WatchedLayer>,
    ) -> Result<(Self, mpsc::Receiver<StackedEvent>)> {
        for layer in &mut layers {
            if let Ok(canonical) = layer.root.canonicalize() {
                layer.root = canonical;
            }
        }
        let roots: Vec<PathBuf> = layers.iter().map(|l| l.root.clone()).collect();
        let (inner, mut batch_rx) = DebouncedDirWatcher::start(&roots, Self::debounce()).await?;

        let (event_tx, event_rx) = mpsc::channel(64);
        let subdirectory = subdirectory.to_string();

        let translator = tokio::spawn(async move {
            while let Some(batch) = batch_rx.recv().await {
                let events = translate_batch(&subdirectory, &layers, batch);
                for event in events {
                    if event_tx.send(event).await.is_err() {
                        return;
                    }
                }
            }
        });

        Ok((
            Self {
                _inner: inner,
                _translator: translator,
                _phantom: std::marker::PhantomData,
            },
            event_rx,
        ))
    }

    /// The debounce window used by this watcher.
    fn debounce() -> Duration {
        DEFAULT_DEBOUNCE
    }
}

impl<C: DirectoryConfig> Drop for Watcher<C> {
    /// Stop watching and abort the translator task.
    ///
    /// The inner [`DebouncedDirWatcher`] stops on its own drop; aborting the
    /// translator ensures no further [`StackedEvent`]s are emitted.
    fn drop(&mut self) {
        self._translator.abort();
    }
}

/// Resolve the writable layer roots for `subdirectory` under config `C`.
///
/// Resolves the user XDG data layer and the project layer, creating the
/// `subdirectory` directory within each so it can be watched. The builtin
/// layer is read-only and intentionally omitted.
///
/// The project layer is resolved exactly as the discovery code in
/// `file_loader.rs` resolves it: the git repository root when inside a git
/// repo, otherwise a fallback rooted at the current working directory. See
/// [`resolve_project_layer`].
///
/// # Errors
///
/// Returns a [`crate::DirectoryError`] if no writable layer can be resolved at
/// all (for example, outside a git repository with no determinable home
/// directory).
fn resolve_writable_layers<C: DirectoryConfig>(subdirectory: &str) -> Result<Vec<WatchedLayer>> {
    let mut layers = Vec::new();

    // User layer: XDG data directory.
    if let Ok(dir) = ManagedDirectory::<C>::xdg_data() {
        if let Ok(root) = dir.ensure_subdir(subdirectory) {
            layers.push(WatchedLayer {
                source: FileSource::User,
                root,
            });
        }
    }

    // Project layer: the git repository root, or the current directory when
    // not inside a git repo — mirroring discovery's `from_custom_root` fallback.
    if let Some(layer) = resolve_project_layer::<C>(
        ManagedDirectory::<C>::from_git_root(),
        std::env::current_dir().ok(),
        subdirectory,
    ) {
        layers.push(layer);
    }

    if layers.is_empty() {
        return Err(crate::DirectoryError::Other {
            message: format!(
                "no writable layer could be resolved to watch subdirectory '{subdirectory}'"
            ),
        });
    }

    Ok(layers)
}

/// Resolve the project (`Local`) layer for `subdirectory` under config `C`.
///
/// Mirrors the project-layer resolution in `VirtualFileSystem::get_directories`
/// / `load_local_files_managed` (`file_loader.rs`): the git repository root is
/// preferred, and when git-root resolution fails — i.e. the caller is not
/// inside a git repository — the layer falls back to
/// `ManagedDirectory::from_custom_root(current_dir)`. Keeping this in lock-step
/// with discovery ensures the watcher monitors the very directory plugin
/// discovery loads from; otherwise hot reload would silently never fire when
/// `watch()` runs outside a git repo.
///
/// The chosen `subdirectory` is created on disk so it can be watched.
///
/// # Parameters
///
/// * `git_root` - The result of [`ManagedDirectory::from_git_root`]; `Ok` when
///   inside a git repository.
/// * `current_dir` - The current working directory used for the non-git
///   fallback, or `None` when it cannot be determined.
/// * `subdirectory` - The subdirectory created and watched within the layer.
///
/// # Returns
///
/// The resolved [`WatchedLayer`], or `None` when neither a git root nor a
/// usable current directory yields a creatable layer root.
fn resolve_project_layer<C: DirectoryConfig>(
    git_root: Result<ManagedDirectory<C>>,
    current_dir: Option<PathBuf>,
    subdirectory: &str,
) -> Option<WatchedLayer> {
    // Prefer the git repository root.
    if let Ok(dir) = git_root {
        if let Ok(root) = dir.ensure_subdir(subdirectory) {
            return Some(WatchedLayer {
                source: FileSource::Local,
                root,
            });
        }
        return None;
    }

    // Fallback: the current directory, exactly as discovery falls back.
    let current_dir = current_dir?;
    let dir = ManagedDirectory::<C>::from_custom_root(current_dir).ok()?;
    let root = dir.ensure_subdir(subdirectory).ok()?;
    Some(WatchedLayer {
        source: FileSource::Local,
        root,
    })
}

/// Translate one debounced batch into coalesced [`StackedEvent`]s.
///
/// Every changed path is mapped back to the layer that owns it and to the
/// top-level entry name within the watched subdirectory. Multiple paths under
/// the same `(layer, name)` collapse to a single event for that entry — so a
/// multi-file save yields one event per affected entry, not one per file.
///
/// # Parameters
///
/// * `subdirectory` - The watched subdirectory, copied into each event.
/// * `layers` - The watched layers, used to attribute each path to a layer.
/// * `batch` - The debounced filesystem events for this window.
fn translate_batch(
    subdirectory: &str,
    layers: &[WatchedLayer],
    batch: Vec<DebouncedEvent>,
) -> Vec<StackedEvent> {
    // Keyed by (layer index, entry name) so each affected entry yields one
    // event per layer; later paths in the batch only escalate the change
    // classification (see `merge_change`) and keep the first-seen path.
    // The layer index stands in for the layer's `FileSource` because
    // `FileSource` is not `Hash`/`Eq`; it is resolved back to a source below.
    let mut coalesced: HashMap<(usize, String), LayerChange> = HashMap::new();

    for event in batch {
        let Some((layer_index, name)) = attribute_path(layers, &event.path) else {
            continue;
        };
        let layer = layers[layer_index].source.clone();
        let change = classify_change(layer, &event);
        coalesced
            .entry((layer_index, name))
            .and_modify(|existing| merge_change(existing, &change))
            .or_insert(change);
    }

    coalesced
        .into_iter()
        .map(|((_, name), change)| StackedEvent {
            subdirectory: subdirectory.to_string(),
            name,
            change,
        })
        .collect()
}

/// Attribute a changed `path` to the layer that owns it and its entry name.
///
/// Returns the index of the owning layer within `layers` and the top-level
/// entry name, or `None` if the path is not under any watched layer or names
/// no entry.
fn attribute_path(layers: &[WatchedLayer], path: &Path) -> Option<(usize, String)> {
    for (index, layer) in layers.iter().enumerate() {
        if let Some(name) = top_level_entry(&layer.root, path) {
            return Some((index, name));
        }
    }
    None
}

/// Classify a single [`DebouncedEvent`] into a [`LayerChange`].
///
/// Because a debounced batch reports the *net* change for a path over the
/// debounce window — and because some backends (notably macOS FSEvents) report
/// an imprecise [`EventKind`] that does not distinguish a removal from a
/// creation — current on-disk existence is the authoritative signal:
///
/// * A path that no longer exists is always [`LayerChange::Removed`].
/// * Otherwise [`EventKind::Create`] becomes [`LayerChange::Added`] and every
///   other kind (content/metadata mutations, renames, unspecified events)
///   becomes [`LayerChange::Modified`].
fn classify_change(layer: FileSource, event: &DebouncedEvent) -> LayerChange {
    let exists = event.path.exists();
    classify_kind(layer, event, exists)
}

/// Pure classification of a [`DebouncedEvent`] given the path's existence.
///
/// Separated from [`classify_change`] so the decision logic can be tested
/// without touching the filesystem.
///
/// # Parameters
///
/// * `layer` - The layer the change occurred in.
/// * `event` - The debounced event to classify.
/// * `exists` - Whether `event.path` currently exists on disk.
fn classify_kind(layer: FileSource, event: &DebouncedEvent, exists: bool) -> LayerChange {
    let path = event.path.clone();
    if !exists {
        return LayerChange::Removed { layer, path };
    }
    match event.event.kind {
        EventKind::Create(_) => LayerChange::Added { layer, path },
        EventKind::Remove(_) => LayerChange::Removed { layer, path },
        _ => LayerChange::Modified { layer, path },
    }
}

/// Merge a newly seen change into the change already recorded for an entry.
///
/// Within one debounce batch a single entry may see several kinds of change.
/// A removal is the most consequential outcome and wins; otherwise an addition
/// is preferred over a plain modification. When the incoming change does not
/// dominate, the existing change — including its first-seen triggering path —
/// is kept unchanged. Any path under the entry is an equally valid trigger, so
/// retaining the first-seen one is deliberate and keeps the result independent
/// of filesystem event ordering.
fn merge_change(existing: &mut LayerChange, incoming: &LayerChange) {
    let dominates = match (&existing, incoming) {
        // A removal always wins — the entry's final state is "gone".
        (_, LayerChange::Removed { .. }) => true,
        (LayerChange::Removed { .. }, _) => false,
        // An addition wins over a plain modification.
        (LayerChange::Modified { .. }, LayerChange::Added { .. }) => true,
        // Otherwise keep the existing change (classification and path) as-is.
        _ => false,
    };
    if dominates {
        *existing = incoming.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SwissarmyhammerConfig;
    use async_watcher::notify::event::{CreateKind, DataChange, Event, ModifyKind, RemoveKind};
    use async_watcher::DebouncedEventKind;
    use tempfile::TempDir;

    /// A content-modification event kind, reused across tests.
    const MODIFY: EventKind = EventKind::Modify(ModifyKind::Data(DataChange::Content));

    /// Build a `DebouncedEvent` for `path` with the given `EventKind`.
    fn make_event(path: &Path, kind: EventKind) -> DebouncedEvent {
        let pb = path.to_path_buf();
        let mut event = Event::new(kind);
        event.paths.push(pb.clone());
        DebouncedEvent {
            path: pb,
            kind: DebouncedEventKind::Any,
            event,
        }
    }

    /// A single project layer watching `<root>/plugins`.
    fn one_layer(root: &Path) -> Vec<WatchedLayer> {
        vec![WatchedLayer {
            source: FileSource::Local,
            root: root.join("plugins"),
        }]
    }

    /// Create `plugins/<name>/<file>` under `root` and return its path.
    fn seed_file(root: &Path, name: &str, file: &str) -> PathBuf {
        let dir = root.join("plugins").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(file);
        std::fs::write(&path, "x").unwrap();
        path
    }

    /// A create event for an existing path classifies as `Added` for its name.
    #[test]
    fn classifies_create_as_added() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let path = seed_file(temp.path(), "foo", "plugin.json");
        let events = translate_batch(
            "plugins",
            &layers,
            vec![make_event(&path, EventKind::Create(CreateKind::File))],
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "foo");
        assert_eq!(events[0].subdirectory, "plugins");
        assert!(matches!(events[0].change, LayerChange::Added { .. }));
        assert_eq!(events[0].change.layer(), &FileSource::Local);
    }

    /// An event for a path that no longer exists classifies as `Removed`.
    #[test]
    fn classifies_missing_path_as_removed() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        // Path is never created on disk, so it is treated as removed.
        let path = temp.path().join("plugins").join("foo").join("plugin.json");
        let events = translate_batch(
            "plugins",
            &layers,
            vec![make_event(&path, EventKind::Remove(RemoveKind::Folder))],
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].change, LayerChange::Removed { .. }));
    }

    /// A modify event for an existing path classifies as `Modified`.
    #[test]
    fn classifies_modify_as_modified() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let path = seed_file(temp.path(), "foo", "main.rs");
        let events = translate_batch("plugins", &layers, vec![make_event(&path, MODIFY)]);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].change, LayerChange::Modified { .. }));
    }

    /// `classify_kind` treats a non-existent path as removed regardless of the
    /// reported event kind — the macOS FSEvents imprecise-kind case.
    #[test]
    fn classify_kind_missing_path_overrides_create_kind() {
        let event = make_event(
            Path::new("/does/not/exist/file"),
            EventKind::Create(CreateKind::File),
        );
        let change = classify_kind(FileSource::Local, &event, false);
        assert!(matches!(change, LayerChange::Removed { .. }));
    }

    /// `classify_kind` honours `EventKind::Create` for an existing path.
    #[test]
    fn classify_kind_existing_create_is_added() {
        let event = make_event(Path::new("/any/path"), EventKind::Create(CreateKind::File));
        let change = classify_kind(FileSource::User, &event, true);
        assert!(matches!(change, LayerChange::Added { .. }));
        assert_eq!(change.layer(), &FileSource::User);
    }

    /// Several modifications under one plugin collapse to a single event.
    #[test]
    fn coalesces_multiple_paths_under_one_name() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let batch = vec![
            make_event(&seed_file(temp.path(), "foo", "a.rs"), MODIFY),
            make_event(&seed_file(temp.path(), "foo", "b.rs"), MODIFY),
            make_event(&seed_file(temp.path(), "foo", "plugin.json"), MODIFY),
        ];
        let events = translate_batch("plugins", &layers, batch);
        assert_eq!(events.len(), 1, "burst under one plugin must coalesce");
        assert_eq!(events[0].name, "foo");
    }

    /// Changes under distinct plugins yield one event each.
    #[test]
    fn distinct_names_yield_distinct_events() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let batch = vec![
            make_event(&seed_file(temp.path(), "foo", "a.rs"), MODIFY),
            make_event(&seed_file(temp.path(), "bar", "b.rs"), MODIFY),
        ];
        let mut names: Vec<String> = translate_batch("plugins", &layers, batch)
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["bar".to_string(), "foo".to_string()]);
    }

    /// A removal within a batch wins over a modification of the same entry.
    #[test]
    fn removal_dominates_modification_in_coalesce() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        // An existing file (modified) and a sibling that was never created
        // (so it classifies as removed) under the same plugin.
        let existing = seed_file(temp.path(), "foo", "a.rs");
        let missing = temp.path().join("plugins").join("foo").join("gone.rs");
        let batch = vec![
            make_event(&existing, MODIFY),
            make_event(&missing, EventKind::Remove(RemoveKind::File)),
        ];
        let events = translate_batch("plugins", &layers, batch);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].change, LayerChange::Removed { .. }));
    }

    /// Paths outside every watched layer are ignored.
    #[test]
    fn ignores_paths_outside_watched_layers() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let batch = vec![make_event(
            Path::new("/elsewhere/file.rs"),
            EventKind::Create(CreateKind::File),
        )];
        assert!(translate_batch("plugins", &layers, batch).is_empty());
    }

    /// A change to the subdirectory root itself names no entry and is ignored.
    #[test]
    fn ignores_change_to_subdirectory_root() {
        let temp = TempDir::new().unwrap();
        let layers = one_layer(temp.path());
        let root = temp.path().join("plugins");
        let batch = vec![make_event(&root, EventKind::Modify(ModifyKind::Any))];
        assert!(translate_batch("plugins", &layers, batch).is_empty());
    }

    /// `merge_change` keeps an addition over a plain modification.
    #[test]
    fn merge_prefers_added_over_modified() {
        let mut existing = LayerChange::Modified {
            layer: FileSource::Local,
            path: PathBuf::from("/p/a"),
        };
        let incoming = LayerChange::Added {
            layer: FileSource::Local,
            path: PathBuf::from("/p/b"),
        };
        merge_change(&mut existing, &incoming);
        assert!(matches!(existing, LayerChange::Added { .. }));
    }

    /// `merge_change` keeps the first-seen path when the incoming change does
    /// not dominate — two modifications of the same entry retain the original
    /// triggering path.
    #[test]
    fn merge_keeps_first_path_when_not_dominating() {
        let mut existing = LayerChange::Modified {
            layer: FileSource::Local,
            path: PathBuf::from("/p/first"),
        };
        let incoming = LayerChange::Modified {
            layer: FileSource::Local,
            path: PathBuf::from("/p/second"),
        };
        merge_change(&mut existing, &incoming);
        match existing {
            LayerChange::Modified { path, .. } => {
                assert_eq!(path, PathBuf::from("/p/first"));
            }
            other => panic!("expected Modified, got {other:?}"),
        }
    }

    /// When git-root resolution succeeds, the project layer is rooted at the
    /// git-root managed directory and the current-directory fallback is unused.
    #[test]
    fn project_layer_prefers_git_root() {
        let git_root = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let dir = ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(
            git_root.path().to_path_buf(),
        )
        .unwrap();
        let layer = resolve_project_layer::<SwissarmyhammerConfig>(
            Ok(dir),
            Some(cwd.path().to_path_buf()),
            "plugins",
        )
        .expect("project layer should resolve from the git root");
        assert_eq!(layer.source, FileSource::Local);
        assert!(layer.root.starts_with(git_root.path()));
    }

    /// When git-root resolution fails (not inside a git repository), the project
    /// layer falls back to the current directory via `from_custom_root` — the
    /// same fallback the discovery code in `file_loader.rs` performs. This keeps
    /// the watcher watching the directory discovery actually loads from.
    #[test]
    fn project_layer_falls_back_to_current_dir_outside_git_repo() {
        let cwd = TempDir::new().unwrap();
        let layer = resolve_project_layer::<SwissarmyhammerConfig>(
            Err(crate::DirectoryError::NotInGitRepository),
            Some(cwd.path().to_path_buf()),
            "plugins",
        )
        .expect("project layer should fall back to the current directory");
        assert_eq!(layer.source, FileSource::Local);
        // The fallback root is `{cwd}/{DIR_NAME}/plugins` and must exist on disk
        // so it can be watched.
        let expected = cwd
            .path()
            .join(SwissarmyhammerConfig::DIR_NAME)
            .join("plugins");
        assert_eq!(layer.root, expected);
        assert!(layer.root.is_dir(), "fallback root must be created");
    }

    /// When git-root resolution fails and no current directory is available,
    /// the project layer cannot be resolved.
    #[test]
    fn project_layer_none_when_no_git_root_and_no_current_dir() {
        let layer = resolve_project_layer::<SwissarmyhammerConfig>(
            Err(crate::DirectoryError::NotInGitRepository),
            None,
            "plugins",
        );
        assert!(layer.is_none());
    }
}
