# collide-probe-a

The first half of the **server-name collision** example pair. This bundle is
the *winning* side: it registers an in-process `{ rust }` source under the
shared name `"collide-probe"` and confirms the registration with a single
`echo` round-trip. Its sibling, [`collide-probe-b`](../collide-probe-b/),
attempts the colliding second registration of the same name.

## What this example demonstrates

The MCP server registry has a **single global namespace** and a
**first-registration-wins** policy: a registered name is held by exactly one
server at a time, and a later `register` of an already-taken name fails with
the platform's `ServerNameTaken` error. There is no override semantics.

This bundle plays the role of the first registrant so an integration test can
prove that:

1. its registration succeeds,
2. a second registrant for the same name fails cleanly without disturbing it,
   and
3. its registered server remains live and callable across the collision.

## Why two distinct `{ rust }` ids back one registered name

An in-process `{ rust }` source is *single-activation*: the host moves the
module out of its available-modules table on the first activation, so a
second `{ rust: "<same-id>" }` resolves to `UnknownServer` rather than
reaching the registry's name-uniqueness check. To genuinely observe
`ServerNameTaken` from a plugin's perspective, each `collide-probe-*` bundle
activates its **own** distinct `{ rust }` module — bundle A uses
`collide-probe-a-mod`, bundle B uses `collide-probe-b-mod` — but both
register under the **same name** (`"collide-probe"`). The collision the test
exercises is on the registered name.

The end-to-end test exposes both `{ rust }` modules through the shared test
support harness before loading either bundle.

## Test

This bundle is exercised by
[`tests/server_name_collision_e2e.rs`](../../../tests/server_name_collision_e2e.rs).
