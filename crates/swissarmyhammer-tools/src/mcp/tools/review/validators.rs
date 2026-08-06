//! Validator introspection ops for the `review` tool:
//! `list/dump/get/check validators`.
//!
//! These ops are loader reads — no agent, no index, fast. They load the full
//! builtin → user → project RuleSet stack via
//! [`swissarmyhammer_validators::load_rules`] and report on it:
//!
//! - `list validators` — one summary row per loaded RuleSet (name, description,
//!   source layer, match globs, probes, rule count, path), optionally
//!   filtered by `source` and/or a path/glob `match`, and optionally carrying
//!   every rule's verbatim body (`rules: true`).
//! - `dump validators` — write every rule the engine enforces on a set of
//!   paths to ONE markdown file in the system temp directory, deduplicated
//!   across paths, and return the file path. One call with one example file
//!   per extension answers "what will a review enforce on the files I will
//!   edit?" in one readable file.
//! - `get validator` — one RuleSet's full rule bodies + probes.
//! - `check validators` — lint every loaded RuleSet: frontmatter is valid (it
//!   parsed), declared globs compile, no stray `triggerMatcher`, and every
//!   declared probe exists in the engine's probe catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use swissarmyhammer_validators::review::probe_exists;
use swissarmyhammer_validators::validators::{
    compile_glob_patterns, matches_any_pattern, MatchContext, ToolSpec, ValidatorLoader,
    ValidatorMatch,
};
use swissarmyhammer_validators::{load_rules, workspace_project_types, AvpError, RuleSet};

use crate::mcp::op_tool_helpers::{forgiving_string_list, is_glob_pattern};

/// Errors the validator introspection ops (`list/dump/get/check validators`)
/// return.
///
/// The op error messages live here, once — a caller renders them with
/// `Display`, and no per-call-site `format!` copy can drift.
#[derive(Debug, thiserror::Error)]
pub enum ValidatorOpError {
    /// The RuleSet stack failed to load.
    #[error("failed to load validators: {0}")]
    Load(#[from] AvpError),
    /// No loaded RuleSet carries the requested name.
    #[error("no validator named '{0}'")]
    UnknownValidator(String),
    /// The `paths` input has a shape the op does not accept.
    #[error(
        "`paths` must be a file path string, an array of file path strings, or a \
         stringified JSON array of file path strings"
    )]
    MalformedPaths,
    /// The `paths` input holds no path.
    #[error("`dump validators` requires at least one path in `paths`")]
    EmptyPaths,
    /// The rules file failed to write.
    #[error("failed to write {path}: {source}")]
    WriteRulesFile {
        /// The temp-dir file path the write targeted.
        path: PathBuf,
        /// The io error the write returned.
        source: std::io::Error,
    },
}

/// A `list validators` summary row.
#[derive(Clone, Debug, Serialize)]
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
#[derive(Clone, Debug, Serialize)]
pub struct RuleDetail {
    /// The rule name.
    pub name: String,
    /// The rule's markdown body verbatim.
    pub body: String,
    /// The prompt rule this tool rule replaces when its tool is healthy.
    /// Absent on prompt rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// The tool block. Present only on tool rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolSpec>,
}

/// A `get validator` response — one RuleSet's full detail.
#[derive(Clone, Debug, Serialize)]
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
#[derive(Clone, Debug, Serialize)]
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
#[derive(Clone, Debug, Serialize)]
pub struct ValidatorProblem {
    /// The RuleSet path (or name) the problem is about.
    pub path: String,
    /// What is wrong.
    pub problem: String,
}

/// A `check validators` response: overall `ok` plus every problem found.
#[derive(Clone, Debug, Serialize)]
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
            supersedes: rule.supersedes.clone(),
            tool: rule.tool.clone(),
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

