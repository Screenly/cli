use std::io::{Read, Write};
use std::path::PathBuf;
use std::{env, fs, io};

use clap::{Parser, Subcommand};
use http_auth_basic::Credentials;
use log::{error, info};
use reqwest::StatusCode;
use rpassword::read_password;
use serde_json::json;
use thiserror::Error;

use crate::authentication::{
    active_profile_name, fetch_profile_info, fetch_profiles_with_info, verify_and_store_token,
    Authentication, AuthenticationError, Config, ProfileEntry,
};
use crate::commands;
use crate::commands::edge_app::instance_manifest::InstanceManifest;
use crate::commands::edge_app::manifest::EdgeAppManifest;
use crate::commands::edge_app::server::MOCK_DATA_FILENAME;
use crate::commands::edge_app::utils::{
    transform_edge_app_path_to_manifest, transform_instance_path_to_instance_manifest,
    validate_manifests_dependacies,
};
use crate::commands::playlist::PlaylistCommand;
use crate::commands::{CommandError, Formatter, OutputType, PlaylistFile};
const DEFAULT_ASSET_DURATION: u32 = 15;

/// Returns a user-friendly error message for authentication errors.
fn get_authentication_error_message(e: &AuthenticationError) -> String {
    let not_logged_in = "Not logged in. Please run `screenly login` first to authenticate.";
    match e {
        // The logged-out state now leaves an empty store behind rather than
        // deleting the file, so it surfaces as NoCredentials, not Io(NotFound).
        AuthenticationError::NoCredentials => not_logged_in.to_string(),
        AuthenticationError::Io(io_err) if io_err.kind() == std::io::ErrorKind::NotFound => {
            not_logged_in.to_string()
        }
        AuthenticationError::NoActiveProfile => {
            "No active profile. Run `screenly auth switch <name>` to choose one, or `screenly auth list` to see what is stored.".to_string()
        }
        AuthenticationError::ProfileNotFound(name) => {
            format!("Active profile '{name}' not found. Run `screenly auth switch` to pick a valid profile.")
        }
        // Already actionable and names the file; pass it through verbatim.
        AuthenticationError::CorruptStore { .. } => e.to_string(),
        _ => {
            format!("Authentication error: {e}. Please run `screenly login` to authenticate.")
        }
    }
}

/// Resolves the profile name a `login` should store under.
///
/// An explicit `--name` is always honored. With no name given, `login`
/// updates the currently active profile (the common re-login-after-rotation
/// flow), falling back to `"default"` on a fresh install with no active
/// profile.
fn resolve_login_name(name: Option<&str>, active: Option<&str>) -> String {
    match name {
        Some(n) => n.to_string(),
        None => active.unwrap_or("default").to_string(),
    }
}

/// Renders the stored profiles as an aligned table, marking the active
/// profile with `*`. Returns a `String` so it can be unit-tested and reused
/// across output formats. Column widths account for the header labels and for
/// the `(unavailable)` placeholder shown when a profile's info can't be
/// fetched.
fn format_profiles_table(entries: &[ProfileEntry]) -> String {
    // Resolve each row's cells first so the widths cover placeholders too.
    let rows: Vec<(&str, String, String, bool)> = entries
        .iter()
        .map(|e| match &e.info {
            Some(info) => (
                e.name.as_str(),
                info.email.clone(),
                info.workspace.clone(),
                e.is_active,
            ),
            None => (
                e.name.as_str(),
                "(unavailable)".to_string(),
                "(unavailable)".to_string(),
                e.is_active,
            ),
        })
        .collect();

    let name_w = rows
        .iter()
        .map(|r| r.0.len())
        .max()
        .unwrap_or(0)
        .max("Profile".len());
    let email_w = rows
        .iter()
        .map(|r| r.1.len())
        .max()
        .unwrap_or(0)
        .max("Email".len());

    let mut lines = vec![
        format!("  {:<name_w$}  {:<email_w$}  Workspace", "Profile", "Email"),
        format!("  {:-<name_w$}  {:-<email_w$}  ---------", "", ""),
    ];
    for (name, email, workspace, is_active) in &rows {
        let marker = if *is_active { "*" } else { " " };
        lines.push(format!(
            "{marker} {name:<name_w$}  {email:<email_w$}  {workspace}"
        ));
    }
    lines.join("\n")
}

/// The current profile's details, rendered for the `me` command.
struct ProfileDetails {
    profile: String,
    email: String,
    workspace: String,
}

impl Formatter for ProfileDetails {
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            OutputType::Json => serde_json::to_string_pretty(&json!({
                "profile": self.profile,
                "email": self.email,
                "workspace": self.workspace,
            }))
            .unwrap(),
            OutputType::Csv => {
                let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
                wtr.write_record(["Profile", "Email", "Workspace"]).unwrap();
                wtr.write_record([
                    self.profile.as_str(),
                    self.email.as_str(),
                    self.workspace.as_str(),
                ])
                .unwrap();
                String::from_utf8(wtr.into_inner().unwrap()).unwrap()
            }
            OutputType::HumanReadable => format!(
                "Profile:   {}\nEmail:     {}\nWorkspace: {}",
                self.profile, self.email, self.workspace
            ),
        }
    }
}

/// The stored profiles, rendered for the `auth list` command.
struct ProfilesTable(Vec<ProfileEntry>);

