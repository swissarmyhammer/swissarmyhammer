//! The static half of `check`: per-field, teaching diagnostics for a single
//! `*.expect.md` spec (`ideas/expect.md` §"expect doctor" and §"Errors that
//! teach").
//!
//! [`diagnose`] validates a spec **without driving any system or consulting any
//! model**, returning a [`FieldDiagnostic`] per problem (and per body/criteria/
//! dynamic-field check) that doubles as a repair instruction: each finding names
//! *what* is wrong, *where* (a 1-based line), the closed `allowed` set it
//! violated, and a concrete `suggestion` to apply verbatim. [`render`] turns that
//! structured `Vec` into the human `✗ … / →` shape from the design example.
//!
//! Validation has a static half and a dynamic half. `surface`, `tiers`,
//! `reliability`, and `isolation` are *static* closed enums. `model` and `setup`
//! are *dynamic*: they are checked against [`DoctorFacts`] — the live model
//! registry and the surface/project provisioning facts — which the caller injects
//! so this function stays pure and deterministic (tests pass fixed facts; the
//! production caller populates [`DoctorFacts::available_models`] from
//! `swissarmyhammer_config::model::ModelManager`). A pinned `model:` that has gone
//! missing is a **warning, not an error**: grading falls back to the default and
//! the golden compare catches any divergence as drift.

use crate::spec::{parse_bullet, parse_criterion, Criterion, Isolation, ReliabilityPolicy, Setup};
use crate::types::{Surface, VerdictTier};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

/// The status of one field-level finding, mirroring the `ok`/`warning`/`error`
/// shape `sah doctor` speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticStatus {
    /// The field is well-formed.
    Ok,
    /// The field is suspect but not fatal — e.g. a pinned model that has gone
    /// missing, which safely falls back to the default.
    Warning,
    /// The field is malformed or uncheckable and must be fixed.
    Error,
}

/// One per-field finding from [`diagnose`], designed as a repair instruction
/// rather than a stack trace (`ideas/expect.md` §"Errors that teach").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDiagnostic {
    /// The field this is about: a frontmatter key, `body`, `criteria`, or
    /// `frontmatter` for whole-block problems.
    pub field: String,
    /// Whether the field is ok, suspect (warning), or broken (error).
    pub status: DiagnosticStatus,
    /// What is wrong (or right), in one line.
    pub message: String,
    /// The closed set of allowed values, when the field violated one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
    /// A concrete fix to apply verbatim, when one can be offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// The 1-based line in the source the finding points at, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// The live facts [`diagnose`] validates the *dynamic* fields against.
///
/// Injected by the caller so [`diagnose`] is pure and deterministic. The
/// production caller fills [`available_models`](Self::available_models) from the
/// live sah registry (`swissarmyhammer_config::model::ModelManager::list_agents`);
/// tests pass a fixed list.
#[derive(Debug, Clone, Default)]
pub struct DoctorFacts {
    /// The model names in the live sah registry; a `model:` outside this set is a
    /// warning.
    pub available_models: Vec<String>,
    /// The provisioning commands the surface/project can actually run. `Some`
    /// means a `setup:` outside the set cannot provision (error); `None` means
    /// the project facts are unavailable, so a `setup:` is merely unverifiable
    /// (warning).
    pub known_setup_commands: Option<Vec<String>>,
}

