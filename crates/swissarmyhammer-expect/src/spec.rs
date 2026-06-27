//! Parsing `*.expect.md` expectation files into the typed [`Expectation`] model.
//!
//! An expectation is a markdown file (`ideas/expect.md` §"Expectation File
//! Format"): a YAML [`Frontmatter`] block followed by a prose body that *is* the
//! intent. The body may carry optional `## Given` / `## When` sections, a
//! `## Then` GFM checklist of acceptance [`Criterion`]s, and a `## Notes` block.
//!
//! The frontmatter is a **closed** enumeration of keys (the "Frontmatter
//! Reference" table): `deny_unknown_fields` makes a typo such as `surfce:` fail
//! loudly rather than be silently ignored. `description` and `surface` are
//! required; everything else carries the documented default.
//!
//! Identity is the file path, not a frontmatter field: an expectation at
//! `src/checkout/coupon.expect.md` is addressed as `src/checkout/coupon` (its
//! repo-relative path with the `.expect.md` extension stripped).
//!
//! Parsing is permissive about *content* — Given/When/Then are all optional and a
//! body with zero criteria still parses. The "at least one acceptance criterion"
//! rule is enforced later, by `doctor`, not here.

use crate::error::ExpectError;
use crate::types::{Surface, VerdictTier};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::Path;
use std::time::Duration;

/// The file extension that marks an expectation spec.
///
/// Shared with [`crate::loader`] so discovery and identity derivation agree on a
/// single source of truth for the `*.expect.md` suffix.
pub(crate) const EXPECT_EXTENSION: &str = ".expect.md";

/// The default wall-clock budget for one run when `timeout` is omitted.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Length of a GFM checkbox marker (`[ ]`, `[x]`, or `[X]`).
const CHECKBOX_MARKER_LEN: usize = 3;

/// A parsed expectation file: its identity, frontmatter, and body content.
///
/// The whole markdown body is the [`intent`](Expectation::intent); the
/// structured fields ([`given`](Expectation::given),
/// [`when`](Expectation::when), [`criteria`](Expectation::criteria),
/// [`notes`](Expectation::notes)) are extracted views of that same body for the
/// driver and grader to consume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    /// Repo-relative path with `.expect.md` stripped — the expectation's identity.
    pub path: String,
    /// The closed-set YAML frontmatter.
    pub frontmatter: Frontmatter,
    /// The whole markdown body after the frontmatter — the intent.
    pub intent: String,
    /// The acceptance criteria from the `## Then` GFM checklist.
    pub criteria: Vec<Criterion>,
    /// The `## Given` arrangement bullets, if any.
    pub given: Vec<String>,
    /// The `## When` action bullets, if any.
    pub when: Vec<String>,
    /// The `## Notes` block, if present — extra intent the example can't pin.
    pub notes: Option<String>,
}

/// The closed enumeration of frontmatter keys from `ideas/expect.md`
/// §"Frontmatter Reference".
///
/// `deny_unknown_fields` rejects anything outside this set so a typo fails the
/// parser loudly. `description` and `surface` are required; every other field
/// carries the documented default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frontmatter {
    /// One-line description, like a skill's `description`. Required.
    pub description: String,
    /// How `expect` perceives and acts on the system under test. Required.
    pub surface: Surface,
    /// The named sah model that grades Tier-3 criteria; omit to use the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `pass^k` reliability policy. Default `pass^1`.
    #[serde(default = "default_reliability")]
    pub reliability: ReliabilityPolicy,
    /// How many times to run before judging reliability; derived when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    /// Which verdict-ladder tiers may decide a criterion. Default: all three.
    #[serde(default = "default_tiers")]
    pub tiers: Vec<VerdictTier>,
    /// Per-expectation Tier-2 cosine cutoff override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity_threshold: Option<f32>,
    /// Wall-clock budget for one run. Default 60s.
    #[serde(
        default = "default_timeout",
        with = "duration_str",
        skip_serializing_if = "is_default_timeout"
    )]
    pub timeout: Duration,
    /// Grouping tags for `list --tag` / glob-by-tag scope. Default `[]`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Provisioning declaration for the surface — how to build/launch the SUT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<Setup>,
    /// Whether this expectation gets its own provision. Default `shared`.
    #[serde(default)]
    pub isolation: Isolation,
}

