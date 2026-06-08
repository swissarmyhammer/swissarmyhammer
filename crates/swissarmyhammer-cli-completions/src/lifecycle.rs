//! Shared lifecycle-CLI scaffolding for SwissArmyHammer tool CLIs.
//!
//! Every standalone tool CLI built on the `Operation` trait (`code-context`,
//! `shelltool`, `kanban`) wraps its schema-driven `noun → verb` operation tree
//! in the same fixed set of lifecycle subcommands — `serve`, `init`, `deinit`,
//! `doctor`, and `completion` — plus the shared `[TARGET]` install argument and
//! a `completion` dispatcher. Before this module each binary carried a
//! byte-identical copy of every one of those builders, differing only in the
//! binary name and `about` text. This module centralises them so each CLI
//! passes only its strings.
//!
//! # The install target
//!
//! [`InstallTarget`] is the canonical typed representation of the
//! `project | local | user` scope vocabulary. It is a clap [`ValueEnum`], so the
//! valid value set and its mapping to [`InitScope`] are single-sourced on the
//! enum — the clap `value_parser` and the scope conversion both derive from it,
//! and there is no stringly-typed match that can panic on an unhandled arm.

use clap::{Arg, ArgAction, Command, ValueEnum};
use clap_complete::Shell;
use serde_json::Value;
use swissarmyhammer_common::lifecycle::InitScope;

/// Where a tool CLI installs or removes its configuration.
///
/// The single source of truth for the `project | local | user` scope set shared
/// by every tool CLI's `init`/`deinit` subcommands. Being a clap [`ValueEnum`]
/// lets the install-target argument derive both its accepted values and its
/// default from this type, and the [`From<InstallTarget>`] impl maps each
/// variant to the lifecycle [`InitScope`] with no fallible arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum InstallTarget {
    /// Project-level configuration (committed to the repo).
    Project,
    /// Local project configuration that is not committed.
    Local,
    /// User-wide (global) configuration.
    User,
}

impl Default for InstallTarget {
    /// Delegates to [`InstallTarget::DEFAULT`] so the standard-trait default and
    /// the const default stay single-sourced.
    fn default() -> Self {
        InstallTarget::DEFAULT
    }
}

impl InstallTarget {
    /// The default scope for a bare `<tool> init` / `<tool> deinit`.
    ///
    /// Used as the install-target argument's clap default. `Default::default()`
    /// is not usable in const contexts (it is the install-target arg's
    /// `default_value`), so this const is the single source of truth and the
    /// [`Default`] impl above delegates to it.
    pub const DEFAULT: InstallTarget = InstallTarget::Project;

    /// The clap value-parser token for this variant (e.g. `"project"`).
    ///
    /// This is the same token clap's `ValueEnum` derive accepts, so it can be
    /// fed back to the parser as a default value.
    pub const fn as_str(self) -> &'static str {
        match self {
            InstallTarget::Project => "project",
            InstallTarget::Local => "local",
            InstallTarget::User => "user",
        }
    }
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<InstallTarget> for InitScope {
    fn from(target: InstallTarget) -> Self {
        match target {
            InstallTarget::Project => InitScope::Project,
            InstallTarget::Local => InitScope::Local,
            InstallTarget::User => InitScope::User,
        }
    }
}

/// The shared `[TARGET]` positional argument used by `init` and `deinit`.
///
/// Restricts inputs to the three [`InstallTarget`] variants (via clap's
/// `ValueEnum` parser) and defaults to [`InstallTarget::Project`] so a bare
/// `<tool> init` installs into the current project. The accepted value set and
/// the default both derive from [`InstallTarget`], so there is exactly one place
/// to change a scope.
pub fn install_target_arg(help: &'static str) -> Arg {
    Arg::new("target")
        .help(help)
        .value_parser(clap::builder::EnumValueParser::<InstallTarget>::new())
        .default_value(InstallTarget::DEFAULT.as_str())
}

/// Read the install [`InstallTarget`] from a lifecycle subcommand's matches and
/// map it to the lifecycle [`InitScope`].
///
/// The argument is built by [`install_target_arg`], whose clap default
/// guarantees a value is always present, so this never falls back.
pub fn target_scope(matches: &clap::ArgMatches) -> InitScope {
    matches
        .get_one::<InstallTarget>("target")
        .copied()
        .expect("install_target_arg sets a clap default, so target is always present")
        .into()
}

