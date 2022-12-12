# Screenly Command Line Interface (CLI)

The purpose of Screenly's CLI is to make developer's life easier. Using our CLI, users are able to quickly interact with Screenly through their terminal. Moreover, this CLI is built such that it can be used for automating tasks.

## Download

Releases are built automatically. You can download the latest release [here](https://github.com/Screenly/cli/releases/latest).

## Building

To build the Screenly CLI, you need to install [Rust](https://www.rust-lang.org). The instructions for installing latest rust can be found [here](https://www.rust-lang.org/tools/install).

Then you just need to invoke the following command from inside the CLI directory:

```bash
cargo build --release
```

the `screenly` binary will be located in `target/release` directory.

