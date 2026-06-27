---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4bj5vqvdrsempy65mt1yp8
  text: 'Picked up. Plan: (1) add SurfaceState::Http(HttpState{status,headers,body}) in types.rs additively; (2) extend assertion.rs with Locator::Status + Locator::Header{name} resolution + Http handling in checkpoint_json/streams/exit, plus status/header prose recognizers (json-path body already compiles); (3) new surface/http.rs HttpAdapter (reqwest::blocking client, ProjectType→cmd reuse from cli for build/launch, spawn launch child, wait_for_ready by polling, drive issues METHOD PATH [body], observe -> SurfaceState::Http, teardown kills child, resolves_mechanically=true). Tests: in-process axum fixture bound to 127.0.0.1:0 in tests/http.rs for provision+wait-ready+drive+observe+locators; never-ready -> clean timeout; teardown stops a spawned child. reqwest blocking is safe since adapters run under spawn_blocking in prod and plain #[test] in tests.'
  timestamp: 2026-06-27T10:56:15.735089+00:00
- actor: claude-code
  id: 01kw4cbfcvta17ac2wcxprvq95
  text: 'Implemented. Files: types.rs (SurfaceState::Http(HttpState{status,headers,body}) additive + round-trip test), assertion.rs (Locator::Status + Locator::Header{name} resolve/Display/serde; Http handling in checkpoint_json/streams/exit + http_state helper; status/header prose recognizers ordered before literal; build_candidate binds them with self-verify; 6 new tests), surface/cli.rs (resolve_commands + abort_child now pub(crate) for reuse), surface/http.rs (NEW: HttpAdapter/HttpSut, reqwest::blocking client, reuses cli resolve_commands for build/launch, spawns launch child + wait_for_ready polling, drive parses METHOD PATH [body], observe -> SurfaceState::Http, teardown kills child via abort_child; 7 module tests), surface/mod.rs + lib.rs (module + exports), ledger.rs scrub_state Http arm, Cargo.toml (reqwest blocking dep + axum dev-dep). Integration tests tests/http.rs: in-process axum fixture bound to 127.0.0.1:0, provision+wait-ready+drive+observe asserts status/header/json-path locators at Tier 1, never-ready clean timeout, teardown stops launched child, build-steps-before-launch. Verified GREEN: nextest -E ''test(http) or test(surface) or test(assertion)'' 63 passed; --test http 4 passed; full crate 244 passed; cargo check --workspace ok; fmt --check clean; clippy -D warnings clean.'
  timestamp: 2026-06-27T11:10:04.699905+00:00
- actor: claude-code
  id: 01kw4cvaxtrjdqwx84wf4qgzv0
  text: 'Addressed all 3 advisory double-check findings (it confirmed acceptance criteria were genuinely met; findings were refinements): (1) status cue now matches as a whole word (word_present) instead of a bare substring, and added a test pinning that "the response status is 200" binds Locator::Status even when the body also has a `status` field (deliberate precedence: status is the most durable http locator, still self-verified); (2) wait_for_ready now uses a short dedicated READINESS_PROBE_TIMEOUT (1s) client so a host that accepts-then-stalls cannot overrun the readiness deadline by a full request budget; (3) never-ready test now holds a bound-but-never-accepted TcpListener (deterministic, no ephemeral-port TOCTOU reuse race) instead of bind-then-drop. Re-verified GREEN: nextest -E ''test(http) or test(surface) or test(assertion)'' 64 passed; full crate 245 passed; cargo check --workspace ok; cargo fmt --check clean; cargo clippy --all-targets -D warnings clean. Task left in doing for /review.'
  timestamp: 2026-06-27T11:18:44.410922+00:00
depends_on:
- 01KW262DJSSC1JX2FXCDQN0X4E
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: doing
position_ordinal: '8280'
project: expect
title: http surface adapter (request/response + json-path locator)
---
## What
Add the `http` surface: drive by issuing requests, observe status/headers/body. Per `ideas/expect.md` §"Surface adapters" (http row) and the locator table.

- New `crates/swissarmyhammer-expect/src/surface/http.rs` implementing `SurfaceAdapter`:
  - Provision: build + launch the service and WAIT FOR READY (poll a health/port) via detected build/launch + `setup:`; teardown stops it.
  - Drive: issue the request (an in-process Rust HTTP client — reqwest or the workspace's existing client; NO Node/Python).
  - Observe: capture `status` / `header:<name>` / body into an http `SurfaceState`.
  - Locator dialect: `status` / `header:<name>` / json-path (json-path stable, preferred). Extend the assertion compiler's locator resolution for http.
- Deterministic surface (mechanical), runs once by default.

## Acceptance Criteria
- [ ] Against a fixture HTTP server, the adapter provisions+waits-for-ready, issues a request, and observes status/headers/json body.
- [ ] json-path / `status` / `header:` locators bind and evaluate at Tier 1.
- [ ] Teardown stops the service; a never-ready service times out cleanly.

## Tests
- [ ] Integration test spinning up a tiny fixture HTTP server (e.g. a hyper/axum test server) and asserting an observed status + json-path value.
- [ ] `cargo nextest run -p swissarmyhammer-expect http` passes.

## Workflow
- Use `/tdd`.