/// A single acceptance criterion from the `## Then` GFM checklist.
///
/// Each `- [ ]` / `- [x]` item is one bounded criterion the grading model
/// evaluates on its own, binary, with evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    /// The criterion text (the checklist item with its marker stripped).
    pub text: String,
    /// Whether the checklist box was ticked (`- [x]`).
    pub checked: bool,
}

/// The `pass^k` reliability policy declared in frontmatter.
///
/// Parsed from the literal `pass^N` form (`pass^1`, `pass^3`); `N` is the number
/// of repeated runs that must all pass. This is the *declared* policy — the
/// per-run result lives in [`crate::Reliability`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReliabilityPolicy {
    /// `k` in `pass^k` — the number of runs that must all pass (≥ 1).
    ///
    /// Private so the ≥ 1 invariant (enforced in [`Deserialize`]) cannot be
    /// bypassed by direct construction; read it via [`ReliabilityPolicy::required`].
    required: u32,
}

impl ReliabilityPolicy {
    /// `k` in `pass^k` — the number of runs that must all pass (always ≥ 1).
    pub fn required(&self) -> u32 {
        self.required
    }
}

impl Default for ReliabilityPolicy {
    /// The documented default policy, `pass^1` — a single run must pass.
    fn default() -> Self {
        ReliabilityPolicy { required: 1 }
    }
}

impl Serialize for ReliabilityPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("pass^{}", self.required))
    }
}

impl<'de> Deserialize<'de> for ReliabilityPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        let digits = raw.strip_prefix("pass^").ok_or_else(|| {
            serde::de::Error::custom(format!(
                "reliability must be of the form `pass^N` (e.g. `pass^3`), got `{raw}`"
            ))
        })?;
        let required: u32 = digits.parse().map_err(|_| {
            serde::de::Error::custom(format!(
                "reliability must be of the form `pass^N` (e.g. `pass^3`), got `{raw}`"
            ))
        })?;
        if required == 0 {
            return Err(serde::de::Error::custom(
                "reliability `pass^N` requires N >= 1",
            ));
        }
        Ok(ReliabilityPolicy { required })
    }
}

/// Whether an expectation runs against the shared SUT or its own provision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Run against the one instance provisioned per `check` (default).
    #[default]
    Shared,
    /// Provision a dedicated, pristine instance for this expectation.
    Fresh,
}

/// A provisioning declaration: either a single command or a list of commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Setup {
    /// A single setup command.
    Command(String),
    /// An ordered list of setup commands.
    Commands(Vec<String>),
}

/// The default `pass^1` reliability policy.
fn default_reliability() -> ReliabilityPolicy {
    ReliabilityPolicy::default()
}

/// The default tier set: all three ladder tiers may decide a criterion.
fn default_tiers() -> Vec<VerdictTier> {
    vec![
        VerdictTier::Deterministic,
        VerdictTier::Tolerance,
        VerdictTier::Judgment,
    ]
}

/// The default 60s run budget.
fn default_timeout() -> Duration {
    DEFAULT_TIMEOUT
}

/// Whether a timeout equals the default (so it can be omitted on serialize).
fn is_default_timeout(timeout: &Duration) -> bool {
    *timeout == DEFAULT_TIMEOUT
}