/// `serve` — run the MCP server over stdio.
///
/// `about` is the per-tool one-line summary (e.g. `"Run MCP server over stdio,
/// exposing kanban tools"`).
pub fn serve_subcommand(about: &'static str) -> Command {
    Command::new("serve").about(about)
}

/// `init` — install the tool's MCP server into detected agent configs.
///
/// `about` is the per-tool one-line summary; `target_help` is the help text for
/// the shared `[TARGET]` argument.
pub fn init_subcommand(about: &'static str, target_help: &'static str) -> Command {
    Command::new("init")
        .about(about)
        .arg(install_target_arg(target_help))
}

/// `deinit` — remove the tool's MCP server from detected agent configs.
///
/// `about` is the per-tool one-line summary; `target_help` is the help text for
/// the shared `[TARGET]` argument.
pub fn deinit_subcommand(about: &'static str, target_help: &'static str) -> Command {
    Command::new("deinit")
        .about(about)
        .arg(install_target_arg(target_help))
}

/// `doctor` — diagnose the tool's setup with optional verbose output.
///
/// `about` is the per-tool one-line summary.
pub fn doctor_subcommand(about: &'static str) -> Command {
    Command::new("doctor").about(about).arg(
        Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Show detailed output including fix suggestions")
            .action(ArgAction::SetTrue),
    )
}

/// `completion` — emit a shell completion script for the given binary.
///
/// The multi-shell `long_about` help block is templated on `bin_name`, so a
/// wording fix lands in one place for every CLI. The positional `shell`
/// argument is restricted to the four shells `clap_complete` knows how to
/// render.
pub fn completion_subcommand(bin_name: &str) -> Command {
    let long_about = format!(
        "Generates shell completion scripts for various shells. Supports:\n\
         - bash\n\
         - zsh\n\
         - fish\n\
         - powershell\n\n\
         Examples:\n  \
         # Bash (add to ~/.bashrc or ~/.bash_profile)\n  \
         {bin} completion bash > ~/.local/share/bash-completion/completions/{bin}\n\n  \
         # Zsh (add to ~/.zshrc or a file in fpath)\n  \
         {bin} completion zsh > ~/.zfunc/_{bin}\n\n  \
         # Fish\n  \
         {bin} completion fish > ~/.config/fish/completions/{bin}.fish\n\n  \
         # PowerShell\n  \
         {bin} completion powershell >> $PROFILE",
        bin = bin_name,
    );

    Command::new("completion")
        .about("Generate shell completion scripts")
        .long_about(long_about)
        .arg(
            Arg::new("shell")
                .help("Shell to generate completion for")
                .required(true)
                .value_parser(clap::builder::EnumValueParser::<Shell>::new()),
        )
}

/// Generate a shell completion script for `cmd` and return a process exit code.
///
/// Reads the required `shell` argument from `matches` (built by
/// [`completion_subcommand`]) and renders the fully-assembled runtime command
/// tree `cmd` via [`crate::print_completion_for`], registered under `bin_name`.
/// Returns 0 on success, 1 if the completion writer fails.
pub fn run_completion(cmd: Command, bin_name: &str, matches: &clap::ArgMatches) -> i32 {
    let shell = matches
        .get_one::<Shell>("shell")
        .copied()
        .expect("clap enforces a required shell argument");

    match crate::print_completion_for(cmd, bin_name, shell) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}

/// Build one of the global boolean flags shared across all subcommands.
///
/// Centralises the `Arg::new(id).long(long).help(help).global(true)
/// .action(SetTrue)` construction every tool CLI's `build_cli` open-coded for
/// `--debug` (and code-context's `--json`/`--no-progress`). `id` is the clap arg
/// id the dispatcher reads (e.g. `no_progress`); `long` is the rendered `--flag`
/// spelling (e.g. `no-progress`); `short` is the optional single-character alias.
pub fn global_flag(id: &'static str, long: &'static str, short: Option<char>, help: &str) -> Arg {
    let mut arg = Arg::new(id)
        .long(long)
        .help(help.to_string())
        .global(true)
        .action(ArgAction::SetTrue);
    if let Some(c) = short {
        arg = arg.short(c);
    }
    arg
}

