use super::*;

use crate::doctor::{check_presence, ToolPresence};
use crate::review::tool_install::install_tool_commands;
use crate::validators::types::{FixHint, ToolDoctor, ToolInstall};

/// The command every shipped doctor check asks presence with.
const WHICH_COMMAND: &str = "which";

/// The shell words that end a `which` argument list inside a doctor check
/// command.
const SHELL_SEPARATORS: &[&str] = &["&&", "||", ";", "|"];

/// What separates the install commands a failure message offers, in order
/// of preference.
const REMEDY_SEPARATOR: &str = "  OR  ";

/// What introduces the one line of a precondition failure a reader can
/// paste into a shell.
const RUNNABLE_PREFIX: &str = "run: ";

/// What introduces the `doctor.fix_hint` line, which a reader cannot paste
/// into a shell.
const ADVICE_PREFIX: &str = "advice for a person, not a command to run: ";

/// The binaries a doctor check command asks `which` for.
///
/// [`check_presence`] is all-or-nothing over the whole
/// `doctor.check_command`: one nonzero status reports the tool missing,
/// whichever word of `which cargo-clippy jq` failed. Reading the names back
/// out of the command is what lets a failure message state the binary that
/// actually failed rather than the rule's headline tool.
fn checked_binaries(check_command: &str) -> Vec<&str> {
    let mut binaries = Vec::new();
    let mut reading = false;
    for word in check_command.split_whitespace() {
        match word {
            WHICH_COMMAND => reading = true,
            _ if SHELL_SEPARATORS.contains(&word) => reading = false,
            _ if reading => binaries.push(word),
            _ => {}
        }
    }
    binaries
}

/// Whether one binary is on this machine's `PATH`.
///
/// The name rides in as the script's one positional parameter, never inside
/// the command string. [`checked_binaries`] reads a name back out of a rule's
/// `doctor.check_command` by splitting on whitespace, so the name can carry
/// any character a shell reads specially; a name inside the string would be
/// shell syntax rather than an argument to `which`.
fn binary_present(binary: &str) -> bool {
    crate::doctor::run_shell(
        &format!("{WHICH_COMMAND} \"$@\""),
        None,
        &[OsStr::new(binary)],
    )
    .is_ok_and(|output| output.status.success())
}

/// What a precondition failure names as missing: the binaries the check
/// command asks for and this machine does not have.
///
/// A check command can also fail on something that is not a binary —
/// `dead-code-swift` asks `test -f Package.swift` beside its `which` — so a
/// check whose binaries are all present is named by the command itself.
fn missing_label(check_command: &str) -> String {
    let absent: Vec<&str> = checked_binaries(check_command)
        .into_iter()
        .filter(|binary| !binary_present(binary))
        .collect();
    match absent.is_empty() {
        true => format!("what `{check_command}` checks for"),
        false => absent.join(", "),
    }
}

/// The failure a tool-rule test reports when the rule's declared install
/// commands could not provide the tool.
///
/// The runnable install commands and the advisory `doctor.fix_hint` are
/// separate lines. A hint is prose a person reads — `dead-code-swift`
/// states `brew install periphery, and run the review from the directory
/// holding Package.swift` — so a reader who pastes what follows `run:`
/// into a shell must never get a hint.
fn precondition_report(rule_name: &str, spec: &ToolSpec, detail: &str) -> String {
    let check_command = spec
        .doctor
        .as_ref()
        .map(|doctor| doctor.check_command.as_str())
        .unwrap_or_default();
    let mut report = vec![
        format!(
            "`{rule_name}` needs {missing}, and the rule's install commands did not \
             provide it, so this test cannot run.",
            missing = missing_label(check_command)
        ),
        format!("the doctor check `{check_command}` reported: {detail}"),
    ];
    let install_commands = spec
        .install
        .as_ref()
        .map(|install| install.commands.as_slice())
        .unwrap_or_default();
    if !install_commands.is_empty() {
        report.push(format!(
            "{RUNNABLE_PREFIX}{}",
            install_commands.join(REMEDY_SEPARATOR)
        ));
    }
    if let Some(hint) = spec
        .doctor
        .as_ref()
        .and_then(|doctor| doctor.fix_hint.as_ref())
    {
        report.push(format!("{ADVICE_PREFIX}{hint}"));
    }
    report.join("\n")
}

