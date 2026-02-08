---
name: no-stringly-typed
description: Use enums and newtypes instead of raw strings for domain concepts
---

# No Stringly-Typed Code

This rule is about **type choice**: when a value represents a domain concept
with known structure or a finite set of possibilities, use an enum or newtype
instead of `String`. Strings are for text that humans read. Everything else
gets a proper type.

Note: this rule is about **what type you pick**. For **how you convert** raw
input into that type (TryFrom, FromStr, constructors), see `parse-dont-validate`.

## What to Check

Look for `String` or `&str` being used where a more specific type would
prevent bugs:

1. **Enum-shaped strings**: A string field or parameter that can only take a
   known set of values (e.g., "error", "warn", "info"). Should be an enum.

2. **Structured string identifiers**: IDs, slugs, paths, URLs, emails stored
   as plain `String`. Should be a newtype like `struct UserId(String)`.

3. **String matching for dispatch**: `if action == "delete"` or
   `match status.as_str() { "active" => ..., "inactive" => ... }`. Should
   match on an enum.

4. **Stringly-typed configuration**: Config structs with `String` fields
   where the possible values are a closed set.

5. **String constants used for comparison**: `const STATUS_ACTIVE: &str = "active"`
   paired with string comparisons. Should be an enum variant.

## What Passes

- `enum LogLevel { Error, Warn, Info, Debug, Trace }`
- `struct UserId(Ulid)` or `struct Slug(String)` newtypes
- `String` fields for genuinely free-form text: names, descriptions, comments
- `String` for user-supplied input that hasn't been validated yet (pre-parse)
- Serialization boundaries where serde converts enums to/from strings
- Display/logging where structured types are formatted as strings

## What Fails

- `fn set_level(level: &str)` where level is one of a known set
- `struct Config { mode: String }` where mode is always "dev", "staging", or "prod"
- `if event_type == "click" { ... } else if event_type == "hover" { ... }`
- `HashMap<String, Value>` where the keys are a known finite set (should be a struct)
- `fn route(method: String, path: String)` where method is always an HTTP method

## Why This Matters

Typos in strings compile. Typos in enum variants don't. dtolnay's serde
processes billions of data items; string typos at that scale would be
catastrophic. The type system is there to help -- use it.
