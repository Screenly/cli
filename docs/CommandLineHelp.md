# Command-Line Help for `screenly`

This document contains the help content for the `screenly` command-line program.

**Command Overview:**

* [`screenly`↴](#screenly)
* [`screenly login`↴](#screenly-login)
* [`screenly logout`↴](#screenly-logout)
* [`screenly screen`↴](#screenly-screen)
* [`screenly screen list`↴](#screenly-screen-list)
* [`screenly screen get`↴](#screenly-screen-get)
* [`screenly screen add`↴](#screenly-screen-add)
* [`screenly screen delete`↴](#screenly-screen-delete)
* [`screenly asset`↴](#screenly-asset)
* [`screenly asset list`↴](#screenly-asset-list)
* [`screenly asset get`↴](#screenly-asset-get)
* [`screenly asset add`↴](#screenly-asset-add)
* [`screenly asset delete`↴](#screenly-asset-delete)
* [`screenly asset inject-js`↴](#screenly-asset-inject-js)
* [`screenly asset set-headers`↴](#screenly-asset-set-headers)
* [`screenly asset update-headers`↴](#screenly-asset-update-headers)
* [`screenly asset basic-auth`↴](#screenly-asset-basic-auth)
* [`screenly asset bearer-auth`↴](#screenly-asset-bearer-auth)
* [`screenly playlist`↴](#screenly-playlist)
* [`screenly playlist create`↴](#screenly-playlist-create)
* [`screenly playlist list`↴](#screenly-playlist-list)
* [`screenly playlist get`↴](#screenly-playlist-get)
* [`screenly playlist delete`↴](#screenly-playlist-delete)
* [`screenly playlist append`↴](#screenly-playlist-append)
* [`screenly playlist prepend`↴](#screenly-playlist-prepend)
* [`screenly playlist update`↴](#screenly-playlist-update)
* [`screenly edge-app`↴](#screenly-edge-app)
* [`screenly edge-app create`↴](#screenly-edge-app-create)
* [`screenly edge-app list`↴](#screenly-edge-app-list)
* [`screenly edge-app rename`↴](#screenly-edge-app-rename)
* [`screenly edge-app run`↴](#screenly-edge-app-run)
* [`screenly edge-app setting`↴](#screenly-edge-app-setting)
* [`screenly edge-app setting list`↴](#screenly-edge-app-setting-list)
* [`screenly edge-app setting set`↴](#screenly-edge-app-setting-set)
* [`screenly edge-app instance`↴](#screenly-edge-app-instance)
* [`screenly edge-app instance list`↴](#screenly-edge-app-instance-list)
* [`screenly edge-app instance create`↴](#screenly-edge-app-instance-create)
* [`screenly edge-app instance delete`↴](#screenly-edge-app-instance-delete)
* [`screenly edge-app instance update`↴](#screenly-edge-app-instance-update)
* [`screenly edge-app deploy`↴](#screenly-edge-app-deploy)
* [`screenly edge-app delete`↴](#screenly-edge-app-delete)
* [`screenly edge-app validate`↴](#screenly-edge-app-validate)
* [`screenly mcp`↴](#screenly-mcp)

## `screenly`

Command line interface is intended for quick interaction with Screenly through terminal. Moreover, this CLI is built such that it can be used for automating tasks.

**Usage:** `screenly [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `login` — Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token
* `logout` — Logouts and removes stored token
* `screen` — Screen related commands
* `asset` — Asset related commands
* `playlist` — Playlist related commands
* `edge-app` — Edge App related commands
* `mcp` — Starts the MCP (Model Context Protocol) server on stdio for AI assistant integration

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly login`

Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token

**Usage:** `screenly login`



## `screenly logout`

Logouts and removes stored token

**Usage:** `screenly logout`



## `screenly screen`

Screen related commands

**Usage:** `screenly screen <COMMAND>`

###### **Subcommands:**

* `list` — Lists your screens
* `get` — Gets a single screen by id
* `add` — Adds a new screen
* `delete` — Deletes a screen. This cannot be undone



## `screenly screen list`

Lists your screens

**Usage:** `screenly screen list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly screen get`

Gets a single screen by id

**Usage:** `screenly screen get [OPTIONS] <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the screen

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly screen add`

Adds a new screen

**Usage:** `screenly screen add [OPTIONS] <PIN> [NAME]`

###### **Arguments:**

* `<PIN>` — Pin code created with registrations endpoint
* `<NAME>` — Optional name of the new screen

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly screen delete`

Deletes a screen. This cannot be undone

**Usage:** `screenly screen delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the screen to be deleted



## `screenly asset`

Asset related commands

**Usage:** `screenly asset <COMMAND>`

###### **Subcommands:**

* `list` — Lists your assets
* `get` — Gets a single asset by id
* `add` — Adds a new asset
* `delete` — Deletes an asset. This cannot be undone
* `inject-js` — Injects JavaScript code inside of the web asset. It will be executed once the asset loads during playback
* `set-headers` — Sets HTTP headers for web asset
* `update-headers` — Updates HTTP headers for web asset
* `basic-auth` — Shortcut for setting up basic authentication headers
* `bearer-auth` — Shortcut for setting up bearer authentication headers



## `screenly asset list`

Lists your assets

**Usage:** `screenly asset list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly asset get`

Gets a single asset by id

**Usage:** `screenly asset get [OPTIONS] <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the asset

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly asset add`

Adds a new asset

**Usage:** `screenly asset add [OPTIONS] <PATH> <TITLE>`

###### **Arguments:**

* `<PATH>` — Path to local file or URL for remote file
* `<TITLE>` — Asset title

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly asset delete`

Deletes an asset. This cannot be undone

**Usage:** `screenly asset delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the asset to be deleted



## `screenly asset inject-js`

Injects JavaScript code inside of the web asset. It will be executed once the asset loads during playback

**Usage:** `screenly asset inject-js <UUID> <PATH>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to inject with JavaScript
* `<PATH>` — Path to local file or URL for remote file



## `screenly asset set-headers`

Sets HTTP headers for web asset

**Usage:** `screenly asset set-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set http headers
* `<HEADERS>` — HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command replaces all headers of the asset with the given headers (when an empty string is given, e.g. --set-headers "", all existing headers are removed, if any)



## `screenly asset update-headers`

Updates HTTP headers for web asset

**Usage:** `screenly asset update-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set http headers
* `<HEADERS>` — HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving any other headers unchanged



## `screenly asset basic-auth`

Shortcut for setting up basic authentication headers

**Usage:** `screenly asset basic-auth <UUID> <CREDENTIALS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set up basic authentication for
* `<CREDENTIALS>` — Basic authentication credentials in "user=password" form



## `screenly asset bearer-auth`

Shortcut for setting up bearer authentication headers

**Usage:** `screenly asset bearer-auth <UUID> <TOKEN>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set up basic authentication for
* `<TOKEN>` — Bearer token



## `screenly playlist`

Playlist related commands

**Usage:** `screenly playlist <COMMAND>`

###### **Subcommands:**

* `create` — Creates a new playlist
* `list` — Lists your playlists
* `get` — Gets a single playlist by id
* `delete` — Deletes a playlist. This cannot be undone
* `append` — Adds an asset to the end of the playlist
* `prepend` — Adds an asset to the beginning of the playlist
* `update` — Patches a given playlist



## `screenly playlist create`

Creates a new playlist

**Usage:** `screenly playlist create [OPTIONS] <TITLE> [PREDICATE]`

###### **Arguments:**

* `<TITLE>` — Title of the new playlist
* `<PREDICATE>` — Predicate for the new playlist. If not specified it will be set to "TRUE"

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist list`

Lists your playlists

**Usage:** `screenly playlist list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist get`

Gets a single playlist by id

**Usage:** `screenly playlist get <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the playlist



## `screenly playlist delete`

Deletes a playlist. This cannot be undone

**Usage:** `screenly playlist delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the playlist to be deleted



## `screenly playlist append`

Adds an asset to the end of the playlist

**Usage:** `screenly playlist append [OPTIONS] <UUID> <ASSET_UUID> [DURATION]`

###### **Arguments:**

* `<UUID>` — UUID of the playlist
* `<ASSET_UUID>` — UUID of the asset
* `<DURATION>` — Duration of the playlist item in seconds. If not specified it will be set to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist prepend`

Adds an asset to the beginning of the playlist

**Usage:** `screenly playlist prepend [OPTIONS] <UUID> <ASSET_UUID> [DURATION]`

###### **Arguments:**

* `<UUID>` — UUID of the playlist
* `<ASSET_UUID>` — UUID of the asset
* `<DURATION>` — Duration of the playlist item in seconds. If not specified it will be set to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist update`

Patches a given playlist

**Usage:** `screenly playlist update`



## `screenly edge-app`

Edge App related commands

**Usage:** `screenly edge-app <COMMAND>`

###### **Subcommands:**

* `create` — Creates Edge App in the store
* `list` — Lists your Edge Apps
* `rename` — Renames Edge App
* `run` — Runs Edge App emulator
* `setting` — Settings commands
* `instance` — Instance commands
* `deploy` — Deploys assets and settings of the Edge App and release it
* `delete` — Deletes an Edge App. This cannot be undone
* `validate` — Validates Edge App manifest file



## `screenly edge-app create`

Creates Edge App in the store

**Usage:** `screenly edge-app create [OPTIONS] --name <NAME>`

###### **Options:**

* `-n`, `--name <NAME>` — Edge App name
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-i`, `--in-place` — Use an existing Edge App directory with the manifest and index.html



## `screenly edge-app list`

Lists your Edge Apps

**Usage:** `screenly edge-app list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly edge-app rename`

Renames Edge App

**Usage:** `screenly edge-app rename [OPTIONS] --name <NAME>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-n`, `--name <NAME>` — Edge App name



## `screenly edge-app run`

Runs Edge App emulator

**Usage:** `screenly edge-app run [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-s`, `--secrets <SECRETS>` — Secrets to be passed to the Edge App in the form KEY=VALUE. Can be specified multiple times
* `-g`, `--generate-mock-data` — Generates mock data to be used with Edge App run



## `screenly edge-app setting`

Settings commands

**Usage:** `screenly edge-app setting <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App settings
* `set` — Sets Edge App setting



## `screenly edge-app setting list`

Lists Edge App settings

**Usage:** `screenly edge-app setting list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-j`, `--json` — Enables JSON output



## `screenly edge-app setting set`

Sets Edge App setting

**Usage:** `screenly edge-app setting set [OPTIONS] <SETTING_PAIR>`

###### **Arguments:**

* `<SETTING_PAIR>` — Key value pair of the setting to be set in the form of `key=value`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly edge-app instance`

Instance commands

**Usage:** `screenly edge-app instance <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App instances
* `create` — Creates Edge App instance
* `delete` — Deletes Edge App instance
* `update` — Update Edge App instance based on changes in the instance.yml



## `screenly edge-app instance list`

Lists Edge App instances

**Usage:** `screenly edge-app instance list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-j`, `--json` — Enables JSON output



## `screenly edge-app instance create`

Creates Edge App instance

**Usage:** `screenly edge-app instance create [OPTIONS]`

###### **Options:**

* `-n`, `--name <NAME>` — Name of the Edge App instance
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly edge-app instance delete`

Deletes Edge App instance

**Usage:** `screenly edge-app instance delete [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly edge-app instance update`

Update Edge App instance based on changes in the instance.yml

**Usage:** `screenly edge-app instance update [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly edge-app deploy`

Deploys assets and settings of the Edge App and release it

**Usage:** `screenly edge-app deploy [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-d`, `--delete-missing-settings <DELETE_MISSING_SETTINGS>`

  Possible values: `true`, `false`




## `screenly edge-app delete`

Deletes an Edge App. This cannot be undone

**Usage:** `screenly edge-app delete [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly edge-app validate`

Validates Edge App manifest file

**Usage:** `screenly edge-app validate [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `screenly mcp`

Starts the MCP (Model Context Protocol) server on stdio for AI assistant integration

**Usage:** `screenly mcp`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

