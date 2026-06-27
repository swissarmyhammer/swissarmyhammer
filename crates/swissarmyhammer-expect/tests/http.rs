//! Integration coverage for the `http` surface adapter against a real, in-process
//! fixture HTTP server.
//!
//! The fixture is a tiny axum app bound to `127.0.0.1:0` (an OS-assigned
//! ephemeral port, read back so the tests never collide on a fixed port) running
//! on a background thread with its own Tokio runtime. The adapter under test is
//! synchronous (it uses a blocking client), so the test functions are plain
//! `#[test]`s and talk to the fixture over real TCP — exercising the production
//! path: provision + wait-for-ready, drive (issue a request), observe
//! (status/headers/json body), and teardown.

use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::oneshot;

use swissarmyhammer_expect::{
    compile, AssertionOutcome, Checkpoint, Criterion, HttpAdapter, Locator, Observation,
    SurfaceAdapter, SurfaceState, Trajectory,
};

/// A generous readiness budget; the in-process fixture is up almost immediately.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// A deliberately tiny readiness budget for the never-ready case.
const READY_TIMEOUT_SHORT: Duration = Duration::from_millis(500);

/// The widgets payload the fixture serves — the single source of truth the body
/// assertions are checked against.
const WIDGETS_BODY: &str = r#"{"total": 40, "items": ["a", "b"]}"#;

/// A tiny axum fixture HTTP server on a background thread, shut down on drop.
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

                let app = Router::new()
                    .route("/health", get(|| async { "ok" }))
                    .route("/widgets", get(widgets));

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

    /// The base URL the fixture listens on.
    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
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

/// The `/widgets` handler: a 200 with a JSON content type and the widgets body.
async fn widgets() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        WIDGETS_BODY,
    )
}

/// Bind a listener that is never `accept()`ed, returning it (the caller keeps it
/// alive) plus a base URL pointing at it.
///
/// Connections complete the TCP handshake via the kernel backlog but never
/// receive an HTTP response, so the service is deterministically "never ready" —
/// and because the listener is held for the test's duration the port cannot be
/// reused (no TOCTOU race that a "bind then drop" approach would have).
fn never_ready_listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind never-ready listener");
    let addr = listener.local_addr().expect("never-ready addr");
    (listener, format!("http://{addr}"))
}

