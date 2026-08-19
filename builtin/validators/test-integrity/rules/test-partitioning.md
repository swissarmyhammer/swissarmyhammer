---
name: test-partitioning
description: >-
  Integration tests must live in their own targets or packages that the
  platform test tool selects. An environment variable must not switch tests on
  or off, and CI must run every test target.
---

# Test Partitioning Validator

You are a test integrity validator. This rule examines how the change divides
tests into unit and integration groups, and how each group runs.

## The two kinds

- **Unit tests** are fast, and they give coverage. Each one examines the
  code's logic, needs no external system, and completes in seconds.
- **Integration tests** can be slower. Each one exercises a real scenario
  against a real system — a real model, a real database, a real server.

## The convention

- Unit tests and integration tests live in different targets or packages.
- Selection uses the platform's NATIVE filtering and selection methods —
  targets, packages, paths, and registered markers. An invented mechanism,
  such as an environment variable, is forbidden.
- The platform test tool selects each target directly:
  - Rust: unit tests are `#[cfg(test)]` modules and fast `tests/` targets, and
    `cargo test` or `cargo nextest run` runs them. An integration test is its
    own target in `tests/`, selected with `cargo test --test <name>` or a
    nextest filter expression — or its own crate that the workspace
    `default-members` list excludes, selected with `cargo test -p <crate>`.
  - Python: an integration test lives in its own directory or package (for
    example `tests/integration/`). The tool selects it by path, or by a marker
    that the pytest configuration registers.
  - JavaScript/TypeScript: an integration test lives in its own directory,
    with its own runner configuration or project entry.
  - Go: an integration test lives in its own package. The tool selects it by
    path (`go test ./integration/...`).
  - Swift: an integration test lives in a nested package (for example
    `IntegrationTests/`). The root `swift test` runs unit tests only — not by
    convention, but because the root manifest declares no integration target.
    The tool selects the integration suite with
    `swift test --package-path IntegrationTests`. Swift Testing tags organize
    scenarios, they do not select: the `swift test` CLI filters by test name
    only (`--filter`/`--skip`), so the package boundary carries the split.
  - Dart/Flutter: an integration test lives in `integration_test/`.
- The separation is structural. The default test command cannot see the
  integration target, so the code needs no guard, no skip, and no switch.
- An environment variable is NOT the convention. Do not use one to select,
  skip, or switch tests.
- Unit tests run all the time: on each local test run, in each TDD cycle, and
  in CI. The TDD red-green loop runs unit tests, never integration tests.
- Integration tests run in CI, and when the developer judges a run is
  appropriate — above all when the work targets that integration. The
  developer then selects the integration target directly with the platform
  test tool. The default test run does not include them.
- CI must run each unit target and each integration target. A target that no
  CI task runs gives no protection.

## What to flag

1. **Environment-variable test switching** — a test, a test helper, or a test
   configuration that reads an environment variable to decide if a test runs,
   which suite runs, or which mode the test uses. Examples:
   - `if std::env::var("RUN_INTEGRATION").is_err() { return; }`
   - `@pytest.mark.skipif(not os.environ.get("INTEGRATION"), ...)`
   - `if (!process.env.INTEGRATION) return;`
   - `if os.Getenv("INTEGRATION") == "" { t.Skip(...) }`
   The correct structure is a separate target or package, never an
   environment variable.
2. **A test-mode switch in production code** — production code that reads an
   environment variable to change its behavior for tests (`if TEST_MODE`).
3. **An integration test inside the unit target** — a test in the unit target
   or package that uses a real external system: the network, a database, a
   spawned server, a real service. It belongs in an integration target.
4. **A test target that CI does not run** — a CI workflow change that removes
   or omits a unit or integration test target, or a new integration target
   with no CI task that runs it. Each unit target and each integration target
   must appear in a CI task.

## Exceptions (Allow)

- An environment variable that carries configuration into a test that always
  runs — a port, a path, a timeout the harness sets. The variable tunes the
  test. It does not decide if the test runs.
- A platform-conditional skip with a clear condition
  (`skipIf(process.platform === 'win32')`). The condition reads the platform,
  not an environment switch.
- A standard runner variable that CI sets for the whole run
  (`RUST_BACKTRACE`, a locale). It selects no test.
- Compile-time test configuration (`#[cfg(test)]`, build tags the test tool
  itself sets). Compile-time selection is a platform mechanism, not an
  environment switch.

## Bottom Line

A test target is the unit of selection. The platform test tool selects
targets, CI runs all of them, and an environment variable selects nothing.
