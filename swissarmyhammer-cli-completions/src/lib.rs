//! Shared shell-completion helpers for SwissArmyHammer CLIs.
//!
//! Every workspace CLI (`avp`, `sah`, `code-context`, `kanban`, `mirdan`,
//! `shelltool`) needs to render a clap command tree into a shell-specific
//! completion script and write it to stdout. Before this crate each CLI
//! carried its own ~130-line `completions.rs` plus a near-identical
//! integration test file. The implementations differed only in:
//!
//! - The `PROGRAM_NAME` constant (binary name).
//! - Whether the clap tree came from a derived `Cli` type or a dynamic
//!   builder (`sah` and `kanban` build their tree at runtime).
//!
//! This crate exposes the shared rendering primitives plus reusable test
//! helpers so each CLI's per-crate module collapses to a thin shim.
//!
//! # Why a dedicated crate
//!
//! `swissarmyhammer-common` is intentionally library-pure — adding `clap`
//! and `clap_complete` there would leak heavy CLI machinery into every
//! library consumer. A small dedicated crate keeps the dependency surface
//! contained to the actual CLI crates that need it.

use clap::{Command, CommandFactory};
use clap_complete::{generate, generate_to, Shell};
use std::io;
use std::path::Path;

/// The four shells `clap_complete` knows how to render. Centralised here so
/// every per-CLI shim — and every test that iterates them — agrees on the
/// supported surface. Adding a shell only needs a change in one place.
pub const SUPPORTED_SHELLS: [Shell; 4] = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];

