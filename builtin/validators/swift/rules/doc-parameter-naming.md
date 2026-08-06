---
name: doc-parameter-naming
description: "- Parameter entries use the internal (local) parameter name — the name DocC resolves; never flag toward the external argument label"
---

# Swift Doc-Comment Parameter Naming

- **`- Parameter` / `- Parameters:` entries name the internal (local) parameter, never the external argument label.** This is the name Swift-DocC and Xcode resolve documentation against: Xcode's doc-stub generation emits internal names, and the "parameter not found in function declaration" diagnostic validates against them. Apple's stdlib and swift-nio document this way. For `func move(from start: Point, to end: Point)` — DON'T: `- Parameter from:` / `- Parameter to:`. DO: `- Parameter start:` / `- Parameter end:`.
  - When a parameter has no separate label (`func cap(_ text: String)` or `func cap(text: String)`), the internal name is the only documentable name: `- Parameter text:`.
  - The prose of the doc comment may freely use the external label to read fluently ("Caps the output *to* the token limit") — only the `- Parameter <name>:` key itself is bound to the internal name.
- **The direction is fixed — never flag internal names toward external labels.** A finding that asks to change a correct `- Parameter <internalName>:` entry to the external argument label is a validator error, in any file, on any declaration. Renaming doc-parameter keys back and forth between the two forms across review rounds is always churn: it changes no behavior and breaks DocC resolution. When a documented name matches NEITHER the internal name nor the external label (a stale name after a signature change), flag it — toward the internal name.
- **DocC symbol links follow the declaration, not this rule.** A cross-reference like ``` ``capped(text:)`` ``` uses the function's external argument labels because that is the symbol's name; do not "fix" symbol links to internal names, and do not cite them as violations of this rule.
