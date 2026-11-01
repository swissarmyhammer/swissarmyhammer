//! Acceptance tests for the Swift JUDGMENT rules — the bullets the `swift`
//! prompt set keeps because no shipped tool decides them.
//!
//! `builtin/validators/swift/VALIDATOR.md` states the standing order: if a tool
//! can decide it, the tool owns it. A prompt bullet a tool ALSO decides is two
//! owners for one requirement, which produces churn on every review round. A
//! prompt bullet no tool decides is the set's own to state.
//!
//! A prompt rule has no tool to run, so a fixture pair cannot measure it the
//! way `builtin/validators/README.md` measures a tool rule. What CAN be
//! measured is the other half of the contract, and this module measures it: the
//! DON'T of each new bullet draws no finding from ANY shipped Swift gate, so
//! the bullet has exactly one owner; and the DO draws none either, so the shape
//! the prompt rule asks the author for does not walk into a tool finding.
//!

use super::*;

/// The prompt rule that decides what `init` does and where a body that
/// branches lives.
const SWIFT_INITIALIZATION_PROMPT_RULE: &str = "initialization";

/// The prompt rule that decides how a scope states its requirements.
const SWIFT_PRECONDITIONS_PROMPT_RULE: &str = "preconditions";

/// The prompt rule that decides how a declaration is named.
const SWIFT_NAMING_CLARITY_PROMPT_RULE: &str = "naming-clarity";

/// The prompt rule that decides the access level of a declaration.
const SWIFT_ACCESS_CONTROL_PROMPT_RULE: &str = "access-control";

/// The prompt rule that decides which classes are `final`.
const SWIFT_VALUE_SEMANTICS_PROMPT_RULE: &str = "value-semantics";

/// Every shipped Swift gate that reads a FILE list.
///
/// The five together are the whole deterministic half for Swift, less
/// `dead-code-swift`, which loads a built package rather than a file and
/// decides whether a declaration is USED — a question no bullet here asks.
const SWIFT_FILE_GATES: &[&str] = &[
    SWIFT_IDIOMS_RULE,
    SWIFT_DISALLOWED_CONSTRUCTS_RULE,
    SWIFT_FUNCTION_LENGTH_RULE,
    SWIFT_MAGIC_NUMBERS_RULE,
    SWIFT_MISSING_DOCS_RULE,
];

/// Where a judgment probe stands inside its probe repository.
const SWIFT_JUDGMENT_PROBE_PATH: &str = "Judgment.swift";

/// One judgment rule, with the Swift that shows the shape it refuses and the
/// Swift that shows the shape it asks for.
struct SwiftJudgmentRule {
    /// The prompt rule, by its name in the `swift` set.
    prompt_rule: &'static str,

    /// Swift written the way the rule states NOT to.
    refusal: &'static str,

    /// The same subjects written the way the rule asks for.
    answer: &'static str,
}

/// The `init` bullets written the way `initialization.md` states NOT to.
///
/// Four subjects, one for each bullet: an `init` that opens a database, a
/// property a later `configure` method fills in, a `didSet` that branches, and
/// a callback closure that branches.
const SWIFT_INITIALIZATION_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "struct Row {}\n\n",
    "enum Database {\n",
    "    static func open(_ path: String) -> Handle { Handle() }\n\n",
    "    struct Handle {\n",
    "        func allRows() -> [Row] { [] }\n",
    "    }\n",
    "}\n\n",
    "struct Session {\n",
    "    init(token: String) {}\n",
    "}\n\n",
    "struct Photo {\n",
    "    let isPlaceholder: Bool\n",
    "}\n\n",
    "final class ReportStore {\n",
    "    private let rows: [Row]\n\n",
    "    init(path: String) { rows = Database.open(path).allRows() }\n",
    "}\n\n",
    "final class Uploader {\n",
    "    private var session: Session?\n\n",
    "    init() {}\n\n",
    "    func configure(token: String) {\n",
    "        session = Session(token: token)\n",
    "    }\n",
    "}\n\n",
    "final class Badge {\n",
    "    static let cap = 99\n",
    "    var shown: [Photo] = []\n\n",
    "    var count: Int = 0 {\n",
    "        didSet {\n",
    "            if count == 0 {\n",
    "                shown = []\n",
    "            } else if count > Badge.cap {\n",
    "                shown = Array(shown.prefix(Badge.cap))\n",
    "            }\n",
    "        }\n",
    "    }\n",
    "}\n\n",
    "final class Gallery {\n",
    "    private var photos: [Photo] = []\n\n",
    "    func reload(loader: (@escaping ([Photo]) -> Void) -> Void) {\n",
    "        loader { loaded in\n",
    "            var kept: [Photo] = []\n",
    "            for photo in loaded where !photo.isPlaceholder {\n",
    "                kept.append(photo)\n",
    "            }\n",
    "            self.photos = kept\n",
    "        }\n",
    "    }\n",
    "}\n",
);

