//! The `browser` surface adapter — the deterministic, no-agent web-UI path.
//!
//! Per `ideas/expect.md` §"Surface adapters" (the browser row) and
//! §"Accessibility is the GUI's drive *and* observe channel": the adapter drives
//! and observes a web UI through the **accessibility tree**, over the Chrome
//! DevTools Protocol via [`chromiumoxide`] — pure Rust, **no Node, no
//! Playwright**. `provision` launches (or attaches to) Chromium and navigates to
//! the page under test; `drive` presses/types by `role[name=…]` through CDP
//! `Input`; `observe` snapshots the a11y tree through CDP `Accessibility`; and
//! `teardown` closes the browser.
//!
//! The locator dialect for the observed state — `role[name=…]` plus a tree
//! relationship (`within` / `ancestor`) — lives in the
//! [assertion compiler](crate::assertion); this module only produces the
//! [`A11yNode`] tree those locators resolve against. This is deliberately **not
//! pixels**: a locator binds to `role + accessible name + tree position`, robust
//! to layout and styling, so a genuine control rename surfaces as honest
//! structural drift rather than the everything-screams-on-a-cosmetic-change noise
//! of a screenshot diff.
//!
//! **Deterministic surface.** Mechanical a11y actuation ("press the button named
//! Go") is reproducible, so browser reclassifies alongside cli/http: it resolves
//! every recognized step itself ([`resolves_mechanically`]) and runs once by
//! default. Non-determinism only enters when an *agent* drives the mechanical
//! loop (the runtime fallback), which is the exception — a step whose action does
//! not parse, or a locator that no longer binds, returns `false` and routes
//! through the subagent.
//!
//! **Sparse a11y → vision/OCR is an explicit last resort, not built here.** When
//! a page's accessibility tree is thin (a `<div onclick>` soup with no roles or
//! names), the honest signal is that the app's accessibility — and thus its
//! testability — is weak; falling back to pixel vision/OCR would only paper over
//! that. This adapter intentionally reads *only* the a11y tree; a sparse tree is
//! itself the diagnostic.
//!
//! [`resolves_mechanically`]: SurfaceAdapter::resolves_mechanically

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::accessibility::{AxNode, AxValue, GetFullAxTreeParams};
use chromiumoxide::cdp::browser_protocol::dom::{BackendNodeId, FocusParams, GetBoxModelParams};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, InsertTextParams, MouseButton,
};
use chromiumoxide::detection::{default_executable, DetectionOptions};
use chromiumoxide::error::CdpError;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::assertion::A11ySelector;
use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::a11y::{
    step_resolves_mechanically, unbound, A11yAction, DEFAULT_ACTION_TIMEOUT,
};
use crate::surface::SurfaceAdapter;
use crate::types::{A11yNode, SurfaceState};

/// The role given to a synthesized root when the observed tree has zero or many
/// top-level nodes, so a snapshot always has a single root.
const SYNTHETIC_ROOT_ROLE: &str = "tree";

/// A single primary (left) button click.
const PRIMARY_CLICK_COUNT: i64 = 1;

/// The number of vertices in a CDP box-model quad (four `(x, y)` corners).
const QUAD_VERTEX_COUNT: usize = 8;

/// The `browser` surface adapter: launches Chromium, drives a web UI by
/// `role[name=…]` through CDP `Input`, and snapshots its accessibility tree.
///
/// Construct with [`BrowserAdapter::new`], passing the URL of the page under
/// test; the builder methods choose headless vs. headed and the per-action
/// budget. Launching requires a Chromium binary — gate any environment-dependent
/// use on [`BrowserAdapter::chromium_available`].
#[derive(Debug, Clone)]
pub struct BrowserAdapter {
    url: String,
    headless: bool,
    action_timeout: Duration,
}