/// Validate one `*.expect.md` spec statically and return a per-field diagnostic
/// for every problem found, plus an `Ok` finding for each dynamic field and
/// criterion that checks out.
///
/// No system is driven and no model is consulted: the dynamic `model:` / `setup:`
/// checks read only the injected [`DoctorFacts`].
///
/// # Examples
///
/// ```
/// use swissarmyhammer_expect::doctor::{diagnose, DiagnosticStatus, DoctorFacts};
///
/// let spec = "---\ndescription: a typo'd surface key\nsurfce: cli\n---\n\nIntent prose.\n\n- [ ] the total is $40\n";
/// let facts = DoctorFacts::default();
/// let diagnostics = diagnose(spec, &facts);
///
/// let unknown = diagnostics.iter().find(|d| d.field == "surfce").expect("flags the typo");
/// assert_eq!(unknown.status, DiagnosticStatus::Error);
/// assert_eq!(unknown.suggestion.as_deref(), Some("surface"));
/// ```
pub fn diagnose(content: &str, facts: &DoctorFacts) -> Vec<FieldDiagnostic> {
    let lines: Vec<&str> = content.lines().collect();
    let mut diagnostics = Vec::new();

    let fence = find_frontmatter(&lines);
    match fence {
        Some((open, close)) => check_frontmatter(&lines, open, close, facts, &mut diagnostics),
        None => diagnostics.push(FieldDiagnostic::finding(
            "frontmatter",
            DiagnosticStatus::Error,
            "missing YAML frontmatter (file must open with `---`)".to_string(),
        )),
    }

    let body_start = fence.map_or(0, |(_, close)| close + 1);
    check_body(&lines, body_start, &mut diagnostics);

    diagnostics
}

/// Render structured `diagnostics` for `path` as the human `✗ … / →` shape from
/// the design example: a `✓`/`✗` header line and an indented `→` fix line under
/// each finding that carries a suggestion or an allowed set.
pub fn render(path: &str, diagnostics: &[FieldDiagnostic]) -> String {
    let healthy = diagnostics.iter().all(|d| d.status == DiagnosticStatus::Ok);
    let mut out = String::new();
    let _ = writeln!(out, "{} {path}", if healthy { '✓' } else { '✗' });

    for d in diagnostics {
        let mark = match d.status {
            DiagnosticStatus::Ok => '✓',
            DiagnosticStatus::Warning => '⚠',
            DiagnosticStatus::Error => '✗',
        };
        let location = d.line.map(|n| format!(" (line {n})")).unwrap_or_default();
        let _ = writeln!(out, "  {mark} {}: {}{location}", d.field, d.message);

        let mut fix = Vec::new();
        if let Some(suggestion) = &d.suggestion {
            fix.push(format!("suggestion: {suggestion}"));
        }
        if let Some(allowed) = &d.allowed {
            fix.push(format!("allowed: {}", allowed.join(" | ")));
        }
        if !fix.is_empty() {
            let _ = writeln!(out, "    → {}", fix.join("; "));
        }
    }

    out
}

impl FieldDiagnostic {
    /// Start a finding for `field` with the given `status` and `message`; chain
    /// [`with_allowed`](Self::with_allowed) / [`with_suggestion`](Self::with_suggestion)
    /// / [`at_line`](Self::at_line) to attach the repair hints.
    fn finding(field: &str, status: DiagnosticStatus, message: String) -> Self {
        FieldDiagnostic {
            field: field.to_string(),
            status,
            message,
            allowed: None,
            suggestion: None,
            line: None,
        }
    }

    /// Attach the closed set of allowed values.
    fn with_allowed(mut self, allowed: Vec<String>) -> Self {
        self.allowed = Some(allowed);
        self
    }

    /// Attach an optional concrete fix.
    fn with_suggestion(mut self, suggestion: Option<String>) -> Self {
        self.suggestion = suggestion;
        self
    }

    /// Attach the optional 1-based source line.
    fn at_line(mut self, line: Option<usize>) -> Self {
        self.line = line;
        self
    }
}

/// The closed set of frontmatter keys (`ideas/expect.md` §"Frontmatter
/// Reference"), kept in lockstep with [`crate::spec::Frontmatter`]. Anything
/// outside this set is an unknown key.
pub(crate) const KNOWN_KEYS: &[&str] = &[
    "description",
    "surface",
    "model",
    "reliability",
    "repeat",
    "tiers",
    "similarity_threshold",
    "timeout",
    "tags",
    "setup",
    "isolation",
];

/// The frontmatter keys that must be present.
pub(crate) const REQUIRED_KEYS: &[&str] = &["description", "surface"];

/// The largest edit distance at which a "did you mean" suggestion is offered for
/// a misspelled key or enum value.
const MAX_SUGGEST_DISTANCE: usize = 3;