impl Formatter for ProfilesTable {
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            // The "no profiles" hint is only useful to a human. The machine
            // formats render an empty array / a bare header instead, so a
            // consumer piping `--output json` always gets parseable output.
            OutputType::HumanReadable if self.0.is_empty() => {
                "No profiles stored. Run `screenly login` to add one.".to_string()
            }
            OutputType::HumanReadable => format_profiles_table(&self.0),
            OutputType::Json => {
                let arr: Vec<serde_json::Value> = self
                    .0
                    .iter()
                    .map(|e| {
                        json!({
                            "profile": e.name,
                            "active": e.is_active,
                            "email": e.info.as_ref().map(|i| i.email.clone()),
                            "workspace": e.info.as_ref().map(|i| i.workspace.clone()),
                        })
                    })
                    .collect();
                serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap()
            }
            OutputType::Csv => {
                let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
                wtr.write_record(["Profile", "Active", "Email", "Workspace"])
                    .unwrap();
                for e in &self.0 {
                    let active = e.is_active.to_string();
                    let (email, workspace) = match &e.info {
                        Some(i) => (i.email.as_str(), i.workspace.as_str()),
                        None => ("", ""),
                    };
                    wtr.write_record([e.name.as_str(), active.as_str(), email, workspace])
                        .unwrap();
                }
                String::from_utf8(wtr.into_inner().unwrap()).unwrap()
            }
        }
    }
}

/// Reports an authentication error and exits.
///
/// Every command that touches the credential store funnels its unhandled
/// authentication errors through here, so a corrupt store or a missing profile
/// reads the same whichever command hit it, and never surfaces as a `Debug`
/// dump.
fn exit_with_authentication_error(e: &AuthenticationError) -> ! {
    error!("{}", get_authentication_error_message(e));
    std::process::exit(1);
}

/// Creates an Authentication instance or exits with a user-friendly error message.
fn get_authentication() -> Authentication {
    match Authentication::new() {
        Ok(auth) => auth,
        Err(e) => exit_with_authentication_error(&e),
    }
}

#[derive(Error, Debug)]
enum ParseError {
    #[error("missing \"=\" symbol")]
    MissingSymbol(),
}

fn parse_key_val(s: &str) -> Result<(String, String), ParseError> {
    let pos = s.find('=').ok_or(ParseError::MissingSymbol())?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq)]
pub enum OutputFormat {
    /// Human-readable table (default).
    #[default]
    Table,
    /// JSON output.
    Json,
    /// CSV output.
    Csv,
}

#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "Command line interface is intended for quick interaction with Screenly through terminal. Moreover, this CLI is built such that it can be used for automating tasks."
)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Output format: table (default), json, or csv.
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    pub(crate) output: OutputFormat,

    /// Deprecated: use --output json instead.
    #[arg(long, hide = true, global = true, conflicts_with = "output")]
    pub(crate) json: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Logs in with the provided token and stores it for further use if valid. You can set the API_TOKEN environment variable to override the stored token.
    Login {
        /// Profile name to store the token under. Defaults to the active
        /// profile, or "default" on a fresh install.
        #[arg(long)]
        name: Option<String>,
    },
    /// Logs out and removes the stored token.
    Logout {
        /// Profile name to remove. Removes the active profile if not specified.
        #[arg(long)]
        name: Option<String>,
    },
    /// Show information about the currently authenticated profile.
    Me {},
    /// Manage stored authentication profiles.
    #[command(subcommand)]
    Auth(AuthCommands),
    /// Screen related commands.
    #[command(subcommand)]
    Screen(ScreenCommands),
    /// Asset related commands.
    #[command(subcommand)]
    Asset(AssetCommands),
    /// Playlist related commands.
    #[command(subcommand)]
    Playlist(PlaylistCommands),
    /// Edge App related commands.
    #[command(subcommand)]
    EdgeApp(EdgeAppCommands),
    /// Starts the MCP (Model Context Protocol) server on stdio for AI assistant integration.
    Mcp {},
    /// For generating `docs/CommandLineHelp.md`.
    #[clap(hide = true)]
    PrintHelpMarkdown {},
}

