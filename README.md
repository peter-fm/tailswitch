# Tailswitch

A Rust TUI for easily switching between Tailscale tailnets using Tailscale's fast user switching feature.

## Features

- **Instant switching** between existing tailnets (no browser popups!)
- **Interactive TUI** showing all your Tailscale profiles
- **Active profile indicator** (★) shows which tailnet you're currently connected to
- **In-app commands** - check status, update flags, logout without leaving the TUI
- **Persistent flags** - configure `--ssh`, `--accept-routes`, etc. that persist across re-authentication
- **Profile-based** - uses Tailscale's built-in profile management (`tailscale switch`)
- **Optional config** for adding new tailnets or custom login servers
- Support for auth keys for automation
- Auto-detects sudo requirements

## How It Works

Tailswitch leverages Tailscale's fast user switching feature:
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

**Main Menu:**
- `↑`/`↓` or `j`/`k`: Navigate through tailnets
- `Enter`: Select and switch to a tailnet
- `s`: Show current Tailscale status
- `u`: Update connection with configured flags (apply `--ssh`, `--accept-routes`, etc.)
- `l`: Logout from current tailnet
- `q`: Quit the application

**Output Screens (status, logout, etc.):**
- `Enter` or `Esc`: Return to main menu
- `q`: Quit the application

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

**Note:** You usually don't need a config file! Tailswitch automatically shows your existing Tailscale profiles.

Only create `~/.config/tailswitch/config.toml` if you want to:
- Add a NEW tailnet you haven't logged into yet
- Use custom login servers (Headscale, etc.)
- Use auth keys for automation
- **Specify persistent flags** for `tailscale up` (like `--ssh`, `--accept-routes`)

### Example Config

```toml
# Add persistent flags that apply when logging in or re-authenticating
# These are especially useful when your connection expires
[[tailnets]]
name = "person@example.com"
flags = ["--ssh", "--accept-routes"]

# Self-hosted Headscale server
[[tailnets]]
name = "headscale-network"
login_server = "https://headscale.example.com"

# Automated with auth key and flags
[[tailnets]]
name = "automation"
auth_key = "tskey-auth-xxxxx"
flags = ["--ssh", "--accept-routes", "--advertise-exit-node"]
```

See [config.toml.example](config.toml.example) for more examples.

### Updating Flags for Existing Connections

If you're already logged in and want to apply new flags (like `--ssh` or `--accept-routes`):

1. Create/update your config file with the desired flags
2. Run `tailswitch`
3. Navigate to your active connection
4. Press `u` to update the connection with the configured flags
5. The flags are applied immediately without logging out!

## Permissions

Tailscale requires elevated permissions. Tailswitch automatically detects this and uses `sudo` when needed.

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
- Tailswitch uses `setsid` and environment variables to launch your browser
- If running from a TUI app like omarchy, the browser may not open automatically
- The authentication URL is displayed in the TUI - you can manually copy and open it

### Permission errors
- Run `sudo tailscale set --operator=$USER` once to avoid password prompts
- Or run with sudo: `sudo tailswitch` (use full path: `sudo ~/.cargo/bin/tailswitch`)

### Profile not appearing
- After logging in via browser, run `sudo tailscale switch --list` to verify the profile was created
- Tailswitch shows profiles from this command

## Common Use Cases

### Applying SSH and Route Acceptance Flags

Many users want SSH access and subnet routes enabled by default:

```toml
[[tailnets]]
name = "your-tailnet@example.com"
flags = ["--ssh", "--accept-routes"]
```

Then press `u` in the TUI to apply these flags immediately.

### Re-authentication After Expiration

When your Tailscale connection expires, TailSwitch will:
1. Detect the logged-out state
2. Look up your configured flags from the config file
3. Re-authenticate with those flags automatically
4. You stay connected with all your preferred settings!

### Quick Status Check

Press `s` at any time to see your current Tailscale status without leaving the TUI.

## Technical Details

- Uses `tailscale switch --list` to enumerate profiles
- Uses `tailscale switch` for instant switching between profiles
- Uses `tailscale login` for adding new profiles
- Uses `tailscale up` to apply flags to existing connections
- No logout required when switching (preserves all profiles)
- Captures authentication URLs for TUI display
- Opens browser with proper environment variable handling
- Config-based persistent flags survive re-authentication

## License

MIT