/// The fix offered for a criterion with no observable signal, naming a concrete
/// threshold to copy (`ideas/expect.md` §"Errors that teach").
const THRESHOLD_SUGGESTION: &str =
    "no observable signal — state a threshold, e.g. \"the cart page responds in under 500ms\", or drop this criterion";

/// Subjective words that, absent any number, mark a criterion as unmeasurable
/// ("feels fast", "snappy"). The presence of a digit overrides this — a threshold
/// is an observable signal.
const VAGUE_TERMS: &[&str] = &[
    "fast",
    "slow",
    "feels",
    "feel",
    "quick",
    "quickly",
    "snappy",
    "responsive",
    "nice",
    "good",
    "smooth",
    "intuitive",
    "clean",
    "performant",
    "usable",
    "pleasant",
    "seamless",
    "sluggish",
    "laggy",
];

/// The closed set of `surface` values, in `Surface` enum order — the single
/// source of truth for the surface `allowed` list and validity check.
const ALL_SURFACES: &[Surface] = &[
    Surface::Cli,
    Surface::Http,
    Surface::Browser,
    Surface::Gui,
    Surface::File,
    Surface::Db,
];

/// The closed `tiers` set, in `VerdictTier` order.
const ALL_TIERS: &[VerdictTier] = &[
    VerdictTier::Deterministic,
    VerdictTier::Tolerance,
    VerdictTier::Judgment,
];

/// The closed `isolation` set, in `Isolation` order.
const ALL_ISOLATIONS: &[Isolation] = &[Isolation::Shared, Isolation::Fresh];

/// The serialized lowercase value of a closed-enum variant (`Surface::Cli` →
/// `"cli"`), so `allowed` lists are derived from the enums, never re-typed.
fn wire<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The allowed `surface` values, derived from [`ALL_SURFACES`].
pub(crate) fn surface_values() -> Vec<String> {
    ALL_SURFACES.iter().map(wire).collect()
}

/// The allowed `tiers` values, derived from [`ALL_TIERS`].
pub(crate) fn tier_values() -> Vec<String> {
    ALL_TIERS.iter().map(wire).collect()
}

/// The allowed `isolation` values, derived from [`ALL_ISOLATIONS`].
pub(crate) fn isolation_values() -> Vec<String> {
    ALL_ISOLATIONS.iter().map(wire).collect()
}

/// The closed `allowed` set for a frontmatter `key`, when it has one.
fn allowed_for_key(key: &str) -> Option<Vec<String>> {
    match key {
        "surface" => Some(surface_values()),
        "tiers" => Some(tier_values()),
        "isolation" => Some(isolation_values()),
        _ => None,
    }
}

/// Locate the `(open, close)` 0-based line indices of the YAML frontmatter fence.
fn find_frontmatter(lines: &[&str]) -> Option<(usize, usize)> {
    let open = lines.iter().position(|l| l.trim() == "---")?;
    let close = lines[open + 1..].iter().position(|l| l.trim() == "---")? + open + 1;
    Some((open, close))
}

/// Validate the frontmatter block between the fence lines and push a finding per
/// problem (and an `Ok` finding for each dynamic field that checks out).
fn check_frontmatter(
    lines: &[&str],
    open: usize,
    close: usize,
    facts: &DoctorFacts,
    diagnostics: &mut Vec<FieldDiagnostic>,
) {
    let block = lines[open + 1..close].join("\n");
    let key_lines = key_line_map(lines, open + 1, close);

    let mapping = match serde_yaml_ng::from_str::<Value>(&block) {
        Ok(Value::Mapping(mapping)) => mapping,
        Ok(Value::Null) => Mapping::new(),
        Ok(_) => {
            diagnostics.push(FieldDiagnostic::finding(
                "frontmatter",
                DiagnosticStatus::Error,
                "frontmatter must be a set of `key: value` pairs".to_string(),
            ));
            return;
        }
        Err(e) => {
            diagnostics.push(FieldDiagnostic::finding(
                "frontmatter",
                DiagnosticStatus::Error,
                format!("invalid frontmatter: {e}"),
            ));
            return;
        }
    };

    let suggested_required = check_unknown_keys(&mapping, &key_lines, diagnostics);
    check_required_keys(&mapping, &key_lines, &suggested_required, diagnostics);
    check_enum_fields(&mapping, &key_lines, diagnostics);
    check_model(&mapping, &key_lines, facts, diagnostics);
    check_setup(&mapping, &key_lines, facts, diagnostics);
}

