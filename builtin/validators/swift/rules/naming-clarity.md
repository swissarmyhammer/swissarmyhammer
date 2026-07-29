---
name: naming-clarity
description: Clarity over brevity, no needless words, name by role, boolean assertions, protocol naming
---

# Swift Naming Clarity

- **Choose clarity over brevity.** Do not abbreviate to save characters — clarity is the goal, small code is not. DON'T: `cnt`, `idx`, `usr`, `mgr`. DO: `count`, `index`, `user`, `manager`.
- **Omit needless words.** Every word must carry important information at the use site. DON'T: `allViews.removeElement(button)`, `Color.colorRed`, `user.userName`. DO: `allViews.remove(button)`, `Color.red`, `user.name`.
- **Name by role, not type.** DON'T: `var string = greeting`, `associatedtype NodeType`. DO: `var greeting`, `associatedtype Node`.
- **Compensate for weak type information.** Put a noun that describes the role before a weakly typed parameter (`Any`, `AnyObject`, `NSObject`, `Int`, `String`). DON'T: `func add(_ mid: NSObject, to path: String)`. DO: `func addObserver(_ observer: NSObject, forKeyPath path: String)`.
- **Write a non-mutating Boolean member as an assertion about the receiver.** DO: `isEmpty`, `isEnabled`, `hasPrefix(_:)`, `line1.intersects(line2)`. DON'T: bare adjectives (`empty`, `enabled`) or `getIsEmpty()`.
- **Name a protocol for a capability with an ending in `-able`/`-ible`/`-ing`. Name a protocol that describes what something *is* with a noun.** DO: `Equatable`, `ProgressReporting`, `Collection`. DON'T: `Equality` for a capability.
- **Do not use a `Protocol`/`Type` suffix as a crutch.** Use it only to break a name clash you cannot avoid another way (as the stdlib does with `IteratorProtocol`). DON'T: `FooProtocol` merely to signal "this is a protocol".
