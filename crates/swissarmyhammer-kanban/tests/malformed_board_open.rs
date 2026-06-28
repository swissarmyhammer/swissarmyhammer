//! Board-open reconciler must guard on board-entity existence.
//!
//! Live incident (task 01KTYVB3TBB6G8FA1J7CKEQ9RG): a stray `.kanban`
//! directory with no `boards/` board entity was opened, and the
//! perspectives reconciler in `KanbanContext::open` dutifully minted a
//! Default perspective INTO the malformed dir — before anything validated
//! that the board actually exists. Open-time side effects (the reconciler)
//! must run ONLY after the board entity is confirmed present.
//!
//! This pins Defense 4 (board-open ordering) at the layer that can be
//! compiled and tested without the kanban-app crate: the reconciler-guard
//! lives in `KanbanContext::open` itself.

use std::path::Path;

use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::{Execute, KanbanContext};
use tempfile::TempDir;

/// A malformed board dir: it has the ancillary subdirs a half-initialized
/// `.kanban` accumulates (perspectives/, views/, entities/, definitions/) but
/// NO `boards/` board entity — exactly the shape that blanked the window.
fn malformed_kanban_dir() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    for sub in ["perspectives", "views", "entities", "definitions"] {
        std::fs::create_dir_all(kanban_dir.join(sub)).unwrap();
    }
    (temp, kanban_dir)
}

/// Count the on-disk perspective YAML files in a board dir.
fn perspective_files_on_disk(kanban_dir: &Path) -> usize {
    let dir = kanban_dir.join("perspectives");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("yaml"))
        .count()
}

#[tokio::test]
async fn open_of_malformed_board_does_not_mint_perspectives() {
    let (_temp, kanban_dir) = malformed_kanban_dir();
    assert!(
        !KanbanContext::new(&kanban_dir).is_initialized(),
        "precondition: a board dir with no boards/ entity is not initialized"
    );

    // Open the malformed dir. This must succeed (open is infallible at this
    // layer — the app-side validation rejects), but the reconciler must NOT
    // run, so no Default perspective is minted into the half-board.
    let ctx = KanbanContext::open(&kanban_dir).await.unwrap();

    assert!(
        !ctx.is_initialized(),
        "opening a malformed dir must not initialize it"
    );
    assert_eq!(
        perspective_files_on_disk(&kanban_dir),
        0,
        "reconciler must not mint a Default perspective into a board with no board entity"
    );
}

#[tokio::test]
async fn open_of_initialized_board_still_reconciles_default() {
    // Guard against over-correction: a real (initialized) board must still
    // get its Default perspective minted at open — the reconciler only skips
    // when the board entity is absent.
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    InitBoard::new("Reconcile Test")
        .execute(&KanbanContext::new(&kanban_dir))
        .await
        .into_result()
        .unwrap();

    let _ctx = KanbanContext::open(&kanban_dir).await.unwrap();

    assert_eq!(
        perspective_files_on_disk(&kanban_dir),
        1,
        "an initialized board must still reconcile exactly one Default perspective at open"
    );
}
