/**
 * E2E fixture helpers.
 *
 * The canonical spatial-nav scenario needs a deterministic 3x3 board on disk
 * before the Tauri binary boots. Reproducing the kanban storage format in
 * TypeScript would duplicate format knowledge that already lives in Rust, so
 * instead we shell out to the debug `kanban-app` binary's hidden
 * `fixture-3x3` subcommand. That subcommand is a thin wrapper over the
 * `build_fixture` helper in `src/test_support.rs` — the same function the
 * in-process Rust tests use — so the fixture the driver sees is
 * byte-identical to what the Rust test suite sees.
 *
 * Release binaries do not include the subcommand (it is gated behind
 * `#[cfg(debug_assertions)]`), so this helper will fail loudly if pointed
 * at a release build.
 */

import { spawnSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Directory of the current module — ESM-safe replacement for the CommonJS
 * `__dirname` global.
 *
 * `tsconfig.json` sets `"module": "ESNext"` with `"moduleResolution":
 * "Bundler"`, which means this file is loaded as an ES module at runtime
 * (via WDIO's ts-node/tsx loader). In that mode `__dirname` is not
 * available as a real binding — `@types/node` declares it globally so
 * typecheck passes, but actually referencing it throws
 * `ReferenceError: __dirname is not defined`.
 *
 * `fileURLToPath(import.meta.url)` works in both ESM and CJS loader modes
 * and pins the resolution to this file's on-disk location.
 */
const THIS_DIR = dirname(fileURLToPath(import.meta.url));

/**
 * Handle to a 3x3 fixture on disk. Mirrors the shape of Rust's
 * `BoardFixture` so the test file reads symmetrically with the
 * `test_support.rs` unit tests.
 */
export interface BoardFixture {
  /** Absolute path to the board root (the directory containing `.kanban/`). */
  path: string;
  /**
   * Task identifiers in row-major layout order (`task-<col>-<row>`).
   *
   * For a 3x3 board: `["task-1-1", "task-1-2", "task-1-3",
   * "task-2-1", ..., "task-3-3"]`.
   */
  tasks: string[];
}

/**
 * Build a deterministic 3x3 board fixture by invoking the debug kanban-app
 * binary's `fixture-3x3` subcommand.
 *
 * Creates a fresh tmpdir under the OS temp root, runs the binary, and
 * returns the manifest it prints. The tmpdir is **persisted by design** —
 * neither this helper nor the wdio harness (`wdio.conf.ts`) deletes it
 * when the session ends. Flake investigation often needs the last
 * fixture's contents to still be on disk, and the OS temp root is
 * cleaned by the system on a cadence that makes per-run leakage
 * immaterial.
 *
 * @param binary Absolute path to the debug `kanban-app` binary.
 * @returns A {@link BoardFixture} describing the freshly-written board.
 * @throws Error if the binary exits non-zero or prints invalid JSON.
 */
export function writeFixture3x3(binary: string): BoardFixture {
  const root = mkdtempSync(join(tmpdir(), "kanban-e2e-"));
  const target = join(root, "board");
  const result = spawnSync(binary, ["fixture-3x3", target], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    throw new Error(
      `fixture-3x3 exited with status ${result.status}: ${result.stderr}`,
    );
  }
  const manifest = JSON.parse(result.stdout.trim()) as BoardFixture;
  if (!manifest.path || !Array.isArray(manifest.tasks)) {
    throw new Error(`fixture-3x3 returned malformed manifest: ${result.stdout}`);
  }
  return manifest;
}

/**
 * Resolve the absolute path to the debug `kanban-app` binary from the
 * workspace target dir. Used by `wdio.conf.ts` to build the Tauri launch
 * command and by tests that need to rebuild fixtures between scenarios.
 *
 * Honours `$KANBAN_APP_BIN` when set — useful in CI or when running against
 * a custom target directory.
 */
export function resolveBinary(): string {
  const override = process.env.KANBAN_APP_BIN;
  if (override) return override;
  // This file lives at `kanban-app/e2e/setup.ts`, and cargo's default
  // target layout for a workspace member is `target/debug/<bin>` at the
  // workspace root. Climb two directories from `kanban-app/e2e/` to reach
  // the workspace root.
  const workspaceRoot = join(THIS_DIR, "..", "..");
  return join(workspaceRoot, "target", "debug", "kanban-app");
}
