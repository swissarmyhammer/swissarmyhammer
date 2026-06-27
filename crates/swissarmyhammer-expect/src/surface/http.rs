//! The `http` surface adapter — the deterministic, no-agent service path.
//!
//! Per `ideas/expect.md` §"Surface adapters" (the http row) and
//! §"Provisioning and Isolation": the adapter **provisions** the service (builds
//! and launches it, then *waits for ready* by polling a health URL), **drives**
//! it by issuing an HTTP request with an in-process Rust client (no Node, no
//! Python), **observes** the response (status / headers / body) into an http
//! [`SurfaceState`], and **tears it down** by stopping the launched process.
//!
//! The locator dialect for the observed state — `status`, `header:<name>`, and a
//! json-path into the body — lives in the [assertion compiler](crate::assertion);
//! this module only produces the [`HttpState`] those locators resolve against.
//!
//! The build/launch commands reuse the cli adapter's `ProjectType → {build,
//! launch}` resolution ([`resolve_commands`]), so detection and `setup:`
//! overrides behave identically across surfaces. Unlike cli, where launch *and*
//! drive are one short-lived run, an http service is long-lived: `provision`
//! spawns the launch command as a child that keeps running, and `teardown` kills
//! it (reusing the cli adapter's process-group [`abort_child`]).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, CONTENT_TYPE};
use reqwest::Method;

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::cli::{abort_child, resolve_commands, CliCommands};
use crate::surface::SurfaceAdapter;
use crate::types::{HttpState, SurfaceState};

/// The default per-request wall-clock budget when an adapter is built without an
/// explicit timeout.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The default budget for the service to become ready during `provision`.
const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// How often `provision` polls the readiness URL before its deadline.
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The per-probe budget for a single readiness poll. Kept short and independent
/// of the per-request timeout so a host that accepts the connection but then
/// stalls cannot overrun the overall readiness deadline by a full request budget.
const READINESS_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// The default path polled (relative to the base URL) to decide the service is
/// up. Any HTTP response — even a 404 — means it is accepting connections.
const DEFAULT_READY_PATH: &str = "/";

/// The HTTP methods recognized as the leading token of a `When` step.
const HTTP_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

/// The content type sent for a request that carries a body (the json dialect).
const JSON_CONTENT_TYPE: &str = "application/json";

/// The http surface adapter: launches a service, waits for it to be ready,
/// issues requests, and reads status/headers/body.
///
/// Construct with [`HttpAdapter::new`], passing the base URL the service listens
/// on; the builder methods refine the readiness path and the request/readiness
/// budgets. The adapter is deterministic and mechanical — an http call is always
/// a concrete request — so it resolves every step itself and never reaches the
/// agent fallback (the trait's default
/// [`resolves_mechanically`](SurfaceAdapter::resolves_mechanically) of `true`).
#[derive(Debug, Clone)]
pub struct HttpAdapter {
    base_url: String,
    ready_path: String,
    request_timeout: Duration,
    readiness_timeout: Duration,
}

impl HttpAdapter {
    /// Create an http adapter for a service reachable at `base_url` (e.g.
    /// `http://127.0.0.1:8080`), with the default readiness path and budgets.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            ready_path: DEFAULT_READY_PATH.to_string(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            readiness_timeout: DEFAULT_READINESS_TIMEOUT,
        }
    }

    /// Poll `ready_path` (relative to the base URL) instead of the default `/`
    /// when waiting for the service to come up.
    pub fn with_ready_path(mut self, ready_path: impl Into<String>) -> Self {
        self.ready_path = ready_path.into();
        self
    }

    /// Set the per-request wall-clock budget; a request that exceeds it surfaces
    /// as [`ExpectError::Timeout`].
    pub fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    /// Set how long `provision` waits for the service to become ready before
    /// giving up with [`ExpectError::Timeout`].
    pub fn with_readiness_timeout(mut self, readiness_timeout: Duration) -> Self {
        self.readiness_timeout = readiness_timeout;
        self
    }

    /// The absolute URL polled for readiness.
    fn ready_url(&self) -> String {
        join_url(&self.base_url, &self.ready_path)
    }
}

/// The provisioned http system under test.
pub struct HttpSut {
    /// The in-process client used for readiness polling and every driven request.
    client: Client,
    /// The base URL the service listens on; `drive` joins each request path onto
    /// it.
    base_url: String,
    /// The launched service process, if `provision` started one. `None` when the
    /// service is externally managed (no launch command resolved).
    launched: Option<Child>,
    /// The most recent response's authoritative read; set by `drive`, read by
    /// `observe`.
    last: Option<HttpState>,
}

impl std::fmt::Debug for HttpSut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSut")
            .field("base_url", &self.base_url)
            .field("launched", &self.launched.is_some())
            .field("last", &self.last)
            .finish()
    }
}

impl SurfaceAdapter for HttpAdapter {
    type ProvisionedSut = HttpSut;