/// The names of the RuleSets the engine pairs with one concrete file path in a
/// workspace carrying `project_types`.
///
/// This is the engine's own matcher — a [`MatchContext`] carrying the file and
/// the workspace's detected project type keys, run through
/// [`ValidatorLoader::matching_rulesets`] — which is exactly how `scope_review`
/// pairs each changed file with its validators. Answering a path-shaped `match`
/// through it means the tool cannot drift from the set a review run will
/// actually enforce on that file.
fn engine_matched_names(
    loader: &ValidatorLoader,
    path: &str,
    project_types: &[String],
) -> BTreeSet<String> {
    let ctx = MatchContext::new()
        .with_file(path.to_string())
        .with_project_types(project_types.iter().cloned());
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
/// `workspace_root` is the session working directory's root — never the process
/// current directory. It resolves the project types a path-shaped
/// `match_filter` is answered against; a `None` root fails closed, so a
/// `project_types`-keyed RuleSet does not match.
///
/// # Errors
///
/// Returns [`ValidatorOpError::Load`] when [`load_rules`] fails (user/project
/// directory read error).
pub fn list_validators(
    source: Option<&str>,
    match_filter: Option<&str>,
    include_rules: bool,
    workspace_root: Option<&Path>,
) -> Result<Vec<ValidatorSummary>, ValidatorOpError> {
    let loader = load_rules()?;
    // A path-shaped `match` is answered by the engine matcher, once for the whole
    // stack rather than per row.
    let project_types = workspace_project_types(workspace_root);
    let engine_matched = match_filter
        .filter(|needle| !is_glob_pattern(needle))
        .map(|path| engine_matched_names(&loader, path, &project_types));

    let mut summaries: Vec<ValidatorSummary> = loader
        .list_rulesets()
        .into_iter()
        .filter(|rs| passes_filters(rs, source, match_filter, engine_matched.as_ref()))
        .map(|rs| summary(rs, include_rules))
        .collect();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

/// A `dump validators` response: where the rules file landed plus what it holds.
#[derive(Clone, Debug, Serialize)]
pub struct DumpValidatorsResponse {
    /// The markdown rules file, written under the system temp directory.
    pub path: String,
    /// The deduplicated, sorted names of every validator any input path matched.
    pub validators: Vec<String>,
    /// The total rule count across the deduplicated validators.
    pub rule_count: usize,
    /// Each input path paired with the sorted validator names the engine
    /// matcher pairs it with (empty when no validator matches the path).
    pub matched: BTreeMap<String, Vec<String>>,
    /// The distinct, sorted extensions of the input paths. A path with no
    /// extension adds no entry.
    pub extensions: Vec<String>,
}

/// Parse the forgiving `paths` input and drop blank entries.
///
/// Shape tolerance comes from [`forgiving_string_list`]: an array of strings,
/// a stringified JSON array, and a single string all yield the same list. An
/// entry that is empty or whitespace carries no path, so it is dropped — a
/// blank-only input therefore parses to an empty list, which
/// [`dump_validators`] rejects as empty.
///
/// # Errors
///
/// Returns [`ValidatorOpError::MalformedPaths`] when the value has none of
/// those shapes, or when an array element is not a string.
fn path_list(value: &serde_json::Value) -> Result<Vec<String>, ValidatorOpError> {
    let paths = forgiving_string_list(value).ok_or(ValidatorOpError::MalformedPaths)?;
    Ok(paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .collect())
}

/// The distinct extensions of the input paths, sorted.
///
/// A path with no extension adds no entry.
fn distinct_extensions(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| Path::new(path).extension())
        .map(|extension| extension.to_string_lossy().to_string())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// A unique rules-file path under the system temp directory.
///
/// Never the CWD: bundled GUI apps start with a read-only CWD.
fn rules_file_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "sah-rules-{}.md",
        swissarmyhammer_common::generate_monotonic_ulid_string()
    ))
}

/// Render the dumped RuleSets as one markdown document.
///
/// Each validator gets a heading with its name, then its description and source
/// layer, then each rule's name, the prompt rule it supersedes (when the rule
/// declares one), and its verbatim body. An empty set states that no rules
/// apply.
fn render_rules_markdown(rulesets: &[&RuleSet], extensions: &[String]) -> String {
    let mut doc = String::from("# Review rules\n\n");
    if !extensions.is_empty() {
        doc.push_str(&format!("Extensions: {}\n\n", extensions.join(", ")));
    }
    if rulesets.is_empty() {
        doc.push_str("No validator matches the given paths. No rules apply.\n");
        return doc;
    }
    for ruleset in rulesets {
        doc.push_str(&format!(
            "## {}\n\n{}\n\nSource layer: {}\n\n",
            ruleset.name(),
            ruleset.description(),
            ruleset.source
        ));
        for rule in rule_details(ruleset) {
            doc.push_str(&format!("### {}\n\n", rule.name));
            if let Some(supersedes) = &rule.supersedes {
                doc.push_str(&format!("Supersedes: {supersedes}\n\n"));
            }
            doc.push_str(&format!("{}\n\n", rule.body));
        }
    }
    doc
}