/// Gives a tool-rule acceptance test the tool it needs, through the rule's
/// own declared install commands.
///
/// The commands are the rule's, never the test's:
/// [`install_tool_commands`] reads `tool.install.commands` from the rule's
/// frontmatter, returns at once when the doctor check already passes, and
/// holds an exclusive lock while it runs, so two test processes that share a
/// temporary directory never write one destination together.
/// [`install_tool_commands`] states how far that lock reaches.
///
/// A tool the commands cannot provide fails the test, naming the binary
/// that actually failed the check. `check_presence` is all-or-nothing over
/// the whole `doctor.check_command`, so the rule's headline tool is the
/// wrong thing to print: `which cargo-clippy jq` failing on `jq` must not
/// ask for `rustup component add clippy`. The runnable install commands and
/// the advisory `doctor.fix_hint` are printed apart, because a hint is
/// prose a person reads and pasting it into a shell fails.
pub(super) fn require_tool_installed(
    loader: &ValidatorLoader,
    project_types: &[&str],
    rule_name: &str,
) {
    let matched = project_tool_rules(loader, project_types)
        .into_iter()
        .find(|matched| matched.rule.name == rule_name)
        .unwrap_or_else(|| {
            panic!("`{rule_name}` must be a shipped tool rule for {project_types:?}")
        });
    if install_tool_commands(matched.spec).tool_present() {
        return;
    }
    let ToolPresence::Missing { detail } = check_presence(matched.spec) else {
        return;
    };
    panic!("{}", precondition_report(rule_name, matched.spec, &detail));
}

/// A binary name no machine carries, for the presence tests below.
const ABSENT_BINARY: &str = "no-such-tool-a4ebnw3";

/// The precondition failure names the binary that actually failed, not
/// every binary the check command lists.
///
/// `check_presence` reports one status for the whole `doctor.check_command`,
/// so a check that names two binaries and fails on the second would
/// otherwise be reported against the first.
#[test]
fn the_precondition_failure_names_the_binary_that_actually_failed() {
    let check_command = format!("{WHICH_COMMAND} sh {ABSENT_BINARY}");

    let missing = missing_label(&check_command);

    assert_eq!(missing, ABSENT_BINARY);
}

/// A binary name read out of a check command is data, never shell syntax.
///
/// [`checked_binaries`] splits `doctor.check_command` on whitespace, so a word
/// it hands back can carry any character a shell reads specially. A name put
/// inside a command string would redirect, expand, or run a command of its
/// own; a name passed as a positional argument cannot.
#[test]
fn a_checked_binary_name_reaches_which_as_one_argument() {
    let temp = tempfile::tempdir().expect("temp dir");
    let written = temp.path().join("written-by-the-shell");
    let binary = format!("{ABSENT_BINARY}>{}", written.display());

    assert!(!binary_present(&binary), "no machine carries that binary");

    assert!(
        !written.exists(),
        "the name must reach `which` as one argument; the shell read it as a redirect instead"
    );
}

/// A check command whose binaries are all present failed on something else,
/// so the message names the command rather than a binary.
#[test]
fn a_check_that_fails_on_no_binary_is_named_by_its_command() {
    let check_command = format!("{WHICH_COMMAND} sh && test -f no-such-file-a4ebnw3");

    let missing = missing_label(&check_command);

    assert!(
        missing.contains(&check_command),
        "every binary is present, so the command itself is the failure; got '{missing}'"
    );
}

/// The `run:` line carries only install commands, and the advisory hint
/// stands apart.
///
/// Both are remedies for a missing tool, and only one of them is runnable:
/// a `doctor.fix_hint` is prose a person reads. A reader who pastes what
/// follows `run:` into a shell must get a command.
#[test]
fn the_precondition_failure_keeps_the_hint_out_of_the_runnable_line() {
    let hint = "brew install it, and run the review from the package directory";
    let spec = ToolSpec {
        scope: ToolScope::Files,
        run: "true".to_string(),
        doctor: Some(ToolDoctor {
            check_command: format!("{WHICH_COMMAND} {ABSENT_BINARY}"),
            check_version_command: None,
            fix_hint: Some(FixHint::from(hint.to_string())),
        }),
        install: Some(ToolInstall {
            commands: vec!["brew install it@1.2.3".to_string()],
        }),
    };

    let report = precondition_report("probe-rule", &spec, "exited with exit status: 1");

    let run_line = report
        .lines()
        .find(|line| line.starts_with(RUNNABLE_PREFIX))
        .expect("the report offers a runnable remedy");
    assert_eq!(run_line, format!("{RUNNABLE_PREFIX}brew install it@1.2.3"));
    assert!(
        report.contains(hint),
        "the advisory hint must still be reported; got '{report}'"
    );
}

/// A rule with no install command offers no `run:` line at all, so the
/// report never presents prose as a command.
#[test]
fn a_rule_with_no_install_command_offers_no_runnable_line() {
    let spec = ToolSpec {
        scope: ToolScope::Files,
        run: "true".to_string(),
        doctor: Some(ToolDoctor {
            check_command: format!("{WHICH_COMMAND} {ABSENT_BINARY}"),
            check_version_command: None,
            fix_hint: Some(FixHint::from("brew install it".to_string())),
        }),
        install: None,
    };

    let report = precondition_report("probe-rule", &spec, "exited with exit status: 1");

    assert!(
        !report.lines().any(|line| line.starts_with(RUNNABLE_PREFIX)),
        "a rule with no install command has nothing runnable to offer; got '{report}'"
    );
}