#[derive(Subcommand)]
pub enum AuthCommands {
    /// List stored authentication profiles.
    List {},
    /// Switch the active authentication profile.
    Switch {
        /// Profile name to activate. If omitted, the available profiles are
        /// listed and the command exits with an error.
        name: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenCommands {
    /// Lists your screens.
    List,
    /// Gets a single screen by id.
    Get {
        /// UUID of the screen.
        uuid: String,
    },
    /// Adds a new screen.
    Add {
        /// Pin code created with registrations endpoint.
        pin: String,
        /// Optional name of the new screen.
        name: Option<String>,
    },
    /// Deletes a screen. This cannot be undone.
    Delete {
        /// UUID of the screen to be deleted.
        uuid: String,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaylistCommands {
    /// Creates a new playlist.
    ///
    /// Playlists use a predicate DSL to control when they are shown.
    /// The predicate is a boolean expression using these variables:
    ///
    ///   $DATE    - Current date as Unix timestamp in milliseconds
    ///   $TIME    - Time of day in ms since midnight (0-86400000)
    ///   $WEEKDAY - Day of week (0=Sun, 1=Mon, ..., 6=Sat)
    ///
    /// Operators: =, <=, >=, <, >, AND, OR, NOT
    /// Special: BETWEEN {min, max}, IN {val1, val2, ...}
    ///
    /// Time reference (ms): 32400000=9AM, 43200000=12PM, 61200000=5PM
    ///
    /// Examples:
    ///   TRUE                                    - Always show
    ///   $WEEKDAY IN {1, 2, 3, 4, 5}             - Weekdays only
    ///   $TIME BETWEEN {32400000, 61200000}     - 9 AM to 5 PM
    ///   NOT $WEEKDAY IN {0, 6}                  - Exclude weekends
    Create {
        /// Title of the new playlist.
        title: String,
        /// Predicate expression controlling when the playlist is shown.
        /// Uses DSL with $DATE, $TIME, $WEEKDAY variables. Default: "TRUE".
        #[arg(
            long_help = "Predicate expression controlling when the playlist is shown.\n\n\
            Variables:\n  \
            $DATE    - Unix timestamp in milliseconds\n  \
            $TIME    - Milliseconds since midnight (0-86400000)\n  \
            $WEEKDAY - Day of week (0=Sun, 1=Mon, ..., 6=Sat)\n\n\
            Operators: =, <=, >=, <, >, AND, OR, NOT\n\
            Special: BETWEEN {min, max}, IN {val1, val2, ...}\n\n\
            Time reference: 32400000=9AM, 43200000=12PM, 61200000=5PM, 72000000=8PM\n\n\
            Examples:\n  \
            TRUE                                - Always show\n  \
            $WEEKDAY IN {1, 2, 3, 4, 5}         - Weekdays only\n  \
            $TIME BETWEEN {32400000, 61200000}  - 9 AM to 5 PM\n  \
            NOT $WEEKDAY IN {0, 6}              - Exclude weekends\n\n\
            Default: TRUE"
        )]
        predicate: Option<String>,
    },
    /// Lists your playlists.
    List,
    /// Gets a single playlist by id.
    Get {
        /// UUID of the playlist.
        uuid: String,
    },
    /// Deletes a playlist. This cannot be undone.
    Delete {
        /// UUID of the playlist to be deleted.
        uuid: String,
    },
    /// Adds an asset to the end of the playlist.
    Append {
        /// UUID of the playlist.
        uuid: String,
        /// UUID of the asset.
        asset_uuid: String,
        /// Duration of the playlist item in seconds. Defaults to 15 seconds.
        duration: Option<u32>,
    },
    /// Adds an asset to the beginning of the playlist.
    Prepend {
        /// UUID of the playlist.
        uuid: String,
        /// UUID of the asset.
        asset_uuid: String,
        /// Duration of the playlist item in seconds. Defaults to 15 seconds.
        duration: Option<u32>,
    },
    /// Updates a playlist from JSON input on stdin.
    Update,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Headers {
    // this struct is only needed because I was getting panic from clap when trying to directly use Vec<(String, String)> and parse it.
    // it really did not want to deal with vector when argaction was not set to Append.
    headers: Vec<(String, String)>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Secrets {
    secrets: Vec<(String, String)>,
}

pub trait KeyValuePairs {
    fn new(pairs: Vec<(String, String)>) -> Self;
}

impl KeyValuePairs for Headers {
    fn new(pairs: Vec<(String, String)>) -> Self {
        Headers { headers: pairs }
    }
}

impl KeyValuePairs for Secrets {
    fn new(pairs: Vec<(String, String)>) -> Self {
        Secrets { secrets: pairs }
    }
}

fn parse_key_values<T: KeyValuePairs>(s: &str) -> Result<T, ParseError> {
    if s.is_empty() {
        return Ok(T::new(Vec::new()));
    }

    let mut pairs = Vec::new();
    let elements = s.split(',');
    for element in elements {
        pairs.push(parse_key_val(element)?);
    }
    Ok(T::new(pairs))
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetCommands {
    /// Lists your assets.
    List,
    /// Gets a single asset by id.
    Get {
        /// UUID of the asset.
        uuid: String,
    },
    /// Adds a new asset.
    Add {
        /// Path to local file or URL for remote file.
        path: String,
        /// Asset title.
        title: String,
    },

    /// Deletes an asset. This cannot be undone.
    Delete {
        /// UUID of the asset to be deleted.
        uuid: String,
    },

    /// Injects JavaScript code inside of the web asset. It will be executed once the asset loads during playback.
    InjectJs {
        /// UUID of the web asset to inject with JavaScript.
        uuid: String,

        /// Path to local file or URL for remote file.
        path: String,
    },

    /// Sets HTTP headers for a web asset.
    SetHeaders {
        /// UUID of the web asset.
        uuid: String,

        /// HTTP headers in the form `header1=value1[,header2=value2[,...]]`. This command
        /// replaces all headers of the asset with the given headers. Use an empty string
        /// (e.g., --set-headers "") to remove all existing headers.
        #[arg(value_parser = parse_key_values::<Headers>)]
        headers: Headers,
    },
    /// Updates HTTP headers for a web asset.
    UpdateHeaders {
        /// UUID of the web asset.
        uuid: String,

        /// HTTP headers in the form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving other headers unchanged.
        #[arg(value_parser=parse_key_values::<Headers>)]
        headers: Headers,
    },

    /// Sets up basic authentication headers for a web asset.
    BasicAuth {
        /// UUID of the web asset.
        uuid: String,
        /// Basic authentication credentials in "user=password" form.
        #[arg(value_parser = parse_key_val)]
        credentials: (String, String),
    },
    /// Sets up bearer authentication headers for a web asset.
    BearerAuth {
        /// UUID of the web asset.
        uuid: String,
        /// Bearer token.
        token: String,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppCommands {
    /// Creates an Edge App in the store.
    Create {
        /// Edge App name.
        #[arg(short, long)]
        name: String,
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
        /// Use an existing Edge App directory with the manifest and index.html.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        in_place: Option<bool>,
        /// Remote entrypoint URL. When set, the created app uses
        /// entrypoint.type = remote-global with this URL and a starter
        /// `screenly_inject.js` is dropped next to `screenly.yml`. The
        /// inject file is shipped with each deploy and the player runs it
        /// on every load.
        #[arg(short, long)]
        entrypoint: Option<String>,
    },

    /// Lists your Edge Apps.
    List,
    /// Renames an Edge App.
    Rename {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// New name for the Edge App.
        #[arg(short, long)]
        name: String,
    },

    /// Runs the Edge App emulator.
    Run {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Secrets to pass to the Edge App in the form KEY=VALUE. Can be specified multiple times.
        #[arg(short, long, value_parser = parse_key_values::<Secrets>)]
        secrets: Option<Secrets>,

        /// Generates mock data for use with the Edge App emulator.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        generate_mock_data: Option<bool>,
    },

    /// Edge App setting commands.
    #[command(subcommand)]
    Setting(EdgeAppSettingsCommands),

    /// Edge App instance commands.
    #[command(subcommand)]
    Instance(EdgeAppInstanceCommands),

    /// Deploys assets and settings of the Edge App and releases it.
    Deploy {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Delete settings that exist on the server but not in the manifest.
        #[arg(short, long)]
        delete_missing_settings: Option<bool>,
    },
    /// Deletes an Edge App. This cannot be undone.
    Delete {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Validates the Edge App manifest file.
    Validate {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppSettingsCommands {
    /// Lists Edge App settings.
    List {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Sets an Edge App setting.
    Set {
        /// Key-value pair of the setting in the form `key=value`.
        #[arg(value_parser = parse_key_val)]
        setting_pair: (String, String),

        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppInstanceCommands {
    /// Lists Edge App instances.
    List {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Creates an Edge App instance.
    Create {
        /// Name of the Edge App instance.
        #[arg(short, long)]
        name: Option<String>,

        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Deletes an Edge App instance.
    Delete {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Updates an Edge App instance based on changes in instance.yml.
    Update {
        /// Path to the directory with the manifest. Defaults to the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

pub fn handle_command_execution_result<T: Formatter>(
    result: anyhow::Result<T, CommandError>,
    output: OutputFormat,
) {
    if output == OutputFormat::Csv && !T::supports_csv() {
        error!("CSV output is not supported for this command. Use --output table or --output json instead.");
        std::process::exit(1);
    }
    match result {
        Ok(screen) => {
            let output_type = match output {
                OutputFormat::Json => OutputType::Json,
                OutputFormat::Csv => OutputType::Csv,
                OutputFormat::Table => OutputType::HumanReadable,
            };
            let formatted = screen.format(output_type);
            if output == OutputFormat::Csv {
                print!("{formatted}");
            } else {
                println!("{formatted}");
            }
        }
        Err(e) => {
            match e {
                CommandError::Authentication(_) => {
                    error!(
                        "Authentication error occurred. Please use login command to authenticate."
                    )
                }
                _ => {
                    error!("Error occurred: {e:?}");
                }
            }
            std::process::exit(1);
        }
    }
}

pub fn get_screen_name(
    id: &str,
    screen_command: &commands::screen::ScreenCommand,
) -> Result<String, CommandError> {
    let target_screen = screen_command.get(id)?;

    if let Some(screens) = target_screen.value.as_array() {
        if screens.is_empty() {
            error!("Screen could not be found.");
            return Err(CommandError::MissingField);
        }

        return if let Some(name) = screens[0]["name"].as_str() {
            Ok(name.to_string())
        } else {
            Err(CommandError::MissingField)
        };
    }

    Err(CommandError::MissingField)
}

pub fn get_asset_title(
    id: &str,
    asset_command: &commands::asset::AssetCommand,
) -> Result<String, CommandError> {
    let target_asset = asset_command.get(id)?;

    if let Some(assets) = target_asset.value.as_array() {
        if assets.is_empty() {
            error!("Asset could not be found.");
            return Err(CommandError::MissingField);
        }

        return if let Some(name) = assets[0]["title"].as_str() {
            Ok(name.to_string())
        } else {
            Err(CommandError::MissingField)
        };
    }

    Err(CommandError::MissingField)
}

pub fn handle_cli(cli: &Cli) {
    let output = if cli.json {
        eprintln!("Warning: --json is deprecated, use --output json instead.");
        OutputFormat::Json
    } else {
        cli.output
    };

    match &cli.command {
        Commands::Login { name } => {
            let active = active_profile_name();
            let resolved_name = resolve_login_name(name.as_deref(), active.as_deref());
            print!("Enter your API Token: ");
            std::io::stdout().flush().unwrap();
            let token = read_password().unwrap();
            match verify_and_store_token(&token, &resolved_name, &Config::default().url) {
                Ok(()) => {
                    info!("Login credentials have been saved under profile '{resolved_name}'.");
                    std::process::exit(0);
                }

                Err(e) => match e {
                    AuthenticationError::WrongCredentials => {
                        error!("Token verification failed.");
                        std::process::exit(1);
                    }
                    _ => exit_with_authentication_error(&e),
                },
            }
        }
        Commands::Screen(command) => handle_cli_screen_command(command, output),
        Commands::Asset(command) => handle_cli_asset_command(command, output),
        Commands::EdgeApp(command) => handle_cli_edge_app_command(command, output),
        Commands::Playlist(command) => handle_cli_playlist_command(command, output),
        Commands::Me {} => {
            let auth = get_authentication();
            match fetch_profile_info(&auth.token, &auth.config.url) {
                Ok(info) => {
                    // read_token() prefers API_TOKEN over the stored profile,
                    // so the label must follow the same precedence, otherwise
                    // it names the wrong profile when both are present.
                    let profile = if env::var("API_TOKEN").is_ok() {
                        "(from API_TOKEN env)".to_string()
                    } else {
                        active_profile_name().unwrap_or_else(|| "unknown".to_string())
                    };
                    let details = ProfileDetails {
                        profile,
                        email: info.email,
                        workspace: info.workspace,
                    };
                    handle_command_execution_result(Ok::<_, CommandError>(details), output);
                }
                Err(AuthenticationError::WrongCredentials) => {
                    error!("Token is invalid. Run `screenly login` to update your credentials.");
                    std::process::exit(1);
                }
                Err(e) => {
                    error!("Failed to fetch profile info: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Logout { name } => match Authentication::remove_token(name.as_deref()) {
            Ok(removal) => {
                info!("Removed profile '{}'.", removal.removed);
                match &removal.active {
                    // A non-active profile was removed, so the CLI still
                    // authenticates as before.
                    Some(profile) => info!("Active profile is still '{profile}'."),
                    None if removal.remaining.is_empty() => info!("No profiles remain."),
                    None => info!(
                        "No profile is active now. Run `screenly auth switch <name>` to pick one of: {}.",
                        removal.remaining.join(", ")
                    ),
                }
                std::process::exit(0);
            }
            Err(AuthenticationError::NoCredentials) => {
                error!("Not logged in.");
                std::process::exit(1);
            }
            Err(AuthenticationError::ProfileNotFound(profile)) => {
                error!("Profile '{profile}' not found.");
                std::process::exit(1);
            }
            Err(e) => exit_with_authentication_error(&e),
        },
        Commands::Auth(auth_command) => match auth_command {
            AuthCommands::List {} => match fetch_profiles_with_info(&Config::default().url) {
                Ok(entries) => {
                    handle_command_execution_result(
                        Ok::<_, CommandError>(ProfilesTable(entries)),
                        output,
                    );
                }
                Err(e) => exit_with_authentication_error(&e),
            },
            AuthCommands::Switch { name } => match name {
                None => {
                    // A missing argument is a usage error, so exit non-zero
                    // (scripts can detect it) but still print the available
                    // profile names as a hint. Names come from the local store,
                    // so this needs no network round-trips. Read them before
                    // reporting the usage error: if the store itself is
                    // unreadable, that is the only message worth printing.
                    let profiles = match Authentication::list_profiles() {
                        Ok(profiles) => profiles,
                        Err(e) => exit_with_authentication_error(&e),
                    };
                    if profiles.is_empty() {
                        error!("No profiles stored. Run `screenly login` to add one.");
                        std::process::exit(1);
                    }
                    error!("No profile name given. Specify one of the profiles below:");
                    for profile in profiles {
                        let marker = if profile.is_active { "*" } else { " " };
                        println!("{marker} {}", profile.name);
                    }
                    std::process::exit(1);
                }
                Some(name) => match Authentication::switch_profile(name) {
                    Ok(()) => {
                        info!("Switched to profile '{name}'.");
                    }
                    Err(AuthenticationError::ProfileNotFound(_)) => {
                        error!("Profile '{name}' not found.");
                        std::process::exit(1);
                    }
                    Err(e) => exit_with_authentication_error(&e),
                },
            },
        },
        Commands::Mcp {} => {
            handle_cli_mcp_command();
        }
        Commands::PrintHelpMarkdown {} => {
            clap_markdown::print_help_markdown::<Cli>();
        }
    }
}

pub fn handle_cli_mcp_command() {
    use crate::mcp::ScreenlyMcpServer;

    let server = match ScreenlyMcpServer::new() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize MCP server: {}", e);
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    if let Err(e) = rt.block_on(server.run()) {
        error!("MCP server error: {}", e);
        std::process::exit(1);
    }
}

fn get_user_input() -> String {
    let stdin = io::stdin();
    let mut user_input = String::new();
    match stdin.read_line(&mut user_input) {
        Ok(_) => {}
        Err(e) => {
            error!("Error occurred: {e}");
            std::process::exit(1);
        }
    }

    user_input.trim().to_string()
}

pub fn handle_cli_screen_command(command: &ScreenCommands, output: OutputFormat) {
    let authentication = get_authentication();
    let screen_command = commands::screen::ScreenCommand::new(authentication);

    match command {
        ScreenCommands::List => {
            handle_command_execution_result(screen_command.list(), output);
        }
        ScreenCommands::Get { uuid } => {
            handle_command_execution_result(screen_command.get(uuid), output);
        }
        ScreenCommands::Add { pin, name } => {
            handle_command_execution_result(screen_command.add(pin, name.clone()), output);
        }
        ScreenCommands::Delete { uuid } => {
            match get_screen_name(uuid, &screen_command) {
                Ok(name) => {
                    info!("You are about to delete the screen named \"{name}\".  This operation cannot be reversed.");
                    info!("Enter the screen name to confirm the screen deletion: ");
                    if name != get_user_input() {
                        error!("The name you entered is incorrect. Aborting.");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Error occurred: {e}");
                    std::process::exit(1);
                }
            }

            match screen_command.delete(uuid) {
                Ok(()) => {
                    info!("Screen deleted successfully.");
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
    }
}

pub fn handle_cli_playlist_command(command: &PlaylistCommands, output: OutputFormat) {
    let playlist_command = PlaylistCommand::new(get_authentication());
    match command {
        PlaylistCommands::Create { title, predicate } => {
            handle_command_execution_result(
                playlist_command.create(title, &predicate.clone().unwrap_or("TRUE".to_owned())),
                output,
            );
        }
        PlaylistCommands::List => {
            handle_command_execution_result(playlist_command.list(), output);
        }
        PlaylistCommands::Get { uuid } => {
            handle_command_execution_result(playlist_command.get_playlist_file(uuid), output);
        }
        PlaylistCommands::Delete { uuid } => match playlist_command.delete(uuid) {
            Ok(()) => {
                println!("Playlist deleted successfully.");
            }
            Err(e) => {
                eprintln!("Error occurred when deleting playlist: {e:?}")
            }
        },
        PlaylistCommands::Append {
            uuid,
            asset_uuid,
            duration,
        } => {
            handle_command_execution_result(
                playlist_command.append_asset(
                    uuid,
                    asset_uuid,
                    (*duration).unwrap_or(DEFAULT_ASSET_DURATION),
                ),
                output,
            );
        }
        PlaylistCommands::Prepend {
            uuid,
            asset_uuid,
            duration,
        } => {
            handle_command_execution_result(
                playlist_command.prepend_asset(
                    uuid,
                    asset_uuid,
                    (*duration).unwrap_or(DEFAULT_ASSET_DURATION),
                ),
                output,
            );
        }
        PlaylistCommands::Update => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .expect("Unable to read stdin.");

            let playlist: PlaylistFile =
                serde_json::from_str(&input).expect("Unable to parse playlist file.");
            match playlist_command.update(&playlist) {
                Ok(_) => {
                    println!("Playlist updated successfully.");
                }
                Err(e) => {
                    eprintln!("Error occurred when updating playlist: {e:?}")
                }
            }
        }
    }
}

pub fn handle_cli_asset_command(command: &AssetCommands, output: OutputFormat) {
    let authentication = get_authentication();
    let asset_command = commands::asset::AssetCommand::new(authentication);

    match command {
        AssetCommands::List => {
            handle_command_execution_result(asset_command.list(), output);
        }
        AssetCommands::Get { uuid } => {
            handle_command_execution_result(asset_command.get(uuid), output);
        }
        AssetCommands::Add { path, title } => {
            handle_command_execution_result(asset_command.add(path, title), output);
        }
        AssetCommands::Delete { uuid } => {
            match get_asset_title(uuid, &asset_command) {
                Ok(title) => {
                    info!("You are about to delete the asset named \"{title}\".  This operation cannot be reversed.");
                    info!("Enter the asset title to confirm the asset deletion: ");
                    io::stdout().flush().unwrap();

                    let stdin = io::stdin();
                    let mut user_input = String::new();
                    match stdin.read_line(&mut user_input) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error occurred: {e}");
                            std::process::exit(1);
                        }
                    }

                    if title != user_input.trim() {
                        error!("The title you entered is incorrect. Aborting.");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Error occurred: {e}");
                    std::process::exit(1);
                }
            }
            match asset_command.delete(uuid) {
                Ok(()) => {
                    info!("Asset deleted successfully.");
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::InjectJs { uuid, path } => {
            let js_code = if path.starts_with("http://") || path.starts_with("https://") {
                match reqwest::blocking::get(path) {
                    Ok(response) => match response.status() {
                        StatusCode::OK => response.text().unwrap_or_default(),
                        status => {
                            error!("Failed to retrieve JS injection code. Wrong response status: {status}");
                            std::process::exit(1);
                        }
                    },
                    Err(e) => {
                        error!("Failed to retrieve JS injection code. Error: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                match fs::read_to_string(path) {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read file with JS injection code. Error: {e}");
                        std::process::exit(1);
                    }
                }
            };

            match asset_command.inject_js(uuid, &js_code) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::SetHeaders { uuid, headers } => {
            match asset_command.set_web_asset_headers(uuid, headers.headers.clone()) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::BasicAuth { uuid, credentials } => {
            let basic_auth = Credentials::new(&credentials.0, &credentials.1);
            match asset_command.update_web_asset_headers(
                uuid,
                vec![("Authorization".to_owned(), basic_auth.as_http_header())],
            ) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::UpdateHeaders { uuid, headers } => {
            match asset_command.update_web_asset_headers(uuid, headers.headers.clone()) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::BearerAuth { uuid, token } => {
            match asset_command.update_web_asset_headers(
                uuid,
                vec![("Authorization".to_owned(), format!("Bearer {token}"))],
            ) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
    }
}

pub fn handle_cli_edge_app_command(command: &EdgeAppCommands, output: OutputFormat) {
    let authentication = get_authentication();
    let edge_app_command = commands::edge_app::EdgeAppCommand::new(authentication);

    match command {
        EdgeAppCommands::Create {
            name,
            path,
            in_place,
            entrypoint,
        } => {
            let manifest_path = match transform_edge_app_path_to_manifest(path) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to create Edge App: {e}.");
                    std::process::exit(1);
                }
            };

            let result = if in_place.unwrap_or(false) {
                if entrypoint.is_some() {
                    eprintln!("--entrypoint cannot be used with --in-place.");
                    std::process::exit(1);
                }
                edge_app_command.create_in_place(name, manifest_path.as_path())
            } else {
                edge_app_command.create(name, manifest_path.as_path(), entrypoint.clone())
            };

            match result {
                Ok(()) => {
                    println!("Edge App successfully created.");
                }
                Err(e) => {
                    eprintln!("Failed to publish Edge App manifest: {e}.");
                    std::process::exit(1);
                }
            }
        }

        EdgeAppCommands::List => {
            handle_command_execution_result(edge_app_command.list(), output);
        }
        EdgeAppCommands::Deploy {
            path,
            delete_missing_settings,
        } => match edge_app_command.deploy(path.clone(), *delete_missing_settings) {
            Ok(revision) => {
                println!("Edge App successfully deployed. Revision: {revision}.");
            }
            Err(e) => {
                eprintln!("Failed to upload Edge App: {e}.");
                std::process::exit(1);
            }
        },
        EdgeAppCommands::Setting(command) => match command {
            EdgeAppSettingsCommands::List { path } => {
                handle_command_execution_result(
                    edge_app_command.list_settings(path.clone()),
                    output,
                );
            }
            EdgeAppSettingsCommands::Set { setting_pair, path } => {
                match edge_app_command.set_setting(path.clone(), &setting_pair.0, &setting_pair.1) {
                    Ok(()) => {
                        println!("Edge App setting successfully set.");
                    }
                    Err(e) => {
                        eprintln!("Failed to set Edge App setting: {e}");
                        std::process::exit(1);
                    }
                }
            }
        },
        EdgeAppCommands::Delete { path } => {
            let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                Ok(id) => id,
                Err(e) => {
                    error!("Error calling delete Edge App: {e}");
                    std::process::exit(1);
                }
            };
            match edge_app_command.get_app_name(&actual_app_id) {
                Ok(name) => {
                    info!("You are about to delete the Edge App named \"{name}\".  This operation cannot be reversed.");
                    info!("Enter the Edge App name to confirm the app deletion: ");
                    if name != get_user_input() {
                        error!("The name you entered is incorrect. Aborting.");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Error occurred: {e}");
                    std::process::exit(1);
                }
            }

            match edge_app_command.delete_app(&actual_app_id) {
                Ok(()) => {
                    println!("Edge App Deletion in Progress.\nRequest to delete the Edge App has been received and is now being processed. The deletion is marked for asynchronous handling, so it won't happen instantly.");

                    let manifest_path = match transform_edge_app_path_to_manifest(path) {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Failed to delete Edge App: {e}.");
                            std::process::exit(1);
                        }
                    };

                    // If the user didn't specify an app id, we need to clear it from the manifest
                    match edge_app_command.clear_app_id(manifest_path.as_path()) {
                        Ok(()) => {
                            println!("App id cleared from manifest.");
                        }
                        Err(e) => {
                            error!("Error occurred while clearing manifest: {e}");
                            std::process::exit(1);
                        }
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Rename { path, name } => {
            let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                Ok(id) => id,
                Err(e) => {
                    error!("Error renaming Edge App: {e}");
                    std::process::exit(1);
                }
            };
            match edge_app_command.update_name(&actual_app_id, name) {
                Ok(()) => {
                    println!("Edge App successfully renamed.");
                }
                Err(e) => {
                    eprintln!("Failed to rename Edge App: {e}.");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Run {
            path,
            secrets,
            generate_mock_data,
        } => {
            let secrets = if let Some(secret_pairs) = secrets {
                secret_pairs.secrets.clone()
            } else {
                Vec::new()
            };

            if generate_mock_data.unwrap_or(false) {
                let manifest_path = match transform_edge_app_path_to_manifest(path) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Failed to generate mock data: {e}.");
                        std::process::exit(1);
                    }
                };

                match edge_app_command.generate_mock_data(&manifest_path) {
                    Ok(_) => std::process::exit(0),
                    Err(e) => {
                        eprintln!("Mock data generation failed: {e}.");
                        std::process::exit(1);
                    }
                }
            }

            let path = match path {
                Some(path) => PathBuf::from(path),
                None => env::current_dir().unwrap(),
            };

            if !path.join(MOCK_DATA_FILENAME).exists() {
                eprintln!("Error: No mock-data exist. Please run \"screenly edge-app run --generate-mock-data\" and try again.");
                std::process::exit(1);
            }

            edge_app_command.run(path.as_path(), secrets).unwrap();
        }
        EdgeAppCommands::Validate { path } => {
            let manifest_path = match transform_edge_app_path_to_manifest(path) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to validate manifest file: {e}.");
                    std::process::exit(1);
                }
            };
            match EdgeAppManifest::ensure_manifest_is_valid(&manifest_path) {
                Ok(()) => {
                    println!("Manifest file is valid.");
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
            let instance_manifest_path = match transform_instance_path_to_instance_manifest(path) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to build instance manifest filepath: {e}.");
                    std::process::exit(1);
                }
            };

            if !instance_manifest_path.exists() {
                println!("Instance manifest file does not exist.");
                std::process::exit(0);
            }

            match InstanceManifest::ensure_manifest_is_valid(&instance_manifest_path) {
                Ok(()) => {
                    println!("Instance manifest file is valid.");
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }

            let manifest = match EdgeAppManifest::new(&manifest_path) {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!("Failed to validate Edge App manifest file: {e}.");
                    std::process::exit(1);
                }
            };
            let instance_manifest = match InstanceManifest::new(&instance_manifest_path) {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!("Failed to validate Edge App instance manifest file: {e}.");
                    std::process::exit(1);
                }
            };

            match validate_manifests_dependacies(&manifest, &instance_manifest) {
                Ok(()) => {
                    println!("Manifest dependencies are valid.");
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Instance(command) => match command {
            EdgeAppInstanceCommands::List { path } => {
                let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Error calling list instances: {e}");
                        std::process::exit(1);
                    }
                };
                handle_command_execution_result(
                    edge_app_command.list_instances(&actual_app_id),
                    output,
                );
            }
            EdgeAppInstanceCommands::Create { path, name } => {
                let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Error calling create instance: {e}");
                        std::process::exit(1);
                    }
                };
                let new_name = match name {
                    Some(name) => name,
                    None => "New Edge App instance",
                };

                let instance_manifest_path =
                    match transform_instance_path_to_instance_manifest(path) {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Failed to create Edge App instance: {e}.");
                            std::process::exit(1);
                        }
                    };

                match edge_app_command.create_instance(
                    &instance_manifest_path,
                    &actual_app_id,
                    new_name,
                ) {
                    Ok(_some_id) => {
                        println!("Edge App instance successfully created.");
                    }
                    Err(e) => {
                        eprintln!("Failed to create Edge App instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
            EdgeAppInstanceCommands::Delete { path } => {
                let actual_installation_id =
                    match edge_app_command.get_installation_id(path.clone()) {
                        Ok(_installation_id) => _installation_id,
                        Err(e) => {
                            error!("Error calling delete setting: {e}");
                            std::process::exit(1);
                        }
                    };

                let instance_manifest_path =
                    match transform_instance_path_to_instance_manifest(path) {
                        Ok(path) => match path.to_str() {
                            Some(path) => path.to_string(),
                            None => {
                                eprintln!("Failed to delete Edge App instance: invalid path.");
                                std::process::exit(1);
                            }
                        },
                        Err(e) => {
                            eprintln!("Failed to delete Edge App instance: {e:?}");
                            std::process::exit(1);
                        }
                    };

                match edge_app_command
                    .delete_instance(&actual_installation_id, instance_manifest_path)
                {
                    Ok(()) => {
                        println!("Edge App instance successfully deleted.");
                    }
                    Err(e) => {
                        eprintln!("Failed to delete Edge App instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
            EdgeAppInstanceCommands::Update { path } => {
                match edge_app_command.update_instance(path.clone()) {
                    Ok(()) => {
                        println!("Edge App instance successfully updated.");
                    }
                    Err(e) => {
                        eprintln!("Failed to update Edge App instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {

    use httpmock::Method::GET;
    use httpmock::MockServer;
    use tempfile::tempdir;

    use super::*;
    use crate::authentication::Config;

    #[test]
    fn test_resolve_login_name_defaults_to_default_on_fresh_install() {
        assert_eq!(resolve_login_name(None, None), "default");
    }

    #[test]
    fn test_resolve_login_name_honors_explicit_name() {
        assert_eq!(resolve_login_name(Some("stage"), Some("prod")), "stage");
    }

    #[test]
    fn test_resolve_login_name_defaults_to_active_profile() {
        // Plain `login` with a profile already active updates that profile
        // rather than failing (the re-login-after-rotation flow).
        assert_eq!(resolve_login_name(None, Some("prod")), "prod");
    }

    #[test]
    fn test_empty_profiles_table_still_renders_machine_formats() {
        // With no profiles stored, `--output json` must stay parseable and
        // `--output csv` must keep its header. Only the human-readable form
        // switches to a hint.
        let table = ProfilesTable(vec![]);

        assert_eq!(table.format(OutputType::Json), "[]");
        assert_eq!(
            table.format(OutputType::Csv),
            "Profile,Active,Email,Workspace\n"
        );
        assert!(table
            .format(OutputType::HumanReadable)
            .contains("No profiles stored"));
    }

    #[test]
    fn test_format_profiles_table_aligns_headers_and_placeholders() {
        use crate::authentication::{ProfileEntry, ProfileInfo};

        // A short name/email (shorter than the headers) and a profile with no
        // info at all -- both used to break alignment.
        let entries = vec![
            ProfileEntry {
                name: "a".to_string(),
                is_active: true,
                info: Some(ProfileInfo {
                    email: "x@y.z".to_string(),
                    workspace: "Team".to_string(),
                }),
            },
            ProfileEntry {
                name: "staging".to_string(),
                is_active: false,
                info: None,
            },
        ];

        let table = format_profiles_table(&entries);
        let lines: Vec<&str> = table.lines().collect();

        // Header column is at least as wide as "Profile" even though the
        // widest name ("staging") is exactly 7 chars.
        assert!(lines[0].starts_with("  Profile  "));

        // The Email and Workspace columns start at the same offset on the
        // header and on every data row (lines[0] header, [1] rule, [2..] data).
        let email_col = lines[0].find("Email").unwrap();
        let ws_col = lines[0].find("Workspace").unwrap();
        assert_eq!(lines[2].find("x@y.z"), Some(email_col));
        assert_eq!(lines[2].find("Team"), Some(ws_col));
        // Row with missing info renders a placeholder in the same columns
        // rather than dropping them.
        assert_eq!(lines[3].find("(unavailable)"), Some(email_col));

        // Active profile is marked.
        assert!(lines[2].starts_with("* a"));
    }

    #[test]
    fn test_get_screen_name_should_return_correct_screen_name() {
        let _tmp_dir = tempdir().unwrap();
        let _tmp_dir = tempdir().unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/screens")
                .query_param("id", "eq.017a5104-524b-33d8-8026-9087b59e7eb5")
                .header("user-agent", format!("screenly-cli {}", env!("CARGO_PKG_VERSION")))
                .header("Authorization", "Token token");
            then
                .status(200)
                .body(b"[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Test name\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let screen_command = commands::screen::ScreenCommand::new(authentication);
        let name =
            get_screen_name("017a5104-524b-33d8-8026-9087b59e7eb5", &screen_command).unwrap();
        assert_eq!(name, "Test name");
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_with_path_should_return_correct_path() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap().to_string();
        let path = Some(dir_path.clone());

        let new_path = transform_edge_app_path_to_manifest(&path).unwrap();

        assert_eq!(new_path, PathBuf::from(format!("{dir_path}/screenly.yml")));
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)]
    fn test_transform_edge_app_path_to_manifest_without_path_should_return_correct_path() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Change current directory to tempdir
        assert!(env::set_current_dir(dir_path).is_ok());

        let new_path = transform_edge_app_path_to_manifest(&None).unwrap();

        assert_eq!(new_path, dir_path.join("screenly.yml"));
    }

    #[test]
    fn test_get_authentication_error_message_when_not_logged_in() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let auth_err = AuthenticationError::Io(io_err);

        let message = get_authentication_error_message(&auth_err);

        assert_eq!(
            message,
            "Not logged in. Please run `screenly login` first to authenticate."
        );
    }

    #[test]
    fn test_get_authentication_error_message_for_other_errors() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let auth_err = AuthenticationError::Io(io_err);

        let message = get_authentication_error_message(&auth_err);

        assert!(message.contains("Authentication error"));
        assert!(message.contains("Please run `screenly login` to authenticate"));
    }

    #[test]
    fn test_json_flag_sets_json_field() {
        let cli = Cli::try_parse_from(["screenly", "--json", "screen", "list"]).unwrap();
        assert!(cli.json);
        assert_eq!(cli.output, OutputFormat::Table);
    }

    #[test]
    fn test_json_flag_conflicts_with_output_flag() {
        let result =
            Cli::try_parse_from(["screenly", "--json", "--output", "json", "screen", "list"]);
        assert!(result.is_err());
    }
}
