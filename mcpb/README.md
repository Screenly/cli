# Screenly for Claude Desktop

Manage your [Screenly](https://www.screenly.io) digital signage network from Claude.

This is the packaging directory for the Screenly MCP Bundle (`.mcpb`). The bundle wraps
the MCP server that ships inside the [Screenly CLI](https://github.com/Screenly/cli) so it
can be installed into Claude Desktop with a single click, with no terminal setup required.

## Installation

1. Download `screenly-cli-<your-platform>.mcpb` from the
   [latest release](https://github.com/Screenly/cli/releases/latest), for example
   `screenly-cli-aarch64-apple-darwin.mcpb` on an Apple Silicon Mac.
2. Open the file. Claude Desktop shows an installation dialog.
3. Paste your Screenly API token when prompted. You can generate a token in the
   [Screenly console](https://console.screenly.io) under **Settings → Tokens**.

## What you can do

Once installed, you can ask Claude to:

- Review your screens and check which ones are offline or out of sync
- Add a web page, image, or video as an asset
- Build a playlist and schedule it, for example "only during business hours on weekdays"
- Organise content with asset groups and labels
- Share a playlist with another team
- Inspect Edge Apps, their settings, and their instances

## Capabilities

The bundle exposes 33 tools. Every tool is annotated so Claude knows whether it only reads
data or modifies your account, which means Claude will ask for confirmation before doing
anything destructive.

| Category | Tools |
| --- | --- |
| Screens | `screen_list`, `screen_get` |
| Assets | `asset_list`, `asset_get`, `asset_create`, `asset_update`, `asset_delete` |
| Asset Groups | `asset_group_list`, `asset_group_create`, `asset_group_update`, `asset_group_delete` |
| Playlists | `playlist_list`, `playlist_create`, `playlist_update`, `playlist_delete` |
| Playlist Items | `playlist_item_list`, `playlist_item_create`, `playlist_item_update`, `playlist_item_delete` |
| Labels | `label_list`, `label_create`, `label_update`, `label_delete`, `label_link_screen`, `label_unlink_screen`, `label_link_playlist`, `label_unlink_playlist` |
| Shared Playlists | `shared_playlist_list`, `shared_playlist_create`, `shared_playlist_delete` |
| Edge Apps | `edge_app_list`, `edge_app_list_settings`, `edge_app_list_instances` |

Twelve of these tools are read-only. Eight are destructive and delete content or remove
associations.

## Authentication

The bundle authenticates with a Screenly API token, which you provide during installation.
The token is marked as sensitive in the bundle manifest, so Claude Desktop stores it in your
operating system's keychain rather than in a plaintext configuration file.

Tokens are scoped to a single Screenly team. To limit what the extension can reach, use a
token for a team that contains only the screens you want Claude to manage. You can revoke a
token at any time from the Screenly console.

## Privacy Policy

Screenly's privacy policy is available at
<https://www.screenly.io/privacy-policy/>.

This extension connects to the Screenly API at `api.screenlyapp.com` over HTTPS (or another
Screenly API host if configured via `API_BASE_URL`). Requests are made directly from your
machine using the API token you supply.

The data returned to Claude is the data you ask for: your screens, assets, playlists,
labels, and Edge Apps, along with their metadata. Your API token is sent only to Screenly,
as the credential for those API requests.

Separately, the Screenly CLI initializes [Sentry](https://sentry.io) crash reporting on
startup (including when run as this extension). If the process panics, diagnostic details
such as the stack trace and device/OS context may be sent to Sentry's ingest endpoint
(`*.ingest.sentry.io`). This is used for reliability debugging, not product analytics.
Screenly's handling of that data is covered by the privacy policy linked above.

## Support

- Documentation: <https://developer.screenly.io/mcp>
- Issues: <https://github.com/Screenly/cli/issues>

## Building the bundle

Bundles are built automatically for each tagged release by
[`.github/workflows/release.yml`](../.github/workflows/release.yml), which injects the
release version into `manifest.json` and packs it together with the compiled `screenly`
binary.

To build one locally:

```bash
npm install -g @anthropic-ai/mcpb

cargo build --release

mkdir -p build/server
cp mcpb/manifest.json mcpb/README.md build/
cp target/release/screenly build/server/
mcpb pack build screenly.mcpb
```

The `version` field in the committed `manifest.json` is a `0.0.0` placeholder. The release
workflow replaces it with the Git tag so it cannot drift from `Cargo.toml`.