impl BrowserAdapter {
    /// Create a browser adapter for the page at `url`, headless, with the default
    /// per-action budget.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headless: true,
            action_timeout: DEFAULT_ACTION_TIMEOUT,
        }
    }

    /// Run headed (a visible window) instead of headless — handy for debugging a
    /// drive that does not bind.
    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    /// Set the per-action wall-clock budget; an action that exceeds it surfaces as
    /// [`ExpectError::Timeout`].
    pub fn with_action_timeout(mut self, action_timeout: Duration) -> Self {
        self.action_timeout = action_timeout;
        self
    }

    /// Whether a Chromium/Chrome binary is installed and can be launched.
    ///
    /// The browser surface needs a real Chromium; callers (and the integration
    /// tests) gate on this so a host without one skips cleanly rather than
    /// failing.
    pub fn chromium_available() -> bool {
        default_executable(DetectionOptions::default()).is_ok()
    }
}

/// The provisioned browser system under test: the launched browser, the open
/// page, the background task draining the CDP event stream, and the Tokio runtime
/// they all run on.
pub struct BrowserSut {
    /// The runtime the CDP session runs on; every adapter method blocks on it.
    runtime: Runtime,
    /// The launched browser handle, closed at teardown.
    browser: Browser,
    /// The open page that is driven and observed.
    page: Page,
    /// The task draining the CDP `Handler` event stream while commands are
    /// issued; aborted at teardown.
    handler: JoinHandle<()>,
}

impl fmt::Debug for BrowserSut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserSut").finish_non_exhaustive()
    }
}

impl SurfaceAdapter for BrowserAdapter {
    type ProvisionedSut = BrowserSut;

    fn provision(
        &self,
        _setup: Option<&Setup>,
        _repo_root: &Path,
    ) -> Result<BrowserSut, ExpectError> {
        let executable = default_executable(DetectionOptions::default()).map_err(|err| {
            ExpectError::Surface(format!(
                "no Chromium available to launch the browser surface: {err}"
            ))
        })?;
        let mut builder = BrowserConfig::builder().chrome_executable(executable);
        if !self.headless {
            builder = builder.with_head();
        }
        let config = builder
            .build()
            .map_err(|err| ExpectError::Surface(format!("browser config: {err}")))?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(ExpectError::Io)?;

        let url = self.url.clone();
        let (browser, page, handler) = runtime.block_on(async move {
            let (mut browser, mut handler_stream) =
                Browser::launch(config).await.map_err(map_cdp)?;
            // The CDP event stream must be drained continuously or commands stall;
            // a background task owns it for the session's lifetime.
            let handler =
                tokio::spawn(async move { while handler_stream.next().await.is_some() {} });
            match open_page(&browser, url).await {
                Ok(page) => Ok::<_, ExpectError>((browser, page, handler)),
                Err(err) => {
                    // Launch succeeded but the page failed to open/navigate: close
                    // the browser and stop the drainer so a failed provision does
                    // not leak a running Chromium process.
                    let _ = browser.close().await;
                    let _ = browser.wait().await;
                    handler.abort();
                    Err(err)
                }
            }
        })?;

        Ok(BrowserSut {
            runtime,
            browser,
            page,
            handler,
        })
    }

    fn drive(&self, sut: &mut BrowserSut, when_step: &str) -> Result<(), ExpectError> {
        if when_step.trim().is_empty() {
            // An empty step drives nothing (mirrors the cli/http/db empty step).
            return Ok(());
        }
        let action = A11yAction::parse(when_step).ok_or_else(|| {
            ExpectError::Surface(format!(
                "browser drive step is not a recognized action \
                 (press/type by `role[name=…]`): `{when_step}`"
            ))
        })?;
        let timeout = self.action_timeout;
        sut.runtime
            .block_on(async { with_timeout(timeout, perform_action(&sut.page, &action)).await })
    }

    fn observe(&self, sut: &BrowserSut) -> Result<SurfaceState, ExpectError> {
        let timeout = self.action_timeout;
        let tree = sut
            .runtime
            .block_on(async { with_timeout(timeout, snapshot_tree(&sut.page)).await })?;
        Ok(SurfaceState::A11y { tree })
    }