impl Expectation {
    /// Parse an expectation from the raw markdown `content` of a `*.expect.md`
    /// file, deriving its identity from `file_path` relative to `repo_root`.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Expectation`] when `file_path` is not under
    /// `repo_root`, when the frontmatter block is missing or malformed, or when
    /// the frontmatter carries an unknown key (`deny_unknown_fields`).
    pub fn parse(content: &str, file_path: &Path, repo_root: &Path) -> Result<Self, ExpectError> {
        let path = derive_path(file_path, repo_root)?;
        let (frontmatter_str, body) = split_frontmatter(content, &path)?;

        let frontmatter: Frontmatter =
            serde_yaml_ng::from_str(frontmatter_str).map_err(|e| ExpectError::Expectation {
                path: path.clone(),
                message: format!("invalid frontmatter: {e}"),
            })?;

        let sections = Sections::extract(body);

        Ok(Expectation {
            path,
            frontmatter,
            intent: body.trim().to_string(),
            criteria: sections.criteria,
            given: sections.given,
            when: sections.when,
            notes: sections.notes,
        })
    }
}

/// Derive the repo-relative identity path (`.expect.md` stripped) for a spec
/// file located at `file_path` within `repo_root`.
///
/// Path components are joined with `/` so identities are stable across
/// platforms, matching the forward-slash form used by [`crate::Observation`].
pub(crate) fn derive_path(file_path: &Path, repo_root: &Path) -> Result<String, ExpectError> {
    let relative = file_path
        .strip_prefix(repo_root)
        .map_err(|_| ExpectError::Expectation {
            path: file_path.display().to_string(),
            message: format!("spec file is not under repo root {}", repo_root.display()),
        })?;
    let joined = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let identity = joined
        .strip_suffix(EXPECT_EXTENSION)
        .unwrap_or(&joined)
        .to_string();
    Ok(identity)
}

/// Split markdown `content` into its YAML frontmatter block and the body that
/// follows.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when the content does not open with a
/// `---` frontmatter delimiter or the closing delimiter is missing.
fn split_frontmatter<'a>(content: &'a str, path: &str) -> Result<(&'a str, &'a str), ExpectError> {
    let trimmed = content.trim_start();
    let after_open = trimmed
        .strip_prefix("---\n")
        .or_else(|| trimmed.strip_prefix("---\r\n"))
        .ok_or_else(|| ExpectError::Expectation {
            path: path.to_string(),
            message: "missing YAML frontmatter (file must open with `---`)".to_string(),
        })?;

    for (offset, line) in line_offsets(after_open) {
        if line.trim_end() == "---" {
            let frontmatter = &after_open[..offset];
            let body = &after_open[offset + line.len()..];
            return Ok((frontmatter, body.trim_start_matches(['\r', '\n'])));
        }
    }

    Err(ExpectError::Expectation {
        path: path.to_string(),
        message: "unterminated YAML frontmatter (missing closing `---`)".to_string(),
    })
}

/// Yield each line of `text` with its starting byte offset, including the line's
/// own trailing newline in [`str::len`] terms via the next offset.
fn line_offsets(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    text.split_inclusive('\n').map(move |line| {
        let start = offset;
        offset += line.len();
        (start, line.trim_end_matches('\n'))
    })
}

/// The structured views extracted from an expectation body.
struct Sections {
    given: Vec<String>,
    when: Vec<String>,
    criteria: Vec<Criterion>,
    notes: Option<String>,
}

impl Sections {
    /// Walk the body once, routing `## Given` / `## When` bullets, `## Notes`
    /// prose, and every GFM checklist item (the acceptance criteria) into place.
    ///
    /// Criteria are collected from any `- [ ]` / `- [x]` item in the body, so a
    /// spec with the criteria checklist but no `## Then` header still parses.
    fn extract(body: &str) -> Self {
        let mut given = Vec::new();
        let mut when = Vec::new();
        let mut criteria = Vec::new();
        let mut notes_lines: Vec<&str> = Vec::new();
        let mut current = Section::None;

        for line in body.lines() {
            if let Some(section) = Section::from_heading(line) {
                current = section;
                continue;
            }

            if let Some(criterion) = parse_criterion(line) {
                criteria.push(criterion);
                continue;
            }

            // Notes captures every line verbatim; Given/When capture only plain
            // bullets. Hoisting the bullet parse above the match keeps the
            // routing at one nesting level instead of one per arm.
            if let Section::Notes = current {
                notes_lines.push(line);
            } else if let Some(item) = parse_bullet(line) {
                match current {
                    Section::Given => given.push(item),
                    Section::When => when.push(item),
                    Section::None | Section::Then | Section::Notes => {}
                }
            }
        }

        let notes = {
            let joined = notes_lines.join("\n");
            let trimmed = joined.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };

        Sections {
            given,
            when,
            criteria,
            notes,
        }
    }
}

