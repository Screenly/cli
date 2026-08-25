[![sbomified](https://sbomify.com/assets/images/logo/badge.svg)](https://app.sbomify.com/component/UUzAdk8ixV)
[![Lint](https://github.com/Screenly/cli/actions/workflows/lint.yml/badge.svg)](https://github.com/Screenly/cli/actions/workflows/lint.yml)
[![Rust](https://github.com/Screenly/cli/actions/workflows/rust.yml/badge.svg)](https://github.com/Screenly/cli/actions/workflows/rust.yml)
[![Nix](https://github.com/Screenly/cli/actions/workflows/nix.yml/badge.svg)](https://github.com/Screenly/cli/actions/workflows/nix.yml)

# Screenly Command Line Interface (CLI)

The Screenly CLI simplifies interactions with Screenly through your terminal, designed for both manual use and task automation.

## Installation

### From Releases

Download the latest release [here](https://github.com/Screenly/cli/releases/latest).

### Homebrew (macOS only)

```bash
$ brew tap screenly/screenly-cli
$ brew install screenly-cli
```

### Nix

```bash
$ nix-shell -p screenly-cli
```

### Docker

For other operating systems or Docker usage:

```bash
$ docker run --rm \
    -e API_TOKEN=YOUR_API_TOKEN \
    screenly/cli:latest help
```

## Building from Source

To build the Screenly CLI from source, ensure you have [Rust](https://www.rust-lang.org) installed:

```bash
$ cargo build --release
```

> [!NOTE]
> If you're building from source in Ubuntu, make sure to install `build-essential`:
> ```bash
> sudo apt-get install -y build-essential
> ```
>
> Otherwise, you'll get the following error:
> ```
> error: linker `cc` not found
> ```

The `screenly` binary will be located in `target/release`.

To configure a non-production API server, set the `API_SERVER_NAME` environment variable:

```bash
$ API_SERVER_NAME=local cargo build --release
```

## Commands

Explore available commands [here](https://developer.screenly.io/cli/#commands).

## Authentication and profiles

Credentials are stored in `~/.screenly` as named profiles, with one profile active at a time. This lets you keep several tokens (for example one per workspace) and switch between them.

```bash
# Log in. On a fresh install this creates the "default" profile; with a
# profile already active it updates that profile (e.g. after a token rotation).
$ screenly login

# Log in under a specific profile name.
$ screenly login --name work

# Log in without a prompt, for scripts and CI. --token-stdin is optional when
# stdin is already a pipe or a file, and required to skip the prompt on a
# terminal.
$ echo "$SCREENLY_TOKEN" | screenly login --token-stdin --name ci

# Show the profile you are currently authenticated as.
$ screenly me

# List stored profiles (the active one is marked with *). Honors --output.
$ screenly auth list

# Switch the active profile.
$ screenly auth switch work

# Remove a profile. Without --name, removes the active one.
$ screenly logout
$ screenly logout --name work
```

Removing the active profile leaves no profile active, even when others are still stored. The CLI will not pick a replacement for you, because doing so would silently point the next command at a different workspace. Run `screenly auth switch <name>` to choose one.

The `API_TOKEN` environment variable overrides the stored profiles when set, so `me` and every other command authenticate with that token regardless of the active profile.

Plain-text `~/.screenly` files from older versions are migrated to the profile format automatically on first write.

If `~/.screenly` becomes malformed (for example from a hand-edit), the CLI reports the problem and leaves the file untouched rather than discarding credentials. Fix the file's YAML, or delete it and run `screenly login` to start fresh.

The file holds every profile's token in plain text. On Linux and macOS it is created with `0600` permissions, so only your user can read it. On Windows it inherits the permissions of your home directory, so treat it the way you would any other credentials file. Avoid running two `login` commands at the same time: the CLI replaces the file atomically, but it does not lock it, so simultaneous writes can drop one of the two profiles.

## Output Formats

All list and get commands support three output formats via the global `--output` (`-o`) flag:

| Format | Flag | Description |
|--------|------|-------------|
| Table | `--output table` | Human-readable table (default) |
| JSON | `--output json` | JSON output |
| CSV | `--output csv` | CSV output, suitable for piping to files or other tools |

```bash
# Human-readable table (default)
$ screenly screen list

# JSON output
$ screenly --output json asset list

# CSV output saved to a file
$ screenly --output csv screen list > screens.csv

# JSON output saved to a file
$ screenly --output json screen list > screens.json
```

> [!NOTE]
> Log messages go to stderr, so redirecting stdout to a file captures only command output.
> Use `RUST_LOG` to change the log level, or `RUST_LOG=off` to silence logging entirely.

## MCP Server (AI Assistant Integration)

The Screenly CLI includes a built-in [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server, enabling AI assistants like Claude, Cursor, and others to interact with your Screenly digital signage network.

### Starting the MCP Server

```bash
$ screenly mcp
```

The server communicates over stdio and exposes the full Screenly API as tools.

### Available Tools

| Category | Tools |
|----------|-------|
| **Screens** | `screen_list`, `screen_get` |
| **Assets** | `asset_list`, `asset_get`, `asset_create`, `asset_update`, `asset_delete` |
| **Asset Groups** | `asset_group_list`, `asset_group_create`, `asset_group_update`, `asset_group_delete` |
| **Playlists** | `playlist_list`, `playlist_create`, `playlist_update`, `playlist_delete` |
| **Playlist Items** | `playlist_item_list`, `playlist_item_create`, `playlist_item_update`, `playlist_item_delete` |
| **Labels** | `label_list`, `label_create`, `label_update`, `label_delete`, `label_link_screen`, `label_unlink_screen`, `label_link_playlist`, `label_unlink_playlist` |
| **Shared Playlists** | `shared_playlist_list`, `shared_playlist_create`, `shared_playlist_delete` |
| **Edge Apps** | `edge_app_list`, `edge_app_list_settings`, `edge_app_list_instances` |

Every tool is annotated with behaviour hints (`readOnlyHint`, `destructiveHint`, `idempotentHint`), so MCP clients can tell read-only tools apart from ones that modify or delete data and prompt for confirmation before destructive actions.

### Configuration Examples

#### Claude Desktop Extension (`.mcpb`)

For [Claude Desktop](https://claude.ai/download), the expected install path is
**Desktop Extensions** (Settings → Extensions) — the same idea as installing
the CLI with Homebrew. Once Screenly is listed, install it there and paste your
API token when prompted. No manual JSON editing required.

For testing before the listing is live, you can sideload a `.mcpb` from the
[latest release](https://github.com/Screenly/cli/releases/latest). macOS release bundles are
not Developer ID–signed yet (same as the CLI `.tar.gz` artifacts); a browser download may be
blocked by Gatekeeper. If that happens, use **System Settings → Privacy & Security → Open Anyway**.
Details: [`mcpb/README.md`](mcpb/README.md).

The token is stored in your operating system's keychain rather than a plaintext config file.

#### Cursor / other clients

Add to your MCP configuration file:

```json
{
  "mcpServers": {
    "screenly": {
      "command": "screenly",
      "args": ["mcp"],
      "env": {
        "API_TOKEN": "your-api-token-here"
      }
    }
  }
}
```

#### Authentication

The MCP server uses the same authentication as the CLI:
- Set the `API_TOKEN` environment variable, or
- Run `screenly login` to store credentials in `~/.screenly`

## GitHub Action

Integrate Screenly CLI into your GitHub workflows:

### Inputs

#### `screenly_api_token`

**Required** Screenly API token for your team.

#### `cli_commands`

**Required** Command to execute (e.g., `screen list`).

#### `cli_version`

Optional CLI version override.

### Example usage

```yaml
uses: screenly/cli@master
with:
  screenly_api_token: ${{ secrets.SCREENLY_API_TOKEN }}
  cli_commands: screen list
```

## Protocol Buffers (Protobuf) Generation

Generate `pb_signature.rs` from `signature.proto`:

```bash
$ cargo install protobuf-codegen
$ protoc --rust_out . signature.proto
$ mv signature.rs src/pb_signature.rs
```

## Release Process

This project follows [Semantic Versioning](https://semver.org/) (M.m.p = Major.minor.patch).

1. **Prepare the release:**
  - Create a release branch (e.g., `release-M.m.p`, like `release-1.0.6`).
  - Update the version in `Cargo.toml`, `action.yml`, and `Dockerfile`
  - Run `cargo build` to update `Cargo.lock` with the new version

2. **Create and merge the pull request:**
  - Create a pull request from the release branch to `master`
  - Once approved, merge the pull request

3. **Create the GitHub release:**
  - Make sure that you're on the `master` branch and have pulled the latest changes
  - Create a version tag (e.g., `vM.m.p`, like `v1.0.6`) and push it to GitHub by running:
    ```bash
    git tag vM.m.p
    git push origin vM.m.p
    ```
  - The release workflow will detect the version tag and create the release automatically
  - Add the release notes to the GitHub release description

4. **Update Homebrew:**
   - Update the [Homebrew repo](https://github.com/Screenly/homebrew-screenly-cli) with the latest version
