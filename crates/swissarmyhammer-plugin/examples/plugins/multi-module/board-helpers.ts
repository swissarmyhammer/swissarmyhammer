// board-helpers.ts — the SIBLING MODULE of the multi-module example.
//
// This file is the whole point of the `multi-module` example: it is a second
// source file in the same plugin bundle, imported by `entry.ts` with the
// RELATIVE specifier `./board-helpers.ts`. The sandboxed module loader resolves
// that specifier against the bundle directory, reads this file from disk,
// transpiles it, and links it into the plugin's V8 isolate — exactly as it
// does the entry module.
//
// A plugin bundle is not limited to a single `entry.ts`. Split helpers, types,
// and shared logic across as many sibling files as you like; `entry.ts` (or
// any module it imports) pulls them in with ordinary relative imports. The one
// hard rule the loader enforces: a relative import may not escape the bundle
// directory — `../outside.ts` is rejected, so the bundle stays a sandbox.
//
// This module exports two helpers, deliberately of different kinds:
//
//   • `normalizeTaskTitle` — a PURE function (no I/O, no SDK use); and
//   • `addBoardTask` — an ASYNC function that drives a server dispatcher.
//
// It imports nothing of its own beyond a shared SDK *type* (`ServerDispatcher`,
// erased at transpile time) — a sibling module need not re-import the SDK to be
// useful.

import type { ServerDispatcher } from "@swissarmyhammer/plugin";

/**
 * Normalizes a raw task title into the form the board should store.
 *
 * A pure helper — no I/O, no SDK calls. It trims surrounding whitespace and
 * collapses every internal run of whitespace to a single space, so a title
 * typed with stray spacing lands on the board tidy. Demonstrates that a
 * sibling module can carry plain, testable logic the entry module reuses.
 *
 * @param raw - the unnormalized title text, possibly padded or with internal
 *   whitespace runs.
 * @returns the trimmed, single-spaced title.
 */
export function normalizeTaskTitle(raw: string): string {
  return raw.trim().replace(/\s+/g, " ");
}

/**
 * Adds one normalized, tagged task to a kanban board through a dispatcher.
 *
 * An async helper that takes a `board` server dispatcher — the same
 * `this.board` index `entry.ts` builds by registering the host-exposed
 * `kanban` tool — and uses it to add a task. It first normalizes `rawTitle`
 * with {@link normalizeTaskTitle}, then dispatches the `kanban` tool's
 * `add task` operation through the SDK's *path form*: `board.kanban.task.add`.
 *
 * The `kanban` tool declares its add-a-task operation under noun `task`, verb
 * `add` (op `"add task"`); the path segments mirror that `_meta` exactly. The
 * `description` argument carries an inline `#tag` — the `kanban` tool extracts
 * `#tag` patterns from a task description — so the produced task is tagged.
 *
 * @param board - the `board` server dispatcher for the host-exposed `kanban`
 *   operation tool.
 * @param rawTitle - the unnormalized task title; normalized before use.
 * @param tag - the bare tag name (no leading `#`) to apply to the task.
 * @returns the dispatcher's `add task` result (a `CallToolResult` shape).
 */
export async function addBoardTask(
  board: ServerDispatcher,
  rawTitle: string,
  tag: string,
): Promise<unknown> {
  const title = normalizeTaskTitle(rawTitle);
  return board.kanban.task.add({
    title,
    description: `Added by the multi-module example's board-helpers module #${tag}`,
  });
}
