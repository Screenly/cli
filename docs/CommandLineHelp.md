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

* `login` — Logs in with the provided token and stores it for further use if valid. You can set the API_TOKEN environment variable to override the stored token
* `logout` — Logs out and removes the stored token
* `screen` — Screen related commands
* `asset` — Asset related commands
* `playlist` — Playlist related commands
* `edge-app` — Edge App related commands
* `mcp` — Starts the MCP (Model Context Protocol) server on stdio for AI assistant integration

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly login`

Logs in with the provided token and stores it for further use if valid. You can set the API_TOKEN environment variable to override the stored token

**Usage:** `screenly login`



## `screenly logout`

Logs out and removes the stored token

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
* `set-headers` — Sets HTTP headers for a web asset
* `update-headers` — Updates HTTP headers for a web asset
* `basic-auth` — Sets up basic authentication headers for a web asset
* `bearer-auth` — Sets up bearer authentication headers for a web asset



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

Sets HTTP headers for a web asset

**Usage:** `screenly asset set-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset
* `<HEADERS>` — HTTP headers in the form `header1=value1[,header2=value2[,...]]`. This command replaces all headers of the asset with the given headers. Use an empty string (e.g., --set-headers "") to remove all existing headers



## `screenly asset update-headers`

Updates HTTP headers for a web asset

**Usage:** `screenly asset update-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset
* `<HEADERS>` — HTTP headers in the form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving other headers unchanged



## `screenly asset basic-auth`

Sets up basic authentication headers for a web asset

**Usage:** `screenly asset basic-auth <UUID> <CREDENTIALS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset
* `<CREDENTIALS>` — Basic authentication credentials in "user=password" form



## `screenly asset bearer-auth`

Sets up bearer authentication headers for a web asset

**Usage:** `screenly asset bearer-auth <UUID> <TOKEN>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset
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
* `update` — Updates a playlist from JSON input on stdin



## `screenly playlist create`

Creates a new playlist.

Playlists use a predicate DSL to control when they are shown. The predicate is a boolean expression using these variables:

$DATE    - Current date as Unix timestamp in milliseconds $TIME    - Time of day in ms since midnight (0-86400000) $WEEKDAY - Day of week (0=Sun, 1=Mon, ..., 6=Sat)

Operators: =, <=, >=, <, >, AND, OR, NOT Special: BETWEEN {min, max}, IN {val1, val2, ...}

Time reference (ms): 32400000=9AM, 43200000=12PM, 61200000=5PM

Examples: TRUE                                    - Always show $WEEKDAY IN {1, 2, 3, 4, 5}             - Weekdays only $TIME BETWEEN {32400000, 61200000}     - 9 AM to 5 PM NOT $WEEKDAY IN {0, 6}                  - Exclude weekends

**Usage:** `screenly playlist create [OPTIONS] <TITLE> [PREDICATE]`

###### **Arguments:**

* `<TITLE>` — Title of the new playlist
* `<PREDICATE>` — Predicate expression controlling when the playlist is shown.

   Variables:
     $DATE    - Unix timestamp in milliseconds
     $TIME    - Milliseconds since midnight (0-86400000)
     $WEEKDAY - Day of week (0=Sun, 1=Mon, ..., 6=Sat)

   Operators: =, <=, >=, <, >, AND, OR, NOT
   Special: BETWEEN {min, max}, IN {val1, val2, ...}

   Time reference: 32400000=9AM, 43200000=12PM, 61200000=5PM, 72000000=8PM

   Examples:
     TRUE                                - Always show
     $WEEKDAY IN {1, 2, 3, 4, 5}         - Weekdays only
     $TIME BETWEEN {32400000, 61200000}  - 9 AM to 5 PM
     NOT $WEEKDAY IN {0, 6}              - Exclude weekends

   Default: TRUE

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
* `<DURATION>` — Duration of the playlist item in seconds. Defaults to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist prepend`

Adds an asset to the beginning of the playlist

