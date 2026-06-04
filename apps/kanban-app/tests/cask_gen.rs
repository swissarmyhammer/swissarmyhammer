//! Integration test for `build-support/gen-cask.sh`, the script that renders
//! a Homebrew cask Ruby file for an app DMG.
//!
//! The release workflow (`.github/workflows/release-app.yml`, job
//! `publish-homebrew-cask`) shells out to this script once per app. Two
//! invocation shapes are exercised here:
//!
//! * `kanban` — passes `--cli-binary Contents/MacOS/kanban`, so the rendered
//!   cask must additionally carry a `binary` stanza (symlinking the bundled
//!   CLI onto PATH) and a `conflicts_with formula: "kanban-cli"` stanza (so the
//!   standalone cargo-dist `kanban-cli` formula and the cask never both own
//!   `kanban` on PATH).
//! * `mirdan` — no `--cli-binary`, so neither stanza may appear.
//!
//! The script lives in the repo-root `build-support/` directory; this crate
//! sits at `apps/kanban-app/`, two levels below the workspace root.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Absolute path to `build-support/gen-cask.sh`, resolved from this crate's
/// manifest directory (`apps/kanban-app/`) by walking up to the repo root.
fn gen_cask_script() -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent() // apps/
        .and_then(Path::parent) // repo root
        .expect("apps/kanban-app must sit two levels below the workspace root");
    repo_root.join("build-support").join("gen-cask.sh")
}

/// Run `gen-cask.sh` with the given arguments and return its captured output,
/// asserting it exited successfully.
fn run_gen_cask(args: &[&str]) -> Output {
    let script = gen_cask_script();
    assert!(
        script.exists(),
        "gen-cask.sh should exist at {}",
        script.display()
    );

    let output = Command::new("bash")
        .arg(&script)
        .args(args)
        .output()
        .expect("gen-cask.sh should be invocable via bash");

    assert!(
        output.status.success(),
        "gen-cask.sh exited with failure: {} (stderr: {})",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

/// Render the cask Ruby for the given arguments and return it as a UTF-8
/// string.
fn render_cask(args: &[&str]) -> String {
    let output = run_gen_cask(args);
    String::from_utf8(output.stdout).expect("generated cask Ruby must be UTF-8")
}

/// Arguments common to every invocation: a `kanban`-shaped cask description.
/// The CLI-specific `--cli-binary` flag is appended by individual tests.
fn kanban_args() -> Vec<&'static str> {
    vec![
        "--name",
        "kanban",
        "--product",
        "Kanban",
        "--version",
        "0.10.0",
        "--sha256",
        "abc123def456abc123def456abc123def456abc123def456abc123def456abcd",
        "--dmg-name",
        "Kanban_aarch64.dmg",
        "--desc",
        "Kanban board for SwissArmyHammer",
        "--homepage",
        "https://github.com/swissarmyhammer/swissarmyhammer",
    ]
}

/// Arguments for a `mirdan`-shaped cask — the app with no bundled CLI.
fn mirdan_args() -> Vec<&'static str> {
    vec![
        "--name",
        "mirdan",
        "--product",
        "Mirdan",
        "--version",
        "0.10.0",
        "--sha256",
        "0000111122223333444455556666777788889999aaaabbbbccccddddeeeeffff",
        "--dmg-name",
        "Mirdan_aarch64.dmg",
        "--desc",
        "Universal package manager for AI coding agents",
        "--homepage",
        "https://mirdan.ai",
    ]
}

/// Run `ruby -c` over the rendered cask to confirm it is syntactically valid
/// Ruby. Skips silently if `ruby` is not on PATH so the test stays portable.
fn assert_ruby_syntax_valid(cask: &str, label: &str) {
    let ruby_available = Command::new("ruby")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ruby_available {
        eprintln!("ruby not on PATH; skipping `ruby -c` check for {label}");
        return;
    }

    let mut child = Command::new("ruby")
        .args(["-c", "-e", cask])
        .output()
        .expect("ruby -c should be invocable");
    // Some ruby builds emit the parse result on stdout, others on stderr;
    // a non-zero exit is the unambiguous failure signal.
    if !child.status.success() {
        child.stderr.extend_from_slice(&child.stdout);
        panic!(
            "generated {label} cask is not valid Ruby: {}",
            String::from_utf8_lossy(&child.stderr),
        );
    }
}

