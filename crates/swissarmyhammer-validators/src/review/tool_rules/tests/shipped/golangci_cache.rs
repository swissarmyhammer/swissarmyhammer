//! Coverage guard: each shipped script that keeps a golangci-lint cache names
//! that cache from a full digest of the workspace path, and sweeps the
//! directories nothing names any more.
//!
//! `GOLANGCI_LINT_CACHE` gives one workspace one cache directory. Two things
//! rest on the NAME being a pure function of `$PWD`, and one thing rests on
//! it being WIDE.
//!
//! The lock golangci-lint takes stands inside the cache directory, and
//! `allow-serial-runners` makes a second instance wait on it. This set ships
//! two rules that drive golangci-lint over one workspace, so both must reach
//! the same directory for one workspace or neither waits for the other. The
//! cache also answers by package CONTENT and stores each finding with the
//! ABSOLUTE path of the run that first cached it, so two workspaces that
//! reach one directory read each other's paths.
//!
//! The earlier name was `cksum` of `$PWD`, which is a 32-bit checksum beside
//! the byte count. Every temporary workspace of a test run holds the same
//! number of bytes, so the count states nothing and two workspaces meet in
//! one directory at the birthday rate of 32 bits.
//!
//! Nothing removes the directory, on purpose: the lock stands in it, and a
//! warm cache is what lets a run that met one file it could not read still
//! report the findings it made. So the answer to the pile-up is a sweep of
//! the directories that are past golangci-lint's own trim limit.
//!
//! This module reads the SHIPPED script of each rule, so the contract is held
//! for the rules that ship today and for the rules that ship next.

use super::*;

/// The environment variable that hands golangci-lint its cache directory.
const GOLANGCI_CACHE_VARIABLE: &str = "GOLANGCI_LINT_CACHE";

/// The one directory of `TMPDIR` every cache of this set stands under.
pub(super) const GOLANGCI_CACHE_DIRECTORY: &str = "sah-golangci-lint";

/// The line that names that directory.
const CACHE_ROOT_ASSIGNMENT: &str = r#"caches="${TMPDIR:-/tmp}/sah-golangci-lint""#;

/// The line that takes the digest of the working directory.
const CACHE_DIGEST_ASSIGNMENT: &str = r#"digest="$(printf '%s' "$PWD" | shasum -a 256)""#;

/// The line that names the cache directory from that digest.
const CACHE_ASSIGNMENT: &str = r#"cache="$caches/${digest%% *}""#;

/// The command whose 32-bit answer merged two workspaces into one cache
/// directory.
const NARROW_CHECKSUM_COMMAND: &str = "cksum";

/// The line that makes the cache directory, which the sweep lines stand
/// under.
const CACHE_MAKE: &str = r#"mkdir -p "$cache""#;

/// The line that marks the cache directory as used by THIS run, so the sweep
/// reads the last run of the workspace rather than whatever golangci-lint
/// happened to write.
const CACHE_TOUCH: &str = r#"touch "$cache""#;

/// The line that names how old a cache directory has to be for the sweep to
/// take it.
const STALE_DAYS_ASSIGNMENT: &str = "stale_days=5";

/// The line that removes every cache directory past that age.
///
/// `-mindepth 1` keeps the parent directory itself out of the sweep, and the
/// sweep reads that parent rather than `TMPDIR` so it costs one directory
/// read of its own entries.
const CACHE_SWEEP: &str = concat!(
    r#"find "$caches" -mindepth 1 -maxdepth 1 -type d "#,
    r#"-mtime "+$stale_days" -exec rm -rf {} + 2>/dev/null || true"#,
);

/// The lines the contract states directly under [`CACHE_MAKE`], in order.
const CACHE_SWEEP_LINES: &[&str] = &[CACHE_TOUCH, STALE_DAYS_ASSIGNMENT, CACHE_SWEEP];

/// Each tool the cache lines run, as `doctor.check_command` names it.
const CACHE_TOOLS: &[&str] = &["shasum", "mkdir", "touch", "find"];

/// How many shipped rules keep a golangci-lint cache.
///
/// The count is the assertion that a rule added later reaches this guard. A
/// third such rule breaks it, and the author then reads the contract before
/// the rule ships.
const GOLANGCI_CACHE_RULE_COUNT: usize = 2;

/// What the rules of this roster have in common, for the failure message.
const GOLANGCI_CACHE_ROSTER: &str = "keep a golangci-lint cache";

/// Every shipped rule that keeps a golangci-lint cache, or a panic when the
/// set ships another number of them.
fn required_golangci_cache_rules(loader: &ValidatorLoader) -> Vec<ShippedToolRule> {
    required_tool_rules(
        loader,
        GOLANGCI_CACHE_ROSTER,
        GOLANGCI_CACHE_RULE_COUNT,
        |rule| rule.script.contains(GOLANGCI_CACHE_VARIABLE),
    )
}

