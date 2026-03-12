// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod deeplink;
mod tray;

use clap::Parser;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use mirdan::{agents, banner};
use mirdan::{Cli, Commands};

/// Initialize tracing for tray mode — routes to macOS Console.app via os_log.
fn init_tray_tracing() {
    let oslog = tracing_oslog::OsLogger::new("ai.mirdan.app", "default");
    tracing_subscriber::registry().with(oslog).init();
}

/// Launch the Tauri tray application.
fn run_tray() {
    init_tray_tracing();
    use tauri_plugin_deep_link::DeepLinkExt;

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            tray::setup_tray(app.handle())?;

            // Handle any URLs that were passed at launch (cold start from URL).
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    deeplink::handle_url(app.handle(), url.to_string());
                }
            }

            // Listen for URLs arriving while the app is already running.
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    deeplink::handle_url(&handle, url.to_string());
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("tauri runtime failed: {e}");
            std::process::exit(1);
        });
}

fn main() {
    // Detect launch context: tray mode (no args + non-TTY) or banner display.
    {
        use std::io::IsTerminal;
        let args: Vec<String> = std::env::args().collect();
        let is_tty = std::io::stdin().is_terminal();

        if args.len() == 1 && !is_tty {
            run_tray();
            return;
        }

        let show_banner = match args.len() {
            1 => is_tty,
            2 => args[1] == "--help" || args[1] == "-h",
            _ => false,
        };
        if show_banner {
            banner::print_banner();
        }
    }

    let cli = Cli::parse();

    // `start` subcommand → tray mode, skip all CLI setup.
    if matches!(cli.command, Commands::Start) {
        run_tray();
        return;
    }

    // Everything else is CLI mode — set up tracing and dispatch.
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let filter = if cli.debug {
        EnvFilter::new("mirdan=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    if let Some(ref agent_id) = cli.agent {
        if let Ok(config) = agents::load_agents_config() {
            if let Err(e) = agents::validate_agent_id(&config, agent_id) {
                tracing::error!("{e}");
                std::process::exit(1);
            }
        }
    }

    let exit_code = rt.block_on(async {
        // dispatch returns None for Commands::Start, which we already handled above.
        mirdan::dispatch(&cli).await.unwrap_or_else(|| unreachable!())
    });

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use mirdan::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn test_start_subcommand_parses() {
        let cli = Cli::parse_from(["mirdan", "start"]);
        assert!(matches!(cli.command, Commands::Start));
    }

    #[test]
    fn test_install_subcommand_parses() {
        let cli = Cli::parse_from(["mirdan", "install", "foo"]);
        assert!(matches!(cli.command, Commands::Install { .. }));
    }

    #[test]
    fn test_list_subcommand_parses() {
        let cli = Cli::parse_from(["mirdan", "list"]);
        assert!(matches!(cli.command, Commands::List { .. }));
    }
}
