/**
 * Frontend wrapper for the focus server's `generate sneak_codes` op.
 *
 * Spatial-nav vocabulary in this workspace is Rust-authoritative — the
 * sneak code algorithm itself lives in `swissarmyhammer-focus` so any
 * future consumer (mirdan-app, CLI front-ends) gets the same prefix-free
 * codes via dep rather than copy-pasting TypeScript. This module is
 * intentionally a thin re-export of the MCP wrapper, not a
 * re-implementation.
 *
 * The Jump-To overlay calls {@link generateSneakCodes} once on open and
 * caches the result for the lifetime of the overlay.
 */

export { generateSneakCodes } from "@/lib/focus-mcp";