/// `dump validators`: write every rule the engine enforces on `paths` to one
/// markdown file in the system temp directory, and return its path.
///
/// Each input path runs through the engine's own matcher
/// ([`engine_matched_names`]) — the same pairing `scope_review` and
/// `list validators` use — so the dumped set cannot differ from what a review
/// run enforces. The validator set is deduplicated across paths: rules match by
/// file pattern, so one example file per distinct extension gives the full set.
///
/// `workspace_root` is the session working directory's root — never the process
/// current directory. Its detected project types are resolved once for the whole
/// call, so a `project_types`-keyed RuleSet (the shape every tool rule uses)
/// reaches the dump in a matching workspace. A `None` root fails closed.
///
/// # Errors
///
/// Returns a [`ValidatorOpError`] when `paths` is empty or malformed, when
/// [`load_rules`] fails, or when the file cannot be written.
pub fn dump_validators(
    paths_value: &serde_json::Value,
    workspace_root: Option<&Path>,
) -> Result<DumpValidatorsResponse, ValidatorOpError> {
    let paths = path_list(paths_value)?;
    if paths.is_empty() {
        return Err(ValidatorOpError::EmptyPaths);
    }

    let loader = load_rules()?;
    let project_types = workspace_project_types(workspace_root);

    let mut matched: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for path in &paths {
        let path_names = engine_matched_names(&loader, path, &project_types);
        names.extend(path_names.iter().cloned());
        matched.insert(path.clone(), path_names.into_iter().collect());
    }

    let rulesets: Vec<&RuleSet> = names
        .iter()
        .filter_map(|name| loader.get_ruleset(name))
        .collect();
    let rule_count = rulesets.iter().map(|ruleset| ruleset.rules.len()).sum();
    let extensions = distinct_extensions(&paths);

    let file_path = rules_file_path();
    std::fs::write(&file_path, render_rules_markdown(&rulesets, &extensions)).map_err(
        |source| ValidatorOpError::WriteRulesFile {
            path: file_path.clone(),
            source,
        },
    )?;

    Ok(DumpValidatorsResponse {
        path: file_path.display().to_string(),
        validators: names.into_iter().collect(),
        rule_count,
        matched,
        extensions,
    })
}

/// `get validator`: load the stack and return one RuleSet's full detail.
///
/// # Errors
///
/// Returns [`ValidatorOpError::Load`] when [`load_rules`] fails, and
/// [`ValidatorOpError::UnknownValidator`] when no RuleSet is named `name`.
pub fn get_validator(name: &str) -> Result<ValidatorDetail, ValidatorOpError> {
    let loader = load_rules()?;
    let ruleset = loader
        .get_ruleset(name)
        .ok_or_else(|| ValidatorOpError::UnknownValidator(name.to_string()))?;

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
/// Returns [`ValidatorOpError::Load`] when [`load_rules`] fails.
pub fn check_validators() -> Result<CheckValidatorsResponse, ValidatorOpError> {
    let loader = load_rules()?;
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

/// Report every glob in `globs` that does not compile.
///
/// `context` prefixes the problem message (empty for a set-level glob, the
/// rule name for a rule-level narrowing `match`), so the set loop and the
/// per-rule loop share one validation body.
fn validate_glob_patterns(
    globs: &[impl AsRef<str>],
    path: &str,
    context: &str,
    errors: &mut Vec<ValidatorProblem>,
) {
    for glob_item in globs {
        let glob = glob_item.as_ref();
        if glob::Pattern::new(glob).is_err() {
            errors.push(ValidatorProblem {
                path: path.to_string(),
                problem: format!("{context}invalid match glob '{glob}'"),
            });
        }
    }
}

/// Lint one RuleSet, appending any problems found.
fn lint_ruleset(ruleset: &RuleSet, path: &str, errors: &mut Vec<ValidatorProblem>) {
    // Globs must compile — the set's and every rule-level narrowing `match`.
    validate_glob_patterns(&match_globs(ruleset), path, "", errors);
    for rule in &ruleset.rules {
        let rule_globs = rule
            .match_criteria
            .as_ref()
            .map(|criteria| criteria.files.as_slice())
            .unwrap_or_default();
        validate_glob_patterns(rule_globs, path, &format!("rule '{}': ", rule.name), errors);
    }

    // A rule file that failed to parse (e.g. a malformed `tool` block) was
    // skipped, never dropped silently: report each one here.
    for failure in &ruleset.rule_failures {
        errors.push(ValidatorProblem {
            path: failure.path.display().to_string(),
            problem: format!("failed to parse rule: {}", failure.error),
        });
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
