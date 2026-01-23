//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Reads JSON from stdin, processes the hook, and writes JSON to stdout.
//! Exit codes:
//! - 0: Success
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::Parser;

use avp::strategy::HookDispatcher;
use avp::AvpError;

/// AVP - Agent Validator Protocol
#[derive(Parser, Debug)]
#[command(name = "avp")]
#[command(version)]
#[command(about = "Agent Validator Protocol")]
struct Args {
    /// Enable debug output to stderr
    #[arg(short, long)]
    debug: bool,
}

fn main() {
    let args = Args::parse();
    let exit_code = match run(&args) {
        Ok(code) => code,
        Err(e) => {
            // Output error as JSON for consistency
            let error_output = serde_json::json!({
                "continue": false,
                "stopReason": e.to_string()
            });
            eprintln!("{}", e);
            let _ = io::stdout().write_all(error_output.to_string().as_bytes());
            2 // Blocking error
        }
    };
    std::process::exit(exit_code);
}

fn run(args: &Args) -> Result<i32, AvpError> {
    // Check if stdin is a terminal (no piped input)
    if io::stdin().is_terminal() {
        eprintln!("avp: no input provided (pipe JSON to stdin)");
        eprintln!("Usage: echo '{{\"hook_event_name\":\"PreToolUse\",...}}' | avp");
        return Ok(0);
    }

    // Read JSON from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    // Handle empty input
    let input_str = input_str.trim();
    if input_str.is_empty() {
        eprintln!("avp: no input provided");
        return Ok(0);
    }

    if args.debug {
        eprintln!("[avp] Input: {}", input_str);
    }

    // Parse input JSON
    let input_value: serde_json::Value = serde_json::from_str(input_str)?;

    // Create dispatcher with default strategies
    let dispatcher = HookDispatcher::with_defaults();

    // Process the hook
    let (output, exit_code) = dispatcher.dispatch(input_value)?;

    if args.debug {
        eprintln!("[avp] Exit code: {}", exit_code);
    }

    // Write JSON to stdout with trailing newline
    let output_json = serde_json::to_string(&output)?;
    io::stdout().write_all(output_json.as_bytes())?;
    io::stdout().write_all(b"\n")?;

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(["avp"]);
        assert!(!args.debug);
    }

    #[test]
    fn test_args_with_debug() {
        let args = Args::parse_from(["avp", "--debug"]);
        assert!(args.debug);
    }
}