**Usage:** `screenly playlist prepend [OPTIONS] <UUID> <ASSET_UUID> [DURATION]`

###### **Arguments:**

* `<UUID>` — UUID of the playlist
* `<ASSET_UUID>` — UUID of the asset
* `<DURATION>` — Duration of the playlist item in seconds. Defaults to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly playlist update`

Updates a playlist from JSON input on stdin

**Usage:** `screenly playlist update`



## `screenly edge-app`

Edge App related commands

**Usage:** `screenly edge-app <COMMAND>`

###### **Subcommands:**

* `create` — Creates an Edge App in the store
* `list` — Lists your Edge Apps
* `rename` — Renames an Edge App
* `run` — Runs the Edge App emulator
* `setting` — Edge App setting commands
* `instance` — Edge App instance commands
* `deploy` — Deploys assets and settings of the Edge App and releases it
* `delete` — Deletes an Edge App. This cannot be undone
* `validate` — Validates the Edge App manifest file



## `screenly edge-app create`

Creates an Edge App in the store

**Usage:** `screenly edge-app create [OPTIONS] --name <NAME>`

###### **Options:**

* `-n`, `--name <NAME>` — Edge App name
* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-i`, `--in-place` — Use an existing Edge App directory with the manifest and index.html



## `screenly edge-app list`

Lists your Edge Apps

**Usage:** `screenly edge-app list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `screenly edge-app rename`

Renames an Edge App

**Usage:** `screenly edge-app rename [OPTIONS] --name <NAME>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-n`, `--name <NAME>` — New name for the Edge App



## `screenly edge-app run`

Runs the Edge App emulator

**Usage:** `screenly edge-app run [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-s`, `--secrets <SECRETS>` — Secrets to pass to the Edge App in the form KEY=VALUE. Can be specified multiple times
* `-g`, `--generate-mock-data` — Generates mock data for use with the Edge App emulator



## `screenly edge-app setting`

Edge App setting commands

**Usage:** `screenly edge-app setting <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App settings
* `set` — Sets an Edge App setting



## `screenly edge-app setting list`

Lists Edge App settings

**Usage:** `screenly edge-app setting list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-j`, `--json` — Enables JSON output



## `screenly edge-app setting set`

Sets an Edge App setting

**Usage:** `screenly edge-app setting set [OPTIONS] <SETTING_PAIR>`

###### **Arguments:**

* `<SETTING_PAIR>` — Key-value pair of the setting in the form `key=value`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly edge-app instance`

Edge App instance commands

**Usage:** `screenly edge-app instance <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App instances
* `create` — Creates an Edge App instance
* `delete` — Deletes an Edge App instance
* `update` — Updates an Edge App instance based on changes in instance.yml



## `screenly edge-app instance list`

Lists Edge App instances

**Usage:** `screenly edge-app instance list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-j`, `--json` — Enables JSON output



## `screenly edge-app instance create`

Creates an Edge App instance

**Usage:** `screenly edge-app instance create [OPTIONS]`

###### **Options:**

* `-n`, `--name <NAME>` — Name of the Edge App instance
* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly edge-app instance delete`

Deletes an Edge App instance

**Usage:** `screenly edge-app instance delete [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly edge-app instance update`

Updates an Edge App instance based on changes in instance.yml

**Usage:** `screenly edge-app instance update [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly edge-app deploy`

Deploys assets and settings of the Edge App and releases it

**Usage:** `screenly edge-app deploy [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory
* `-d`, `--delete-missing-settings <DELETE_MISSING_SETTINGS>` — Delete settings that exist on the server but not in the manifest

  Possible values: `true`, `false`




## `screenly edge-app delete`

Deletes an Edge App. This cannot be undone

**Usage:** `screenly edge-app delete [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly edge-app validate`

Validates the Edge App manifest file

**Usage:** `screenly edge-app validate [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. Defaults to the current working directory



## `screenly mcp`

Starts the MCP (Model Context Protocol) server on stdio for AI assistant integration

**Usage:** `screenly mcp`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

