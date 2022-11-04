# Screenly Command Line Interface (CLI)

The purpose of Screenly's CLI is to make developer's life easier. Using our CLI, users are able to quickly interact with Screenly through their terminal. Morover, this CLI is built such that it can be used for automating tasks.

# Building

To build the screenly cli you need to install rust. The instructions for installing latest rust can be found here https://www.rust-lang.org/tools/install
Then you just need to invoke the following command from inside the cli directory.

```
cargo build --release
```

`screenly` binary will be located in `target/release` directory.
