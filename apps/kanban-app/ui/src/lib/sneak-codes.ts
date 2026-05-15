/**
 * Frontend wrapper for the `generate_jump_codes` Tauri command.
 *
 * Spatial-nav vocabulary in this workspace is Rust-authoritative — the
 * sneak code algorithm itself lives in `swissarmyhammer-focus` so any
 * future consumer (mirdan-app, CLI front-ends) gets the same prefix-free
 * codes via dep rather than copy-pasting TypeScript. This module is
 * intentionally a thin invoke wrapper, not a re-implementation.
 *
 * The Jump-To overlay calls {@link generateSneakCodes} once on open and
 * caches the result for the lifetime of the overlay.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Generate `count` distinct, prefix-free Jump-To codes via the Rust
 * kernel.
 *
 * Codes are returned in ergonomic priority order — single-letter codes
 * first (home row, then top, then bottom), then two-letter codes for
 * larger target counts. The 23-letter alphabet supports up to 529 codes
 * (`23²`); requesting more rejects with the kernel's error message.
 *
 * @param count - Number of distinct codes to generate. Must be
 *   non-negative and `<= 529`.
 * @returns The generated codes in priority order. Empty array when
 *   `count === 0`.
 */
export async function generateSneakCodes(count: number): Promise<string[]> {
  return await invoke<string[]>("generate_jump_codes", { count });
}