/// Whether `script` stands its cache under the one parent directory, names it
/// from a full digest of the working directory, and names it from no narrower
/// answer.
fn names_the_cache_from_a_digest(script: &str) -> bool {
    script.contains(CACHE_ROOT_ASSIGNMENT)
        && script.contains(CACHE_DIGEST_ASSIGNMENT)
        && script.contains(CACHE_ASSIGNMENT)
        && !script.contains(NARROW_CHECKSUM_COMMAND)
}

/// Whether each line that makes the cache directory of `script` carries the
/// sweep lines directly under it.
fn sweeps_the_stale_directories(script: &str) -> bool {
    let lines = trimmed_script_lines(script);
    let made = script_lines_that_read(&lines, CACHE_MAKE);
    !made.is_empty()
        && made
            .into_iter()
            .all(|at| script_lines_under(&lines, at, CACHE_SWEEP_LINES))
}

/// Whether `check_command` names each tool of [`CACHE_TOOLS`] as a word of
/// its own.
fn checks_for_the_cache_tools(check_command: Option<&str>) -> bool {
    check_command.is_some_and(|command| {
        let checked: Vec<&str> = command.split_whitespace().collect();
        CACHE_TOOLS.iter().all(|tool| checked.contains(tool))
    })
}

/// Coverage: each shipped script that keeps a golangci-lint cache names that
/// cache from a full digest of the workspace path.
///
/// The name has to separate every workspace from every other one, because the
/// cache answers by package content and hands back the ABSOLUTE path of the
/// run that first cached it. `cksum` is 32 bits beside a byte count, and the
/// count is the same for every temporary workspace of one test run, so two
/// probe workspaces reached one directory at the birthday rate of 2^32.
/// Measured over 200000 temporary-directory paths: the checksum key gave
/// 199996 distinct names, and a sha-256 key gave 200000.
///
/// The digest is taken of `$PWD` and of nothing else, so the two rules of
/// this roster reach the SAME directory for one workspace, which is what
/// makes them share one lock.
#[test]
fn each_shipped_script_that_keeps_a_golangci_lint_cache_names_it_from_a_digest() {
    let loader = builtin_loader();
    let rules = required_golangci_cache_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| names_the_cache_from_a_digest(&rule.script));

    assert!(
        deviating.is_empty(),
        "each script must write `{CACHE_ROOT_ASSIGNMENT}`, `{CACHE_DIGEST_ASSIGNMENT}` \
         and `{CACHE_ASSIGNMENT}`, and must not fall back to \
         `{NARROW_CHECKSUM_COMMAND}`; these rules name their cache another way: \
         {deviating:?}"
    );
}

/// Coverage: each shipped script that keeps a golangci-lint cache sweeps the
/// directories nothing names any more.
///
/// The cache stands between runs on purpose, so the script cannot remove the
/// one it just used, and a run over a temporary workspace leaves a directory
/// behind for ever. Measured on one machine: 6609 such directories, 422804
/// KiB, and none older than three days only because the platform swept them.
///
/// golangci-lint states the lifetime itself. `internal/go/cache/cache.go`
/// sets `trimLimit = 5 * 24 * time.Hour` and drops every entry older than
/// that from a cache it is GIVEN; it never removes a directory nobody names
/// again. So a directory past that age holds nothing the tool would have
/// kept. Measured with `find -mtime +5`: it removed the directories of 6, 7
/// and 30 days and kept those of 0, 1, 4, 5 and 5.5 days, so the cut is at
/// six days — past the tool's own limit, never short of it.
///
/// The three lines stand directly under the line that makes the directory, so
/// no statement between them can exit and leave the sweep unrun, and the
/// `touch` marks this run's own directory before the sweep reads any age.
#[test]
fn each_shipped_script_that_keeps_a_golangci_lint_cache_sweeps_the_stale_ones() {
    let loader = builtin_loader();
    let rules = required_golangci_cache_rules(&loader);

    let deviating =
        tool_rules_that_deviate(&rules, |rule| sweeps_the_stale_directories(&rule.script));

    assert!(
        deviating.is_empty(),
        "`{CACHE_TOUCH}`, `{STALE_DAYS_ASSIGNMENT}` and `{CACHE_SWEEP}` must stand \
         directly under `{CACHE_MAKE}`; these rules leave a cache directory behind for \
         each workspace, for ever: {deviating:?}"
    );
}

/// Coverage: each shipped script that keeps a golangci-lint cache names the
/// tools it keeps the cache with for the doctor.
///
/// `check_command` alone decides whether a rule is usable, and it names every
/// tool the script runs. A machine without one of these breaks the run while
/// the doctor reports the rule ready.
#[test]
fn each_shipped_script_that_keeps_a_golangci_lint_cache_checks_for_its_tools() {
    let loader = builtin_loader();
    let rules = required_golangci_cache_rules(&loader);

    let deviating = tool_rules_that_deviate(&rules, |rule| {
        checks_for_the_cache_tools(rule.check_command.as_deref())
    });

    assert!(
        deviating.is_empty(),
        "`doctor.check_command` must name {CACHE_TOOLS:?}, the tools the script runs to \
         name and sweep its cache; these rules leave one unnamed: {deviating:?}"
    );
}
