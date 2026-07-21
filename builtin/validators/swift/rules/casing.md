---
name: casing
description: UpperCamelCase types, lowerCamelCase members, uniform acronym casing, no Hungarian/k-prefix
---

# Swift Casing

- **Types and protocols are `UpperCamelCase`; everything else is `lowerCamelCase`** — properties, variables, functions, parameters, enum cases, and constants (including `static let`). DON'T: `struct graphicsContext`, `let MaxRetries`. DO: `struct GraphicsContext`, `let maxRetries`.
- **Enum cases are `lowerCamelCase`.** DON'T: `case NotFound`, `case JSON_error`. DO: `case notFound`, `case jsonError`.
- **Acronym spelling is flexible — the uniform form (`URL`, `ID`, `JSON`, `HTTP`) and the capitalized-word form (`Url`, `Id`, `Json`, `Http`) are BOTH accepted — never flag one toward the other.** `entryID` and `entryId`, `baseURL` and `baseUrl`, `schemaJSON` and `schemaJson` are all valid; so are the leading lower forms `id`/`idToken`/`urlString`. Do not raise a finding whose only substance is converting between the two spellings of the same acronym, in either direction, on any declaration — new or pre-existing, public or private. Such a rename is always churn: it touches every call site, widens fork-merge surface, and changes no behavior. A finding that proposes one is a validator error.
  - Position rules still hold: an acronym leading a `lowerCamelCase` name is down-cased as a unit (`urlSession`, `idToken` — never `URLSession` as a property name).
  - Within a SINGLE declaration's own name, don't mix spellings of the same term (`tokenIdToEntryIDMap` is flaggable — pick one spelling inside one name). Consistency across different declarations, files, or with surrounding code is NOT required and NOT flaggable.
  - A term whose established spelling is mixed-case as a unit (`LoRA`, `OAuth`, `GraphQL`, `gRPC`, `IPv6`, `macOS`) is NOT flattened by this rule: it keeps its canonical spelling wherever the up-cased form would appear, and is down-cased as one unit only when it leads a `lowerCamelCase` name. DON'T: `LORALayer`, `saveLORAWeights`, `LoraLossFunction`, `GRAPHQLQuery`. DO: `LoRALayer`, `saveLoRAWeights`, `LoRALossFunction`, `loraRank`, `GraphQLQuery`. Never flag the canonical mixed-case form toward an all-caps flattening; renaming between `LoRA`-style and `LORA`-style across review rounds is always a validator error, and the canonical spelling is the tiebreaker.
  - A term that is NOT commonly all-caps in English is an ordinary word: `radarDetector`, `scubaDiving` — not `RADARDetector`.
- **No `SCREAMING_SNAKE_CASE` and no `k`-prefixed constants.** Swift has neither convention. DON'T: `MAX_RETRY_COUNT`, `kMaximumRetries`. DO: `maximumRetryCount`.
- **No Hungarian notation or type-encoding affixes.** DON'T: `strName`, `bIsValid`, `intCount`, `m_count`, or Objective-C-style class prefixes (`NSFoo`, `MYView`) on new Swift types. Swift namespaces by module, so type prefixes are non-idiomatic. (A deliberate leading underscore on a `@usableFromInline`/underscored internal is a separate, sanctioned convention — not Hungarian notation.)
