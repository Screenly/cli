# Screenly Command Line Interface (CLI)

The purpose of Screenly's CLI is to make developer's life easier. Using our CLI, users are able to quickly interact with Screenly through their terminal. Moreover, this CLI is built such that it can be used for automating tasks.

## Installation

Releases are built automatically. You can download the latest release [here](https://github.com/Screenly/cli/releases/latest).

On macOS you can also use [Homebrew](https://brew.sh/) to install the latest version.

```bash
$ brew tap screenly/screenly-cli
$ brew install screenly-cli
```

For other operating systems, you can either use the pre-compiled binaries, or use our Docker wrapper:

```bash
$ docker run --rm \
    -e API_TOKEN=YOUR_API_TOKEN \
    screenly/cli:latest \
    help
[...]
```

## Usage

### Assets

You can:

* List your assets (`list`)
* Get info on a particular asset (`get`)
* Delete an asset (`delete`)
* Inject JavaScript that runs when a web asset is rendered (`inject-js`)
  * See the [JavaScript Injector Examples](https://github.com/Screenly/playground/tree/master/javascript-injectors) on [Screenly Playground](https://github.com/Screenly/playground/)
* Set custom HTTP heeaders when web assets are rendered (`set-headers`)
  * This is useful for for Bearer Tokens (e.g. [logging into Grafana](https://www.screenly.io/use-cases/dashboard/grafana/)) or Basic Auth
* Helper function to set [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication) for authentication against web asset  (`basic-auth`)


#### Examples

Add a web asset:

```bash
$ screenly asset add https://news.ycombinator.com "Hacker News"
+----------------------------+-------------+------+--------+
| Id                         | Title       | Type | Status |
+----------------------------+-------------+------+--------+
| XXXXXXXXXXXXXXXXXXXXXXXXX  | Hacker News | N/A  | none   |
+----------------------------+-------------+------+--------+
```

Upload a HTML file:

```bash
$ screenly asset add path/to/file.html "My File"
+----------------------------+-------------+------+--------+
| Id                         | Title       | Type | Status |
+----------------------------+-------------+------+--------+
| XXXXXXXXXXXXXXXXXXXXXXXXX  | My File     | N/A  | none   |
+----------------------------+-------------+------+--------+
```

This file will be served locally on your Screenly Player. You (currently) need to inline HTML/CSS/Images.

You can also use the `--json` feature, which is handy in conjuction with `jq` for getting say the Asset ID of a particular asset:

```bash
$ screenly asset list --json | \
    jq -r '.[] | select (.title|test("Hacker News")) | .id'
XXXXXXXXXXXXXXXXXXXXXXXXXX

```
### Interact with screens

You can:

* List your screens (`list`)
* Get info on a particular screen (`get`)
* Add/Pair a screen (`add`)
* Revoke/delete (`delete`)


#### Examples

Listing screens:

```bash
> $ screenly screen list
+----------------------------+-----------------------+-----------------------+---------+---------------------------------+-------------------+
| Id                         | Name                  | Hardware Version      | In Sync | Last Ping                       | Uptime            |
+----------------------------+-----------------------+-----------------------+---------+---------------------------------+-------------------+
| XXXXXXXXXXXXXXXXXXXXXXXXXX | Lobby Screen          | Screenly Player Max   |   ✅    | 2023-01-22T09:56:23.89686+00:00 | 8days 23h 18m 53s |
+----------------------------+-----------------------+-----------------------+---------+---------------------------------+-------------------+
| XXXXXXXXXXXXXXXXXXXXXXXXXX | Grafana Dashboard     | Raspberry Pi 3B+      |   ✅    | 2023-01-22T09:54:17.88319+00:00 | 10days 22h 9m 32s |
+----------------------------+-----------------------+-----------------------+---------+---------------------------------+-------------------+
```

## Building

To build the Screenly CLI, you need to install [Rust](https://www.rust-lang.org). The instructions for installing latest rust can be found [here](https://www.rust-lang.org/tools/install).

Then you just need to invoke the following command from inside the CLI directory:

```bash
$ cargo build --release
```

the `screenly` binary will be located in `target/release` directory.


## GitHub Action

Our CLI is also available as a GitHub Action workflow.

## Inputs

### `screenly_api_token`

**Required** The Screenly API token for your team. You can retrieve this by going to `Settings` -> `Team` -> `Tokens`. Note that API tokens are limited in scope to your team.

You should use a [GitHub Action Secret](https://docs.github.com/en/actions/security-guides/encrypted-secrets) to store this rather than hard coding this in your code base.

### `cli_commands`

**Required** This is the command you want to pass on, such as `screen list`.

### `cli_version`

Use this option to override the CLI version used by the Action. Must point to a [valid release](https://github.com/Screenly/cli/releases).

## Example usage

```yaml
uses: screenly/cli@master
with:
  screenly_api_token: ${{ secrets.SCREENLY_API_TOKEN }}
  cli_commands: screen list
```