    fn teardown(&self, sut: BrowserSut) -> Result<(), ExpectError> {
        let BrowserSut {
            runtime,
            mut browser,
            page,
            handler,
        } = sut;
        // Close the page and browser, reap the child, and stop the event drainer —
        // a `check` must not leak a running Chromium.
        runtime.block_on(async move {
            drop(page);
            let _ = browser.close().await;
            let _ = browser.wait().await;
            handler.abort();
        });
        Ok(())
    }

    fn resolves_mechanically(&self, when_step: &str) -> bool {
        step_resolves_mechanically(when_step)
    }
}

/// Open `url` in a new page and wait for its initial navigation to settle.
async fn open_page(browser: &Browser, url: String) -> Result<Page, ExpectError> {
    let page = browser.new_page(url).await.map_err(map_cdp)?;
    page.wait_for_navigation().await.map_err(map_cdp)?;
    Ok(page)
}

/// Perform one parsed [`A11yAction`] against the page through CDP.
async fn perform_action(page: &Page, action: &A11yAction) -> Result<(), ExpectError> {
    let nodes = full_ax_tree(page).await?;
    match action {
        A11yAction::Press { selector } => {
            let backend = find_backend_node(&nodes, selector).ok_or_else(|| unbound(selector))?;
            click_backend_node(page, backend).await
        }
        A11yAction::Type { selector, value } => {
            let backend = find_backend_node(&nodes, selector).ok_or_else(|| unbound(selector))?;
            focus_backend_node(page, backend).await?;
            insert_text(page, value).await
        }
    }
}

/// Snapshot the page's accessibility tree into an [`A11yNode`].
async fn snapshot_tree(page: &Page) -> Result<A11yNode, ExpectError> {
    let nodes = full_ax_tree(page).await?;
    Ok(build_tree(&nodes))
}

/// Fetch the full CDP accessibility tree as a flat list of nodes.
async fn full_ax_tree(page: &Page) -> Result<Vec<AxNode>, ExpectError> {
    let response = page
        .execute(GetFullAxTreeParams::builder().build())
        .await
        .map_err(map_cdp)?;
    Ok(response.nodes.clone())
}

/// The backend DOM node id of the first non-ignored node matching `selector`, for
/// CDP `Input`/`DOM` actions.
fn find_backend_node(nodes: &[AxNode], selector: &A11ySelector) -> Option<BackendNodeId> {
    nodes
        .iter()
        .find(|node| {
            !node.ignored
                && ax_string(&node.role).as_deref() == Some(selector.role.as_str())
                && selector
                    .name
                    .as_ref()
                    .is_none_or(|name| ax_string(&node.name).as_deref() == Some(name.as_str()))
        })
        .and_then(|node| node.backend_dom_node_id)
}

/// Click the node at `backend` by computing its box-model center and dispatching
/// a press/release mouse pair through CDP `Input`.
async fn click_backend_node(page: &Page, backend: BackendNodeId) -> Result<(), ExpectError> {
    let box_model = page
        .execute(
            GetBoxModelParams::builder()
                .backend_node_id(backend)
                .build(),
        )
        .await
        .map_err(map_cdp)?;
    let (x, y) = quad_center(box_model.model.content.inner())?;
    dispatch_mouse(page, DispatchMouseEventType::MousePressed, x, y).await?;
    dispatch_mouse(page, DispatchMouseEventType::MouseReleased, x, y).await?;
    Ok(())
}

/// Focus the node at `backend` through CDP `DOM.focus` (so a typed value lands in
/// it).
async fn focus_backend_node(page: &Page, backend: BackendNodeId) -> Result<(), ExpectError> {
    page.execute(FocusParams::builder().backend_node_id(backend).build())
        .await
        .map_err(map_cdp)?;
    Ok(())
}

/// Insert `text` at the focused element through CDP `Input.insertText`.
async fn insert_text(page: &Page, text: &str) -> Result<(), ExpectError> {
    let params = InsertTextParams::builder()
        .text(text)
        .build()
        .map_err(|err| ExpectError::Surface(format!("insert text: {err}")))?;
    page.execute(params).await.map_err(map_cdp)?;
    Ok(())
}

