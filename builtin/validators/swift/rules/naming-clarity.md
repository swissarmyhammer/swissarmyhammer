---
name: naming-clarity
description: Clarity over brevity, no needless words, name by role, boolean assertions, protocol naming
---

# Swift Naming Clarity

- **Clarity over brevity.** Don't abbreviate to save characters — clarity is the goal, small code is not. DON'T: `cnt`, `idx`, `usr`, `mgr`. DO: `count`, `index`, `user`, `manager`.
- **Omit needless words.** Every word must carry salient information at the use site. DON'T: `allViews.removeElement(button)`, `Color.colorRed`, `user.userName`. DO: `allViews.remove(button)`, `Color.red`, `user.name`.
- **Name by role, not type.** DON'T: `var string = greeting`, `associatedtype NodeType`. DO: `var greeting`, `associatedtype Node`.
- **Compensate for weak type information.** Precede a weakly typed parameter (`Any`, `AnyObject`, `NSObject`, `Int`, `String`) with a noun describing its role. DON'T: `func add(_ mid: NSObject, to path: String)`. DO: `func addObserver(_ observer: NSObject, forKeyPath path: String)`.
- **Non-mutating Boolean members read as assertions about the receiver.** DO: `isEmpty`, `isEnabled`, `hasPrefix(_:)`, `line1.intersects(line2)`. DON'T: bare adjectives (`empty`, `enabled`) or `getIsEmpty()`.
- **Protocol naming: capabilities end in `-able`/`-ible`/`-ing`; a protocol describing what something *is* is a noun.** DO: `Equatable`, `ProgressReporting`, `Collection`. DON'T: `Equality` for a capability.
- **No `Protocol`/`Type` suffix as a crutch.** Use it only to break an otherwise unavoidable name clash (as the stdlib does with `IteratorProtocol`). DON'T: `FooProtocol` merely to signal "this is a protocol".
