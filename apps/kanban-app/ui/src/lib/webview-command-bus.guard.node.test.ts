/**
 * Architectural guard for the webview command handler bus.
 *
 * The bus (`webview-command-bus.ts`) routes presentation-only command
 * behaviors. Its handler invariant (see that module's doc comment) is that a
 * handler must be PURE PRESENTATION: it may touch live webview state, but any
 * durable domain effect must route BACK through `useDispatchCommand` to a
 * plugin command that owns a backend op — never inline. The only way the
 * frontend mutates durable state is the MCP transport (`@/lib/mcp-transport`,
 * `callCommandTool`), so the structural smell is a handler-registration site
 * that imports the transport directly: that file is doing command logic in
 * React instead of dispatching to Rust.
 *
 * This guard fails when any file that calls `registerWebviewCommandHandler`
 * also imports the MCP transport. It is intentionally in place BEFORE the
 * cards that populate the bus (C–F) so the first violation introduced fails
 * immediately rather than being caught only in review.
 *
 * To avoid false comfort while the registration-site set is still small, the
 * detector is unit-proven against known-good and known-bad source below — the
 * scan is only trustworthy because the detector it relies on is itself tested.
 */
import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { join, basename } from "node:path";

// ---------------------------------------------------------------------------
// Pure detectors — unit-proven below so the directory scan is trustworthy.
// ---------------------------------------------------------------------------

/** Whether `source` imports (or requires) the MCP transport module. */
export function importsMcpTransport(source: string): boolean {
  // Matches `from "@/lib/mcp-transport"`, relative forms like
  // `from "./mcp-transport"`, and `require("…/mcp-transport")`.
  return /(?:from\s+|require\(\s*)["'][^"']*mcp-transport["']/.test(source);
}

/** Whether `source` *calls* `registerWebviewCommandHandler` (a registration site).
 *
 * Excludes the bus module's own `export function registerWebviewCommandHandler`
 * declaration so the mechanism file is not mistaken for a consumer. */
export function registersWebviewHandler(source: string): boolean {
  return /(?<!function\s)registerWebviewCommandHandler\s*\(/.test(
    source.replace(/export\s+function\s+registerWebviewCommandHandler/g, ""),
  );
}

// Vitest runs with cwd = the ui project root (where vite.config.ts lives).
const SRC_ROOT = join(process.cwd(), "src");
const BUS_FILE = join(SRC_ROOT, "lib", "webview-command-bus.ts");

/** Recursively collect non-test `.ts`/`.tsx` source files under `dir`. */
function collectSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "test") continue;
      out.push(...collectSourceFiles(full));
      continue;
    }
    if (!/\.tsx?$/.test(entry.name)) continue;
    if (/\.test\.tsx?$/.test(entry.name)) continue;
    out.push(full);
  }
  return out;
}

describe("webview-command-bus presentation invariant", () => {
  it("detects an mcp-transport import (detector is sound)", () => {
    expect(
      importsMcpTransport(
        'import { callCommandTool } from "@/lib/mcp-transport";',
      ),
    ).toBe(true);
    expect(
      importsMcpTransport('const t = require("../lib/mcp-transport");'),
    ).toBe(true);
  });

  it("does not flag unrelated imports (detector has no false positives)", () => {
    expect(
      importsMcpTransport(
        'import { useDispatchCommand } from "./command-scope";',
      ),
    ).toBe(false);
    // A comment mentioning the transport must not trip the import detector.
    expect(importsMcpTransport("// never import mcp-transport here")).toBe(
      false,
    );
  });

  it("recognizes a registration site but not the bus declaration itself", () => {
    expect(
      registersWebviewHandler('registerWebviewCommandHandler("grid.edit", h);'),
    ).toBe(true);
    expect(
      registersWebviewHandler(
        "export function registerWebviewCommandHandler(id, handler) {}",
      ),
    ).toBe(false);
  });

  it("no handler-registration site imports the MCP transport directly", () => {
    const offenders = collectSourceFiles(SRC_ROOT)
      .filter((f) => basename(f) !== "webview-command-bus.ts")
      .filter((f) => {
        const src = readFileSync(f, "utf8");
        return registersWebviewHandler(src) && importsMcpTransport(src);
      });

    // A registration site importing the transport is doing durable command
    // logic in React. Route the durable effect through `useDispatchCommand` to
    // a backend-op plugin command instead. See `webview-command-bus.ts`.
    expect(offenders).toEqual([]);
  });

  it("the bus module stays transport-free", () => {
    const busSrc = readFileSync(BUS_FILE, "utf8");
    expect(importsMcpTransport(busSrc)).toBe(false);
  });
});
