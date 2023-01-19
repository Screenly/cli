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

## Building

To build the Screenly CLI, you need to install [Rust](https://www.rust-lang.org). The instructions for installing latest rust can be found [here](https://www.rust-lang.org/tools/install).

Then you just need to invoke the following command from inside the CLI directory:

```bash
cargo build --release
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
