---
assignees:
- claude-code
depends_on:
- 01KW262DJSSC1JX2FXCDQN0X4E
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: todo
position_ordinal: be80
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