/// Dispatch one mouse event at `(x, y)` (primary button) through CDP `Input`.
async fn dispatch_mouse(
    page: &Page,
    event_type: DispatchMouseEventType,
    x: f64,
    y: f64,
) -> Result<(), ExpectError> {
    let params = DispatchMouseEventParams::builder()
        .r#type(event_type)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(PRIMARY_CLICK_COUNT)
        .build()
        .map_err(|err| ExpectError::Surface(format!("mouse event: {err}")))?;
    page.execute(params).await.map_err(map_cdp)?;
    Ok(())
}

/// The center `(x, y)` of a CDP box-model content quad (four corners, `x` then
/// `y` per vertex).
fn quad_center(quad: &[f64]) -> Result<(f64, f64), ExpectError> {
    if quad.len() < QUAD_VERTEX_COUNT {
        return Err(ExpectError::Surface(format!(
            "box-model quad has {} coordinates, expected {QUAD_VERTEX_COUNT}",
            quad.len()
        )));
    }
    let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
    let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
    Ok((x, y))
}

/// Build a nested [`A11yNode`] tree from the flat CDP node list.
///
/// Ignored nodes are spliced out, their children re-parented, so the snapshot
/// carries only semantically meaningful nodes. When the document has a single
/// root the snapshot is that node; otherwise the roots are gathered under a
/// synthetic root so the snapshot is always one tree.
fn build_tree(nodes: &[AxNode]) -> A11yNode {
    let by_id: HashMap<&str, &AxNode> = nodes
        .iter()
        .map(|node| (node.node_id.as_ref(), node))
        .collect();
    let built: Vec<A11yNode> = nodes
        .iter()
        .filter(|node| node.parent_id.is_none())
        .flat_map(|root| build_node(root, &by_id))
        .collect();
    if built.len() == 1 {
        built.into_iter().next().expect("checked length is one")
    } else {
        A11yNode {
            role: SYNTHETIC_ROOT_ROLE.to_string(),
            name: String::new(),
            value: None,
            children: built,
        }
    }
}

/// Convert one CDP node (and its subtree) into [`A11yNode`]s, splicing an ignored
/// node out by returning its children in its place.
fn build_node(node: &AxNode, by_id: &HashMap<&str, &AxNode>) -> Vec<A11yNode> {
    let children: Vec<A11yNode> = node
        .child_ids
        .iter()
        .flatten()
        .filter_map(|id| by_id.get(id.as_ref()).copied())
        .flat_map(|child| build_node(child, by_id))
        .collect();
    if node.ignored {
        return children;
    }
    vec![A11yNode {
        role: ax_string(&node.role).unwrap_or_default(),
        name: ax_string(&node.name).unwrap_or_default(),
        value: ax_string(&node.value),
        children,
    }]
}

/// The string content of a CDP [`AxValue`], or `None` when it is absent or not a
/// scalar.
fn ax_string(value: &Option<AxValue>) -> Option<String> {
    value.as_ref()?.value.as_ref().and_then(json_scalar_string)
}

/// Render a JSON scalar as a string, or `None` for null/array/object.
fn json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// Run `future` under a wall-clock `timeout`, mapping an overrun to
/// [`ExpectError::Timeout`].
async fn with_timeout<F, T>(timeout: Duration, future: F) -> Result<T, ExpectError>
where
    F: Future<Output = Result<T, ExpectError>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result,
        Err(_) => Err(ExpectError::Timeout {
            timeout_ms: timeout.as_millis() as u64,
        }),
    }
}

