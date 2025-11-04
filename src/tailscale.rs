use crate::config::Tailnet;
use anyhow::{Context, Result};
use std::process::Command;

pub struct TailscaleClient {
    use_sudo: bool,
}

impl TailscaleClient {
    pub fn new(use_sudo: bool) -> Self {
        Self { use_sudo }
    }

    fn create_command(&self) -> Command {
        if self.use_sudo {
            let mut cmd = Command::new("sudo");
            cmd.arg("tailscale");
            cmd
        } else {
            Command::new("tailscale")
        }
    }

    /// Logout from current tailnet
    pub fn logout(&self) -> Result<()> {
        let mut cmd = self.create_command();
        cmd.arg("logout");

        // Use spawn + wait instead of output to allow sudo password prompt
        let status = cmd
            .spawn()
            .context("Failed to execute tailscale logout")?
            .wait()
            .context("Failed to wait for tailscale logout")?;

        if !status.success() {
            anyhow::bail!(
                "Tailscale logout failed with exit code: {:?}",
                status.code()
            );
        }

        Ok(())
    }

    /// Get list of existing tailscale profiles
    pub fn list_profiles(&self) -> Result<Vec<(String, String)>> {
        let mut cmd = self.create_command();
        cmd.arg("switch");
        cmd.arg("--list");

        let output = cmd
            .output()
            .context("Failed to execute tailscale switch --list")?;

        if !output.status.success() {
            anyhow::bail!("Failed to list profiles");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut profiles = Vec::new();

        // Parse output (skip header line)
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Format: ID    Tailnet             Account
                let tailnet = parts[1].to_string();
                let account = if parts.len() > 2 {
                    // Keep the * in the account field - we use it to detect active profile
                    parts[2].to_string()
                } else {
                    String::new()
                };
                profiles.push((tailnet, account));
            }
        }

        Ok(profiles)
    }

    /// Switch to an existing profile by tailnet name
    pub fn switch_to(&self, tailnet_name: &str) -> Result<()> {
        let mut cmd = self.create_command();
        cmd.arg("switch");
        cmd.arg(tailnet_name);

        let status = cmd
            .spawn()
            .context("Failed to execute tailscale switch")?
            .wait()
            .context("Failed to wait for tailscale switch")?;

        if !status.success() {
            anyhow::bail!("Failed to switch to {}", tailnet_name);
        }

        Ok(())
    }

    /// Login to a tailnet and return the authentication URL if one is needed
    pub fn login_and_get_url(&self, tailnet: &Tailnet) -> Result<Option<String>> {
        // With auth key, just run normally and wait
        if let Some(ref auth_key) = tailnet.auth_key {
            let mut cmd = self.create_command();
            cmd.arg("up");

            if let Some(ref server) = tailnet.login_server {
                cmd.arg("--login-server").arg(server);
            }
            cmd.arg("--auth-key").arg(auth_key);

            // Add custom flags if specified
            if let Some(ref flags) = tailnet.flags {
                for flag in flags {
                    cmd.arg(flag);
                }
            }

            let status = cmd
                .spawn()
                .context("Failed to execute tailscale up")?
                .wait()
                .context("Failed to wait for tailscale up")?;

            if !status.success() {
                anyhow::bail!("Tailscale login failed with exit code: {:?}", status.code());
            }
            return Ok(None);
        }

        // For interactive auth: use 'tailscale login' which always requires auth
        // Unlike 'tailscale up', login always opens a new auth flow
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let log_file = format!("/tmp/tailscale-auth-{}.log", timestamp);

        // Build the command - use 'login' not 'up'
        let mut cmd_args = vec!["login".to_string()];
        if let Some(ref server) = tailnet.login_server {
            cmd_args.push("--login-server".to_string());
            cmd_args.push(server.clone());
        }

        // Add custom flags if specified
        if let Some(ref flags) = tailnet.flags {
            for flag in flags {
                cmd_args.push(flag.clone());
            }
        }

        let script = if self.use_sudo {
            format!(
                "sudo tailscale {} > {} 2>&1 &",
                cmd_args.join(" "),
                log_file
            )
        } else {
            format!("tailscale {} > {} 2>&1 &", cmd_args.join(" "), log_file)
        };

        // Start tailscale in background
        Command::new("bash")
            .arg("-c")
            .arg(&script)
            .spawn()
            .context("Failed to start tailscale login")?;

        // Wait for the URL to appear in the log file
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(200));

            if let Ok(contents) = std::fs::read_to_string(&log_file) {
                // Look for the URL in the output
                for line in contents.lines() {
                    if let Some(start) = line.find("https://login.tailscale.com") {
                        let url_part = &line[start..];
                        // Extract just the URL (stop at whitespace)
                        let url = url_part.split_whitespace().next().unwrap_or("").to_string();

                        if !url.is_empty() {
                            return Ok(Some(url));
                        }
                    }
                }
            }
        }

        // No URL found within timeout
        Ok(None)
    }

    /// Get current tailscale status
    pub fn status(&self) -> Result<String> {
        let mut cmd = self.create_command();
        let output = cmd
            .arg("status")
            .output()
            .context("Failed to execute tailscale status")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Tailscale status failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Check if currently logged out
    pub fn is_logged_out(&self) -> Result<bool> {
        let mut cmd = self.create_command();
        let output = cmd
            .arg("status")
            .output()
            .context("Failed to execute tailscale status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check both stdout and stderr for logout indicators
        // "Logged out." appears in stdout
        // "Log in at:" appears in stderr
        Ok(stdout.contains("Logged out") || stderr.contains("Log in at:"))
    }

    /// Check if we need sudo
    /// On most systems, tailscale operations require root regardless of operator setting
    pub fn check_needs_sudo() -> bool {
        // Check if we're already running as root or with sudo
        std::env::var("USER").unwrap_or_default() != "root" && std::env::var("SUDO_USER").is_err()
    }

    /// Check if tailscale is installed
    pub fn check_installed() -> Result<bool> {
        let output = Command::new("which")
            .arg("tailscale")
            .output()
            .context("Failed to check if tailscale is installed")?;

        Ok(output.status.success())
    }

    /// Run tailscale up with configured flags
    pub fn run_up(&self, tailnet: &Tailnet) -> Result<()> {
        let mut cmd = self.create_command();
        cmd.arg("up");

        if let Some(ref server) = tailnet.login_server {
            cmd.arg("--login-server").arg(server);
        }

        if let Some(ref auth_key) = tailnet.auth_key {
            cmd.arg("--auth-key").arg(auth_key);
        }

        // Add custom flags if specified
        if let Some(ref flags) = tailnet.flags {
            for flag in flags {
                cmd.arg(flag);
            }
        }

        let status = cmd
            .spawn()
            .context("Failed to execute tailscale up")?
            .wait()
            .context("Failed to wait for tailscale up")?;

        if !status.success() {
            anyhow::bail!("Tailscale up failed with exit code: {:?}", status.code());
        }

        Ok(())
    }
}
