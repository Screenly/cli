# Command-Line Help for `cli`

This document contains the help content for the `cli` command-line program.

**Command Overview:**

* [`cli`↴](#cli)
* [`cli login`↴](#cli-login)
* [`cli logout`↴](#cli-logout)
* [`cli screen`↴](#cli-screen)
* [`cli screen list`↴](#cli-screen-list)
* [`cli screen get`↴](#cli-screen-get)
* [`cli screen add`↴](#cli-screen-add)

* [`cli asset delete`↴](#cli-asset-delete)
* [`cli asset inject-js`↴](#cli-asset-inject-js)
* [`cli asset set-headers`↴](#cli-asset-set-headers)
* [`cli asset update-headers`↴](#cli-asset-update-headers)
* [`cli asset basic-auth`↴](#cli-asset-basic-auth)
* [`cli asset bearer-auth`↴](#cli-asset-bearer-auth)
* [`cli playlist`↴](#cli-playlist)
* [`cli playlist create`↴](#cli-playlist-create)
* [`cli playlist list`↴](#cli-playlist-list)
* [`cli playlist get`↴](#cli-playlist-get)
* [`cli playlist delete`↴](#cli-playlist-delete)
* [`cli playlist append`↴](#cli-playlist-append)
* [`cli playlist prepend`↴](#cli-playlist-prepend)
* [`cli playlist update`↴](#cli-playlist-update)
* [`cli edge-app`↴](#cli-edge-app)
* [`cli edge-app create`↴](#cli-edge-app-create)
* [`cli edge-app list`↴](#cli-edge-app-list)
* [`cli edge-app rename`↴](#cli-edge-app-rename)
* [`cli edge-app run`↴](#cli-edge-app-run)
* [`cli edge-app version`↴](#cli-edge-app-version)
* [`cli edge-app version list`↴](#cli-edge-app-version-list)
* [`cli edge-app version promote`↴](#cli-edge-app-version-promote)
* [`cli edge-app setting`↴](#cli-edge-app-setting)
* [`cli edge-app setting list`↴](#cli-edge-app-setting-list)
* [`cli edge-app setting set`↴](#cli-edge-app-setting-set)
* [`cli edge-app secret`↴](#cli-edge-app-secret)
* [`cli edge-app secret list`↴](#cli-edge-app-secret-list)
* [`cli edge-app secret set`↴](#cli-edge-app-secret-set)
* [`cli edge-app upload`↴](#cli-edge-app-upload)
* [`cli edge-app delete`↴](#cli-edge-app-delete)
* [`cli edge-app validate`↴](#cli-edge-app-validate)

## `cli`

Command line interface is intended for quick interaction with Screenly through terminal. Moreover, this CLI is built such that it can be used for automating tasks.

**Usage:** `cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `login` — Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token
* `logout` — Logouts and removes stored token
* `screen` — Screen related commands
* `asset` — Asset related commands
* `playlist` — Playlist related commands
* `edge-app` — Edge App related commands

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli login`

Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token

**Usage:** `cli login`



## `cli logout`

Logouts and removes stored token

**Usage:** `cli logout`



## `cli screen`

Screen related commands

**Usage:** `cli screen <COMMAND>`

###### **Subcommands:**

* `list` — Lists your screens
* `get` — Gets a single screen by id
* `add` — Adds a new screen
* `delete` — Deletes a screen. This cannot be undone



## `cli screen list`

Lists your screens

**Usage:** `cli screen list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli screen get`

Gets a single screen by id

**Usage:** `cli screen get [OPTIONS] <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the screen

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli screen add`

Adds a new screen

**Usage:** `cli screen add [OPTIONS] <PIN> [NAME]`

###### **Arguments:**

* `<PIN>` — Pin code created with registrations endpoint
* `<NAME>` — Optional name of the new screen

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli screen delete`

Deletes a screen. This cannot be undone

**Usage:** `cli screen delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the screen to be deleted



## `cli asset`

Asset related commands

**Usage:** `cli asset <COMMAND>`

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



## `cli asset list`

Lists your assets

**Usage:** `cli asset list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli asset get`

Gets a single asset by id

**Usage:** `cli asset get [OPTIONS] <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the asset

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli asset add`

Adds a new asset

**Usage:** `cli asset add [OPTIONS] <PATH> <TITLE>`

###### **Arguments:**

* `<PATH>` — Path to local file or URL for remote file
* `<TITLE>` — Asset title

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli asset delete`

Deletes an asset. This cannot be undone

**Usage:** `cli asset delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the asset to be deleted



## `cli asset inject-js`

Injects JavaScript code inside of the web asset. It will be executed once the asset loads during playback

**Usage:** `cli asset inject-js <UUID> <PATH>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to inject with JavaScript
* `<PATH>` — Path to local file or URL for remote file



## `cli asset set-headers`

Sets HTTP headers for web asset

**Usage:** `cli asset set-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set http headers
* `<HEADERS>` — HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command replaces all headers of the asset with the given headers (when an empty string is given, e.g. --set-headers "", all existing headers are removed, if any)



## `cli asset update-headers`

Updates HTTP headers for web asset

**Usage:** `cli asset update-headers <UUID> <HEADERS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set http headers
* `<HEADERS>` — HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving any other headers unchanged



## `cli asset basic-auth`

Shortcut for setting up basic authentication headers

**Usage:** `cli asset basic-auth <UUID> <CREDENTIALS>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set up basic authentication for
* `<CREDENTIALS>` — Basic authentication credentials in "user=password" form



## `cli asset bearer-auth`

Shortcut for setting up bearer authentication headers

**Usage:** `cli asset bearer-auth <UUID> <TOKEN>`

###### **Arguments:**

* `<UUID>` — UUID of the web asset to set up basic authentication for
* `<TOKEN>` — Bearer token



## `cli playlist`

Playlist related commands

**Usage:** `cli playlist <COMMAND>`

###### **Subcommands:**

* `create` — Creates a new playlist
* `list` — Lists your playlists
* `get` — Gets a single playlist by id
* `delete` — Deletes a playlist. This cannot be undone
* `append` — Adds an asset to the end of the playlist
* `prepend` — Adds an asset to the beginning of the playlist
* `update` — Patches a given playlist



## `cli playlist create`

Creates a new playlist

**Usage:** `cli playlist create [OPTIONS] <TITLE> [PREDICATE]`

###### **Arguments:**

* `<TITLE>` — Title of the new playlist
* `<PREDICATE>` — Predicate for the new playlist. If not specified it will be set to "TRUE"

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli playlist list`

Lists your playlists

**Usage:** `cli playlist list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli playlist get`

Gets a single playlist by id

**Usage:** `cli playlist get <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the playlist



## `cli playlist delete`

Deletes a playlist. This cannot be undone

**Usage:** `cli playlist delete <UUID>`

###### **Arguments:**

* `<UUID>` — UUID of the playlist to be deleted



## `cli playlist append`

Adds an asset to the end of the playlist

**Usage:** `cli playlist append [OPTIONS] <UUID> <ASSET_UUID> [DURATION]`

###### **Arguments:**

* `<UUID>` — UUID of the playlist
* `<ASSET_UUID>` — UUID of the asset
* `<DURATION>` — Duration of the playlist item in seconds. If not specified it will be set to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli playlist prepend`

Adds an asset to the beginning of the playlist

**Usage:** `cli playlist prepend [OPTIONS] <UUID> <ASSET_UUID> [DURATION]`

###### **Arguments:**

* `<UUID>` — UUID of the playlist
* `<ASSET_UUID>` — UUID of the asset
* `<DURATION>` — Duration of the playlist item in seconds. If not specified it will be set to 15 seconds

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli playlist update`

Patches a given playlist

**Usage:** `cli playlist update`



## `cli edge-app`

Edge App related commands

**Usage:** `cli edge-app <COMMAND>`

###### **Subcommands:**

* `create` — Creates Edge App in the store
* `list` — Lists your Edge Apps
* `rename` — Renames Edge App
* `run` — Runs Edge App emulator
* `version` — Version commands
* `setting` — Settings commands
* `secret` — Secrets commands
* `upload` — Uploads assets and settings of the Edge App
* `delete` — Deletes an Edge App. This cannot be undone
* `validate` — Validates Edge App manifest file



## `cli edge-app create`

Creates Edge App in the store

**Usage:** `cli edge-app create [OPTIONS] --name <NAME>`

###### **Options:**

* `-n`, `--name <NAME>` — Edge App name
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-i`, `--in-place` — Use an existing Edge App directory with the manifest and index.html



## `cli edge-app list`

Lists your Edge Apps

**Usage:** `cli edge-app list [OPTIONS]`

###### **Options:**

* `-j`, `--json` — Enables JSON output



## `cli edge-app rename`

Renames Edge App

**Usage:** `cli edge-app rename [OPTIONS] --name <NAME>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-a`, `--app-id <APP_ID>` — Edge App id. If not specified CLI will use the id from the manifest
* `-n`, `--name <NAME>` — Edge App name



## `cli edge-app run`

Runs Edge App emulator

**Usage:** `cli edge-app run [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-s`, `--secrets <SECRETS>`
* `-g`, `--generate-mock-data` — Generates mock data to be used with Edge App run



## `cli edge-app version`

Version commands

**Usage:** `cli edge-app version <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App versions
* `promote` — Promotes Edge App revision to channel



## `cli edge-app version list`

Lists Edge App versions

**Usage:** `cli edge-app version list [OPTIONS]`

###### **Options:**

* `-a`, `--app-id <APP_ID>` — Edge app id. If not specified CLI will use the id from the manifest
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-j`, `--json` — Enables JSON output



## `cli edge-app version promote`

Promotes Edge App revision to channel

**Usage:** `cli edge-app version promote [OPTIONS]`

###### **Options:**

* `-r`, `--revision <REVISION>` — Edge app revision to promote
* `-c`, `--channel <CHANNEL>` — Channel to promote to. If not specified CLI will use stable channel

  Default value: `stable`
* `-i`, `--installation-id <INSTALLATION_ID>` — Edge App Installation id. If not specified, CLI will use the installation_id from the manifest
* `--latest` — Use the latest revision of the Edge App

  Default value: `false`
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `cli edge-app setting`

Settings commands

**Usage:** `cli edge-app setting <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App settings
* `set` — Sets Edge App setting



## `cli edge-app setting list`

Lists Edge App settings

**Usage:** `cli edge-app setting list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-i`, `--installation-id <INSTALLATION_ID>` — Edge App Installation id. If not specified, CLI will use the installation_id from the manifest
* `-j`, `--json` — Enables JSON output



## `cli edge-app setting set`

Sets Edge App setting

**Usage:** `cli edge-app setting set [OPTIONS] <SETTING_PAIR>`

###### **Arguments:**

* `<SETTING_PAIR>` — Key value pair of the setting to be set in the form of `key=value`

###### **Options:**

* `-i`, `--installation-id <INSTALLATION_ID>` — Edge App Installation id. If not specified, CLI will use the installation_id from the manifest
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `cli edge-app secret`

Secrets commands

**Usage:** `cli edge-app secret <COMMAND>`

###### **Subcommands:**

* `list` — Lists Edge App secrets
* `set` — Sets Edge App secret



## `cli edge-app secret list`

Lists Edge App secrets

**Usage:** `cli edge-app secret list [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-i`, `--installation-id <INSTALLATION_ID>` — Edge App Installation id. If not specified, CLI will use the installation_id from the manifest
* `-j`, `--json` — Enables JSON output



## `cli edge-app secret set`

Sets Edge App secret

**Usage:** `cli edge-app secret set [OPTIONS] <SECRET_PAIR>`

###### **Arguments:**

* `<SECRET_PAIR>` — Key value pair of the secret to be set in the form of `key=value`

###### **Options:**

* `-i`, `--installation-id <INSTALLATION_ID>` — Edge App Installation id. If not specified, CLI will use the installation_id from the manifest
* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



## `cli edge-app upload`

Uploads assets and settings of the Edge App

**Usage:** `cli edge-app upload [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-a`, `--app-id <APP_ID>` — Edge App id. If not specified CLI will use the id from the manifest
* `-d`, `--delete-missing-settings <DELETE_MISSING_SETTINGS>`

  Possible values: `true`, `false`




## `cli edge-app delete`

Deletes an Edge App. This cannot be undone

**Usage:** `cli edge-app delete [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory
* `-a`, `--app-id <APP_ID>` — Edge App id. If not specified CLI will use the id from the manifest



## `cli edge-app validate`

Validates Edge App manifest file

**Usage:** `cli edge-app validate [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the directory with the manifest. If not specified CLI will use the current working directory



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

