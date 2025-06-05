# Screenly CLI - Development Reference

## Project Structure

This is a Rust CLI application for interacting with the Screenly digital signage platform API.

### Key Directories
- `src/` - Main source code
- `src/commands/` - Command implementations
- `src/api/` - API client abstractions
- `docs/` - Documentation
- `data/` - Static assets

## Architecture Overview

### Entry Point
- **File**: `src/main.rs:16`
- **Function**: `main()` - Sets up logging, Sentry error reporting, parses CLI args
- **Key dependency**: Uses `clap::Parser` for command parsing

### Command Line Interface
- **File**: `src/cli.rs`
- **Main struct**: `Cli` (line 36) - Root CLI structure
- **Command enum**: `Commands` (line 52) - All available commands
- **Handler**: `handle_cli()` (line 496) - Routes commands to handlers

### Authentication
- **File**: `src/authentication.rs`
- **Main struct**: `Authentication` (line 36)
- **Config**: Environment-based, supports `API_TOKEN` and `API_BASE_URL` env vars
- **Token storage**: Secure local storage using `dirs` crate

### Commands Structure

#### Screen Commands (`src/commands/screen.rs`)
- `list()` - GET /v4/screens
- `get(id)` - GET /v4/screens?id=eq.{id}
- `add(pin, name)` - POST /v4/screens
- `delete(id)` - DELETE /v4/screens?id=eq.{id}

#### Asset Commands (`src/commands/asset.rs`)
- `list()` - Lists all assets
- `add(path, title)` - Upload new asset (file or URL)
- `inject_js(uuid, code)` - Inject JavaScript into web assets
- `set_web_asset_headers()` - Set HTTP headers for web assets
- Authentication helpers: `basic_auth()`, `bearer_auth()`

#### Playlist Commands (`src/commands/playlist.rs`)
- `create(title, predicate)` - Create new playlist
- `append_asset()` - Add asset to end of playlist
- `prepend_asset()` - Add asset to beginning of playlist
- `update()` - Update playlist from JSON stdin

#### Edge App Commands (`src/commands/edge_app/`)
- **Main file**: `src/commands/edge_app/mod.rs`
- **Manifest handling**: `src/commands/edge_app/manifest.rs`
- **Local server**: `src/commands/edge_app/server.rs`
- Key operations:
  - `create()` - Create new Edge App
  - `deploy()` - Deploy app to Screenly
  - `run()` - Local development server
  - `validate()` - Validate manifest files

### API Layer
- **Base**: `src/api/mod.rs`
- **HTTP helpers**: `get()`, `post()`, `delete()`, `patch()` functions in `src/commands/mod.rs:137-234`
- **Authentication**: All requests use stored token via `Authentication::build_client()`

### Error Handling
- **Main enum**: `CommandError` in `src/commands/mod.rs:75-135`
- **Authentication errors**: `AuthenticationError` in `src/authentication.rs:16-34`
- **Pattern**: Functions return `Result<T, CommandError>`

### Output Formatting
- **Trait**: `Formatter` in `src/commands/mod.rs:29-31`
- **Types**: `OutputType::HumanReadable` (tables) vs `OutputType::Json`
- **Tables**: Uses `prettytable-rs` crate
- **JSON**: Pretty-printed via `serde_json`

## Key Dependencies

### Core
- `clap` (v4.0.17) - Command line parsing with derive macros
- `reqwest` (v0.11.12) - HTTP client with JSON support
- `serde` + `serde_json` - Serialization/deserialization
- `tokio` - Async runtime
- `anyhow` + `thiserror` - Error handling

### CLI Experience  
- `prettytable-rs` - Table formatting
- `indicatif` - Progress bars and human-readable durations
- `rpassword` - Secure password input
- `simple_logger` - Logging

### Edge Apps
- `serde_yaml` - YAML manifest parsing
- `warp` - Local development server
- `walkdir` - Directory traversal
- `sha1` + `sha2` - File hashing

## Development Workflows

### Adding New Commands
1. Add command variant to `Commands` enum in `src/cli.rs`
2. Create handler function following pattern: `handle_cli_{resource}_command()`
3. Implement command logic in `src/commands/{resource}.rs`
4. Add API calls if needed in `src/api/{resource}.rs`

### Testing
- Unit tests in each module (`#[cfg(test)]`)
- HTTP mocking via `httpmock` crate
- Temp directories via `tempfile` crate

### Build & Release
- **Build**: `cargo build --release`
- **Binary location**: `target/release/screenly`
- **Cross-platform**: Supports Windows, macOS, Linux
- **Packaging**: Homebrew, Nix, Docker, GitHub releases

### Configuration
- **API Server**: Set via `API_SERVER_NAME` at build time or `API_BASE_URL` at runtime
- **Tokens**: `API_TOKEN` env var or stored locally
- **Logging**: `RUST_LOG` env var

## File Patterns

### Command Handler Pattern
```rust
pub fn handle_cli_{resource}_command(command: &{Resource}Commands) {
    let authentication = Authentication::new().expect("Failed to load authentication.");
    let {resource}_command = commands::{resource}::{Resource}Command::new(authentication);
    
    match command {
        {Resource}Commands::List { json } => {
            handle_command_execution_result({resource}_command.list(), json);
        }
        // ... other commands
    }
}
```

### API Call Pattern
```rust
pub fn {operation}(&self, params...) -> Result<ResponseType, CommandError> {
    let endpoint = format!("v4/{resource}");
    let response = post(&self.authentication, &endpoint, &payload)?;
    Ok(ResponseType::new(response))
}
```

### Error Handling Pattern
- Use `?` operator for error propagation
- Convert to appropriate `CommandError` variant
- Display user-friendly messages in CLI handlers

## Security Notes
- API tokens stored securely using OS-specific directories
- HTTPS-only API communication
- Input validation on file paths and user input
- Confirmation prompts for destructive operations (delete commands)