/// Map each top-level frontmatter key to its 1-based source line.
fn key_line_map(lines: &[&str], from: usize, to: usize) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (idx, line) in lines.iter().enumerate().take(to).skip(from) {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(colon) = line.find(':') {
            let key = &line[..colon];
            if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                map.entry(key.to_string()).or_insert(idx + 1);
            }
        }
    }
    map
}

/// Flag every key outside [`KNOWN_KEYS`] with a "did you mean" suggestion, and
/// return the set of required keys those typos plausibly stand in for (so the
/// missing-required check does not double-report them).
fn check_unknown_keys(
    mapping: &Mapping,
    key_lines: &HashMap<String, usize>,
    diagnostics: &mut Vec<FieldDiagnostic>,
) -> BTreeSet<String> {
    let mut suggested_required = BTreeSet::new();
    for (key, _) in mapping {
        let Some(name) = key.as_str() else { continue };
        if KNOWN_KEYS.contains(&name) {
            continue;
        }
        let suggestion = closest(name, KNOWN_KEYS.iter().copied(), Some(MAX_SUGGEST_DISTANCE));
        let allowed = suggestion.as_deref().and_then(allowed_for_key);
        if let Some(s) = &suggestion {
            if REQUIRED_KEYS.contains(&s.as_str()) {
                suggested_required.insert(s.clone());
            }
        }
        let mut finding = FieldDiagnostic::finding(
            name,
            DiagnosticStatus::Error,
            format!("unknown key `{name}`"),
        )
        .with_suggestion(suggestion)
        .at_line(key_lines.get(name).copied());
        if let Some(allowed) = allowed {
            finding = finding.with_allowed(allowed);
        }
        diagnostics.push(finding);
    }
    suggested_required
}

/// Flag a missing or blank `description` / `surface`, skipping any required key a
/// typo already pointed at.
fn check_required_keys(
    mapping: &Mapping,
    key_lines: &HashMap<String, usize>,
    suggested_required: &BTreeSet<String>,
    diagnostics: &mut Vec<FieldDiagnostic>,
) {
    for &key in REQUIRED_KEYS {
        if suggested_required.contains(key) {
            continue;
        }
        let present = map_get(mapping, key).is_some_and(|v| !is_blank(v));
        if !present {
            let mut finding = FieldDiagnostic::finding(
                key,
                DiagnosticStatus::Error,
                format!("`{key}` is required"),
            )
            .at_line(key_lines.get(key).copied());
            if let Some(allowed) = allowed_for_key(key) {
                finding = finding.with_allowed(allowed);
            }
            diagnostics.push(finding);
        }
    }
}

