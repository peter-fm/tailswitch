# TailSwitch

A Rust TUI (Text User Interface) application for easily switching between Tailscale tailnets using Tailscale's fast user switching feature.

## Features

- **Instant switching** between existing tailnets (no browser popups!)
- **Interactive TUI** showing all your Tailscale profiles
- **Active profile indicator** (★) shows which tailnet you're currently connected to
- **Profile-based** - uses Tailscale's built-in profile management (`tailscale switch`)
- **Optional config** for adding new tailnets or custom login servers
- Support for auth keys for automation
- Auto-detects sudo requirements

## How It Works

TailSwitch leverages Tailscale's fast user switching feature:
1. Shows your existing profiles from `tailscale switch --list`
2. Select a profile → instant switch (no authentication needed)
3. Optionally add new tailnets through config file
4. First-time login adds the profile for future instant switching

## Installation

### Prerequisites

- Rust toolchain (cargo)
- Tailscale installed and available in PATH

### Build from source

```bash
cargo build --release
```

The binary will be available at `target/release/tailswitch`

### Install

```bash
cargo install --path .
```

## Usage

Simply run:

```bash
tailswitch
```

Or if not installed:

```bash
cargo run
```

### TUI Controls

- `↑`/`↓` or `j`/`k`: Navigate through tailnets
- `Enter`: Select and switch to a tailnet
- `q` or `Esc`: Quit without switching

### What You'll See

```
★ tailnet1.example.com (active)
    user@example.com

  tailnet2.example.com
    user@example.com

+ NewTailnet (add new)
```

- **★** = Currently active profile
- Existing profiles switch instantly
- **+** = New profile from config (requires one-time login)

## Configuration (Optional)

**Note:** You usually don't need a config file! TailSwitch automatically shows your existing Tailscale profiles.

Only create `~/.config/tailswitch/config.toml` if you want to:
- Add a NEW tailnet you haven't logged into yet
- Use custom login servers (Headscale, etc.)
- Use auth keys for automation

### Example Config

```toml
# Add a new tailnet (browser auth on first use)
[[tailnets]]
name = "person@example.com"

# Self-hosted Headscale server
[[tailnets]]
name = "headscale-network"
login_server = "https://headscale.example.com"

# Automated with auth key
[[tailnets]]
name = "automation"
auth_key = "tskey-auth-xxxxx"
```

See [config.toml.example](config.toml.example) for more examples.

## Permissions

Tailscale requires elevated permissions. TailSwitch automatically detects this and uses `sudo` when needed.

**Recommended setup (run once):**
```bash
sudo tailscale set --operator=$USER
```

This allows you to run `tailswitch` without entering your password each time.

**Alternative:** The app will automatically use sudo and prompt for your password when needed.

## How Switching Works

### Switching to Existing Profile
1. Select a profile from the list
2. Runs `tailscale switch <profile-name>`
3. Done! Instant switch, no browser popup

### Adding New Profile
1. Add entry to config or select "add new" option
2. Runs `tailscale login`
3. Browser opens for one-time authentication
4. Select your tailnet in the browser
5. Profile is saved for future instant switching

## Troubleshooting

### Browser doesn't open
- TailSwitch uses `setsid` and environment variables to launch your browser
- If running from a TUI app like omarchy, the browser may not open automatically
- The authentication URL is displayed in the TUI - you can manually copy and open it

### Permission errors
- Run `sudo tailscale set --operator=$USER` once to avoid password prompts
- Or run with sudo: `sudo tailswitch` (use full path: `sudo ~/.cargo/bin/tailswitch`)

### Profile not appearing
- After logging in via browser, run `sudo tailscale switch --list` to verify the profile was created
- TailSwitch shows profiles from this command

## Technical Details

- Uses `tailscale switch --list` to enumerate profiles
- Uses `tailscale switch` for instant switching between profiles
- Uses `tailscale login` for adding new profiles
- No logout required when switching (preserves all profiles)
- Captures authentication URLs for TUI display
- Opens browser with proper environment variable handling

## License

MIT
