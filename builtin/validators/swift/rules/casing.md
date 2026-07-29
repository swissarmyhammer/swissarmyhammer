---
name: casing
description: UpperCamelCase types, lowerCamelCase members, uniform acronym casing, no Hungarian/k-prefix
---

# Swift Casing

- **Write types and protocols in `UpperCamelCase`.** Write everything else in `lowerCamelCase`. This includes properties, variables, functions, parameters, enum cases, and constants, including `static let`. DON'T: `struct graphicsContext`, `let MaxRetries`. DO: `struct GraphicsContext`, `let maxRetries`.
- **Write enum cases in `lowerCamelCase`.** DON'T: `case NotFound`, `case JSON_error`. DO: `case notFound`, `case jsonError`.
- **Acronym spelling is flexible.** Accept both the uniform form (`URL`, `ID`, `JSON`, `HTTP`) and the capitalized-word form (`Url`, `Id`, `Json`, `Http`). Do not flag one form in favor of the other. `entryID` and `entryId` are both valid. `baseURL` and `baseUrl` are both valid. `schemaJSON` and `schemaJson` are both valid. The leading lower forms `id`, `idToken`, and `urlString` are also valid. Do not raise a finding that only proposes a change between the two spellings of the same acronym. This applies in either direction, on any declaration — new or existing, public or private. This kind of rename is always churn: it touches every call site, it widens the surface for merge conflicts across forks, and it changes no behavior. A finding that proposes this kind of rename is a validator error.
  - Position rules still apply. Down-case an acronym as one unit when it leads a `lowerCamelCase` name (`urlSession`, `idToken` — never `URLSession` as a property name).
  - Do not mix spellings of the same term within one declaration's own name (`tokenIdToEntryIDMap` is flaggable — pick one spelling inside one name). Consistency across different declarations, files, or surrounding code is not required. Do not flag it.
  - Some terms have an established mixed-case spelling as one unit: `LoRA`, `OAuth`, `GraphQL`, `gRPC`, `IPv6`, `macOS`. This rule does not flatten these terms. Each term keeps its canonical spelling wherever the up-cased form would appear. Down-case the term as one unit only when it leads a `lowerCamelCase` name. DON'T: `LORALayer`, `saveLORAWeights`, `LoraLossFunction`, `GRAPHQLQuery`. DO: `LoRALayer`, `saveLoRAWeights`, `LoRALossFunction`, `loraRank`, `GraphQLQuery`. Never flag the canonical mixed-case form toward an all-caps flattening. Renaming between `LoRA`-style and `LORA`-style across review rounds is always a validator error. The canonical spelling is the tiebreaker.
  - A term that is not commonly all-caps in English is an ordinary word: `radarDetector`, `scubaDiving` — not `RADARDetector`.
- **Do not use `SCREAMING_SNAKE_CASE`. Do not use `k`-prefixed constants.** Swift uses neither convention. DON'T: `MAX_RETRY_COUNT`, `kMaximumRetries`. DO: `maximumRetryCount`.
- **Do not use Hungarian notation or type-encoding affixes.** DON'T: `strName`, `bIsValid`, `intCount`, `m_count`, or Objective-C-style class prefixes (`NSFoo`, `MYView`) on new Swift types. Swift namespaces types by module, so type prefixes are not idiomatic. A deliberate leading underscore on a `@usableFromInline`/underscored internal symbol is a separate, sanctioned convention — not Hungarian notation.
