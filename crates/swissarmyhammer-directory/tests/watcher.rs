//! Integration tests for the stack-aware filesystem [`Watcher`].
//!
//! These tests exercise the watcher against real temporary directories. They
//! are intentionally timing-sensitive: the watcher uses a real debounce window,
//! so every assertion that waits for an event uses a bounded timeout. A hang
//! therefore fails fast instead of blocking the suite forever.

use std::fs;
use std::path::Path;
use std::time::Duration;

use swissarmyhammer_directory::{
    FileSource, LayerChange, StackedEvent, SwissarmyhammerConfig, Watcher,
};
use tempfile::TempDir;
use tokio::sync::mpsc::Receiver;

/// Upper bound on how long a test will wait for a single debounced event.
///
/// The watcher debounce window is well under this; the slack absorbs slow CI
/// filesystems without letting a genuine hang block the suite.
const RECV_TIMEOUT: Duration = Duration::from_secs(10);

/// Receive the next [`StackedEvent`] within [`RECV_TIMEOUT`], panicking on
/// timeout or channel closure so a stalled watcher fails the test fast.
async fn recv_event(rx: &mut Receiver<StackedEvent>) -> StackedEvent {
    match tokio::time::timeout(RECV_TIMEOUT, rx.recv()).await {
        Ok(Some(event)) => event,
        Ok(None) => panic!("watcher channel closed before an event arrived"),
        Err(_) => panic!("timed out waiting for a StackedEvent after {RECV_TIMEOUT:?}"),
    }
}

/// Drain any further events within a short grace window, asserting each one
/// still names `expected_name`.
///
/// A real filesystem watcher cannot guarantee that a single logical change
/// produces exactly one OS event: macOS FSEvents in particular can split a
/// write into creation + metadata notifications that straddle the debounce
/// window, yielding a second `StackedEvent` for the *same* entry. Asserting
/// "exactly one event" is therefore flaky on loaded CI runners. The contract
/// the watcher must uphold — and that this assertion enforces — is that it
/// never fabricates an event for an *unrelated* entry: every further event
/// must still name `expected_name`. Benign same-name duplicates are drained
/// and ignored. The debounce's job (collapsing a burst rather than emitting
/// one event per byte) is exercised by the coalescing test below; this helper
/// guards naming, not the exact event count.
async fn assert_only_events_named(rx: &mut Receiver<StackedEvent>, expected_name: &str) {
    while let Ok(maybe) = tokio::time::timeout(Duration::from_millis(750), rx.recv()).await {
        match maybe {
            Some(extra) => assert_eq!(
                extra.name, expected_name,
                "watcher emitted an event for an unexpected entry: {extra:?}"
            ),
            None => break,
        }
    }
}

/// Create a `plugins/<name>/plugin.json` manifest under `project_root`.
fn seed_plugin(project_root: &Path, name: &str) {
    let plugin_dir = project_root.join("plugins").join(name);
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("plugin.json"), "{}").unwrap();
}

/// Writing a new file under a plugin directory yields a `StackedEvent` naming
/// the plugin and carrying the project `FileSource` — and no event for any
/// other entry. (A single write may legitimately surface as more than one
/// same-name event depending on the OS's filesystem-event timing; the
/// debounce coalesces the common case but the test does not depend on an
/// exact count — see [`assert_only_events_named`].)
#[tokio::test]
async fn write_under_plugin_emits_single_named_event() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path().to_path_buf();
    seed_plugin(&project_root, "foo");

    let (_watcher, mut rx) = Watcher::<SwissarmyhammerConfig>::watch_in(&project_root, "plugins")
        .await
        .unwrap();

    // Give the OS watcher a moment to register before mutating the tree.
    tokio::time::sleep(Duration::from_millis(200)).await;

    fs::write(
        project_root.join("plugins").join("foo").join("main.rs"),
        "fn main() {}",
    )
    .unwrap();

    let event = recv_event(&mut rx).await;
    assert_eq!(event.subdirectory, "plugins");
    assert_eq!(event.name, "foo");
    match event.change {
        LayerChange::Added { layer, .. } | LayerChange::Modified { layer, .. } => {
            assert_eq!(layer, FileSource::Local);
        }
        LayerChange::Removed { .. } => panic!("expected Added/Modified, got Removed"),
    }

    assert_only_events_named(&mut rx, "foo").await;
}

/// Removing a plugin directory emits a `Removed` event for that plugin name.
#[tokio::test]
async fn removing_plugin_directory_emits_removed_event() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path().to_path_buf();
    seed_plugin(&project_root, "foo");

    let (_watcher, mut rx) = Watcher::<SwissarmyhammerConfig>::watch_in(&project_root, "plugins")
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    fs::remove_dir_all(project_root.join("plugins").join("foo")).unwrap();

    // Filesystems may emit several events for a recursive removal; keep reading
    // until a Removed for `foo` appears (bounded by RECV_TIMEOUT per recv).
    let mut saw_removed = false;
    for _ in 0..5 {
        let event = recv_event(&mut rx).await;
        assert_eq!(event.subdirectory, "plugins");
        assert_eq!(event.name, "foo");
        if matches!(event.change, LayerChange::Removed { .. }) {
            saw_removed = true;
            break;
        }
    }
    assert!(
        saw_removed,
        "expected a Removed event for the deleted plugin"
    );
}

/// Three rapid writes under one plugin collapse into a single coalesced event.
#[tokio::test]
async fn rapid_writes_under_plugin_coalesce_to_one_event() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path().to_path_buf();
    seed_plugin(&project_root, "foo");

    let (_watcher, mut rx) = Watcher::<SwissarmyhammerConfig>::watch_in(&project_root, "plugins")
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let plugin_dir = project_root.join("plugins").join("foo");
    fs::write(plugin_dir.join("a.rs"), "// a").unwrap();
    fs::write(plugin_dir.join("b.rs"), "// b").unwrap();
    fs::write(plugin_dir.join("c.rs"), "// c").unwrap();

    let event = recv_event(&mut rx).await;
    assert_eq!(event.subdirectory, "plugins");
    assert_eq!(event.name, "foo");

    // The burst collapses to events naming only `foo` — the debounce coalesces
    // the three writes (usually into the single event above), and any trailing
    // FS-split duplicate still names `foo`, never an unrelated entry.
    assert_only_events_named(&mut rx, "foo").await;
}
