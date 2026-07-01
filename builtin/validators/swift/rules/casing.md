---
name: casing
description: UpperCamelCase types, lowerCamelCase members, uniform acronym casing, no Hungarian/k-prefix
---

# Swift Casing

- **Types and protocols are `UpperCamelCase`; everything else is `lowerCamelCase`** — properties, variables, functions, parameters, enum cases, and constants (including `static let`). DON'T: `struct graphicsContext`, `let MaxRetries`. DO: `struct GraphicsContext`, `let maxRetries`.
- **Enum cases are `lowerCamelCase`.** DON'T: `case NotFound`, `case JSON_error`. DO: `case notFound`, `case jsonError`.
- **Acronyms and initialisms are cased uniformly — all-upper or all-lower as one unit — per the position's convention.** A common acronym is never mixed-case (`Url`, `Json`, `Http`, `deviceId` are all wrong). Down-case it when it leads a `lowerCamelCase` name; up-case it when interior or leading an `UpperCamelCase` name.
  - DO: `utf8Bytes`, `parseURL`, `deviceID`, `userID`, `totalRAM`, `htmlBody`, `URLSession`, `JSONDecoder`, `SecureSMTPServer`.
  - DON'T: `parseUrl`, `deviceId`, `userId`, `totalRam`, `UrlSession`, `JsonDecoder`, `SecureSmtpServer`.
  - A term that is NOT commonly all-caps in English is an ordinary word: `radarDetector`, `scubaDiving` — not `RADARDetector`.
- **No `SCREAMING_SNAKE_CASE` and no `k`-prefixed constants.** Swift has neither convention. DON'T: `MAX_RETRY_COUNT`, `kMaximumRetries`. DO: `maximumRetryCount`.
- **No Hungarian notation or type-encoding affixes.** DON'T: `strName`, `bIsValid`, `intCount`, `m_count`, or Objective-C-style class prefixes (`NSFoo`, `MYView`) on new Swift types. Swift namespaces by module, so type prefixes are non-idiomatic. (A deliberate leading underscore on a `@usableFromInline`/underscored internal is a separate, sanctioned convention — not Hungarian notation.)
