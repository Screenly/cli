# Screenly Command Line Interface (CLI)

The Screenly CLI simplifies interactions with Screenly through your terminal, designed for both manual use and task automation.

## Installation

### From Releases

Download the latest release [here](https://github.com/Screenly/cli/releases/latest).

#### macOS (via Homebrew)

```bash
$ brew tap screenly/screenly-cli
$ brew install screenly-cli
```

### Docker

For other operating systems or Docker usage:

```bash
$ docker run --rm -e API_TOKEN=YOUR_API_TOKEN screenly/cli:latest help
```

## Building from Source

To build the Screenly CLI from source, ensure you have [Rust](https://www.rust-lang.org) installed:

```bash
$ cargo build --release
```

The `screenly` binary will be located in `target/release`.

To configure a non-production API server, set the `API_SERVER_NAME` environment variable:

```bash
$ API_SERVER_NAME=local cargo build --release
```

## Commands

Explore available commands [here](https://github.com/Screenly/cli/blob/master/docs/CommandLineHelp.md).

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

- Merge PRs into `master`.
- Update version in `Cargo.toml`, `action.yml`, `Dockerfile`, and GitHub Actions configurations.
- Create release branch (e.g., `release-1.0.0`) and tag (e.g., `v1.0.0`).
- Update Homebrew repo with the latest version.