/// The same four subjects written the way `initialization.md` asks for.
const SWIFT_INITIALIZATION_ANSWER: &str = concat!(
    "import Foundation\n\n",
    "struct Row {}\n\n",
    "enum Database {\n",
    "    static func open(_ path: String) -> Handle { Handle() }\n\n",
    "    struct Handle {\n",
    "        func allRows() -> [Row] { [] }\n",
    "    }\n",
    "}\n\n",
    "struct Session {\n",
    "    init(token: String) {}\n",
    "}\n\n",
    "struct Photo {\n",
    "    let isPlaceholder: Bool\n",
    "}\n\n",
    "final class ReportStore {\n",
    "    private let path: String\n",
    "    private var rows: [Row] = []\n\n",
    "    init(path: String) { self.path = path }\n\n",
    "    func load() { rows = Database.open(path).allRows() }\n",
    "}\n\n",
    "final class Uploader {\n",
    "    private let session: Session\n\n",
    "    init(token: String) {\n",
    "        session = Session(token: token)\n",
    "    }\n",
    "}\n\n",
    "final class Badge {\n",
    "    static let cap = 99\n",
    "    var shown: [Photo] = []\n\n",
    "    var count: Int = 0 { didSet { countDidChange() } }\n\n",
    "    private func countDidChange() {\n",
    "        if count == 0 {\n",
    "            shown = []\n",
    "        } else if count > Badge.cap {\n",
    "            shown = Array(shown.prefix(Badge.cap))\n",
    "        }\n",
    "    }\n",
    "}\n\n",
    "final class Gallery {\n",
    "    private var photos: [Photo] = []\n\n",
    "    func reload(loader: (@escaping ([Photo]) -> Void) -> Void) {\n",
    "        loader { loaded in self.photosDidLoad(loaded) }\n",
    "    }\n\n",
    "    private func photosDidLoad(_ loaded: [Photo]) {\n",
    "        photos = loaded.filter { !$0.isPlaceholder }\n",
    "    }\n",
    "}\n",
);

/// The immutability bullets written the way `immutability.md` states NOT to.
///
/// The accumulator bullet names TWO loop shapes, and both stand here.
///
/// `Roster` carries the `where` clause.
///
/// `Directory` carries the nested `if`, which is the shape
/// `UseWhereClausesInForLoops` reads. That rule is OFF in the shipped gate. A
/// gate that turned it on reports this probe, so the row is what would catch
/// that.
const SWIFT_IMMUTABILITY_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "struct User {\n",
    "    let name: String\n",
    "    let isActive: Bool\n",
    "}\n\n",
    "enum Roster {\n",
    "    static var attemptLimit = 3\n\n",
    "    static func activeNames(of users: [User]) -> [String] {\n",
    "        var names: [String] = []\n",
    "        for user in users where user.isActive { names.append(user.name) }\n",
    "        return names\n",
    "    }\n",
    "}\n\n",
    "enum Directory {\n",
    "    static func activeNames(of users: [User]) -> [String] {\n",
    "        var names: [String] = []\n",
    "        for user in users { if user.isActive { names.append(user.name) } }\n",
    "        return names\n",
    "    }\n",
    "}\n\n",
    "func formatDuration(_ seconds: Int) -> String {\n",
    "    \"\\(seconds)s\"\n",
    "}\n",
);

/// The same subjects written the way `immutability.md` asks for.
const SWIFT_IMMUTABILITY_ANSWER: &str = concat!(
    "import Foundation\n\n",
    "struct User {\n",
    "    let name: String\n",
    "    let isActive: Bool\n",
    "}\n\n",
    "enum Roster {\n",
    "    static let attemptLimit = 3\n",
    "    static let backoffSeconds = 2\n",
    "    static var totalBudgetSeconds: Int { attemptLimit * backoffSeconds }\n\n",
    "    static func activeNames(of users: [User]) -> [String] {\n",
    "        users.filter(\\.isActive).map(\\.name)\n",
    "    }\n",
    "}\n\n",
    "enum Directory {\n",
    "    static func activeNames(of users: [User]) -> [String] {\n",
    "        users.compactMap { user in user.isActive ? user.name : nil }\n",
    "    }\n",
    "}\n\n",
    "extension Int {\n",
    "    var formattedDuration: String {\n",
    "        \"\\(self)s\"\n",
    "    }\n",
    "}\n",
);

