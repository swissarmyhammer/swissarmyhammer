//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Reads JSON from stdin, processes the hook, and writes JSON to stdout.
//! Validators now run in parallel with adaptive concurrency control.
//! Exit codes:
//! - 0: Success
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use avp_common::context::AvpContext;
use avp_common::strategy::HookDispatcher;
use avp_common::AvpError;

/// AVP - Agent Validator Protocol
///
/// Claude Code hook processor that reads JSON from stdin and outputs JSON to stdout.
#[derive(Parser, Debug)]
#[command(name = "avp")]
#[command(version)]
#[command(about = "Agent Validator Protocol - Claude Code hook processor")]
struct Args {
    /// Enable debug output to stderr
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize tracing with appropriate level
    let filter = if args.debug {
        EnvFilter::new("avp=debug,avp_common=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    let exit_code = match run(&args).await {
        Ok(code) => code,
        Err(e) => {
            // Output error as JSON for consistency
            let error_output = serde_json::json!({
                "continue": false,
                "stopReason": e.to_string()
            });
            tracing::error!("{}", e);
            let _ = io::stdout().write_all(error_output.to_string().as_bytes());
            2 // Blocking error
        }
    };
    std::process::exit(exit_code);
}

async fn run(_args: &Args) -> Result<i32, AvpError> {
    // Check if stdin is a terminal (no piped input)
    if io::stdin().is_terminal() {
        tracing::warn!("no input provided (pipe JSON to stdin)");
        tracing::info!("Usage: echo '{{\"hook_event_name\":\"PreToolUse\",...}}' | avp");
        return Ok(0);
    }

    // Read JSON from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    // Handle empty input
    let input_str = input_str.trim();
    if input_str.is_empty() {
        tracing::warn!("no input provided");
        return Ok(0);
    }

    tracing::debug!("Input: {}", input_str);

    // Parse input JSON
    let input_value: serde_json::Value = serde_json::from_str(input_str)?;

    // Extract hook event name for debug logging
    let hook_event_name: String = input_value
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    // Initialize context (required for dispatcher)
    let ctx = AvpContext::init()?;

    // Create dispatcher with default strategies, passing the context
    let dispatcher = HookDispatcher::with_defaults(ctx);

    // Process the hook (async) - logging is handled by ClaudeCodeHookStrategy
    let (output, exit_code) = dispatcher.dispatch(input_value).await?;

    tracing::debug!(
        hook = %hook_event_name,
        exit_code = exit_code,
        "Hook processed"
    );

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