/// Map a chromiumoxide CDP failure to an [`ExpectError::Surface`].
fn map_cdp(err: CdpError) -> ExpectError {
    ExpectError::Surface(format!("browser CDP error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn selector(role: &str, name: Option<&str>) -> A11ySelector {
        A11ySelector {
            role: role.to_string(),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn resolves_mechanically_delegates_to_the_shared_a11y_dialect() {
        // The browser adapter resolves a recognized press/type or blank step
        // itself, and routes an unparseable step to the agent fallback — the
        // shared `step_resolves_mechanically` gate, wired through the trait.
        let adapter = BrowserAdapter::new("http://127.0.0.1:0/");
        assert!(adapter.resolves_mechanically("press button[name=\"Go\"]"));
        assert!(adapter.resolves_mechanically("   "));
        assert!(!adapter.resolves_mechanically("do something clever"));
    }

    /// Deserialize a flat CDP a11y node list from `value` (the wire shape).
    fn ax_nodes(value: serde_json::Value) -> Vec<AxNode> {
        serde_json::from_value(value).expect("valid AxNode list")
    }

    #[test]
    fn builds_a_nested_tree_with_roles_names_and_values() {
        let nodes = ax_nodes(json!([
            {
                "nodeId": "1", "ignored": false,
                "role": {"type": "role", "value": "RootWebArea"},
                "name": {"type": "computedString", "value": "Fixture"},
                "childIds": ["2", "3"]
            },
            {
                "nodeId": "2", "ignored": false, "parentId": "1",
                "role": {"type": "role", "value": "button"},
                "name": {"type": "computedString", "value": "Go"}
            },
            {
                "nodeId": "3", "ignored": false, "parentId": "1",
                "role": {"type": "role", "value": "textbox"},
                "name": {"type": "computedString", "value": "result"},
                "value": {"type": "string", "value": "clicked"}
            }
        ]));
        let tree = build_tree(&nodes);
        assert_eq!(tree.role, "RootWebArea");
        assert_eq!(tree.name, "Fixture");
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].role, "button");
        assert_eq!(tree.children[0].name, "Go");
        assert_eq!(tree.children[0].value, None);
        assert_eq!(tree.children[1].role, "textbox");
        assert_eq!(tree.children[1].value.as_deref(), Some("clicked"));
    }

    #[test]
    fn splices_ignored_nodes_and_reparents_their_children() {
        let nodes = ax_nodes(json!([
            {
                "nodeId": "1", "ignored": false,
                "role": {"type": "role", "value": "RootWebArea"},
                "name": {"type": "computedString", "value": ""},
                "childIds": ["2"]
            },
            {
                // An ignored wrapper: it should vanish, lifting its child up.
                "nodeId": "2", "ignored": true, "parentId": "1",
                "role": {"type": "role", "value": "generic"},
                "childIds": ["3"]
            },
            {
                "nodeId": "3", "ignored": false, "parentId": "2",
                "role": {"type": "role", "value": "button"},
                "name": {"type": "computedString", "value": "Go"}
            }
        ]));
        let tree = build_tree(&nodes);
        assert_eq!(tree.role, "RootWebArea");
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].role, "button");
        assert_eq!(tree.children[0].name, "Go");
    }

    #[test]
    fn find_backend_node_matches_role_and_name() {
        let nodes = ax_nodes(json!([
            {
                "nodeId": "1", "ignored": false,
                "role": {"type": "role", "value": "button"},
                "name": {"type": "computedString", "value": "Go"},
                "backendDOMNodeId": 42
            },
            {
                "nodeId": "2", "ignored": false,
                "role": {"type": "role", "value": "button"},
                "name": {"type": "computedString", "value": "Stop"},
                "backendDOMNodeId": 7
            }
        ]));
        let backend = find_backend_node(&nodes, &selector("button", Some("Stop")));
        assert_eq!(backend, Some(BackendNodeId::new(7)));
        // A role-only selector binds the first matching node.
        let any_button = find_backend_node(&nodes, &selector("button", None));
        assert_eq!(any_button, Some(BackendNodeId::new(42)));
        // An unmatched name does not bind.
        assert_eq!(
            find_backend_node(&nodes, &selector("button", Some("Nope"))),
            None
        );
    }

    #[test]
    fn quad_center_averages_the_four_corners() {
        // A 10x20 box at origin: corners (0,0)(10,0)(10,20)(0,20) → center (5,10).
        let quad = [0.0, 0.0, 10.0, 0.0, 10.0, 20.0, 0.0, 20.0];
        assert_eq!(quad_center(&quad).unwrap(), (5.0, 10.0));
        // Too few coordinates is a surface error, not a panic.
        assert!(quad_center(&[0.0, 0.0]).is_err());
    }
}
