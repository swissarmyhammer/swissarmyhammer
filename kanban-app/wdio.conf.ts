/**
 * WebdriverIO configuration for the tauri-driver E2E harness.
 *
 * Drives the `kanban-app` Tauri binary via `tauri-driver`, the standard
 * Tauri v2 end-to-end path. `tauri-driver` is a cargo-installed binary
 * that speaks the WebDriver protocol on localhost:4444 and launches the
 * binary pointed at by `tauri:options.application`.
 *
 * Test lifecycle per scenario:
 *   1. `onPrepare` launches `tauri-driver` as a child process on :4444.
 *   2. `beforeSession` writes a fresh 3x3 fixture via the debug binary's
 *      `fixture-3x3` subcommand and forwards its path to the launched
 *      binary as `--only <path>`.
 *   3. The scenario runs; the webview is reachable via `browser.*`.
 *   4. `onComplete` kills `tauri-driver`.
 *
 * Running:
 *   - Install tauri-driver once: `cargo install tauri-driver`
 *   - Build the binary debug: `cargo build -p kanban-app`
 *   - Build the UI: `(cd ui && npm install && npm run build)`
 *   - Run the suite: `npm run e2e`
 *
 * Platform notes:
 *   - macOS: needs a logged-in session (WebKitWebDriver ships with the OS).
 *   - Linux: use `xvfb-run npm run e2e` on headless hosts so WebKitWebDriver
 *     can reach an X server.
 *   - Windows: tauri-driver launches Microsoft Edge WebDriver; this config
 *     has not been validated on Windows.
 */

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { resolveBinary, writeFixture3x3, type BoardFixture } from "./e2e/setup.js";

let tauriDriver: ChildProcess | undefined;

// Populated per-scenario in `beforeSession` and consumed by the scenario
// through the `global` object — WebdriverIO's recommended handoff path for
// config-scoped data that worker-side code needs to see.
declare global {
  var __fixture: BoardFixture | undefined;
}

const binary = resolveBinary();

export const config: WebdriverIO.Config = {
  runner: "local",
  tsConfigPath: "./tsconfig.json",

  specs: ["./e2e/**/*.e2e.ts"],
  maxInstances: 1,

  // `tauri:options.application` is the binary path that `tauri-driver`
  // will exec. `args` are passed through to the binary's clap parser.
  // We fill `--only <fixture>` in `beforeSession` once the fixture has
  // been written.
  capabilities: [
    {
      browserName: "wry",
      "tauri:options": {
        application: binary,
      },
    } as WebdriverIO.Capabilities,
  ],

  logLevel: "info",
  waitforTimeout: 10_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 3,

  hostname: "127.0.0.1",
  port: 4444,

  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  reporters: ["spec"],

  /**
   * Launch `tauri-driver` before the WebdriverIO session opens. The driver
   * listens on :4444 by default, which matches the `hostname`/`port`
   * configured above.
   */
  onPrepare: function () {
    if (!existsSync(binary)) {
      throw new Error(
        `kanban-app debug binary not found at ${binary}. ` +
          `Run \`cargo build -p kanban-app\` first, or set KANBAN_APP_BIN.`,
      );
    }
    tauriDriver = spawn("tauri-driver", [], {
      stdio: [null, process.stdout, process.stderr],
    });
    tauriDriver.on("error", (err) => {
      console.error("[wdio] tauri-driver failed to launch:", err.message);
      console.error(
        "[wdio] Install it with: cargo install tauri-driver --locked",
      );
    });
  },

  /**
   * Build a fresh 3x3 fixture per scenario and rewrite the capability's
   * args so the binary launches with `--only <fixture-path>`.
   *
   * Running once per scenario instead of once per suite is deliberate:
   * a test that leaves the fixture in a mutated state (e.g. creates a new
   * task) must not leak into the next scenario.
   */
  beforeSession: function (_config, capabilities) {
    const fixture = writeFixture3x3(binary);
    globalThis.__fixture = fixture;
    // Cast through `unknown` because `tauri:options` is a tauri-driver
    // extension not known to `WebdriverIO.Capabilities`. The runtime
    // contract is stable; the type gap is the price of an ecosystem
    // driver that hasn't shipped its own .d.ts.
    const caps = capabilities as unknown as {
      "tauri:options"?: { application?: string; args?: string[] };
    };
    caps["tauri:options"] = {
      ...(caps["tauri:options"] ?? {}),
      application: binary,
      args: ["--only", fixture.path],
    };
  },

  onComplete: function () {
    if (tauriDriver && !tauriDriver.killed) {
      tauriDriver.kill();
    }
  },
};