/// The precondition bullets written the way `preconditions.md` states NOT to.
///
/// The `Settings` lookup answers its surprise with SILENCE rather than with a
/// crash. `error-handling.md` owns `fatalError`/`preconditionFailure` whole, so
/// `preconditions.md` states neither and no probe here holds one.
const SWIFT_PRECONDITIONS_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "protocol Transport {\n",
    "    func write(_ message: String)\n",
    "}\n\n",
    "struct Channel {\n",
    "    let transport: Transport\n",
    "    let isConnected: Bool\n\n",
    "    func send(_ message: String) { if isConnected { if !message.isEmpty ",
    "{ transport.write(message) } } }\n",
    "}\n\n",
    "struct Settings {\n",
    "    let table: [String: String]\n\n",
    "    func value(for key: String) -> String {\n",
    "        guard let found = table[key] else { return \"\" }\n",
    "        return found\n",
    "    }\n",
    "}\n",
);

/// The same subjects written the way `preconditions.md` asks for.
const SWIFT_PRECONDITIONS_ANSWER: &str = concat!(
    "import Foundation\n",
    "import os\n\n",
    "protocol Transport {\n",
    "    func write(_ message: String)\n",
    "}\n\n",
    "struct Channel {\n",
    "    let transport: Transport\n",
    "    let isConnected: Bool\n\n",
    "    func send(_ message: String) {\n",
    "        guard isConnected else { return }\n",
    "        guard !message.isEmpty else { return }\n",
    "        transport.write(message)\n",
    "    }\n",
    "}\n\n",
    "struct Settings {\n",
    "    let table: [String: String]\n",
    "    let logger: Logger\n\n",
    "    func value(for key: String) -> String {\n",
    "        guard let found = table[key] else {\n",
    "            assertionFailure(\"no value for \\(key)\")\n",
    "            logger.error(\"settings key is missing; the empty default stands\")\n",
    "            return \"\"\n",
    "        }\n",
    "        return found\n",
    "    }\n",
    "}\n",
);

/// Event handlers named the way `naming-clarity.md` states NOT to.
const SWIFT_NAMING_CLARITY_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "final class EditorScreen {\n",
    "    func handleSaveTap() {}\n\n",
    "    func onCancelPressed() {}\n",
    "}\n",
);

/// The same handlers named the way `naming-clarity.md` asks for.
///
/// The bullet names THREE forms, and all three stand here. `SettingsScreen`
/// carries the third: the subject stays in the name where two sources send the
/// same event.
const SWIFT_NAMING_CLARITY_ANSWER: &str = concat!(
    "import Foundation\n\n",
    "final class EditorScreen {\n",
    "    func didTapSave() {}\n\n",
    "    func didTapCancel() {}\n",
    "}\n\n",
    "final class SettingsScreen {\n",
    "    func settingsButtonDidTap() {}\n",
    "}\n",
);

/// A SwiftUI view whose access levels are the reverse of what
/// `access-control.md` asks for.
const SWIFT_ACCESS_CONTROL_REFUSAL: &str = concat!(
    "import SwiftUI\n\n",
    "struct BannerView: View {\n",
    "    private let title: String\n",
    "    private let subtitle: String\n",
    "    @State var isExpanded = false\n\n",
    "    var body: some View {\n",
    "        Text(isExpanded ? title : subtitle)\n",
    "    }\n",
    "}\n",
);

/// The same view with the access levels `access-control.md` asks for.
const SWIFT_ACCESS_CONTROL_ANSWER: &str = concat!(
    "import SwiftUI\n\n",
    "struct BannerView: View {\n",
    "    let title: String\n",
    "    let subtitle: String\n",
    "    @State private var isExpanded = false\n\n",
    "    var body: some View {\n",
    "        Text(isExpanded ? title : subtitle)\n",
    "    }\n",
    "}\n",
);

/// A class nothing subclasses, left open.
///
/// The toolchain carries no rule like `preferFinalClasses`, so `value-semantics.md`
/// is the only owner of this requirement and every gate must stay silent here.
const SWIFT_VALUE_SEMANTICS_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "class Cache {\n",
    "    var entries: [String: Int] = [:]\n",
    "}\n",
);

/// The same class marked `final`.
const SWIFT_VALUE_SEMANTICS_ANSWER: &str = concat!(
    "import Foundation\n\n",
    "final class Cache {\n",
    "    var entries: [String: Int] = [:]\n",
    "}\n",
);

/// The idiom bullets `idioms.md` keeps, written the way it states NOT to.
///
/// Three subjects, one for each bullet: a static member that repeats its type
/// name in a shape the tool rule does not read, an empty `Set` written as a
/// call, and a `public init` that repeats the synthesized memberwise
/// initializer on an internal type.
const SWIFT_IDIOMS_REFUSAL: &str = concat!(
    "import Foundation\n\n",
    "struct Color {\n",
    "    static let redColor = 1\n",
    "}\n\n",
    "struct Registry {\n",
    "    var ids = Set<String>()\n",
    "}\n\n",
    "struct Point {\n",
    "    var x: Int\n",
    "    var y: Int\n\n",
    "    /// Creates a point at `x` and `y`.\n",
    "    public init(x: Int, y: Int) {\n",
    "        self.x = x\n",
    "        self.y = y\n",
    "    }\n",
    "}\n",
);

