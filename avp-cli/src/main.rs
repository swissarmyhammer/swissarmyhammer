//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Reads JSON from stdin, processes the hook, and writes JSON to stdout.
//! Exit codes:
//! - 0: Success
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::Parser;

use avp_common::context::{AvpContext, Decision};
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
    let exit_code = match run(&args).await {
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

async fn run(args: &Args) -> Result<i32, AvpError> {
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

    // Extract hook event name for logging
    let hook_event_name = input_value
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    // Initialize context (required for dispatcher)
    let ctx = AvpContext::init()?;

    // Create dispatcher with default strategies, passing the context
    let dispatcher = HookDispatcher::with_defaults(ctx);

    // Process the hook (async)
    let (output, exit_code) = dispatcher.dispatch(input_value.clone()).await?;

    // Log the event (smart details based on hook type)
    let decision = if exit_code == 0 {
        Decision::Allow
    } else {
        Decision::Block
    };

    let details = extract_details(&input_value, hook_event_name, &output);

    // Access context from the dispatcher's strategy
    // Note: For now, we can't easily access the context since it's owned by strategies.
    // We'll need to revisit logging when we have a proper context accessor.
    // For now, just log to stderr in debug mode.
    if args.debug {
        eprintln!(
            "[avp] {} decision={} {}",
            hook_event_name,
            decision,
            details.as_deref().unwrap_or("")
        );
    }

    if args.debug {
        eprintln!("[avp] Exit code: {}", exit_code);
    }

    // Write JSON to stdout with trailing newline
    let output_json = serde_json::to_string(&output)?;
    io::stdout().write_all(output_json.as_bytes())?;
    io::stdout().write_all(b"\n")?;

    Ok(exit_code)
}

/// Extract smart details based on hook type.
/// Only logs relevant info, not full payloads.
fn extract_details(
    input: &serde_json::Value,
    hook_type: &str,
    output: &avp_common::HookOutput,
) -> Option<String> {
    match hook_type {
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => {
            // Log tool name
            input
                .get("tool_name")
                .and_then(|v| v.as_str())
                .map(|name| format!("tool={}", name))
        }
        "UserPromptSubmit" => {
            // Log prompt length, not content
            input
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|p| format!("prompt_len={}", p.len()))
        }
        "SessionStart" | "SessionEnd" => {
            // Log session ID
            input
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(|id| format!("session={}", id))
        }
        _ => {
            // For block decisions, include stop reason
            if !output.continue_execution {
                output
                    .stop_reason
                    .as_ref()
                    .map(|r| format!("reason=\"{}\"", r))
            } else {
                None
            }
        }
    }
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
