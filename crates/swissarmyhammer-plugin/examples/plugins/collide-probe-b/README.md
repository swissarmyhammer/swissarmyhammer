# collide-probe-b

The second half of the **server-name collision** example pair. This bundle is
the *losing* side when loaded after [`collide-probe-a`](../collide-probe-a/):
it tries to register the shared name `"collide-probe"` that bundle A already
claimed, observes the platform reject the attempt with `ServerNameTaken`, and
re-raises that error so its own load fails.

## What this example demonstrates

Two things, together:

1. The MCP server registry's **no-override policy** is enforced at runtime,
   from a plugin author's perspective. The second registrant of a name cannot
   silently displace the first, and it cannot block the first's ongoing
   operation either.
2. The `ServerNameTaken` failure **propagates from the Rust registry across
   the SDK bridge into the V8 isolate as a real JavaScript `Error`** —
   catchable and inspectable. Because `register` is synchronous, the throw is
   synchronous too: there is no promise to await.

The bundle catches the thrown error, logs a brief diagnostic so a passing
test leaves an audit trail, then re-raises to fail the load.

The bundle is also designed to be loaded *fresh* (with no prior claim on the
shared name): when there is nothing for it to collide with the `register`
simply succeeds and the bundle stays live. The end-to-end test exercises that
post-unload path as its fourth assertion.

## Why this bundle activates its own `{ rust }` module

See [`../collide-probe-a/README.md`](../collide-probe-a/README.md) for the
long-form explanation: a `{ rust }` source is single-activation, so two
bundles sharing one `{ rust }` id would hit `UnknownServer` on the second
`register` rather than reaching the name-uniqueness check. Bundle B uses its
own `collide-probe-b-mod` `{ rust }` module behind the shared registered name
`"collide-probe"`.

## Test

This bundle is exercised by
[`tests/server_name_collision_e2e.rs`](../../../tests/server_name_collision_e2e.rs).
