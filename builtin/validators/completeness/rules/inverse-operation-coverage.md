---
name: inverse-operation-coverage
description: A change to one direction of a paired operation must exercise the inverse direction
---

# Inverse-Operation Coverage Validator

You are a completeness validator. Many operations come in inverse pairs:
**write/read, serialize/deserialize, encode/decode, marshal/unmarshal,
compress/decompress, classify/parse, format/scan, to_X/from_X, push/pop,
set/get, open/close, lock/unlock**. When a change teaches one side of such a
pair a new capability, the other side almost always needs the matching change —
and is the part that gets forgotten.

## What to Check

1. **One-sided capability change.** The diff adds or changes behaviour on the
   producing side (e.g. a writer/serializer/encoder gains an option) but neither
   the consuming side (reader/parser/decoder) nor a test exercises the inverse
   of that same capability. Ask: "if you can now *write* this, can you *read it
   back*? Is that proven anywhere in the diff?"

2. **A test that lies about its scope.** A test whose name or docstring contains
   `round.?trip`, `roundtrip`, `symmetry`, `inverse`, `both directions`, or
   `read.?back` but whose body only calls ONE direction's API (e.g. it only
   `write(...)`s and asserts the output string, never `read(...)`s it back). The
   label promises a round trip the body never makes.

3. **New input variant not round-tripped.** The change supports a new input shape
   (a new header row, a new field, a new token, lower-case as well as upper) but
   the test only feeds the variant in one direction. The classic miss: writing
   `name`+`unit` header rows works, but reading a table whose header also carries
   a `dtype` row is never tried, so the deserializer consumes that row as data.

## Why This Matters

The producing side passing its own assertions proves nothing about whether the
output can be consumed again. Real users round-trip; hidden/regression tests
round-trip; the author's write-only test does not.

## What to Report

Name the paired operation, point at the direction that changed, and state the
direction (or round-trip test) that is missing. Prefer: "writer learned
`header_rows`; no test reads a table back with those header rows — add a
round-trip read."

## Exceptions (Don't Flag)

- Genuinely one-way operations with no inverse (a hash, a logger, a one-shot
  side effect, a destructive migration with a documented no-reverse).
- The inverse direction is already covered by an existing, unchanged test that
  the diff clearly still exercises.
- The task explicitly scopes to one direction and records why the inverse is out
  of scope.
