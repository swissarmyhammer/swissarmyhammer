//! Validator introspection ops for the `review` tool: `list/get/check validators`.
//!
//! These three ops are pure loader reads — no agent, no index, fast. They load
//! the full builtin → user → project RuleSet stack via
//! [`swissarmyhammer_validators::load_rules`] and report on it:
//!
//! - `list validators` — one summary row per loaded RuleSet (name, description,
//!   source layer, match globs, probes, rule count, path), optionally
//!   filtered by `source` and/or a path/glob `match`, and optionally carrying
//!   every rule's verbatim body (`rules: true`). One call with
//!   `match: <file>` + `rules: true` therefore answers "what will a review
//!   enforce on this file?" without a `get validator` call per name.
//! - `get validator` — one RuleSet's full rule bodies + probes.
//! - `check validators` — lint every loaded RuleSet: frontmatter is valid (it
//!   parsed), declared globs compile, no stray `triggerMatcher`, and every
//!   declared probe exists in the engine's probe catalog.

use std::collections::BTreeSet;

use serde::Serialize;
use swissarmyhammer_validators::review::probe_exists;
use swissarmyhammer_validators::validators::{
    compile_glob_patterns, matches_any_pattern, MatchContext, ValidatorLoader, ValidatorMatch,
};
use swissarmyhammer_validators::{load_rules, RuleSet};

use crate::mcp::op_tool_helpers::is_glob_pattern;

/// A `list validators` summary row.
#[derive(Debug, Serialize)]
pub struct ValidatorSummary {
    /// The RuleSet name.
    pub name: String,
    /// The RuleSet description (mandate).
    pub description: String,
    /// Which precedence layer it came from (`builtin` / `user` / `project`).
    pub source_layer: String,
    /// The file globs the RuleSet matches against.
    pub match_globs: Vec<String>,
    /// The probe names the RuleSet declares.
    pub probes: Vec<String>,
    /// How many rules the RuleSet carries.
    pub rule_count: usize,
    /// The RuleSet directory path.
    pub path: String,
    /// Every rule's name and verbatim body — present only when the call asked
    /// for them (`rules: true`), so a plain listing stays a summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RuleDetail>>,
}

/// One rule in a `get validator` response.
#[derive(Debug, Serialize)]
pub struct RuleDetail {
    /// The rule name.
    pub name: String,
    /// The rule's markdown body verbatim.
    pub body: String,
}

/// A `get validator` response — one RuleSet's full detail.
#[derive(Debug, Serialize)]
pub struct ValidatorDetail {
    /// The RuleSet name.
    pub name: String,
    /// The full parsed frontmatter (manifest), as the loader holds it.
    pub frontmatter: ValidatorFrontmatterView,
    /// Which precedence layer it came from.
    pub source_layer: String,
    /// The RuleSet directory path.
    pub path: String,
    /// The probe names the RuleSet declares.
    pub probes: Vec<String>,
    /// Each rule's name and full body.
    pub rules: Vec<RuleDetail>,
}

/// The frontmatter view rendered into a `get validator` response.
#[derive(Debug, Serialize)]
pub struct ValidatorFrontmatterView {
    /// The RuleSet name.
    pub name: String,
    /// The RuleSet description.
    pub description: String,
    /// The file globs.
    pub match_globs: Vec<String>,
    /// The declared tags.
    pub tags: Vec<String>,
    /// The package version.
    pub version: String,
}

/// One `check validators` problem.
#[derive(Debug, Serialize)]
pub struct ValidatorProblem {
    /// The RuleSet path (or name) the problem is about.
    pub path: String,
    /// What is wrong.
    pub problem: String,
}

/// A `check validators` response: overall `ok` plus every problem found.
#[derive(Debug, Serialize)]
pub struct CheckValidatorsResponse {
    /// True when no problem was found across every loaded RuleSet.
    pub ok: bool,
    /// How many RuleSets were loaded and linted.
    pub count: usize,
    /// Every lint problem found (empty when `ok`).
    pub errors: Vec<ValidatorProblem>,
}

