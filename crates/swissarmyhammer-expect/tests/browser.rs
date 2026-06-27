//! Integration coverage for the `browser` surface adapter against a real,
//! in-process fixture web page driven through a real Chromium over CDP.
//!
//! The fixture is a tiny axum app bound to `127.0.0.1:0` (an OS-assigned
//! ephemeral port) serving one static HTML page on a background thread with its
//! own Tokio runtime. The adapter under test launches Chromium via
//! `chromiumoxide`, navigates to the page, presses a `button[name="…"]` by its
//! accessibility role+name through CDP `Input`, snapshots the resulting a11y tree
//! through CDP `Accessibility`, and the observed node value is asserted via the
//! a11y locator dialect.
//!
//! **Gated on Chromium.** Launching needs a real Chromium/headless binary, which
//! is not present on every CI host, so the test skips cleanly (and logs) when
//! [`BrowserAdapter::chromium_available`] is false rather than failing the suite.
//! The browser-free coverage — the a11y locator parsing/resolution and the
//! adapter's pure drive-dialect/tree-building logic — lives in the unit tests and
//! always runs.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::sync::oneshot;

use swissarmyhammer_expect::{
    compile, AssertionOutcome, BrowserAdapter, Checkpoint, Criterion, Observation, SurfaceAdapter,
    SurfaceState, Trajectory,
};

/// A generous per-action budget; the in-process fixture and local Chromium are
/// fast, but headless launch can be slow on a cold cache.
const ACTION_TIMEOUT: Duration = Duration::from_secs(60);

/// The accessible name of the button the test presses.
const BUTTON_NAME: &str = "Go";

/// The accessible name (aria-label) of the textbox the page updates.
const TEXTBOX_NAME: &str = "result";

/// The value the page writes into the textbox when the button is pressed — the
/// single source of truth the observed-value assertion is checked against.
const CLICKED_VALUE: &str = "clicked";

/// The static page served by the fixture: a button that, when pressed, writes
/// `CLICKED_VALUE` into a labelled text input. Built from the same constants the
/// assertions use so the page and the checks cannot drift apart.
fn fixture_html() -> String {
    format!(
        "<!doctype html><html><body>\
         <button id=\"go\">{BUTTON_NAME}</button>\
         <input id=\"out\" type=\"text\" aria-label=\"{TEXTBOX_NAME}\" value=\"\" readonly>\
         <script>\
         document.getElementById('go').addEventListener('click', function() {{\
           document.getElementById('out').value = '{CLICKED_VALUE}';\
         }});\
         </script>\
         </body></html>"
    )
}

/// A tiny axum fixture web server on a background thread, shut down on drop.
struct FixtureServer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FixtureServer {
    /// Start the fixture on an OS-assigned ephemeral port and return once it is
    /// bound (its address is read back over a channel).
    fn start() -> Self {
        let (addr_tx, addr_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build fixture runtime");
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("bind fixture listener");
                addr_tx
                    .send(listener.local_addr().expect("fixture local addr"))
                    .expect("send fixture addr");

                let app = Router::new().route("/app", get(|| async { Html(fixture_html()) }));

                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.await;
                    })
                    .await
                    .expect("serve fixture");
            });
        });

        let addr = addr_rx.recv().expect("receive fixture addr");
        FixtureServer {
            addr,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    /// The URL of the served page.
    fn page_url(&self) -> String {
        format!("http://{}/app", self.addr)
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Wrap an observed surface state in a single-checkpoint observation so the a11y
/// locator dialect can be compiled and evaluated against it.
fn observation_of(state: SurfaceState) -> Observation {
    Observation {
        path: "fixture".to_string(),
        checkpoints: vec![Checkpoint {
            after: "final".to_string(),
            state,
            duration: Duration::from_millis(1),
        }],
        trajectory: Trajectory { steps: Vec::new() },
    }
}

/// An unchecked criterion from `text`.
fn criterion(text: &str) -> Criterion {
    Criterion {
        text: text.to_string(),
        checked: false,
    }
}

#[test]
fn presses_a_button_by_role_name_and_observes_an_a11y_node_value() {
    if !BrowserAdapter::chromium_available() {
        eprintln!(
            "SKIP presses_a_button_by_role_name_and_observes_an_a11y_node_value: \
             no Chromium/headless binary found (chromiumoxide detection failed); \
             the browser-free a11y locator and adapter unit tests still cover the logic"
        );
        return;
    }

    let server = FixtureServer::start();
    let adapter = BrowserAdapter::new(server.page_url()).with_action_timeout(ACTION_TIMEOUT);

    // Provision launches Chromium and navigates to the fixture page.
    let mut sut = adapter
        .provision(None, Path::new("."))
        .expect("provision launches chromium and opens the page");

    // Drive: press the button by accessibility role + name through CDP Input.
    let press = format!("press button[name=\"{BUTTON_NAME}\"]");
    adapter.drive(&mut sut, &press).expect("press the button");

    // Observe: snapshot the resulting accessibility tree.
    let state = adapter.observe(&sut).expect("observe the a11y tree");
    let SurfaceState::A11y { tree } = &state else {
        adapter.teardown(sut).expect("teardown");
        panic!("expected an a11y surface state, got {state:?}");
    };
    assert!(
        !tree.children.is_empty(),
        "the observed a11y tree should not be empty"
    );

    // The a11y locator dialect binds and evaluates over the real observation:
    // the textbox the button updated now carries the observed value.
    let observation = observation_of(state.clone());
    let text = format!("textbox[name=\"{TEXTBOX_NAME}\"] equals {CLICKED_VALUE}");
    let assertion = compile(&criterion(&text), &observation)
        .unwrap_or_else(|err| panic!("compile `{text}`: {err}"));
    assert_eq!(
        assertion.evaluate(&observation),
        AssertionOutcome::Holds,
        "the observed textbox value should equal `{CLICKED_VALUE}`"
    );

    // Teardown closes the browser.
    adapter.teardown(sut).expect("teardown closes the browser");
}
