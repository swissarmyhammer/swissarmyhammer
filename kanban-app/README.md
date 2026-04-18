# kanban-app

Tauri v2 desktop binary that hosts the kanban GUI. Rust sources live under
`src/`, the React UI under `ui/`, and the end-to-end tauri-driver harness
under `e2e/`.

## Layout

```
kanban-app/
  src/              Tauri backend (Rust)
  ui/               React frontend
  e2e/              WebdriverIO + tauri-driver scenarios
  wdio.conf.ts      WebdriverIO config
  package.json      E2E dev-dependencies and scripts
  Cargo.toml        kanban-app crate manifest
```

The crate is a workspace member — all Rust commands run from the workspace
root; all UI commands run from `kanban-app/ui/`; all E2E commands run from
`kanban-app/`.

## Running the E2E spatial-nav scenario

The canonical scenario under `e2e/spatial-nav.e2e.ts` boots a real debug
binary against a deterministic 3x3 fixture and walks the whole spatial-nav
loop: cold boot → bulk registration → click focus → cardinal nav → inspector
open → in-layer nav → Escape → focus restore. See the file header for the
assertion contract.

### One-time setup

1. Install `tauri-driver` (a cargo-managed binary that speaks WebDriver and
   launches the Tauri binary for each scenario):

   ```sh
   cargo install tauri-driver --locked
   ```

2. Install E2E JavaScript dependencies:

   ```sh
   cd kanban-app
   npm install
   ```

### Per-run

```sh
cd kanban-app

# Build the React bundle (the Tauri binary reads ./ui/dist/).
(cd ui && npm install && npm run build)

# Build the debug Rust binary. The fixture subcommand and `__spatial_dump`
# are both gated behind `#[cfg(debug_assertions)]`, so release builds will
# not work for E2E.
cargo build -p kanban-app

# Run the scenario.
npm run e2e
```

Or all three at once:

```sh
npm run e2e:all
```

### Platform notes

- **macOS**: needs a logged-in graphical session. `WebKitWebDriver` ships
  with the OS. Running via SSH without `launchctl bootstrap` into a user
  session will fail to open a window.
- **Linux**: `WebKitWebDriver` needs an X server. On headless hosts:
  ```sh
  xvfb-run --auto-servernum npm run e2e
  ```
- **Windows**: not validated. `tauri-driver` uses Microsoft Edge WebDriver;
  paths may need adjustment.

### Overriding the binary path

By default the harness looks for the binary at
`../target/debug/kanban-app` relative to `kanban-app/`. Set
`KANBAN_APP_BIN=/absolute/path/to/kanban-app` to point at a different
location (e.g. a custom cargo target directory or a CI artifact).

## Hidden debug-only subcommands

These are compiled into debug builds only:

- `kanban-app fixture-3x3 <path>` — write a deterministic 3x3 board fixture
  to `<path>/.kanban/`. Emits a single JSON line `{path, tasks}` describing
  the result. Used by `e2e/setup.ts` so the fixture format stays owned by
  Rust even though the harness runs from Node.

- `kanban-app --only <board-path>` — hermetic launch mode: skips session
  restore and auto-open, opens exactly the given board. See `src/cli.rs`
  for the full contract.

Release builds omit both (`fixture-3x3` via `#[cfg(debug_assertions)]` on
the `Command` enum variant; `--only` stays in release builds but is never
exercised without the other test-support scaffolding).

## Related

- `src/test_support.rs` — in-process fixture factory. `build_fixture`
  and `BoardFixture` are also the source of truth for in-process Rust
  integration tests.
- `src/spatial.rs` — `__spatial_dump` debug command used by the E2E
  scenario for Rust-side assertions.