/// Assemble the standard root command shared by every schema-driven tool CLI.
///
/// This is the invariant skeleton each tool CLI's `build_cli` previously
/// hand-copied: a root [`Command`] named `name` with `version`/`about`, a global
/// `--debug` flag, `allow_external_subcommands(true)`, the schema-driven
/// `noun → verb` operation subcommands (via
/// [`swissarmyhammer_operations::cli_gen::build_commands_from_schema`]), and the
/// five lifecycle subcommands `serve`/`init`/`deinit`/`doctor`/`completion`.
///
/// Each CLI calls this with only its strings/schema, then appends its own
/// genuinely per-binary pieces — extra global flags (code-context's
/// `--json`/`--no-progress`) and app-specific subcommands (kanban's
/// `open`/`merge`, code-context's `skill`).
///
/// The version is sourced internally from this crate's `CARGO_PKG_VERSION`,
/// which matches every workspace binary, so callers do not thread a version
/// string through. The lifecycle subcommand `about`/help strings are derived
/// from `name` so they stay consistent across CLIs while still naming the right
/// binary.
pub fn standard_op_cli(name: &'static str, about: &'static str, schema: &Value) -> Command {
    // Every workspace CLI shares the workspace-level version, so this crate's
    // own `CARGO_PKG_VERSION` is identical to each binary's — no need to thread
    // a version string through every caller.
    let mut cmd = Command::new(name)
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
        .arg(global_flag(
            "debug",
            "debug",
            Some('d'),
            "Enable debug output to stderr",
        ))
        .allow_external_subcommands(true);

    for subcmd in swissarmyhammer_operations::cli_gen::build_commands_from_schema(schema) {
        cmd = cmd.subcommand(subcmd);
    }

    cmd.subcommand(serve_subcommand(intern(format!(
        "Run MCP server over stdio, exposing {name} tools"
    ))))
    .subcommand(init_subcommand(
        intern(format!(
            "Install {name} MCP server into detected agent configs"
        )),
        "Where to install the server configuration",
    ))
    .subcommand(deinit_subcommand(
        intern(format!(
            "Remove {name} MCP server from detected agent configs"
        )),
        "Where to remove the server configuration from",
    ))
    .subcommand(doctor_subcommand(intern(format!(
        "Diagnose {name} configuration and setup"
    ))))
    .subcommand(completion_subcommand(name))
}

