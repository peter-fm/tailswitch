mod config;
mod tailscale;
mod ui;

use anyhow::{Context, Result};
use config::Config;
use tailscale::TailscaleClient;
use ui::{App, AppAction, UrlDisplayApp};

fn main() -> Result<()> {
    // Check if tailscale is installed
    if !TailscaleClient::check_installed()? {
        eprintln!("Error: tailscale is not installed or not in PATH");
        eprintln!("Please install tailscale first: https://tailscale.com/download");
        std::process::exit(1);
    }

    // Check if we need sudo
    let needs_sudo = TailscaleClient::check_needs_sudo();
    if needs_sudo {
        eprintln!("Note: tailscale requires elevated permissions.");
        eprintln!("You can either:");
        eprintln!("  1. Run with sudo: sudo tailswitch");
        eprintln!("  2. Set yourself as operator once: sudo tailscale set --operator=$USER");
        eprintln!();
        eprintln!("Attempting to use sudo for tailscale commands...");
        eprintln!();

        // Prime sudo by running a simple command so password is cached
        eprintln!("Requesting sudo access...");
        let status = std::process::Command::new("sudo").arg("-v").status();

        if status.is_err() || !status.unwrap().success() {
            eprintln!("Failed to obtain sudo access. Exiting.");
            std::process::exit(1);
        }
        eprintln!();
    }

    // Load config (optional - for adding new tailnets)
    let config = Config::load().context("Failed to load configuration")?;

    // Get existing tailscale profiles
    let client = TailscaleClient::new(needs_sudo);
    let profiles = client.list_profiles().unwrap_or_default();

    if profiles.is_empty() && config.tailnets.is_empty() {
        eprintln!("No tailscale profiles found and no tailnets configured!");
        eprintln!("Please either:");
        eprintln!("  1. Login to tailscale first: sudo tailscale login");
        eprintln!(
            "  2. Or configure tailnets in: {}",
            Config::get_config_path_string()
                .unwrap_or_else(|_| "~/.config/tailswitch/config.toml".to_string())
        );
        std::process::exit(1);
    }

    // Get current status to see which profile is active
    let current_status = client.status().unwrap_or_default();
    let is_logged_in = !current_status.contains("Logged out");

    // Parse the active tailnet from switch --list (has * at the end of account)
    let active_tailnet = if is_logged_in {
        profiles
            .iter()
            .find(|(_, account)| account.ends_with('*'))
            .map(|(name, _)| name.clone())
    } else {
        None
    };

    // Build list of options: existing profiles + config entries
    let mut all_options = Vec::new();

    // Add existing profiles first
    for (tailnet, account) in &profiles {
        let is_active = active_tailnet
            .as_ref()
            .map(|t| t == tailnet)
            .unwrap_or(false);
        // Remove * from account name for display
        let clean_account = account.trim_end_matches('*').to_string();
        all_options.push((tailnet.clone(), Some(clean_account), true, is_active)); // (name, account, is_profile, is_active)
    }

    // Add config entries that don't already exist as profiles
    for tailnet in &config.tailnets {
        if !profiles.iter().any(|(name, _)| name == &tailnet.name) {
            all_options.push((tailnet.name.clone(), None, false, false)); // (name, no account, not a profile, not active)
        }
    }

    // Run the TUI with all options in a loop
    let mut app = App::new_with_options(all_options, config.clone());

    loop {
        let action = app.run().context("Failed to run TUI")?;

        // Handle the action
        let should_exit = match action {
            Some(AppAction::SelectTailnet(tailnet)) => {
                println!("Switching to tailnet: {}", tailnet.name);

                let client = TailscaleClient::new(needs_sudo);

                // Check if this profile already exists
                println!("Checking existing profiles...");
                let profiles = client.list_profiles().unwrap_or_default();

                let profile_exists = profiles.iter().any(|(name, _)| name == &tailnet.name);

                if profile_exists {
                    // Profile exists - use fast switching
                    println!("Found existing profile for '{}'", tailnet.name);
                    println!("Switching...");

                    match client.switch_to(&tailnet.name) {
                        Ok(()) => {
                            println!("✓ Successfully switched to {}!", tailnet.name);

                            // Check if we're logged in after switching
                            let is_logged_out = client.is_logged_out().unwrap_or(false);

                            if is_logged_out {
                                println!("\n⚠ Profile is logged out. Starting authentication...");

                                // Look up config for this tailnet to get flags
                                let tailnet_with_config = if let Some(config_entry) =
                                    config.tailnets.iter().find(|t| t.name == tailnet.name)
                                {
                                    // Use config entry which has flags
                                    config_entry.clone()
                                } else {
                                    // No config entry, use the selected tailnet
                                    tailnet.clone()
                                };

                                // Re-authenticate with proper flags
                                println!("Connecting to {}...", tailnet_with_config.name);
                                println!("Starting authentication process...");

                                match client
                                    .login_and_get_url(&tailnet_with_config)
                                    .context("Failed to start tailscale connection")?
                                {
                                    Some(url) => {
                                        // We got an auth URL - show it in a TUI
                                        println!(
                                            "Authentication URL received. Opening URL display..."
                                        );

                                        let mut url_app = UrlDisplayApp::new(
                                            url.clone(),
                                            tailnet_with_config.name.clone(),
                                        );
                                        let should_open_browser =
                                            url_app.run().context("Failed to run URL display")?;

                                        if should_open_browser {
                                            println!("Opening browser...");
                                            let script_content = format!(
                                                r#"#!/bin/sh
export DISPLAY="${{DISPLAY:-:0}}"
export WAYLAND_DISPLAY="${{WAYLAND_DISPLAY:-wayland-0}}"
export XDG_RUNTIME_DIR="${{XDG_RUNTIME_DIR:-/run/user/$(id -u)}}"
export DBUS_SESSION_BUS_ADDRESS="${{DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}}"
exec chromium '{}' >/dev/null 2>&1 &
"#,
                                                url.replace("'", "'\\''")
                                            );

                                            let script_path = "/tmp/tailswitch-open-browser.sh";
                                            if let Err(e) =
                                                std::fs::write(script_path, script_content)
                                            {
                                                eprintln!(
                                                    "✗ Failed to create browser script: {}",
                                                    e
                                                );
                                                eprintln!(
                                                    "\nPlease manually open this URL in your browser:"
                                                );
                                                eprintln!("{}", url);
                                            } else {
                                                let _ = std::process::Command::new("chmod")
                                                    .arg("+x")
                                                    .arg(script_path)
                                                    .status();

                                                let result = std::process::Command::new("setsid")
                                                    .arg("-f")
                                                    .arg(script_path)
                                                    .spawn();

                                                match result {
                                                    Ok(_) => {
                                                        std::thread::sleep(
                                                            std::time::Duration::from_millis(1000),
                                                        );
                                                        println!("✓ Browser launch initiated!");
                                                        println!(
                                                            "✓ Please complete authentication in your browser."
                                                        );
                                                        println!(
                                                            "✓ Select the '{}' tailnet when prompted.",
                                                            tailnet_with_config.name
                                                        );
                                                        println!(
                                                            "\nTailscale is running in the background."
                                                        );
                                                        println!(
                                                            "Run 'tailscale status' in a few moments to verify connection."
                                                        );
                                                        println!(
                                                            "\nIf browser didn't open, manually open this URL:"
                                                        );
                                                        println!("{}", url);
                                                    }
                                                    Err(e) => {
                                                        eprintln!(
                                                            "✗ Failed to launch browser: {}",
                                                            e
                                                        );
                                                        eprintln!(
                                                            "\nPlease manually open this URL in your browser:"
                                                        );
                                                        eprintln!("{}", url);
                                                    }
                                                }
                                            }
                                        } else {
                                            println!("Exited without opening browser.");
                                            println!(
                                                "You can manually open this URL to complete authentication:"
                                            );
                                            println!("{}", url);
                                            println!(
                                                "\nTailscale is still running in the background waiting for authentication."
                                            );
                                        }
                                    }
                                    None => {
                                        println!(
                                            "Successfully connected to {}!",
                                            tailnet_with_config.name
                                        );
                                        if let Ok(status) = client.status() {
                                            println!("\nCurrent status:");
                                            println!("{}", status);
                                        }
                                    }
                                }
                                return Ok(());
                            } else {
                                // Successfully switched and logged in
                                if let Ok(status) = client.status() {
                                    println!("\nCurrent status:");
                                    println!("{}", status);
                                }
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            eprintln!("✗ Switch failed: {}", e);
                            eprintln!("Will try logging in instead...");
                        }
                    }
                } else {
                    println!(
                        "No existing profile for '{}'. Will log in to add it...",
                        tailnet.name
                    );
                }

                // If we get here, need to login (either profile doesn't exist or switch failed)
                // NOTE: We do NOT logout first! Just run tailscale login/up
                // This adds a new profile without disturbing existing ones
                println!("Connecting to {}...", tailnet.name);
                println!("Starting authentication process...");

                match client
                    .login_and_get_url(&tailnet)
                    .context("Failed to start tailscale connection")?
                {
                    Some(url) => {
                        // We got an auth URL - show it in a TUI
                        println!("Authentication URL received. Opening URL display...");

                        // Log the URL to a file for debugging
                        let debug_log = format!(
                            "/tmp/tailswitch-debug-{}.txt",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        );
                        let _ = std::fs::write(
                            &debug_log,
                            format!("Captured URL: {}\nTailnet: {}\n", url, tailnet.name),
                        );
                        println!("Debug info written to: {}", debug_log);

                        let mut url_app = UrlDisplayApp::new(url.clone(), tailnet.name.clone());
                        let should_open_browser =
                            url_app.run().context("Failed to run URL display")?;

                        if should_open_browser {
                            // User pressed Enter - open the browser
                            println!("Opening browser...");

                            // Create a temporary script with all necessary environment variables
                            let script_content = format!(
                                r#"#!/bin/sh
export DISPLAY="${{DISPLAY:-:0}}"
export WAYLAND_DISPLAY="${{WAYLAND_DISPLAY:-wayland-0}}"
export XDG_RUNTIME_DIR="${{XDG_RUNTIME_DIR:-/run/user/$(id -u)}}"
export DBUS_SESSION_BUS_ADDRESS="${{DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}}"
exec chromium '{}' >/dev/null 2>&1 &
"#,
                                url.replace("'", "'\\''")
                            );

                            let script_path = "/tmp/tailswitch-open-browser.sh";
                            if let Err(e) = std::fs::write(script_path, script_content) {
                                eprintln!("✗ Failed to create browser script: {}", e);
                                eprintln!("\nPlease manually open this URL in your browser:");
                                eprintln!("{}", url);
                            } else {
                                // Make it executable
                                let _ = std::process::Command::new("chmod")
                                    .arg("+x")
                                    .arg(script_path)
                                    .status();

                                // Run with setsid for complete detachment
                                let result = std::process::Command::new("setsid")
                                    .arg("-f")
                                    .arg(script_path)
                                    .spawn();

                                match result {
                                    Ok(_) => {
                                        std::thread::sleep(std::time::Duration::from_millis(1000));
                                        println!("✓ Browser launch initiated!");
                                        println!(
                                            "✓ Please complete authentication in your browser."
                                        );
                                        println!(
                                            "✓ Select the '{}' tailnet when prompted.",
                                            tailnet.name
                                        );
                                        println!("\nTailscale is running in the background.");
                                        println!(
                                            "Run 'tailscale status' in a few moments to verify connection."
                                        );
                                        println!(
                                            "\nIf browser didn't open, manually open this URL:"
                                        );
                                        println!("{}", url);
                                    }
                                    Err(e) => {
                                        eprintln!("✗ Failed to launch browser: {}", e);
                                        eprintln!(
                                            "\nPlease manually open this URL in your browser:"
                                        );
                                        eprintln!("{}", url);
                                    }
                                }
                            }
                        } else {
                            // User pressed 'q' - exit without opening browser
                            println!("Exited without opening browser.");
                            println!("You can manually open this URL to complete authentication:");
                            println!("{}", url);
                            println!(
                                "\nTailscale is still running in the background waiting for authentication."
                            );
                        }
                    }
                    None => {
                        // No URL needed (auth key was used) - connection completed
                        println!("Successfully connected to {}!", tailnet.name);

                        // Show status
                        if let Ok(status) = client.status() {
                            println!("\nCurrent status:");
                            println!("{}", status);
                        }
                    }
                }
                true // Exit after switching
            }
            Some(AppAction::RunTailscaleUp) => {
                // Get the currently selected tailnet name
                let tailnet_name = match app
                    .get_selected_tailnet_name()
                    .or_else(|| app.get_active_tailnet_name())
                {
                    Some(name) => name,
                    None => {
                        app.show_output(
                            "Error".to_string(),
                            "No tailnet selected or active".to_string(),
                        );
                        continue;
                    }
                };

                // Look up config for this tailnet to get flags
                let tailnet_config = config
                    .tailnets
                    .iter()
                    .find(|t| t.name == tailnet_name)
                    .cloned()
                    .unwrap_or_else(|| config::Tailnet {
                        name: tailnet_name.clone(),
                        login_server: None,
                        auth_key: None,
                        flags: None,
                    });

                let client = TailscaleClient::new(needs_sudo);

                let output = match client.run_up(&tailnet_config) {
                    Ok(()) => {
                        let mut result = format!(
                            "✓ Successfully updated connection settings for '{}'!\n",
                            tailnet_name
                        );
                        if let Some(ref flags) = tailnet_config.flags {
                            result.push_str(&format!("\nApplied flags: {}\n", flags.join(" ")));
                        }

                        // Show status
                        if let Ok(status) = client.status() {
                            result.push('\n');
                            result.push_str(&status);
                        }
                        result
                    }
                    Err(e) => {
                        format!("✗ Failed to run tailscale up: {}", e)
                    }
                };

                app.show_output(format!("Tailscale Up - {}", tailnet_name), output);
                false // Don't exit, show output
            }
            Some(AppAction::ShowStatus) => {
                let client = TailscaleClient::new(needs_sudo);
                let output = match client.status() {
                    Ok(status) => status,
                    Err(e) => format!("✗ Failed to get status: {}", e),
                };

                app.show_output("Tailscale Status".to_string(), output);
                false // Don't exit, show output
            }
            Some(AppAction::Logout) => {
                let client = TailscaleClient::new(needs_sudo);

                let output = match client.logout() {
                    Ok(()) => "✓ Successfully logged out!".to_string(),
                    Err(e) => format!("✗ Failed to logout: {}", e),
                };

                app.show_output("Logout".to_string(), output);
                false // Don't exit, show output
            }
            Some(AppAction::Quit) | None => {
                true // Exit
            }
        };

        if should_exit {
            break;
        }
    }

    Ok(())
}