/// The `kanban` invocation (with `--cli-binary`) must render a `binary`
/// stanza pointing at the bundled CLI inside the app bundle.
#[test]
fn kanban_cask_has_binary_stanza() {
    let mut args = kanban_args();
    args.extend(["--cli-binary", "Contents/MacOS/kanban"]);
    let cask = render_cask(&args);

    assert!(
        cask.contains(r##"binary "#{appdir}/Kanban.app/Contents/MacOS/kanban""##),
        "kanban cask must contain a binary stanza for the bundled CLI; got:\n{cask}",
    );
}

/// The `kanban` invocation must render a `conflicts_with formula: "kanban-cli"`
/// stanza so the cask and the standalone cargo-dist formula never both own the
/// `kanban` symlink on PATH. The formula is named `kanban-cli` (workspace
/// convention: CLIs are `*-cli`, the short token is the app cask), so the
/// conflict must name that formula -- not the cask token "kanban", which is a
/// formula that does not exist.
#[test]
fn kanban_cask_has_conflicts_with_formula() {
    let mut args = kanban_args();
    args.extend(["--cli-binary", "Contents/MacOS/kanban"]);
    let cask = render_cask(&args);

    assert!(
        cask.contains(r#"conflicts_with formula: "kanban-cli""#),
        "kanban cask must declare a conflict with the standalone kanban-cli formula; got:\n{cask}",
    );
}

/// The `mirdan` invocation (no `--cli-binary`) must render neither the
/// `binary` stanza nor the `conflicts_with` stanza — mirdan ships no CLI.
#[test]
fn mirdan_cask_omits_cli_stanzas() {
    let cask = render_cask(&mirdan_args());

    assert!(
        !cask.contains("binary "),
        "mirdan cask must NOT contain a binary stanza; got:\n{cask}",
    );
    assert!(
        !cask.contains("conflicts_with"),
        "mirdan cask must NOT contain a conflicts_with stanza; got:\n{cask}",
    );
}

/// Both invocations must render the core cask stanzas — `cask`, `version`,
/// `sha256`, `url`, and `app` — derived from the passed arguments.
#[test]
fn cask_contains_core_stanzas() {
    let mut kanban = kanban_args();
    kanban.extend(["--cli-binary", "Contents/MacOS/kanban"]);
    let kanban_cask = render_cask(&kanban);

    for needle in [
        r#"cask "kanban" do"#,
        r#"version "0.10.0""#,
        r#"sha256 "abc123def456abc123def456abc123def456abc123def456abc123def456abcd""#,
        "releases/download/v0.10.0/Kanban_aarch64.dmg",
        r#"app "Kanban.app""#,
    ] {
        assert!(
            kanban_cask.contains(needle),
            "kanban cask must contain `{needle}`; got:\n{kanban_cask}",
        );
    }

    let mirdan_cask = render_cask(&mirdan_args());
    for needle in [
        r#"cask "mirdan" do"#,
        r#"version "0.10.0""#,
        r#"sha256 "0000111122223333444455556666777788889999aaaabbbbccccddddeeeeffff""#,
        "releases/download/v0.10.0/Mirdan_aarch64.dmg",
        r#"app "Mirdan.app""#,
    ] {
        assert!(
            mirdan_cask.contains(needle),
            "mirdan cask must contain `{needle}`; got:\n{mirdan_cask}",
        );
    }
}

/// Both rendered casks must be syntactically valid Ruby.
#[test]
fn generated_casks_are_valid_ruby() {
    let mut kanban = kanban_args();
    kanban.extend(["--cli-binary", "Contents/MacOS/kanban"]);
    assert_ruby_syntax_valid(&render_cask(&kanban), "kanban");

    assert_ruby_syntax_valid(&render_cask(&mirdan_args()), "mirdan");
}