/// Leak an owned string into a `&'static str`.
///
/// The lifecycle subcommand builders take `&'static str` `about` strings, but
/// [`standard_op_cli`] derives them from the runtime `name`. Leaking the small,
/// bounded set of per-CLI about strings (one per binary, built once at startup)
/// keeps the builders' signatures unchanged without threading owned strings
/// through them.
fn intern(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `InstallTarget` maps each variant to the matching `InitScope` with no
    /// fallible arm — the typed conversion replaces the stringly match the
    /// per-binary copies used.
    #[test]
    fn install_target_maps_to_init_scope() {
        assert_eq!(InitScope::from(InstallTarget::Project), InitScope::Project);
        assert_eq!(InitScope::from(InstallTarget::Local), InitScope::Local);
        assert_eq!(InitScope::from(InstallTarget::User), InitScope::User);
    }

    /// `as_str` must agree with the clap `ValueEnum` token for every variant,
    /// so feeding `as_str` back as the default value round-trips through the
    /// parser. If they diverge, the default would be rejected at parse time.
    #[test]
    fn as_str_matches_value_enum_token() {
        for target in [
            InstallTarget::Project,
            InstallTarget::Local,
            InstallTarget::User,
        ] {
            let token = target
                .to_possible_value()
                .expect("every variant has a possible value")
                .get_name()
                .to_string();
            assert_eq!(target.as_str(), token);
        }
    }

    /// `Default::default()` agrees with the `DEFAULT` const so the standard
    /// trait and the const default stay single-sourced.
    #[test]
    fn default_matches_default_const() {
        assert_eq!(InstallTarget::default(), InstallTarget::DEFAULT);
        assert_eq!(InstallTarget::default(), InstallTarget::Project);
    }

    /// A bare invocation (no `target` given) resolves to `Project` via the clap
    /// default baked into `install_target_arg`.
    #[test]
    fn target_scope_defaults_to_project() {
        let cmd = init_subcommand("install", "where");
        let matches = cmd.try_get_matches_from(["init"]).unwrap();
        assert_eq!(target_scope(&matches), InitScope::Project);
    }

    /// An explicit `local` target resolves through the typed parser.
    #[test]
    fn target_scope_reads_explicit_target() {
        let cmd = init_subcommand("install", "where");
        let matches = cmd.try_get_matches_from(["init", "local"]).unwrap();
        assert_eq!(target_scope(&matches), InitScope::Local);
    }

    /// The install-target argument rejects values outside the typed enum.
    #[test]
    fn install_target_arg_rejects_unknown_value() {
        let cmd = init_subcommand("install", "where");
        let result = cmd.try_get_matches_from(["init", "bogus"]);
        assert!(result.is_err(), "unknown target should be rejected");
    }

    /// The lifecycle subcommand builders carry the per-tool about strings.
    #[test]
    fn lifecycle_subcommands_use_supplied_about() {
        assert_eq!(serve_subcommand("serve about").get_name(), "serve");
        assert_eq!(init_subcommand("init about", "h").get_name(), "init");
        assert_eq!(deinit_subcommand("deinit about", "h").get_name(), "deinit");
        assert_eq!(doctor_subcommand("doctor about").get_name(), "doctor");
    }

    /// The completion subcommand templates its examples on the binary name.
    #[test]
    fn completion_subcommand_templates_binary_name() {
        let cmd = completion_subcommand("widget");
        let long_about = cmd.get_long_about().unwrap().to_string();
        assert!(
            long_about.contains("widget completion bash"),
            "long_about should mention the binary name; got: {long_about}"
        );
        assert!(
            long_about.contains("completions/widget"),
            "long_about should template the install path on the binary name"
        );
    }

    /// `run_completion` renders a script for the assembled command and returns 0.
    #[test]
    fn run_completion_succeeds() {
        let root = Command::new("widget").subcommand(completion_subcommand("widget"));
        let matches = root
            .clone()
            .try_get_matches_from(["widget", "completion", "bash"])
            .unwrap();
        let (_, sub_m) = matches.subcommand().unwrap();
        assert_eq!(run_completion(root, "widget", sub_m), 0);
    }

    /// `global_flag` builds a global `SetTrue` boolean flag whose id is read by
    /// `get_flag`, with the optional short alias wired when supplied.
    #[test]
    fn global_flag_builds_global_set_true_flag() {
        let cmd = Command::new("widget")
            .arg(global_flag("debug", "debug", Some('d'), "Enable debug"))
            .subcommand(Command::new("sub"));
        // The flag id is read via get_flag regardless of position.
        let matches = cmd
            .clone()
            .try_get_matches_from(["widget", "sub", "--debug"])
            .unwrap();
        assert!(matches.get_flag("debug"));
        // The short alias also sets the flag.
        let matches = cmd.try_get_matches_from(["widget", "-d", "sub"]).unwrap();
        assert!(matches.get_flag("debug"));
    }

    /// A minimal schema with one `verb noun` op, mirroring the shape every tool
    /// CLI hands `standard_op_cli`.
    fn minimal_schema() -> Value {
        serde_json::json!({
            "properties": { "op": { "enum": ["list things"] } },
            "x-operation-schemas": [
                { "title": "list things", "description": "List things",
                  "properties": {}, "required": [] }
            ]
        })
    }

    /// `standard_op_cli` attaches the five lifecycle subcommands, the global
    /// `--debug` flag, and the schema-driven noun subcommand.
    #[test]
    fn standard_op_cli_has_lifecycle_and_schema_commands() {
        let schema = minimal_schema();
        let cmd = standard_op_cli("widget", "A widget", &schema);
        let names: std::collections::HashSet<&str> =
            cmd.get_subcommands().map(|c| c.get_name()).collect();
        for name in ["serve", "init", "deinit", "doctor", "completion", "things"] {
            assert!(names.contains(name), "standard_op_cli missing: {name}");
        }
        // The global --debug flag parses regardless of subcommand.
        let matches = cmd
            .try_get_matches_from(["widget", "--debug", "things", "list"])
            .unwrap();
        assert!(matches.get_flag("debug"));
    }

    /// The lifecycle `about` strings derived from `name` name the right binary.
    #[test]
    fn standard_op_cli_about_strings_name_the_binary() {
        let schema = minimal_schema();
        let cmd = standard_op_cli("widget", "A widget", &schema);
        let serve = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "serve")
            .unwrap();
        assert!(serve
            .get_about()
            .map(|a| a.to_string().contains("widget"))
            .unwrap_or(false));
    }
}
