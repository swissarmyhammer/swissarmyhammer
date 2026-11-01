//! What the shipped Swift guidelines partial has to agree with.
//!
//! `builtin/_partials/project-types/swift.md` is the Swift guidance an agent
//! receives before it writes a line of Swift, and this project ships a gate
//! that judges the Swift it then writes. Guidance that disagrees with the gate
//! walks the author into a finding from this project's own review, so the
//! agreement is a requirement rather than a coincidence.
//!
//! The partial is RENDERED here through the path `detect projects` uses: an
//! `{% include "_partials/project-types/swift" %}` directive, rendered by the
//! `TemplateLibrary` a `PromptResolver` fills. An assertion here therefore
//! measures the text an agent actually RECEIVES, after the front matter and
//! the template engine have had their say, rather than a file on disk.
//!
//! What the partial is HELD TO is read from the shipped `idioms-swift` rule:
//! the lint command out of its script, the exemption directive out of its
//! body and the Swift version floor out of its `doctor.fix_hint`. Each value
//! has ONE owner, and a change to the gate fails a test here rather than
//! leaving the partial to drift.
//!
//! This module stands in this crate rather than beside the partial in
//! `swissarmyhammer-templating`, because that crate cannot read a validator:
//! `swissarmyhammer-validators` depends on `swissarmyhammer-templating`, so
//! the edge the other way is a dependency cycle. This crate reaches both
//! sides already.

use super::*;

use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_templating::{PromptResolver, TemplateLibrary};

/// The path `detect projects` includes the Swift project-type guidelines
/// under.
///
/// It is the `_partials/project-types/{key}` path
/// `swissarmyhammer-project-detection` builds for the `swift` project type.
const SWIFT_GUIDELINES_PARTIAL_INCLUDE: &str = "_partials/project-types/swift";

/// The Swift project-type guidelines, rendered as `detect projects` renders
/// them for an agent.
fn rendered_swift_guidelines() -> String {
    let mut library = TemplateLibrary::new();
    PromptResolver::new()
        .load_all_prompts(&mut library)
        .expect("the builtin partials must load into the template library");
    library
        .render_text(
            &format!("{{% include \"{SWIFT_GUIDELINES_PARTIAL_INCLUDE}\" %}}"),
            &TemplateContext::new(),
        )
        .expect("the Swift guidelines partial must render through the template library")
}

/// Assert the rendered Swift guidelines carry each phrase they must keep.
///
/// Each row pairs the exact text the guidelines have to hold with the
/// requirement that text satisfies, so a failure names the requirement rather
/// than only the missing string.
fn assert_swift_guidelines_state(rendered: &str, requirements: &[(&str, &str)]) {
    for (needle, requirement) in requirements {
        assert!(
            rendered.contains(needle),
            "the swift guidelines must {requirement} (missing {needle:?})"
        );
    }
}

/// The needles among `needles` the rendered Swift guidelines name.
///
/// Tool names in prose are not case sensitive, so the match is made on the
/// rendered text with the case ignored, and every needle must be lower case.
fn named_in_swift_guidelines<'a>(rendered: &str, needles: &[&'a str]) -> Vec<&'a str> {
    let rendered = rendered.to_lowercase();
    needles
        .iter()
        .copied()
        .filter(|needle| rendered.contains(needle))
        .collect()
}

/// Acceptance: a capitalised mention of a forbidden needle is caught, so the
/// negative checks cannot be slipped past with a different case.
#[test]
fn swift_partial_negative_checks_ignore_case() {
    assert_eq!(
        named_in_swift_guidelines(
            "Run `SwiftLint --fix` and SwiftFormat:Disable",
            &[
                "swiftlint --fix",
                "swiftformat:disable",
                "swiftlint autocorrect",
            ]
        ),
        ["swiftlint --fix", "swiftformat:disable"]
    );
}

/// Acceptance: the guidelines name ONE formatter, the toolchain's own
/// `swift format`, with the command that writes, the command that checks, and
/// the configuration rules.
///
/// `swift format lint` without `-s` writes warnings and exits 0, so the check
/// command keeps `-s`. The tool reads `.swift-format` on its own, so the
/// guidelines obey a file the project holds, and make a first one only from
/// `dump-configuration`.
#[test]
fn swift_partial_states_the_one_toolchain_formatter() {
    assert_swift_guidelines_state(
        &rendered_swift_guidelines(),
        &[
            (
                "swift format -i -r Sources Tests",
                "give the command that writes the formatting",
            ),
            (
                "swift format lint -s -r Sources Tests",
                "give the strict command that checks the formatting",
            ),
            (
                "`.swift-format`",
                "name the configuration file the toolchain formatter reads",
            ),
            (
                "Obey a `.swift-format` the project already holds",
                "obey a configuration the project already holds",
            ),
            (
                "Do not write one as a side effect of formatting",
                "forbid creating a configuration file while formatting",
            ),
            (
                "`swift format dump-configuration`",
                "name the command that makes a first configuration",
            ),
        ],
    );
}