/// The same three subjects written the way `idioms.md` asks for.
const SWIFT_IDIOMS_ANSWER: &str = concat!(
    "import Foundation\n\n",
    "struct Color {\n",
    "    static let red = 1\n",
    "}\n\n",
    "struct Registry {\n",
    "    var ids: Set<String> = []\n",
    "}\n\n",
    "struct Point {\n",
    "    var x: Int\n",
    "    var y: Int\n",
    "}\n",
);

/// Every judgment rule this module measures, with its probe pair.
const SWIFT_JUDGMENT_RULES: &[SwiftJudgmentRule] = &[
    SwiftJudgmentRule {
        prompt_rule: SWIFT_INITIALIZATION_PROMPT_RULE,
        refusal: SWIFT_INITIALIZATION_REFUSAL,
        answer: SWIFT_INITIALIZATION_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_IMMUTABILITY_PROMPT_RULE,
        refusal: SWIFT_IMMUTABILITY_REFUSAL,
        answer: SWIFT_IMMUTABILITY_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_PRECONDITIONS_PROMPT_RULE,
        refusal: SWIFT_PRECONDITIONS_REFUSAL,
        answer: SWIFT_PRECONDITIONS_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_NAMING_CLARITY_PROMPT_RULE,
        refusal: SWIFT_NAMING_CLARITY_REFUSAL,
        answer: SWIFT_NAMING_CLARITY_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_ACCESS_CONTROL_PROMPT_RULE,
        refusal: SWIFT_ACCESS_CONTROL_REFUSAL,
        answer: SWIFT_ACCESS_CONTROL_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_VALUE_SEMANTICS_PROMPT_RULE,
        refusal: SWIFT_VALUE_SEMANTICS_REFUSAL,
        answer: SWIFT_VALUE_SEMANTICS_ANSWER,
    },
    SwiftJudgmentRule {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        refusal: SWIFT_IDIOMS_REFUSAL,
        answer: SWIFT_IDIOMS_ANSWER,
    },
];

/// Drives the shipped script of `gate` over `source`, staged at
/// [`SWIFT_JUDGMENT_PROBE_PATH`], and answers the rule name of each finding it
/// reported.
///
/// The probe stages the source alone. No shipped Swift gate reads a
/// `.swift-version` any longer: `idioms-swift` runs the toolchain's own
/// `swift format`, which reads no such file — measured over two directories
/// holding the same source, with and without one, at the same finding count —
/// and the swiftlint gates never read it.
fn swift_judgment_gate_rules(gate: &str, source: &str) -> Vec<String> {
    swift_gate_reporting_rules(gate, SWIFT_JUDGMENT_PROBE_PATH, source, NO_SUPPORT_FILES)
}

/// Acceptance: no shipped Swift gate decides any judgment rule of the `swift`
/// set, in either direction.
///
/// Both halves are load-bearing, and they answer the two ways a judgment rule
/// can go wrong.
///
/// A gate that reports the DON'T is a SECOND owner for a requirement the prompt
/// rule states, and two owners produce churn on every review round — the tool
/// finding and the prompt finding arrive together, and the author cannot tell
/// which edit satisfies both. That bullet belongs to the tool instead, and the
/// tool rule records the deletion.
///
/// A gate that reports the DO is worse: the prompt rule then walks the author
/// into a tool finding no edit can satisfy while the bullet stands.
///
/// Measured over the five shipped Swift gates that read a file list, with the
/// toolchain's `swift format` on Apple Swift 6.4 and with swiftlint 0.65.0: 10
/// probes, 50 runs, 0 findings.
#[test]
fn no_shipped_swift_gate_decides_a_swift_judgment_rule() {
    for rule in SWIFT_JUDGMENT_RULES {
        for gate in SWIFT_FILE_GATES {
            let refused = swift_judgment_gate_rules(gate, rule.refusal);
            assert!(
                refused.is_empty(),
                "`{gate}` reports the shape `{}.md` refuses, so that requirement has two \
                 owners; the run reported {refused:?}",
                rule.prompt_rule
            );

            let answered = swift_judgment_gate_rules(gate, rule.answer);
            assert!(
                answered.is_empty(),
                "`{gate}` reports the shape `{}.md` ASKS for, so the prompt rule walks the \
                 author into a tool finding; the run reported {answered:?}",
                rule.prompt_rule
            );
        }
    }
}