/// The match globs of a RuleSet, or an empty vec when it matches everything.
fn match_globs(ruleset: &RuleSet) -> Vec<String> {
    ruleset
        .manifest
        .match_criteria
        .as_ref()
        .map(|m: &ValidatorMatch| m.files.clone())
        .unwrap_or_default()
}

/// Every rule of a RuleSet, name plus verbatim body.
///
/// The one place a `RuleDetail` list is built, so `get validator` and a
/// `rules: true` listing always report the same bytes.
fn rule_details(ruleset: &RuleSet) -> Vec<RuleDetail> {
    ruleset
        .rules
        .iter()
        .map(|rule| RuleDetail {
            name: rule.name.clone(),
            body: rule.body.clone(),
        })
        .collect()
}

/// Build the summary row for one RuleSet, carrying its rule bodies when
/// `include_rules` is set.
fn summary(ruleset: &RuleSet, include_rules: bool) -> ValidatorSummary {
    ValidatorSummary {
        name: ruleset.name().to_string(),
        description: ruleset.description().to_string(),
        source_layer: ruleset.source.to_string(),
        match_globs: match_globs(ruleset),
        probes: ruleset.manifest.probes.clone(),
        rule_count: ruleset.rules.len(),
        path: ruleset.base_path.display().to_string(),
        rules: include_rules.then(|| rule_details(ruleset)),
    }
}

/// The names of the RuleSets the engine pairs with one concrete file path.
///
/// This is the engine's own matcher — a [`MatchContext`] carrying the file, run
/// through [`ValidatorLoader::matching_rulesets`] — which is exactly how
/// `scope_review` pairs each changed file with its validators. Answering a
/// path-shaped `match` through it means the tool cannot drift from the set a
/// review run will actually enforce on that file.
fn engine_matched_names(loader: &ValidatorLoader, path: &str) -> BTreeSet<String> {
    let ctx = MatchContext::new().with_file(path.to_string());
    loader
        .matching_rulesets(&ctx)
        .into_iter()
        .map(|rs| rs.name().to_string())
        .collect()
}

/// Whether a RuleSet passes the `source` and `match` filters.
///
/// `source` is one of `builtin` / `user` / `project` / `all` (or absent = all).
///
/// `match` is answered two ways, by the shape of the value:
///
/// - A concrete path (no glob metacharacter) is delegated to the engine matcher,
///   pre-computed into `engine_matched` by [`list_validators`]; membership in that
///   set is the whole test.
/// - A glob fragment (`*.rs`, `**/*.ts`) keeps the documented lenient behavior:
///   the fragment is tested as a path against the RuleSet's globs, and also
///   matched as a substring of them, so a caller can find a validator by the
///   pattern it declares.
fn passes_filters(
    ruleset: &RuleSet,
    source: Option<&str>,
    match_filter: Option<&str>,
    engine_matched: Option<&BTreeSet<String>>,
) -> bool {
    if let Some(source) = source {
        if !source.eq_ignore_ascii_case("all")
            && !ruleset.source.to_string().eq_ignore_ascii_case(source)
        {
            return false;
        }
    }

    if let Some(matched) = engine_matched {
        return matched.contains(ruleset.name());
    }

    if let Some(needle) = match_filter {
        let globs = match_globs(ruleset);
        let compiled = compile_glob_patterns(&globs);
        let hit =
            matches_any_pattern(needle, &compiled) || globs.iter().any(|g| g.contains(needle));
        if !hit {
            return false;
        }
    }

    true
}

/// `list validators`: load the full RuleSet stack, filter, and return summaries
/// sorted by name.
///
/// `include_rules` adds each RuleSet's rules (name + verbatim body) to its row,
/// so one call with a path-shaped `match_filter` returns the full rule text a
/// review run will enforce on that file.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails (user/project directory read
/// error).
pub fn list_validators(
    source: Option<&str>,
    match_filter: Option<&str>,
    include_rules: bool,
) -> Result<Vec<ValidatorSummary>, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    // A path-shaped `match` is answered by the engine matcher, once for the whole
    // stack rather than per row.
    let engine_matched = match_filter
        .filter(|needle| !is_glob_pattern(needle))
        .map(|path| engine_matched_names(&loader, path));

    let mut summaries: Vec<ValidatorSummary> = loader
        .list_rulesets()
        .into_iter()
        .filter(|rs| passes_filters(rs, source, match_filter, engine_matched.as_ref()))
        .map(|rs| summary(rs, include_rules))
        .collect();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

