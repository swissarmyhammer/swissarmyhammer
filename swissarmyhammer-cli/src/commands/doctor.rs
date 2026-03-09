//! Doctor command - validates project type and LSP server availability
//!
//! Usage: sah doctor [--path <PATH>]

use std::path::PathBuf;
use swissarmyhammer_tools::mcp::tools::code_context::doctor;
use crate::context::CliContext;

/// Run doctor diagnostics on the project
pub async fn run(ctx: &CliContext, path: Option<PathBuf>) -> i32 {
    let target_path = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let report = doctor::run_doctor(&target_path);

    // Display results
    println!("📋 Project Type: {}",
        report.project_type.as_ref().unwrap_or(&"Unknown".to_string())
    );

    println!("\n🔍 LSP Servers:");
    if report.lsp_servers.is_empty() {
        println!("  (no LSP servers configured for this project type)");
    } else {
        for lsp in &report.lsp_servers {
            let status = if lsp.installed { "✓" } else { "✗" };
            println!("  {} {} ({})",
                status,
                lsp.name,
                if lsp.installed { "installed" } else { "not installed" }
            );
            if let Some(ref path) = lsp.path {
                println!("    📍 {}", path);
            }
        }
    }

    0
}