    fn provision(&self, setup: Option<&Setup>, repo_root: &Path) -> Result<HttpSut, ExpectError> {
        let commands = resolve_launch_commands(setup, repo_root)?;
        for build in &commands.build {
            run_build_step(build, repo_root)?;
        }
        let mut launched = if commands.launch.is_empty() {
            None
        } else {
            Some(spawn_launch(&commands.launch, repo_root)?)
        };

        let client = build_client(self.request_timeout)?;
        // Readiness polls with a short per-probe timeout, independent of the
        // request budget, so a host that accepts but stalls cannot overrun the
        // readiness deadline by a full request timeout.
        let probe_client = build_client(READINESS_PROBE_TIMEOUT)?;
        if let Err(err) = wait_for_ready(
            &probe_client,
            &self.ready_url(),
            self.readiness_timeout,
            &mut launched,
        ) {
            // Do not leave a launched-but-never-ready process running.
            if let Some(mut child) = launched {
                abort_child(&mut child);
            }
            return Err(err);
        }

        Ok(HttpSut {
            client,
            base_url: self.base_url.clone(),
            launched,
            last: None,
        })
    }

    fn drive(&self, sut: &mut HttpSut, when_step: &str) -> Result<(), ExpectError> {
        let request = Request::parse(when_step);
        let url = join_url(&sut.base_url, &request.path);
        let mut builder = sut.client.request(request.method, url);
        if let Some(body) = request.body {
            builder = builder.header(CONTENT_TYPE, JSON_CONTENT_TYPE).body(body);
        }
        let response = builder
            .send()
            .map_err(|err| map_request_error(err, self.request_timeout))?;

        let status = response.status().as_u16();
        let headers = collect_headers(response.headers());
        let body = response
            .text()
            .map_err(|err| map_request_error(err, self.request_timeout))?;

        sut.last = Some(HttpState {
            status,
            headers,
            body,
        });
        Ok(())
    }

    fn observe(&self, sut: &HttpSut) -> Result<SurfaceState, ExpectError> {
        let state = sut.last.clone().ok_or_else(|| {
            ExpectError::Surface("nothing to observe: drive the http SUT first".to_string())
        })?;
        Ok(SurfaceState::Http(state))
    }

    fn teardown(&self, sut: HttpSut) -> Result<(), ExpectError> {
        // Stop the launched service so a `check` does not leak a running server.
        // A process group kill (via `abort_child`) reaps any worker children the
        // server spawned, mirroring the cli adapter's timeout abort.
        if let Some(mut child) = sut.launched {
            abort_child(&mut child);
        }
        Ok(())
    }
}

/// Resolve the build-and-launch commands for the http SUT.
///
/// When `setup` is present it is authoritative (last command launches, earlier
/// ones build), reusing the cli adapter's [`resolve_commands`]. When `setup` is
/// absent the adapter attempts project detection, but — unlike cli — tolerates
/// "no project type": an http surface may target an already-running service, so
/// a missing project simply yields no launch command rather than an error.
fn resolve_launch_commands(
    setup: Option<&Setup>,
    repo_root: &Path,
) -> Result<CliCommands, ExpectError> {
    if setup.is_some() {
        resolve_commands(setup, repo_root)
    } else {
        Ok(
            resolve_commands(None, repo_root).unwrap_or_else(|_| CliCommands {
                build: Vec::new(),
                launch: Vec::new(),
            }),
        )
    }
}

/// Build the blocking HTTP client used for readiness polling and every request.
fn build_client(request_timeout: Duration) -> Result<Client, ExpectError> {
    Client::builder()
        .timeout(request_timeout)
        .build()
        .map_err(|err| ExpectError::Surface(format!("failed to build http client: {err}")))
}

/// Run one provisioning build step to completion, failing on a non-zero exit.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] for an empty command or a non-zero exit, and
/// [`ExpectError::Io`] when the process cannot be spawned.
fn run_build_step(argv: &[String], work_dir: &Path) -> Result<(), ExpectError> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| ExpectError::Surface("empty build command: nothing to run".to_string()))?;
    let output = Command::new(program)
        .args(args)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(ExpectError::Surface(format!(
            "build step `{}` failed (exit {:?}): {}",
            argv.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Spawn the launch command as a long-lived service child, detaching its streams.
///
/// On unix the child leads its own process group so [`abort_child`] can reap the
/// whole tree at teardown. Its stdio is discarded — a chatty server must not
/// deadlock against a full pipe buffer, and the verdict reads the HTTP response,
/// never the process streams.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] for an empty launch command and
/// [`ExpectError::Io`] when the process cannot be spawned.
fn spawn_launch(argv: &[String], work_dir: &Path) -> Result<Child, ExpectError> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| ExpectError::Surface("empty launch command: nothing to run".to_string()))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map_err(ExpectError::Io)
}

/// Poll `ready_url` until the service answers, the deadline passes, or a launched
/// service exits early.
///
/// Readiness means any completed HTTP response (even an error status): the server
/// is accepting connections. A connection that is still refused is retried until
/// `timeout` elapses.
///
/// # Errors
///
/// Returns [`ExpectError::Timeout`] when the service is not ready within
/// `timeout`, and [`ExpectError::Surface`] when a launched service process exits
/// before becoming ready.
fn wait_for_ready(
    client: &Client,
    ready_url: &str,
    timeout: Duration,
    launched: &mut Option<Child>,
) -> Result<(), ExpectError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(child) = launched.as_mut() {
            if let Some(status) = child.try_wait()? {
                return Err(ExpectError::Surface(format!(
                    "launched service exited before becoming ready (status {status})"
                )));
            }
        }
        if client.get(ready_url).send().is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ExpectError::Timeout {
                timeout_ms: timeout.as_millis() as u64,
            });
        }
        std::thread::sleep(READINESS_POLL_INTERVAL);
    }
}