/// Wrap an observed http state in a single-checkpoint observation so the locator
/// dialect can be compiled and evaluated against it.
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
fn provisions_waits_for_ready_drives_and_observes_status_headers_and_body() {
    let server = FixtureServer::start();
    let repo = tempfile::TempDir::new().unwrap();
    let adapter = HttpAdapter::new(server.base_url())
        .with_ready_path("/health")
        .with_readiness_timeout(READY_TIMEOUT);

    // Provision waits for the service to be ready (no launch command needed — the
    // fixture is already up — so provision polls /health until it answers).
    let mut sut = adapter.provision(None, repo.path()).expect("provision");

    // Observing before driving is an error: there is nothing captured yet.
    assert!(
        adapter.observe(&sut).is_err(),
        "observe before drive must error"
    );

    // Drive: issue the request in the http dialect.
    adapter.drive(&mut sut, "GET /widgets").expect("drive");

    // Observe: capture status / headers / body into an http SurfaceState.
    let state = adapter.observe(&sut).expect("observe");
    let SurfaceState::Http(http) = &state else {
        panic!("expected an http surface state, got {state:?}");
    };
    assert_eq!(http.status, 200, "observed status");
    assert_eq!(
        http.headers.get("content-type").map(String::as_str),
        Some("application/json"),
        "observed content-type header (lowercased name)"
    );
    let body: serde_json::Value = serde_json::from_str(&http.body).expect("body is json");
    assert_eq!(body["total"], serde_json::json!(40), "observed json body");

    // The http locator dialect binds and evaluates at Tier 1 over the real
    // observation: status, header:, and a json-path into the body.
    let observation = observation_of(state.clone());
    for text in [
        "the response status is 200",
        "the content-type header is application/json",
        "the total is 40",
    ] {
        let assertion = compile(&criterion(text), &observation)
            .unwrap_or_else(|err| panic!("compile `{text}`: {err}"));
        assert_eq!(
            assertion.tier,
            swissarmyhammer_expect::VerdictTier::Deterministic,
            "`{text}` compiles at Tier 1"
        );
        assert_eq!(
            assertion.evaluate(&observation),
            AssertionOutcome::Holds,
            "`{text}` evaluates Holds"
        );
    }
    // The json-path is the durable body locator the compiler preferred.
    let total = compile(&criterion("the total is 40"), &observation).unwrap();
    assert_eq!(
        total.locator,
        Locator::JsonPath {
            path: "$.total".to_string()
        }
    );

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn provision_times_out_cleanly_when_the_service_never_becomes_ready() {
    let repo = tempfile::TempDir::new().unwrap();
    // The listener is held for the test's duration: connections are accepted by
    // the kernel backlog but never answered, so readiness never succeeds.
    let (_listener, base_url) = never_ready_listener();
    let adapter = HttpAdapter::new(base_url).with_readiness_timeout(READY_TIMEOUT_SHORT);

    let start = Instant::now();
    let err = adapter
        .provision(None, repo.path())
        .expect_err("must time out");

    assert!(
        matches!(err, swissarmyhammer_expect::ExpectError::Timeout { .. }),
        "expected a timeout, got {err:?}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "provision hung instead of timing out cleanly"
    );
}

#[cfg(unix)]
fn write_executable(dir: &std::path::Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
}

/// Whether process `pid` is still alive, via `kill -0` (no extra dependency).
#[cfg(unix)]
fn process_alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Poll `predicate` until it holds or `budget` elapses; returns whether it held.
#[cfg(unix)]
fn poll_until(budget: Duration, predicate: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    predicate()
}

#[cfg(unix)]
#[test]
fn teardown_stops_the_launched_service_process() {
    // The readiness fixture lets provision succeed; the launched process is a
    // real long-lived child whose teardown we verify. The launch script records
    // its own pid (kept across `exec`) so the test can assert it is reaped.
    let server = FixtureServer::start();
    let repo = tempfile::TempDir::new().unwrap();
    write_executable(
        repo.path(),
        "launch.sh",
        "#!/bin/sh\necho $$ > server.pid\nexec sleep 30\n",
    );

    let adapter = HttpAdapter::new(server.base_url())
        .with_ready_path("/health")
        .with_readiness_timeout(READY_TIMEOUT);
    let setup = swissarmyhammer_expect::Setup::Command("./launch.sh".to_string());
    let sut = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision launches the service");

    let pid_file = repo.path().join("server.pid");
    assert!(
        poll_until(Duration::from_secs(2), || pid_file.is_file()),
        "the launched process should record its pid"
    );
    let pid = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .to_string();
    assert!(process_alive(&pid), "the launched process is running");

    adapter.teardown(sut).expect("teardown");

    assert!(
        poll_until(Duration::from_secs(2), || !process_alive(&pid)),
        "teardown should stop the launched process (pid {pid})"
    );
}

#[cfg(unix)]
#[test]
fn provision_runs_build_steps_before_launching_the_service() {
    // A build step writes a marker before the launch step runs; provision must
    // run it to completion first.
    let server = FixtureServer::start();
    let repo = tempfile::TempDir::new().unwrap();
    write_executable(
        repo.path(),
        "build.sh",
        "#!/bin/sh\necho built > built.marker\n",
    );
    write_executable(repo.path(), "launch.sh", "#!/bin/sh\nexec sleep 30\n");

    let adapter = HttpAdapter::new(server.base_url())
        .with_ready_path("/health")
        .with_readiness_timeout(READY_TIMEOUT);
    let setup = swissarmyhammer_expect::Setup::Commands(vec![
        "./build.sh".to_string(),
        "./launch.sh".to_string(),
    ]);
    let sut = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision builds then launches");

    assert!(
        repo.path().join("built.marker").is_file(),
        "the build step ran before launch"
    );

    adapter.teardown(sut).expect("teardown");
}