/// Validate the static closed-enum fields (`surface`, `tiers`, `reliability`,
/// `isolation`) by round-tripping each value through its domain type.
fn check_enum_fields(
    mapping: &Mapping,
    key_lines: &HashMap<String, usize>,
    diagnostics: &mut Vec<FieldDiagnostic>,
) {
    if let Some(v) = map_get(mapping, "surface").filter(|v| !is_blank(v)) {
        if serde_yaml_ng::from_value::<Surface>(v.clone()).is_err() {
            let got = v.as_str().unwrap_or_default();
            diagnostics.push(
                FieldDiagnostic::finding(
                    "surface",
                    DiagnosticStatus::Error,
                    format!("`{got}` is not a valid surface"),
                )
                .with_allowed(surface_values())
                .with_suggestion(closest(
                    got,
                    surface_values().iter().map(String::as_str),
                    Some(MAX_SUGGEST_DISTANCE),
                ))
                .at_line(key_lines.get("surface").copied()),
            );
        }
    }

    if let Some(v) = map_get(mapping, "tiers").filter(|v| !is_blank(v)) {
        if serde_yaml_ng::from_value::<Vec<VerdictTier>>(v.clone()).is_err() {
            diagnostics.push(
                FieldDiagnostic::finding(
                    "tiers",
                    DiagnosticStatus::Error,
                    "`tiers` must be a subset of the verdict ladder".to_string(),
                )
                .with_allowed(tier_values())
                .at_line(key_lines.get("tiers").copied()),
            );
        }
    }

    if let Some(v) = map_get(mapping, "reliability").filter(|v| !is_blank(v)) {
        if serde_yaml_ng::from_value::<ReliabilityPolicy>(v.clone()).is_err() {
            diagnostics.push(
                FieldDiagnostic::finding(
                    "reliability",
                    DiagnosticStatus::Error,
                    "`reliability` must be of the form `pass^N` with N >= 1".to_string(),
                )
                .with_suggestion(Some("pass^1".to_string()))
                .at_line(key_lines.get("reliability").copied()),
            );
        }
    }

    if let Some(v) = map_get(mapping, "isolation").filter(|v| !is_blank(v)) {
        if serde_yaml_ng::from_value::<Isolation>(v.clone()).is_err() {
            diagnostics.push(
                FieldDiagnostic::finding(
                    "isolation",
                    DiagnosticStatus::Error,
                    "`isolation` is not a valid value".to_string(),
                )
                .with_allowed(isolation_values())
                .at_line(key_lines.get("isolation").copied()),
            );
        }
    }
}

/// Validate the dynamic `model:` against the injected live registry. A missing
/// pinned model is a **warning**, not an error (grading falls back to default).
fn check_model(
    mapping: &Mapping,
    key_lines: &HashMap<String, usize>,
    facts: &DoctorFacts,
    diagnostics: &mut Vec<FieldDiagnostic>,
) {
    let Some(name) = map_get(mapping, "model").and_then(Value::as_str) else {
        return;
    };
    let line = key_lines.get("model").copied();
    if facts.available_models.iter().any(|m| m == name) {
        diagnostics.push(
            FieldDiagnostic::finding(
                "model",
                DiagnosticStatus::Ok,
                format!("`{name}` is available"),
            )
            .at_line(line),
        );
    } else {
        diagnostics.push(
            FieldDiagnostic::finding(
                "model",
                DiagnosticStatus::Warning,
                format!("`{name}` is not an available model"),
            )
            .with_allowed(facts.available_models.clone())
            .with_suggestion(closest(
                name,
                facts.available_models.iter().map(String::as_str),
                None,
            ))
            .at_line(line),
        );
    }
}

/// Validate the dynamic `setup:` against the injected project facts: each command
/// must be a known provisioning command (error if not), or — when no facts are
/// available — is flagged unverifiable (warning).
fn check_setup(
    mapping: &Mapping,
    key_lines: &HashMap<String, usize>,
    facts: &DoctorFacts,
    diagnostics: &mut Vec<FieldDiagnostic>,
) {
    let Some(v) = map_get(mapping, "setup") else {
        return;
    };
    let line = key_lines.get("setup").copied();

    let commands = match serde_yaml_ng::from_value::<Setup>(v.clone()) {
        Ok(Setup::Command(c)) => vec![c],
        Ok(Setup::Commands(cs)) => cs,
        Err(_) => {
            diagnostics.push(
                FieldDiagnostic::finding(
                    "setup",
                    DiagnosticStatus::Error,
                    "`setup` must be a string or a list of strings".to_string(),
                )
                .at_line(line),
            );
            return;
        }
    };

    for command in commands {
        diagnostics.push(classify_setup_command(&command, facts, line));
    }
}

/// Classify a single `setup` command against the project facts.
fn classify_setup_command(
    command: &str,
    facts: &DoctorFacts,
    line: Option<usize>,
) -> FieldDiagnostic {
    match &facts.known_setup_commands {
        None => FieldDiagnostic::finding(
            "setup",
            DiagnosticStatus::Warning,
            format!(
                "cannot verify `{command}` provisions the surface (no project facts available)"
            ),
        )
        .at_line(line),
        Some(known) if known.iter().any(|k| k == command) => FieldDiagnostic::finding(
            "setup",
            DiagnosticStatus::Ok,
            format!("`{command}` will provision the surface"),
        )
        .at_line(line),
        Some(known) => FieldDiagnostic::finding(
            "setup",
            DiagnosticStatus::Error,
            format!("`{command}` does not match a known build target, fixture, or command"),
        )
        .with_suggestion(closest(
            command,
            known.iter().map(String::as_str),
            Some(MAX_SUGGEST_DISTANCE),
        ))
        .at_line(line),
    }
}