/// Acceptance: the guidelines name no second formatter, no plugin that wraps
/// one, and no mechanism of one.
///
/// Every needle is matched in lower case, so a capitalised mention fails the
/// test as well.
#[test]
fn swift_partial_names_no_second_formatter() {
    let rendered = rendered_swift_guidelines().to_lowercase();
    for (needle, why) in [
        ("swiftformat", "a different program from `swift format`"),
        ("airbnb", "a plugin that wraps a second formatter"),
        ("--property-types", "an option of a second formatter"),
        (".swift-version", "a file a second formatter reads"),
        ("allow-writing-to-package-directory", "a plugin command"),
    ] {
        assert!(
            !rendered.contains(needle),
            "the swift guidelines must not name {needle:?}, {why}"
        );
    }
}

/// Acceptance: the guidelines name `swiftlint` and `periphery` as the tools
/// the review gates need, say Homebrew installs them, and never offer either as
/// a formatter.
#[test]
fn swift_partial_names_the_review_gate_tools_and_no_formatter_use() {
    let rendered = rendered_swift_guidelines();
    assert_swift_guidelines_state(
        &rendered,
        &[
            (
                "`brew install swiftlint periphery`",
                "say Homebrew installs the two review gate tools",
            ),
            (
                "`swiftlint`",
                "name the tool the disallowed-construct gate needs",
            ),
            ("`periphery`", "name the tool the dead-code gate needs"),
        ],
    );
    let formatter_uses = named_in_swift_guidelines(
        &rendered,
        &["swiftlint --fix", "swiftlint autocorrect", ".swiftlint.yml"],
    );
    assert!(
        formatter_uses.is_empty(),
        "the swift guidelines must not offer swiftlint as a formatter ({formatter_uses:?})"
    );
}

/// What the `idioms-swift` rule body writes before the exemption directive an
/// author writes.
const SWIFT_IDIOMS_DIRECTIVE_MARK: &str = "The directive is `";

/// What closes a quoted span of a rule body.
const SWIFT_GUIDELINES_QUOTE: char = '`';

/// What follows the Swift version `doctor.fix_hint` names as its floor.
const SWIFT_GUIDELINES_FLOOR_MARK: &str = " or newer";

/// The markdown source of the shipped `idioms-swift` rule, front matter and
/// body alike.
fn shipped_swift_idioms_source(loader: &ValidatorLoader) -> String {
    std::fs::read_to_string(shipped_asset(loader, &RULE_SOURCE_ASSET, SWIFT_IDIOMS_RULE))
        .expect("read the shipped Swift idioms rule source")
}

/// The exemption directive the shipped `idioms-swift` rule body tells an
/// author to write.
fn shipped_swift_idioms_directive(source: &str) -> String {
    let opened = source
        .split_once(SWIFT_IDIOMS_DIRECTIVE_MARK)
        .unwrap_or_else(|| {
            panic!("the `{SWIFT_IDIOMS_RULE}` body must state {SWIFT_IDIOMS_DIRECTIVE_MARK:?}")
        })
        .1;
    opened
        .split_once(SWIFT_GUIDELINES_QUOTE)
        .unwrap_or_else(|| panic!("the directive the `{SWIFT_IDIOMS_RULE}` body states must close"))
        .0
        .to_string()
}

/// The Swift version the shipped `idioms-swift` doctor names as its floor.
///
/// The first `or newer` of the source stands in `doctor.fix_hint`, the
/// sentence a person reads when the doctor reports the rule unhealthy.
fn shipped_swift_idioms_floor(source: &str) -> String {
    let at = source
        .find(SWIFT_GUIDELINES_FLOOR_MARK)
        .unwrap_or_else(|| panic!("the fix hint must state {SWIFT_GUIDELINES_FLOOR_MARK:?}"));
    source[..at]
        .split_whitespace()
        .next_back()
        .unwrap_or_else(|| panic!("the fix hint must name a version before the mark"))
        .to_string()
}

/// Acceptance: the guidelines and the `idioms-swift` gate name the SAME
/// formatter, the SAME exemption directive and the SAME Swift version floor,
/// each read out of the shipped gate.
///
/// A gate that moved to another tool or another directive fails here rather
/// than leaving the guidelines to name a tool no review runs, or a directive
/// the gate ignores.
#[test]
fn swift_partial_agrees_with_the_idioms_swift_gate() {
    let loader = builtin_loader();
    let rendered = rendered_swift_guidelines();

    let script = required_shipped_tool_rule(&loader, SWIFT_IDIOMS_RULE).script;
    assert!(
        script.contains("swift format lint"),
        "the shipped `{SWIFT_IDIOMS_RULE}` script must run `swift format lint`, or the \
         guidelines below name a tool no review runs"
    );

    let source = shipped_swift_idioms_source(&loader);
    let directive = shipped_swift_idioms_directive(&source);
    let floor = shipped_swift_idioms_floor(&source);
    let floor_sentence = format!("Swift {floor}{SWIFT_GUIDELINES_FLOOR_MARK}");
    assert_swift_guidelines_state(
        &rendered,
        &[
            (
                "swift format lint",
                "name the lint command the idioms-swift gate itself runs",
            ),
            (
                directive.as_str(),
                "name the exemption directive the idioms-swift gate reads",
            ),
            (
                floor_sentence.as_str(),
                "state the Swift version floor the idioms-swift doctor asks for",
            ),
        ],
    );
    assert!(
        named_in_swift_guidelines(&rendered, &["swiftformat:disable"]).is_empty(),
        "the swift guidelines must not name the directive of a different formatter"
    );
}
