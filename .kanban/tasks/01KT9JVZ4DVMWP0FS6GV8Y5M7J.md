---
assignees:
- claude-code
depends_on:
- 01KT9JTDE3EX2BQNQ4F3HMZYTP
- 01KT9JV0N1RF0MMEBRZV4A6F3J
- 01KT9FYTVE2CMAGZQW29G1M6Q6
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff180
project: plugin-arch
title: 'Codegen: emit typed .on(event, cb) overloads in the .d.ts from notifications _meta'
---
Make the declared notification vocabulary show up as TYPED, autocompleted `.on()` in the generated `.d.ts` so plugin authors see exactly which events each server emits, by string name, with a typed callback param. This is the payoff of declaring notifications in `_meta`.

The TypesEmitter (`crates/swissarmyhammer-plugin/src/codegen.rs`, `TypesEmitter` at :191) already generates the per-server namespace from the operations `_meta`:
- `render_server_namespace` (:538) branches on `operations_meta(tool)` (:559, reads `_meta["io.swissarmyhammer/operations"]`) → `render_operation_tool` (:600) → `render_operation_verb` (:626) emits `<noun>.<verb>(input): Promise<unknown>`; `ts_object_from_parameters` (:652) builds the input type; `ts_key` (:780) quotes non-ident keys.

## Scope (codegen.rs)
- Add a `notifications_meta(tool)` reader (sibling of `operations_meta` :559) for `NOTIFICATIONS_META_KEY`.
- In `render_server_namespace`, when a tool declares notifications, emit a set of `on(event, cb)` overloads on the server namespace — one per declared event — typed as `on(event: "<event>", cb: (params: <typed-from-parameters>) => void): () => void` (the `() => void` is the off handle). Reuse `ts_object_from_parameters` / `ts_type_from_json_type` (:734) for the params type.
- A server with no notifications emits no `on` overloads (unchanged output).

## Tests
- Dev-mode emitter test: a server declaring a notification produces a `.d.ts` containing the typed `on(event: "...", cb: (params: {...}) => void): () => void` overload (extend the existing codegen/emitter tests).

## Acceptance
The generated `.d.ts` gives `this.<server>.on("<event>", cb)` autocomplete + a typed `params`, derived from the service's `#[notification]` declarations.