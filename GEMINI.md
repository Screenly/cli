# Screenly

The `screenly` MCP server manages a Screenly digital signage network: screens,
assets, asset groups, playlists, labels, shared playlists and Edge Apps. It runs
the official Screenly CLI (`screenly mcp`), which must be installed and on
`PATH`. See https://github.com/Screenly/cli#installation.

## How content reaches a screen

1. An **asset** is a piece of content: a web page, image or video
   (`asset_create` takes a public URL).
2. A **playlist** holds assets as **playlist items**, each with a duration and
   position. A playlist's **predicate** decides when it plays; the server's
   instructions describe the predicate syntax.
3. **Labels** connect playlists to screens: link the same label to a playlist
   (`label_link_playlist`) and to screens (`label_link_screen`).

To put HTML (for example a dashboard you just wrote) on screens, use
`edge_app_publish_from_html`, not `asset_create`.

## Working safely

- Prefer list and get tools to look things up before changing anything, and
  refer to objects by UUID.
- Confirm with the user before any `*_delete` tool. `asset_group_delete` also
  deletes every asset in the group.
- Changes apply to live screens right away, so state what will change before
  making updates to playlists, labels or assets that are already in use.

## Authentication

The server reads the `API_TOKEN` setting provided when this extension was
installed. To change it, run `gemini extensions config screenly`.
