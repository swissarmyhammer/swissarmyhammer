// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod deeplink;
mod tray;

use clap::Parser;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use mirdan::{agents, banner};
use mirdan::{Cli, Commands};
use tauri::Manager;

/// Initialize tracing for tray mode — routes to macOS Console.app via os_log.
fn init_tray_tracing() {
    let oslog = tracing_oslog::OsLogger::new("ai.mirdan.app", "default");
    tracing_subscriber::registry().with(oslog).init();
}

/// Launch the Tauri tray application.
fn run_tray() {
    // Set CWD to HOME once at startup. The .app bundle's CWD is read-only,
    // and mirdan operations need a writable directory for lockfiles/temp files.
    // Done here instead of per-command to avoid a process-global data race.
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::env::set_current_dir(home);
    }

    init_tray_tracing();
    use tauri_plugin_deep_link::DeepLinkExt;

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_packages,
            commands::search_registry,
            commands::install_package,
            commands::uninstall_package,
            commands::update_package,
            commands::get_package_path,
            commands::get_registry_url,
            commands::open_external,
        ])
        .setup(|app| {
            // Accessory app: no Dock icon, tray only.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

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

            // Hide the window on close instead of destroying it.
            // The app stays running as a tray accessory.
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("tauri runtime failed: {e}");
            std::process::exit(1);
        });
}

fn main() {
    // No args → tray mode (Finder launch, cargo tauri dev, or bare invocation).
    // The app binary's primary job is the tray — CLI commands are the exception.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        run_tray();
        return;
    }

    // Show banner for --help.
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        banner::print_banner();
    }

    let cli = Cli::parse();

    // `start` subcommand → tray mode, same as no-args.
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
