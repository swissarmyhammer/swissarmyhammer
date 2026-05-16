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

/// Assert that no further event arrives within a short grace window.
///
/// Used to confirm debounce coalescing — that a burst of writes produced
/// exactly one event, not several.
async fn assert_no_more_events(rx: &mut Receiver<StackedEvent>) {
    match tokio::time::timeout(Duration::from_millis(750), rx.recv()).await {
        Ok(Some(extra)) => panic!("expected no further events, got an extra: {extra:?}"),
        Ok(None) => {}
        Err(_) => {}
    }
}

/// Create a `plugins/<name>/plugin.json` manifest under `project_root`.
fn seed_plugin(project_root: &Path, name: &str) {
    let plugin_dir = project_root.join("plugins").join(name);
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("plugin.json"), "{}").unwrap();
}

/// Writing a new file under a plugin directory yields exactly one
/// `StackedEvent` naming the plugin and carrying the project `FileSource`.
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

    assert_no_more_events(&mut rx).await;
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

    // The whole burst must collapse to that one event.
    assert_no_more_events(&mut rx).await;
}