/// Check the body: it must state intent (have prose, not be all mechanics) and
/// carry at least one criterion, and each criterion must be checkable.
fn check_body(lines: &[&str], body_start: usize, diagnostics: &mut Vec<FieldDiagnostic>) {
    let mut has_intent = false;
    let mut criteria: Vec<(usize, Criterion)> = Vec::new();
    for (idx, line) in lines.iter().enumerate().skip(body_start) {
        if let Some(criterion) = parse_criterion(line) {
            criteria.push((idx + 1, criterion));
        } else if is_intent_line(line) {
            has_intent = true;
        }
    }

    diagnostics.push(if has_intent {
        FieldDiagnostic::finding(
            "body",
            DiagnosticStatus::Ok,
            "states intended behavior".to_string(),
        )
    } else {
        FieldDiagnostic::finding(
            "body",
            DiagnosticStatus::Error,
            "states no intent — the body is all mechanics; describe the behavior this expectation pins".to_string(),
        )
    });

    if criteria.is_empty() {
        diagnostics.push(FieldDiagnostic::finding(
            "criteria",
            DiagnosticStatus::Error,
            "no acceptance criteria — add at least one `- [ ]` checklist item".to_string(),
        ));
        return;
    }

    for (line, criterion) in criteria {
        diagnostics.push(classify_criterion(&criterion.text, line));
    }
}

/// Classify one criterion as checkable (`Ok`) or — when it has a subjective term
/// and no observable signal — uncheckable (`Error` with a threshold suggestion).
fn classify_criterion(text: &str, line: usize) -> FieldDiagnostic {
    if is_vague(text) {
        FieldDiagnostic::finding(
            "criteria",
            DiagnosticStatus::Error,
            format!("\"{text}\" is not checkable"),
        )
        .with_suggestion(Some(THRESHOLD_SUGGESTION.to_string()))
        .at_line(Some(line))
    } else {
        FieldDiagnostic::finding(
            "criteria",
            DiagnosticStatus::Ok,
            format!("\"{text}\" is checkable"),
        )
        .at_line(Some(line))
    }
}

/// Whether a body `line` is intent-bearing prose: non-empty and not a heading,
/// code fence, bullet, or checklist item (the "mechanics").
fn is_intent_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('#')
        && !trimmed.starts_with("```")
        && parse_criterion(line).is_none()
        && parse_bullet(line).is_none()
}

/// Whether a criterion has no observable signal: it uses a subjective term and
/// carries no number that could serve as a threshold.
fn is_vague(text: &str) -> bool {
    if text.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    text.split(|c: char| !c.is_alphanumeric())
        .any(|word| VAGUE_TERMS.contains(&word.to_ascii_lowercase().as_str()))
}

/// Look up a frontmatter value by string key without relying on the `Index`
/// trait's surface form.
fn map_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping
        .iter()
        .find_map(|(k, v)| (k.as_str() == Some(key)).then_some(v))
}

/// Whether a YAML value is effectively absent — null or an empty/whitespace
/// string.
fn is_blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        _ => false,
    }
}

/// The closest candidate to `target` by Levenshtein distance, within `max` edits
/// when given (no bound when `None`).
fn closest<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    max: Option<usize>,
) -> Option<String> {
    let (distance, best) = candidates
        .into_iter()
        .map(|c| (levenshtein(target, c), c))
        .min_by_key(|(d, _)| *d)?;
    max.is_none_or(|m| distance <= m).then(|| best.to_string())
}