/// Print a shell completion script for a `CommandFactory`-derived CLI to
/// stdout.
///
/// `name` is the binary name the script registers under (the value a user
/// types at the shell prompt, matching `[[bin]] name` in `Cargo.toml`).
/// `shell` selects the rendering target. The script is written directly to
/// `io::stdout()` so callers can redirect into a completion install path
/// (`avp completion bash > /etc/bash_completion.d/avp`).
///
/// Used by the derive-based CLIs whose clap tree is fully known at compile
/// time. CLIs that build their tree dynamically (`sah`, `kanban`) should
/// instead call [`print_completion_for`] with the assembled `Command`.
///
/// # Errors
///
/// Returns `io::Result<()>` for symmetry with other I/O entry points;
/// `clap_complete::generate` does not currently surface a fallible path for
/// the supported shells, but keeping the signature fallible avoids a
/// breaking change if it grows one.
pub fn print_completion<C: CommandFactory>(name: &str, shell: Shell) -> io::Result<()> {
    let mut cmd = C::command();
    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

/// Print a shell completion script for a fully-assembled clap `Command` to
/// stdout.
///
/// `cmd` is the runtime tree — typically built by the calling CLI's dynamic
/// builder (`dynamic_cli::CliBuilder::build_cli()` in `sah`, the
/// schema-driven builder in `kanban`) so the generated script reflects every
/// dynamic subcommand alongside the static ones. `name` is the binary name
/// the script registers under. The script is written directly to
/// `io::stdout()`.
///
/// # Errors
///
/// Returns `io::Result<()>` for symmetry with other I/O entry points;
/// `clap_complete::generate` does not currently surface a fallible path for
/// the supported shells.
pub fn print_completion_for(mut cmd: Command, name: &str, shell: Shell) -> io::Result<()> {
    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

/// Write completion scripts for every supported shell into `outdir`.
///
/// Intended for build-script use — `build.rs` calls this to produce the
/// completion files that ship in the release archive. The directory is
/// created if it does not exist. One file per shell is written, named
/// according to each shell's filename convention (`<name>.bash`, `_<name>`,
/// `<name>.fish`, `_<name>.ps1`).
///
/// # Errors
///
/// Returns an error if the directory cannot be created or any completion
/// script cannot be written to disk.
pub fn generate_completions_to_dir<C: CommandFactory>(name: &str, outdir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(outdir)?;
    let mut cmd = C::command();
    for shell in SUPPORTED_SHELLS {
        generate_to(shell, &mut cmd, name, outdir)?;
    }
    Ok(())
}

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    //! Reusable assertions for verifying completion-script rendering.
    //!
    //! Each per-CLI integration test collapses to a single call into one of
    //! these helpers so the rendering contract (non-empty output, correct
    //! shell-specific registration directive, binary-name mentioned) is
    //! enforced uniformly across every workspace CLI.

    use clap::{Command, CommandFactory};
    use clap_complete::{generate, Shell};
    use std::path::Path;
    use std::process::Command as StdCommand;

    use super::SUPPORTED_SHELLS;

    /// Render a completion script for `cmd` into an in-memory buffer.
    ///
    /// Mirrors what [`super::print_completion_for`] does for stdout, but
    /// writes into a `Vec<u8>` so tests can inspect the bytes without
    /// stdout capture. Returns the UTF-8 decoded script — completion
    /// scripts are always valid UTF-8 by construction.
    fn render_to_buf(mut cmd: Command, name: &str, shell: Shell) -> String {
        let mut buf: Vec<u8> = Vec::new();
        generate(shell, &mut cmd, name, &mut buf);
        String::from_utf8(buf).expect("completion output must be valid UTF-8")
    }

    /// Assert that an already-rendered completion script satisfies the
    /// per-shell contract.
    ///
    /// Verifies the rendered `out`:
    ///
    /// - is non-empty,
    /// - mentions `name` (the binary name),
    /// - contains the shell-specific registration directive
    ///   (`#compdef` for zsh, `complete -c <name>` for fish).
    ///
    /// Split out from [`assert_renders_for_command`] so unit tests in this
    /// crate can exercise the failure paths with hand-crafted strings —
    /// `clap_complete` always inserts the `name` it is given, which makes
    /// the failure paths otherwise unreachable.
    ///
    /// Panics with a clear message on the first failed check.
    pub fn assert_rendered_script_matches(out: &str, name: &str, shell: Shell) {
        assert!(
            !out.is_empty(),
            "{shell:?} completion for `{name}` should not be empty",
        );
        assert!(
            out.contains(name),
            "{shell:?} completion for `{name}` should mention `{name}`; got: {out}",
        );
        match shell {
            Shell::Zsh => {
                assert!(
                    out.contains("#compdef"),
                    "zsh completion for `{name}` should contain #compdef directive; got: {out}",
                );
            }
            Shell::Fish => {
                let expected = format!("complete -c {name}");
                assert!(
                    out.contains(&expected),
                    "fish completion for `{name}` should contain `{expected}`; got: {out}",
                );
            }
            _ => {}
        }
    }

    /// Assert that rendering succeeds for every supported shell.
    ///
    /// Drives the per-shell render against `cmd` and feeds each rendered
    /// script to [`assert_rendered_script_matches`]. Used by the shared
    /// unit tests in this crate and by any caller that wants to assert a
    /// manually-built `Command` renders cleanly.
    ///
    /// Panics with a clear message on the first failure.
    pub fn assert_renders_for_command(cmd: Command, name: &str) {
        for shell in SUPPORTED_SHELLS {
            let out = render_to_buf(cmd.clone(), name, shell);
            assert_rendered_script_matches(&out, name, shell);
        }
    }

    /// Assert that rendering succeeds for every supported shell using the
    /// clap tree of a `CommandFactory`-derived CLI.
    ///
    /// Convenience wrapper around [`assert_renders_for_command`] for the
    /// derive-based CLIs. The type parameter `C` is the user's `Cli` type
    /// (`#[derive(Parser)]`); `name` is the binary name to register under.
    pub fn assert_renders_for_all_shells<C: CommandFactory>(name: &str) {
        assert_renders_for_command(C::command(), name);
    }

    /// Assert that a compiled CLI binary's `<bin> completion <shell>`
    /// subcommand emits a usable script for every supported shell.
    ///
    /// Launches `bin_path` four times — once per shell — and verifies the
    /// child process:
    ///
    /// - exits with status 0,
    /// - emits a non-empty UTF-8 script to stdout,
    /// - mentions `bin_name` in the script.
    ///
    /// `bin_path` is the absolute path to the compiled binary, typically
    /// passed via `env!("CARGO_BIN_EXE_<bin>")` from an integration test.
    /// `bin_name` is the binary name a user types at the shell prompt
    /// (matching `[[bin]] name` in `Cargo.toml`).
    ///
    /// This pins both the dispatch wiring (clap parses
    /// `completion <shell>` and routes through the CLI's main loop) and
    /// the binary-name contract (the registration directive uses the
    /// expected name, not the crate name).
    pub fn assert_compiled_binary_completion_works(bin_path: &Path, bin_name: &str) {
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let output = StdCommand::new(bin_path)
                .args(["completion", shell])
                .output()
                .unwrap_or_else(|e| panic!("failed to launch {bin_name} binary for {shell}: {e}"));

            let stdout = String::from_utf8(output.stdout)
                .unwrap_or_else(|_| panic!("{shell} completion stdout must be UTF-8"));
            let stderr = String::from_utf8_lossy(&output.stderr);

            assert!(
                output.status.success(),
                "{bin_name} completion {shell} should exit 0, got {:?} (stderr: {stderr})",
                output.status.code(),
            );
            assert!(
                !stdout.trim().is_empty(),
                "{bin_name} completion {shell} should print a non-empty script; stderr: {stderr}",
            );
            assert!(
                stdout.contains(bin_name),
                "{bin_name} completion {shell} script should mention the binary name `{bin_name}`; got: {stdout}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use clap_complete::Shell;
    use tempfile::TempDir;

    /// A minimal stand-in for a real CLI's `Cli` type. Used to exercise the
    /// shared rendering primitives without depending on any consumer crate.
    #[derive(Parser)]
    #[command(name = "test-bin", version = "0.0.0")]
    struct TestCli {
        /// A flag to make the rendered output non-trivial.
        #[arg(long)]
        #[allow(dead_code)]
        verbose: bool,
    }

    /// Helper to render via the public `print_completion_for` path into a
    /// buffer so tests can inspect bytes (stdout capture in unit tests is
    /// brittle). Mirrors what the function does at runtime.
    fn render_via_command(mut cmd: clap::Command, name: &str, shell: Shell) -> String {
        let mut buf: Vec<u8> = Vec::new();
        generate(shell, &mut cmd, name, &mut buf);
        String::from_utf8(buf).expect("completion output must be valid UTF-8")
    }

    /// `print_completion::<TestCli>` must emit a non-empty script that
    /// mentions the binary name for every supported shell.
    #[test]
    fn print_completion_renders_for_all_shells() {
        for shell in SUPPORTED_SHELLS {
            let out = render_via_command(TestCli::command(), "test-bin", shell);
            assert!(!out.is_empty(), "{shell:?} should produce non-empty output");
            assert!(
                out.contains("test-bin"),
                "{shell:?} should mention binary name; got: {out}",
            );
        }
    }

    /// `print_completion::<TestCli>` zsh output must contain `#compdef`.
    #[test]
    fn zsh_output_has_compdef() {
        let out = render_via_command(TestCli::command(), "test-bin", Shell::Zsh);
        assert!(
            out.contains("#compdef"),
            "zsh completion should contain #compdef; got: {out}",
        );
    }

    /// `print_completion::<TestCli>` fish output must register the binary
    /// name via `complete -c <name>`.
    #[test]
    fn fish_output_registers_program_name() {
        let out = render_via_command(TestCli::command(), "test-bin", Shell::Fish);
        assert!(
            out.contains("complete -c test-bin"),
            "fish completion should contain `complete -c test-bin`; got: {out}",
        );
    }

    /// `print_completion_for` must accept a hand-built `clap::Command` —
    /// the path used by `sah` and `kanban` whose trees are assembled at
    /// runtime — and render a valid script for every shell.
    #[test]
    fn print_completion_for_renders_hand_built_command() {
        let cmd = Command::new("hand-built")
            .version("0.0.0")
            .subcommand(Command::new("serve"))
            .subcommand(Command::new("doctor"));
        for shell in SUPPORTED_SHELLS {
            let out = render_via_command(cmd.clone(), "hand-built", shell);
            assert!(
                !out.is_empty(),
                "{shell:?} for hand-built command should not be empty",
            );
            assert!(
                out.contains("hand-built"),
                "{shell:?} for hand-built command should mention name; got: {out}",
            );
        }
    }

    /// `print_completion::<TestCli>` and `print_completion_for(TestCli::command(), ...)`
    /// must succeed (return `Ok`) for every shell. Render correctness is
    /// covered by the buffer-based tests above; this drives the public
    /// stdout-writing entry points.
    #[test]
    fn public_entry_points_succeed_for_all_shells() {
        for shell in SUPPORTED_SHELLS {
            print_completion::<TestCli>("test-bin", shell)
                .expect("print_completion should not fail");
            print_completion_for(TestCli::command(), "test-bin", shell)
                .expect("print_completion_for should not fail");
        }
    }

    /// `generate_completions_to_dir::<TestCli>` must write one file per
    /// shell into the target directory, each file non-empty.
    #[test]
    fn generate_completions_to_dir_writes_one_file_per_shell() {
        let tmp = TempDir::new().expect("tempdir");
        generate_completions_to_dir::<TestCli>("test-bin", tmp.path())
            .expect("generate_completions_to_dir should succeed");

        // clap_complete uses these per-shell filename conventions.
        let expected = [
            tmp.path().join("test-bin.bash"),
            tmp.path().join("_test-bin"),
            tmp.path().join("test-bin.fish"),
            tmp.path().join("_test-bin.ps1"),
        ];
        for path in &expected {
            assert!(
                path.exists(),
                "expected completion file to exist: {}",
                path.display(),
            );
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            assert!(
                !content.is_empty(),
                "completion file should not be empty: {}",
                path.display(),
            );
        }
    }

    /// `generate_completions_to_dir` must create the target directory if
    /// it does not exist (matches the contract `build.rs` callers rely on).
    #[test]
    fn generate_completions_to_dir_creates_missing_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let nested = tmp.path().join("nested").join("dir");
        generate_completions_to_dir::<TestCli>("test-bin", &nested)
            .expect("generate_completions_to_dir should create the directory");
        assert!(nested.exists(), "nested directory should have been created");
        assert!(nested.join("test-bin.bash").exists());
    }

    /// `test_helpers::assert_renders_for_all_shells::<TestCli>("test-bin")`
    /// must pass for a normal CLI.
    #[test]
    fn test_helpers_assert_renders_for_all_shells_passes_for_real_cli() {
        test_helpers::assert_renders_for_all_shells::<TestCli>("test-bin");
    }

    /// `test_helpers::assert_rendered_script_matches` must panic with a
    /// clear message when handed an empty rendered script — proving the
    /// helper actually inspects the bytes rather than rubber-stamping
    /// any input.
    #[test]
    #[should_panic(expected = "should not be empty")]
    fn test_helpers_panics_on_empty_render() {
        test_helpers::assert_rendered_script_matches("", "any-bin", Shell::Bash);
    }

    /// `test_helpers::assert_rendered_script_matches` must panic with a
    /// clear message when the rendered script does not mention the
    /// expected binary name.
    #[test]
    #[should_panic(expected = "should mention")]
    fn test_helpers_panics_when_name_absent() {
        // A non-empty script that lacks `expected-bin` — the second
        // assertion in the helper must fire.
        test_helpers::assert_rendered_script_matches(
            "some unrelated content",
            "expected-bin",
            Shell::Bash,
        );
    }

    /// `test_helpers::assert_rendered_script_matches` must panic when a
    /// zsh script omits the `#compdef` directive.
    #[test]
    #[should_panic(expected = "#compdef")]
    fn test_helpers_panics_on_zsh_without_compdef() {
        test_helpers::assert_rendered_script_matches(
            "this contains the-bin name but no directive",
            "the-bin",
            Shell::Zsh,
        );
    }

    /// `test_helpers::assert_rendered_script_matches` must panic when a
    /// fish script omits `complete -c <name>`.
    #[test]
    #[should_panic(expected = "complete -c")]
    fn test_helpers_panics_on_fish_without_registration() {
        test_helpers::assert_rendered_script_matches(
            "mentions the-bin but no fish registration",
            "the-bin",
            Shell::Fish,
        );
    }
}
