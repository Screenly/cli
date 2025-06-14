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
- Update [Homebrew repo](https://github.com/Screenly/homebrew-screenly-cli) with the latest version.
