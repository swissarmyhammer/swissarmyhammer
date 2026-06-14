//! Open a board through the real `KanbanContext::open` pipeline so its
//! board-open reconciliation runs (duplicate-default convergence, orphan
//! pruning, zero-perspective recovery), then print the surviving
//! perspectives.
//!
//! Usage:
//!
//! ```text
//! cargo run -p swissarmyhammer-kanban --example reconcile_perspectives -- <path/to/.kanban>
//! ```
//!
//! This is the same self-heal every consumer gets when it opens the board;
//! the example exists so a board can be healed (and the result inspected)
//! without launching the GUI app.

use swissarmyhammer_kanban::KanbanContext;

#[tokio::main]
async fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: reconcile_perspectives <path/to/.kanban>");

    let ctx = KanbanContext::open(&dir)
        .await
        .expect("failed to open kanban context");
    let pctx = ctx
        .perspective_context()
        .await
        .expect("failed to open perspective context");
    let pctx = pctx.read().await;

    println!("perspectives after reconcile: {}", pctx.all().len());
    for p in pctx.all() {
        println!(
            "  {}\tname={:?}\tview={}\tview_id={:?}\tfilter={:?}",
            p.id, p.name, p.view, p.view_id, p.filter
        );
    }
}