/// Which `##` section the parser is currently inside.
#[derive(Clone, Copy)]
enum Section {
    None,
    Given,
    When,
    Then,
    Notes,
}

impl Section {
    /// Recognize a `## Given` / `## When` / `## Then` / `## Notes` heading line,
    /// case-insensitively, or `None` for any other line.
    fn from_heading(line: &str) -> Option<Section> {
        let heading = line.trim().strip_prefix("##")?.trim();
        match heading.to_ascii_lowercase().as_str() {
            "given" => Some(Section::Given),
            "when" => Some(Section::When),
            "then" => Some(Section::Then),
            "notes" => Some(Section::Notes),
            _ => None,
        }
    }
}

/// Parse a plain `- ` / `* ` bullet line into its text, or `None`.
pub(crate) fn parse_bullet(line: &str) -> Option<String> {
    let rest = line
        .trim()
        .strip_prefix("- ")
        .or_else(|| line.trim().strip_prefix("* "))?;
    let text = rest.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Parse a GFM task-list item (`- [ ]` / `- [x]`) into a [`Criterion`], or
/// `None` if the line is not a checklist item.
pub(crate) fn parse_criterion(line: &str) -> Option<Criterion> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))?;
    let marker = rest.get(..CHECKBOX_MARKER_LEN)?;
    let checked = match marker {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let text = rest[CHECKBOX_MARKER_LEN..].trim();
    Some(Criterion {
        text: text.to_string(),
        checked,
    })
}

/// Serialize a [`Duration`] as a human duration string (`30s`, `5m`) and parse
/// the same forms back.
mod duration_str {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    /// Milliseconds in one hour.
    const MILLIS_PER_HOUR: u128 = 3_600_000;
    /// Milliseconds in one minute.
    const MILLIS_PER_MINUTE: u128 = 60_000;
    /// Milliseconds in one second.
    const MILLIS_PER_SECOND: u128 = 1_000;
    /// Milliseconds in one millisecond.
    const MILLIS_PER_MILLIS: u128 = 1;