/// One parsed HTTP request from a `When` step in the http dialect.
struct Request {
    /// The HTTP method.
    method: Method,
    /// The request path (always starts with `/` unless an absolute URL).
    path: String,
    /// The request body, when the step supplied one.
    body: Option<String>,
}

impl Request {
    /// Parse a `When` step in the http dialect into a request.
    ///
    /// The dialect is `[METHOD] <path> [body]`: a leading HTTP method token
    /// (case-insensitive) is optional and defaults to `GET`; the next token is
    /// the path; any remainder is the request body. An empty step is `GET /`.
    fn parse(when_step: &str) -> Self {
        let trimmed = when_step.trim();
        if trimmed.is_empty() {
            return Request {
                method: Method::GET,
                path: "/".to_string(),
                body: None,
            };
        }
        let (first, rest) = split_first_word(trimmed);
        match parse_method(first) {
            Some(method) => {
                let (path, body) = split_first_word(rest);
                let path = if path.is_empty() { "/" } else { path };
                Request {
                    method,
                    path: path.to_string(),
                    body: non_empty(body),
                }
            }
            None => Request {
                method: Method::GET,
                path: first.to_string(),
                body: non_empty(rest),
            },
        }
    }
}

/// Parse a token as an HTTP method (case-insensitive), or `None` when it is not
/// one of [`HTTP_METHODS`].
fn parse_method(token: &str) -> Option<Method> {
    let upper = token.to_ascii_uppercase();
    if HTTP_METHODS.contains(&upper.as_str()) {
        Method::from_bytes(upper.as_bytes()).ok()
    } else {
        None
    }
}

/// Split `text` into its first whitespace-delimited word and the trimmed
/// remainder (`("", "")` for blank input).
fn split_first_word(text: &str) -> (&str, &str) {
    let trimmed = text.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim_start()),
        None => (trimmed, ""),
    }
}

/// `Some(text)` when `text` is non-empty, else `None`.
fn non_empty(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Join a request `path` onto `base_url`, leaving an absolute URL untouched.
fn join_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let base = base_url.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// Collect response headers into a name-lowercased map (sorted for stable
/// serialization). A header repeated across lines keeps its last value.
fn collect_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_ascii_lowercase(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Map a request failure to an [`ExpectError`]: a timed-out request becomes
/// [`ExpectError::Timeout`]; anything else is a surface error.
fn map_request_error(err: reqwest::Error, request_timeout: Duration) -> ExpectError {
    if err.is_timeout() {
        ExpectError::Timeout {
            timeout_ms: request_timeout.as_millis() as u64,
        }
    } else {
        ExpectError::Surface(format!("http request failed: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_an_empty_step_to_get_root() {
        let request = Request::parse("");
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path, "/");
        assert!(request.body.is_none());
    }

    #[test]
    fn parse_reads_method_path_and_body() {
        let request = Request::parse("POST /cart {\"item\": 1}");
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.path, "/cart");
        assert_eq!(request.body.as_deref(), Some("{\"item\": 1}"));
    }

    #[test]
    fn parse_treats_a_bare_path_as_a_get() {
        let request = Request::parse("/widgets");
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path, "/widgets");
        assert!(request.body.is_none());
    }

    #[test]
    fn parse_accepts_a_lowercase_method() {
        let request = Request::parse("get /health");
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path, "/health");
    }

    #[test]
    fn parse_defaults_a_method_only_step_to_root() {
        let request = Request::parse("DELETE");
        assert_eq!(request.method, Method::DELETE);
        assert_eq!(request.path, "/");
        assert!(request.body.is_none());
    }

    #[test]
    fn join_url_normalizes_slashes_and_relative_paths() {
        assert_eq!(
            join_url("http://127.0.0.1:8080", "/widgets"),
            "http://127.0.0.1:8080/widgets"
        );
        // A trailing slash on the base is not doubled.
        assert_eq!(
            join_url("http://127.0.0.1:8080/", "/widgets"),
            "http://127.0.0.1:8080/widgets"
        );
        // A relative path gains a leading slash.
        assert_eq!(
            join_url("http://127.0.0.1:8080", "widgets"),
            "http://127.0.0.1:8080/widgets"
        );
        // An absolute URL passes through untouched.
        assert_eq!(
            join_url("http://127.0.0.1:8080", "http://example.test/x"),
            "http://example.test/x"
        );
    }

    #[test]
    fn collect_headers_lowercases_names() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, JSON_CONTENT_TYPE.parse().unwrap());
        let collected = collect_headers(&headers);
        assert_eq!(
            collected.get("content-type").map(String::as_str),
            Some(JSON_CONTENT_TYPE)
        );
    }
}
