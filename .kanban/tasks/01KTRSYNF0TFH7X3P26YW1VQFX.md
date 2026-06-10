---
assignees:
- claude-code
position_column: todo
position_ordinal: e280
title: Stale focus-kernel layers linger after webview reload / window destroy
---
Discovered while verifying 01KTCQFJAR5EJSKMTRD95AX1M8 (HMR command re-mount).

When a webview fully reloads (Vite full page reload in dev) or a window is destroyed, the old page's React effect cleanups never run, so `spatial_pop_layer` is never called for layers that were mounted at that moment. The window-root layer self-heals because the new page re-pushes the same FQM (`SpatialRegistry::push_layer` is an idempotent keyed insert), but OVERLAY layers that were open at reload time (e.g. `/<label>/window/inspector`, `/<label>/window/palette`) linger forever in the kernel's `SpatialRegistry.layers` map.

On window destroy there is no cleanup at all: `on_window_destroyed` in apps/kanban-app/src/main.rs only rebuilds the menu; the focus crate has no remove-layers-for-window op (registry.rs only has push_layer / remove_layer by fq).

Potential impact: stale child layers show up in `children_of_layer` and any topmost-layer / dismiss enumeration over the registry, which could misroute Escape/dismiss after a dev reload, and leak entries when window labels are reused. Low severity — dev-leaning — but the registry should be reconciled.

Suggested fix shape: on `WindowEvent::Destroyed` (and/or webview page-load, e.g. the idempotent `mcp_subscribe` bind for a (label, board) pair) remove all layers whose `window_label` matches the window, before the new page pushes fresh ones. Needs a focus-crate op like `remove layers for window` plus a host call site, TDD with a kernel-level test that a re-bound window starts with only the layers the new page pushed.