/// The Levenshtein edit distance between `a` and `b`.
fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0usize; b_chars.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The injected model registry the dynamic-field tests validate against.
    fn facts() -> DoctorFacts {
        DoctorFacts {
            available_models: vec![
                "claude-sonnet-4-6".to_string(),
                "qwen-coder-flash".to_string(),
                "claude-haiku-4-5".to_string(),
            ],
            known_setup_commands: Some(vec!["make build".to_string(), "make seed".to_string()]),
        }
    }

    /// Assemble a spec from a frontmatter block and a body so line numbers are
    /// predictable: line 1 is `---`, line 2 is the first frontmatter key.
    fn spec(frontmatter: &str, body: &str) -> String {
        format!("---\n{frontmatter}\n---\n\n{body}\n")
    }

    /// A body that states intent and carries one deterministic criterion, so a
    /// frontmatter-focused test isolates the field under test.
    const GOOD_BODY: &str = "When a shopper applies a coupon the displayed total drops.\n\n## Then\n- [ ] After the first apply, the total is $40";

    /// A spec exercising every known frontmatter key — guards [`super`]'s known-key
    /// set against drift from [`crate::spec::Frontmatter`].
    const FULL_SPEC: &str = "---\ndescription: A valid coupon reduces the total exactly once\nsurface: cli\nmodel: qwen-coder-flash\nreliability: pass^3\nrepeat: 3\ntiers: [deterministic, tolerance, judgment]\nsimilarity_threshold: 0.8\ntimeout: 30s\ntags: [checkout]\nsetup: make build\nisolation: shared\n---\n\nWhen a shopper applies a coupon the displayed total drops.\n\n## Then\n- [ ] After the first apply, the total is $40\n";

    fn find<'a>(diagnostics: &'a [FieldDiagnostic], field: &str) -> Option<&'a FieldDiagnostic> {
        diagnostics.iter().find(|d| d.field == field)
    }

    #[test]
    fn unknown_key_is_error_with_did_you_mean_and_allowed() {
        let diagnostics = diagnose(&spec("description: typo\nsurfce: cli", GOOD_BODY), &facts());
        let d = find(&diagnostics, "surfce").expect("diagnostic naming the unknown key");
        assert_eq!(d.status, DiagnosticStatus::Error);
        assert_eq!(d.suggestion.as_deref(), Some("surface"));
        assert_eq!(
            d.allowed.as_ref().expect("allowed surfaces"),
            &surface_values()
        );
        // line 1 `---`, line 2 description, line 3 surfce.
        assert_eq!(d.line, Some(3));
    }

    #[test]
    fn missing_required_field_is_error() {
        for (frontmatter, missing) in [
            ("surface: cli", "description"),
            ("description: present but no surface", "surface"),
        ] {
            let diagnostics = diagnose(&spec(frontmatter, GOOD_BODY), &facts());
            let d = find(&diagnostics, missing)
                .unwrap_or_else(|| panic!("expected an error on `{missing}`"));
            assert_eq!(d.status, DiagnosticStatus::Error, "field `{missing}`");
        }
    }

    #[test]
    fn invalid_surface_value_is_error_with_allowed_and_suggestion() {
        let diagnostics = diagnose(&spec("description: d\nsurface: clii", GOOD_BODY), &facts());
        let d = find(&diagnostics, "surface").expect("surface value diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Error);
        assert_eq!(
            d.allowed.as_ref().expect("allowed surfaces"),
            &surface_values()
        );
        assert_eq!(d.suggestion.as_deref(), Some("cli"));
    }

    #[test]
    fn body_without_stated_intent_is_error() {
        let body = "## When\n- the shopper applies a coupon\n\n## Then\n- [ ] the total is $40";
        let diagnostics = diagnose(&spec("description: d\nsurface: cli", body), &facts());
        let d = find(&diagnostics, "body").expect("body diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Error);
    }

    #[test]
    fn body_with_stated_intent_is_ok() {
        let diagnostics = diagnose(&spec("description: d\nsurface: cli", GOOD_BODY), &facts());
        let d = find(&diagnostics, "body").expect("body diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Ok);
    }

    #[test]
    fn zero_criteria_is_error() {
        let body = "Some intent prose describing the intended behavior, but no checklist.";
        let diagnostics = diagnose(&spec("description: d\nsurface: cli", body), &facts());
        let d = find(&diagnostics, "criteria").expect("criteria diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Error);
    }

    #[test]
    fn vague_criterion_is_error_and_deterministic_is_ok() {
        let body = "Intent prose.\n\n## Then\n- [ ] the checkout feels fast\n- [ ] After the first apply, the total is $40";
        let diagnostics = diagnose(&spec("description: d\nsurface: cli", body), &facts());

        let vague = diagnostics
            .iter()
            .find(|d| d.field == "criteria" && d.message.contains("feels fast"))
            .expect("vague criterion diagnostic");
        assert_eq!(vague.status, DiagnosticStatus::Error);
        assert!(
            vague
                .suggestion
                .as_deref()
                .is_some_and(|s| s.contains("threshold")),
            "vague criterion suggests a threshold, got: {:?}",
            vague.suggestion
        );

        let ok = diagnostics
            .iter()
            .find(|d| d.field == "criteria" && d.message.contains("$40"))
            .expect("deterministic criterion diagnostic");
        assert_eq!(ok.status, DiagnosticStatus::Ok);
    }

    #[test]
    fn missing_model_is_warning_with_available_and_suggestion() {
        let f = facts();
        let diagnostics = diagnose(
            &spec("description: d\nsurface: cli\nmodel: qwen-flash", GOOD_BODY),
            &f,
        );
        let d = find(&diagnostics, "model").expect("model diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Warning);
        assert_eq!(
            d.allowed.as_ref().expect("available models"),
            &f.available_models
        );
        assert_eq!(d.suggestion.as_deref(), Some("qwen-coder-flash"));
    }

    #[test]
    fn present_model_is_ok() {
        let diagnostics = diagnose(
            &spec(
                "description: d\nsurface: cli\nmodel: qwen-coder-flash",
                GOOD_BODY,
            ),
            &facts(),
        );
        let d = find(&diagnostics, "model").expect("model diagnostic");
        assert_eq!(d.status, DiagnosticStatus::Ok);
    }

    #[test]
    fn setup_validity_is_classified_against_project_facts() {
        struct Case {
            setup: &'static str,
            known: Option<Vec<String>>,
            want: DiagnosticStatus,
        }
        let cases = [
            Case {
                setup: "make build",
                known: Some(vec!["make build".to_string(), "make seed".to_string()]),
                want: DiagnosticStatus::Ok,
            },
            Case {
                setup: "make nonexistent",
                known: Some(vec!["make build".to_string()]),
                want: DiagnosticStatus::Error,
            },
            Case {
                setup: "make anything",
                known: None,
                want: DiagnosticStatus::Warning,
            },
        ];
        for case in cases {
            let f = DoctorFacts {
                available_models: facts().available_models,
                known_setup_commands: case.known,
            };
            let frontmatter = format!("description: d\nsurface: cli\nsetup: {}", case.setup);
            let diagnostics = diagnose(&spec(&frontmatter, GOOD_BODY), &f);
            let d = find(&diagnostics, "setup").expect("setup diagnostic");
            assert_eq!(d.status, case.want, "setup `{}`", case.setup);
        }
    }

    #[test]
    fn full_spec_has_no_unknown_key_and_no_errors() {
        let diagnostics = diagnose(FULL_SPEC, &facts());
        let unknown: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("unknown key"))
            .collect();
        assert!(
            unknown.is_empty(),
            "unexpected unknown-key findings: {unknown:?}"
        );
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.status == DiagnosticStatus::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "a fully valid spec should have no errors: {errors:?}"
        );
    }

    #[test]
    fn render_shows_path_field_and_fix_arrow() {
        let diagnostics = diagnose(&spec("description: typo\nsurfce: cli", GOOD_BODY), &facts());
        let out = render("checkout/coupon.expect.md", &diagnostics);
        assert!(out.contains("checkout/coupon.expect.md"), "render: {out}");
        assert!(out.contains("surfce"), "render: {out}");
        assert!(out.contains('→'), "render: {out}");
    }
}