    /// Serialize as the most compact whole-unit form (`h`, `m`, `s`, or `ms`).
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = duration.as_millis();
        let text = if millis == 0 {
            "0s".to_string()
        } else if millis.is_multiple_of(MILLIS_PER_HOUR) {
            format!("{}h", millis / MILLIS_PER_HOUR)
        } else if millis.is_multiple_of(MILLIS_PER_MINUTE) {
            format!("{}m", millis / MILLIS_PER_MINUTE)
        } else if millis.is_multiple_of(MILLIS_PER_SECOND) {
            format!("{}s", millis / MILLIS_PER_SECOND)
        } else {
            format!("{millis}ms")
        };
        serializer.serialize_str(&text)
    }

    /// Parse `<integer><unit>` where unit is `ms`, `s`, `m`, or `h`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        parse_duration(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "invalid duration `{raw}` (expected forms like `30s`, `5m`, `1h`, `500ms`)"
            ))
        })
    }

    /// Parse a human duration string into a [`Duration`].
    fn parse_duration(raw: &str) -> Option<Duration> {
        let raw = raw.trim();
        let (value, unit_millis): (&str, u128) = if let Some(value) = raw.strip_suffix("ms") {
            (value, MILLIS_PER_MILLIS)
        } else if let Some(value) = raw.strip_suffix('s') {
            (value, MILLIS_PER_SECOND)
        } else if let Some(value) = raw.strip_suffix('m') {
            (value, MILLIS_PER_MINUTE)
        } else if let Some(value) = raw.strip_suffix('h') {
            (value, MILLIS_PER_HOUR)
        } else {
            return None;
        };
        let count: u64 = value.trim().parse().ok()?;
        Some(Duration::from_millis(count * unit_millis as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The worked coupon example from `ideas/expect.md` §"Expectation File Format".
    const COUPON_SPEC: &str = r#"---
description: A valid coupon reduces the order total by its discount, exactly once
surface: cli            # cli | http | browser | gui | file | db
reliability: pass^3     # all of 3 runs must pass (default: pass^1)
model: qwen-coder-flash # named sah model for grading; omit to use the sah model default
tags: [checkout, pricing]
---

# A valid coupon reduces the total, exactly once

When a shopper applies a valid coupon to an order, the displayed total drops by
the coupon's discount amount, and applying the same coupon a second time does not
stack. The discount must come off the subtotal, not be a coincidence of some
other arithmetic.

## Given
- A freshly created cart with one $50 item (arranged per run, so `pass^3` stays independent)
- A coupon `SAVE10` worth $10 off, currently valid

## When
- The shopper applies `SAVE10`
- The shopper applies `SAVE10` again

## Then
- [ ] After the first apply, the total is $40
- [ ] The UI confirms the coupon was applied
- [ ] After the second apply, the total is still $40 (no stacking)
- [ ] An error or notice explains the coupon is already applied

## Notes
The discount must come off subtotal before tax. Don't accept a $40 total that
was reached by the wrong arithmetic (e.g. a 20% rounding coincidence) — the
reason must be the coupon.
"#;

    fn repo_root() -> PathBuf {
        PathBuf::from("/repo")
    }

    fn coupon_path() -> PathBuf {
        PathBuf::from("/repo/src/checkout/coupon.expect.md")
    }

    #[test]
    fn parses_the_coupon_worked_example() {
        let expectation =
            Expectation::parse(COUPON_SPEC, &coupon_path(), &repo_root()).expect("parse coupon");

        assert_eq!(expectation.path, "src/checkout/coupon");
        assert_eq!(
            expectation.frontmatter.description,
            "A valid coupon reduces the order total by its discount, exactly once"
        );
        assert_eq!(expectation.frontmatter.surface, Surface::Cli);
        assert_eq!(expectation.frontmatter.reliability.required(), 3);
        assert_eq!(
            expectation.frontmatter.model.as_deref(),
            Some("qwen-coder-flash")
        );
        assert_eq!(expectation.frontmatter.tags, vec!["checkout", "pricing"]);

        assert_eq!(expectation.given.len(), 2);
        assert_eq!(expectation.when.len(), 2);
        assert_eq!(expectation.criteria.len(), 4);
        assert!(expectation.criteria.iter().all(|c| !c.checked));
        assert_eq!(
            expectation.criteria[0].text,
            "After the first apply, the total is $40"
        );

        let notes = expectation.notes.expect("notes block");
        assert!(notes.contains("subtotal before tax"));
        assert!(notes.contains("the coupon"));

        // The intent is the whole body, including the criteria checklist.
        assert!(expectation.intent.contains("displayed total drops"));
    }

    #[test]
    fn rejects_an_unknown_frontmatter_key() {
        let spec = r#"---
description: typo in surface key
surfce: cli
---

## Then
- [ ] something holds
"#;
        let err = Expectation::parse(spec, &coupon_path(), &repo_root())
            .expect_err("unknown key must fail");
        let message = err.to_string();
        assert!(
            message.contains("surfce"),
            "error should name the bad key, got: {message}"
        );
    }

    #[test]
    fn applies_defaults_when_optional_keys_are_omitted() {
        let spec = r#"---
description: minimal spec relying on defaults
surface: http
---

## Then
- [ ] the service responds
"#;
        let expectation =
            Expectation::parse(spec, &coupon_path(), &repo_root()).expect("parse minimal");

        assert_eq!(expectation.frontmatter.reliability.required(), 1);
        assert_eq!(expectation.frontmatter.isolation, Isolation::Shared);
        assert_eq!(expectation.frontmatter.timeout, Duration::from_secs(60));
        assert_eq!(
            expectation.frontmatter.tiers,
            vec![
                VerdictTier::Deterministic,
                VerdictTier::Tolerance,
                VerdictTier::Judgment,
            ]
        );
        assert!(expectation.frontmatter.tags.is_empty());
        assert!(expectation.frontmatter.model.is_none());
        assert!(expectation.frontmatter.repeat.is_none());
        assert!(expectation.frontmatter.similarity_threshold.is_none());
        assert!(expectation.frontmatter.setup.is_none());
    }

    #[test]
    fn parses_a_spec_with_given_when_then_omitted() {
        let spec = r#"---
description: pure intent plus a criteria list, no G/W/T headers
surface: file
---

The output file must exist and contain the rendered report.

- [ ] the report file exists
- [x] the report contains a total row
"#;
        let expectation =
            Expectation::parse(spec, &coupon_path(), &repo_root()).expect("parse no-GWT spec");

        assert!(expectation.given.is_empty());
        assert!(expectation.when.is_empty());
        assert_eq!(expectation.criteria.len(), 2);
        assert!(!expectation.criteria[0].checked);
        assert!(expectation.criteria[1].checked);
        assert!(expectation.notes.is_none());
    }

    #[test]
    fn parsing_tolerates_zero_criteria() {
        let spec = r#"---
description: intent with no criteria yet
surface: cli
---

Some prose describing the intended behavior, but no checklist yet.
"#;
        let expectation =
            Expectation::parse(spec, &coupon_path(), &repo_root()).expect("parse zero-criteria");
        assert!(expectation.criteria.is_empty());
    }

    #[test]
    fn rejects_a_file_outside_the_repo_root() {
        let err = Expectation::parse(
            COUPON_SPEC,
            Path::new("/elsewhere/coupon.expect.md"),
            &repo_root(),
        )
        .expect_err("file outside repo root must fail");
        assert!(err.to_string().contains("repo root"));
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let spec = "# Just a heading\n\n- [ ] a criterion\n";
        let err = Expectation::parse(spec, &coupon_path(), &repo_root())
            .expect_err("missing frontmatter must fail");
        assert!(err.to_string().contains("frontmatter"));
    }

    #[test]
    fn parses_timeout_and_reliability_forms() {
        let spec = r#"---
description: explicit timeout and reliability
surface: cli
reliability: pass^5
timeout: 5m
---

## Then
- [ ] holds
"#;
        let expectation =
            Expectation::parse(spec, &coupon_path(), &repo_root()).expect("parse durations");
        assert_eq!(expectation.frontmatter.reliability.required(), 5);
        assert_eq!(expectation.frontmatter.timeout, Duration::from_secs(300));
    }

    #[test]
    fn reliability_policy_round_trips_through_yaml() {
        let policy = ReliabilityPolicy { required: 3 };
        let yaml = serde_yaml_ng::to_string(&policy).unwrap();
        assert_eq!(yaml.trim(), "pass^3");
        let parsed: ReliabilityPolicy = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed, policy);
    }

    #[test]
    fn setup_accepts_string_or_list() {
        let one: Setup = serde_yaml_ng::from_str("\"make build\"").unwrap();
        assert_eq!(one, Setup::Command("make build".to_string()));

        let many: Setup = serde_yaml_ng::from_str("- make build\n- make seed\n").unwrap();
        assert_eq!(
            many,
            Setup::Commands(vec!["make build".to_string(), "make seed".to_string()])
        );
    }
}