/// `get validator`: load the stack and return one RuleSet's full detail.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails or when no RuleSet is named
/// `name`.
pub fn get_validator(name: &str) -> Result<ValidatorDetail, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    let ruleset = loader
        .get_ruleset(name)
        .ok_or_else(|| format!("no validator named '{name}'"))?;

    Ok(ValidatorDetail {
        name: ruleset.name().to_string(),
        frontmatter: ValidatorFrontmatterView {
            name: ruleset.name().to_string(),
            description: ruleset.description().to_string(),
            match_globs: match_globs(ruleset),
            tags: ruleset.manifest.tags.clone(),
            version: ruleset.manifest.metadata.version.clone(),
        },
        source_layer: ruleset.source.to_string(),
        path: ruleset.base_path.display().to_string(),
        probes: ruleset.manifest.probes.clone(),
        rules: rule_details(ruleset),
    })
}

/// `check validators`: lint every loaded RuleSet and report load failures.
///
/// Reports a problem when a RuleSet declares a glob that does not compile, sets a
/// stray `triggerMatcher` (review validators match by file, not by event), or
/// declares a probe that is not in the engine's probe catalog.
///
/// A RuleSet whose frontmatter does not parse never reaches the loaded set, but
/// it is **not** silently dropped: the loader retains each parse failure
/// ([`load_failures`](swissarmyhammer_validators::ValidatorLoader::load_failures))
/// and this lint surfaces every one as an error naming the offending path and its
/// parse problem. A broken validator never aborts the run — the rest still load.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails.
pub fn check_validators() -> Result<CheckValidatorsResponse, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    let mut errors: Vec<ValidatorProblem> = Vec::new();

    let rulesets = loader.list_rulesets();
    let count = rulesets.len();
    for ruleset in rulesets {
        let path = ruleset.base_path.display().to_string();
        lint_ruleset(ruleset, &path, &mut errors);
    }

    // Dropped (unparseable) validators: each is reported, not swallowed.
    for failure in loader.load_failures() {
        errors.push(ValidatorProblem {
            path: failure.path.display().to_string(),
            problem: format!(
                "failed to load ({} validator): {}",
                failure.source, failure.error
            ),
        });
    }

    errors.sort_by(|a, b| {
        (a.path.as_str(), a.problem.as_str()).cmp(&(b.path.as_str(), b.problem.as_str()))
    });
    Ok(CheckValidatorsResponse {
        ok: errors.is_empty(),
        count,
        errors,
    })
}

/// Lint one RuleSet, appending any problems found.
fn lint_ruleset(ruleset: &RuleSet, path: &str, errors: &mut Vec<ValidatorProblem>) {
    // Globs must compile.
    for glob in match_globs(ruleset) {
        if glob::Pattern::new(&glob).is_err() {
            errors.push(ValidatorProblem {
                path: path.to_string(),
                problem: format!("invalid match glob '{glob}'"),
            });
        }
    }

    // A review validator matches by changed file, never by a hook-event string;
    // a stray triggerMatcher is a misconfiguration.
    if ruleset.manifest.trigger_matcher.is_some() {
        errors.push(ValidatorProblem {
            path: path.to_string(),
            problem: "stray `triggerMatcher`: review validators match by file, not by event"
                .to_string(),
        });
    }

    // Every declared probe must exist in the engine's probe catalog.
    for probe in &ruleset.manifest.probes {
        if !probe_exists(probe) {
            errors.push(ValidatorProblem {
                path: path.to_string(),
                problem: format!("declared probe '{probe}' is not in the probe catalog"),
            });
        }
    }
}
