use std::io::{Read, Write};
use std::path::PathBuf;
use std::{env, fs, io};

use clap::{Parser, Subcommand};
use http_auth_basic::Credentials;
use log::{error, info};
use reqwest::StatusCode;
use rpassword::read_password;
use thiserror::Error;

use crate::authentication::{verify_and_store_token, Authentication, AuthenticationError, Config};
use crate::commands;
use crate::commands::edge_app_server::MOCK_DATA_FILENAME;
use crate::commands::playlist::PlaylistCommand;
use crate::commands::{CommandError, EdgeAppManifest, Formatter, OutputType, PlaylistFile};
const DEFAULT_ASSET_DURATION: u32 = 15;

#[derive(Error, Debug)]
enum ParseError {
    #[error("missing \"=\" symbol")]
    MissingSymbol(),
}

fn parse_key_val(s: &str) -> Result<(String, String), ParseError> {
    let pos = s.find('=').ok_or(ParseError::MissingSymbol())?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "Command line interface is intended for quick interaction with Screenly through terminal. Moreover, this CLI is built such that it can be used for automating tasks."
)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enables JSON output.
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    json: Option<bool>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token.
    Login {},
    /// Logouts and removes stored token.
    Logout {},
    /// Screen related commands.
    #[command(subcommand)]
    Screen(ScreenCommands),
    /// Asset related commands.
    #[command(subcommand)]
    Asset(AssetCommands),
    #[command(subcommand)]
    Playlist(PlaylistCommands),
    #[command(subcommand)]
    EdgeApp(EdgeAppCommands),
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenCommands {
    /// Lists your screens.
    List {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Gets a single screen by id.
    Get {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// UUID of the screen.
        uuid: String,
    },
    /// Adds a new screen.
    Add {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
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
    ///Creates a new playlist.
    Create {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// Title of the new playlist.
        title: String,
        /// Predicate for the new playlist. If not specified it will be set to "TRUE".
        predicate: Option<String>,
    },
    /// Lists your playlists.
    List {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
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
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// UUID of the playlist.
        uuid: String,
        /// UUID of the asset.
        asset_uuid: String,
        /// Duration of the playlist item in seconds. If not specified it will be set to 15 seconds.
        duration: Option<u32>,
    },
    /// Adds an asset to the beginning of the playlist.
    Prepend {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// UUID of the playlist.
        uuid: String,
        /// UUID of the asset.
        asset_uuid: String,
        /// Duration of the playlist item in seconds. If not specified it will be set to 15 seconds.
        duration: Option<u32>,
    },
    /// Patches a given playlist.
    Update {},
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
    List {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Gets a single asset by id.
    Get {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// UUID of the asset.
        uuid: String,
    },
    /// Adds a new asset.
    Add {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
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

    /// Sets HTTP headers for web asset.
    SetHeaders {
        /// UUID of the web asset to set http headers.
        uuid: String,

        /// HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command
        /// replaces all headers of the asset with the given headers (when an empty string is given, e.g. --set-headers "",
        /// all existing headers are removed, if any)
        #[arg(value_parser = parse_key_values::<Headers>)]
        headers: Headers,
    },
    /// Updates HTTP headers for web asset.
    UpdateHeaders {
        /// UUID of the web asset to set http headers.
        uuid: String,

        /// HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving any other headers unchanged.
        #[arg(value_parser=parse_key_values::<Headers>)]
        headers: Headers,
    },

    /// Shortcut for setting up basic authentication headers.
    BasicAuth {
        /// UUID of the web asset to set up basic authentication for.
        uuid: String,
        /// Basic authentication credentials in "user=password" form.
        #[arg(value_parser = parse_key_val)]
        credentials: (String, String),
    },
    /// Shortcut for setting up bearer authentication headers.
    BearerAuth {
        /// UUID of the web asset to set up basic authentication for.
        uuid: String,
        /// Bearer token.
        token: String,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppCommands {
    /// Creates edge-app in the store.
    Create {
        /// Edge app name
        #[arg(short, long)]
        name: String,
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Lists your edge apps.
    List {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },

    /// Runs Edge App emulator.
    Run {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        #[arg(short, long, value_parser = parse_key_values::<Secrets>)]
        secrets: Option<Secrets>,
    },

    // Generates mock data to be used with Edge App run.
    GenerateMockData {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Version commands.
    #[command(subcommand)]
    Version(EdgeAppVersionCommands),

    /// Settings commands.
    #[command(subcommand)]
    Setting(EdgeAppSettingsCommands),

    /// Secrets commands.
    #[command(subcommand)]
    Secret(EdgeAppSecretsCommands),

    /// Uploads assets and settings of the edge app.
    Upload {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppVersionCommands {
    List {
        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    Promote {
        /// Edge app revision to promote.
        #[arg(short, long)]
        revision: u32,
        /// Channel to promote to. If not specified CLI will use stable channel.
        #[arg(short, long, default_value = "stable")]
        channel: String,

        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppSettingsCommands {
    List {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,

        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },

    Set {
        /// Key value pair of the setting to be set in the form of `key=value`.
        #[arg(value_parser = parse_key_val)]
        setting_pair: (String, String),

        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppSecretsCommands {
    Set {
        /// Key value pair of the secret to be set in the form of `key=value`.
        #[arg(value_parser = parse_key_val)]
        secret_pair: (String, String),

        /// Edge app id. If not specified CLI will use the id from the manifest.
        #[arg(short, long)]
        app_id: Option<String>,

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

pub fn handle_command_execution_result<T: Formatter>(
    result: anyhow::Result<T, CommandError>,
    json: &Option<bool>,
) {
    match result {
        Ok(screen) => {
            let output_type = if json.unwrap_or(false) {
                OutputType::Json
            } else {
                OutputType::HumanReadable
            };
            println!("{}", screen.format(output_type));
        }
        Err(e) => {
            match e {
                CommandError::Authentication(_) => {
                    error!(
                        "Authentication error occurred. Please use login command to authenticate."
                    )
                }
                _ => {
                    error!("Error occurred: {:?}", e);
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

fn get_actual_app_id(app_id: &Option<String>, path: &Option<String>) -> String {
    match app_id {
        Some(id) => id.clone(),
        None => {
            let manifest =
                EdgeAppManifest::new(transform_edge_app_path_to_manifest(path).as_path()).unwrap();
            manifest.app_id
        }
    }
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
    match &cli.command {
        Commands::Login {} => {
            print!("Enter your API Token: ");
            std::io::stdout().flush().unwrap();
            let token = read_password().unwrap();
            match verify_and_store_token(&token, &Config::default().url) {
                Ok(()) => {
                    info!("Login credentials have been saved.");
                    std::process::exit(0);
                }

                Err(e) => match e {
                    AuthenticationError::WrongCredentials => {
                        error!("Token verification failed.");
                        std::process::exit(1);
                    }
                    _ => {
                        error!("Error occurred: {:?}", e);
                        std::process::exit(1);
                    }
                },
            }
        }
        Commands::Screen(command) => handle_cli_screen_command(command),
        Commands::Asset(command) => handle_cli_asset_command(command),
        Commands::EdgeApp(command) => handle_cli_edge_app_command(command),
        Commands::Playlist(command) => handle_cli_playlist_command(command),
        Commands::Logout {} => {
            Authentication::remove_token().expect("Failed to remove token.");
            info!("Logout successful.");
            std::process::exit(0);
        }
    }
}

fn transform_edge_app_path_to_manifest(path: &Option<String>) -> PathBuf {
    let mut result = match path {
        Some(path) => PathBuf::from(path),
        None => env::current_dir().unwrap(),
    };

    result.push("screenly.yml");
    result
}

fn get_user_input() -> String {
    let stdin = io::stdin();
    let mut user_input = String::new();
    match stdin.read_line(&mut user_input) {
        Ok(_) => {}
        Err(e) => {
            error!("Error occurred: {}", e);
            std::process::exit(1);
        }
    }

    user_input.trim().to_string()
}

pub fn handle_cli_screen_command(command: &ScreenCommands) {
    let authentication = Authentication::new().expect("Failed to load authentication.");
    let screen_command = commands::screen::ScreenCommand::new(authentication);

    match command {
        ScreenCommands::List { json } => {
            handle_command_execution_result(screen_command.list(), json);
        }
        ScreenCommands::Get { uuid, json } => {
            handle_command_execution_result(screen_command.get(uuid), json);
        }
        ScreenCommands::Add { pin, name, json } => {
            handle_command_execution_result(screen_command.add(pin, name.clone()), json);
        }
        ScreenCommands::Delete { uuid } => {
            match get_screen_name(uuid, &screen_command) {
                Ok(name) => {
                    info!("You are about to delete the screen named \"{}\".  This operation cannot be reversed.", name);
                    info!("Enter the screen name to confirm the screen deletion: ");
                    if name != get_user_input() {
                        error!("The name you entered is incorrect. Aborting.");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Error occurred: {}", e);
                    std::process::exit(1);
                }
            }

            match screen_command.delete(uuid) {
                Ok(()) => {
                    info!("Screen deleted successfully.");
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

pub fn handle_cli_playlist_command(command: &PlaylistCommands) {
    let playlist_command =
        PlaylistCommand::new(Authentication::new().expect("Failed to load authentication."));
    match command {
        PlaylistCommands::Create {
            json,
            title,
            predicate,
        } => {
            handle_command_execution_result(
                playlist_command.create(title, &predicate.clone().unwrap_or("TRUE".to_owned())),
                json,
            );
        }
        PlaylistCommands::List { json } => {
            handle_command_execution_result(playlist_command.list(), json);
        }
        PlaylistCommands::Get { uuid } => {
            let playlist_file = playlist_command.get_playlist_file(uuid);
            match playlist_file {
                Ok(playlist) => {
                    let pretty_playlist_file = serde_json::to_string_pretty(&playlist).unwrap();
                    println!("{}", pretty_playlist_file);
                }
                Err(e) => {
                    println!("Error occurred when getting playlist: {e:?}")
                }
            }
        }
        PlaylistCommands::Delete { uuid } => match playlist_command.delete(uuid) {
            Ok(()) => {
                println!("Playlist deleted successfully.");
            }
            Err(e) => {
                println!("Error occurred when deleting playlist: {e:?}")
            }
        },
        PlaylistCommands::Append {
            json,
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
                json,
            );
        }
        PlaylistCommands::Prepend {
            json,
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
                json,
            );
        }
        PlaylistCommands::Update {} => {
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
                    println!("Error occurred when updating playlist: {e:?}")
                }
            }
        }
    }
}

pub fn handle_cli_asset_command(command: &AssetCommands) {
    let authentication = Authentication::new().expect("Failed to load authentication.");
    let asset_command = commands::asset::AssetCommand::new(authentication);

    match command {
        AssetCommands::List { json } => {
            handle_command_execution_result(asset_command.list(), json);
        }
        AssetCommands::Get { uuid, json } => {
            handle_command_execution_result(asset_command.get(uuid), json);
        }
        AssetCommands::Add { path, title, json } => {
            handle_command_execution_result(asset_command.add(path, title), json);
        }
        AssetCommands::Delete { uuid } => {
            match get_asset_title(uuid, &asset_command) {
                Ok(title) => {
                    info!("You are about to delete the asset named \"{}\".  This operation cannot be reversed.", title);
                    info!("Enter the asset title to confirm the asset deletion: ");
                    io::stdout().flush().unwrap();

                    let stdin = io::stdin();
                    let mut user_input = String::new();
                    match stdin.read_line(&mut user_input) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error occurred: {}", e);
                            std::process::exit(1);
                        }
                    }

                    if title != user_input.trim() {
                        error!("The title you entered is incorrect. Aborting.");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Error occurred: {}", e);
                    std::process::exit(1);
                }
            }
            match asset_command.delete(uuid) {
                Ok(()) => {
                    info!("Asset deleted successfully.");
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        AssetCommands::InjectJs { uuid, path } => {
            let js_code = if path.starts_with("http://") || path.starts_with("https://") {
                match reqwest::blocking::get(path) {
                    Ok(response) => {
                        match response.status() {
                            StatusCode::OK => response.text().unwrap_or_default(),
                            status => {
                                error!("Failed to retrieve JS injection code. Wrong response status: {}", status);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to retrieve JS injection code. Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                match fs::read_to_string(path) {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read file with JS injection code. Error: {}", e);
                        std::process::exit(1);
                    }
                }
            };

            match asset_command.inject_js(uuid, &js_code) {
                Ok(()) => {
                    info!("Asset updated successfully.");
                }
                Err(e) => {
                    error!("Error occurred: {:?}", e);
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
                    error!("Error occurred: {:?}", e);
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
                    error!("Error occurred: {:?}", e);
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
                    error!("Error occurred: {:?}", e);
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
                    error!("Error occurred: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

pub fn handle_cli_edge_app_command(command: &EdgeAppCommands) {
    let authentication = Authentication::new().expect("Failed to load authentication.");
    let edge_app_command = commands::edge_app::EdgeAppCommand::new(authentication);

    match command {
        EdgeAppCommands::Create { name, path } => {
            match edge_app_command.create(name, transform_edge_app_path_to_manifest(path).as_path())
            {
                Ok(()) => {
                    println!("Edge app successfully created.");
                }
                Err(e) => {
                    println!("Failed to publish edge app manifest: {e}.");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::List { json } => {
            handle_command_execution_result(edge_app_command.list(), json);
        }
        EdgeAppCommands::Upload { path, app_id } => {
            match edge_app_command.upload(
                transform_edge_app_path_to_manifest(path).as_path(),
                app_id.clone(),
            ) {
                Ok(revision) => {
                    println!(
                        "Edge app successfully uploaded. Revision: {revision}.",
                        revision = revision
                    );
                }
                Err(e) => {
                    println!("Failed to upload edge app: {e}.");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Version(command) => match command {
            EdgeAppVersionCommands::List { app_id, path, json } => {
                let actual_app_id = get_actual_app_id(app_id, path);
                handle_command_execution_result(
                    edge_app_command.list_versions(&actual_app_id),
                    json,
                );
            }
            EdgeAppVersionCommands::Promote {
                path,
                revision,
                app_id,
                channel,
            } => {
                let actual_app_id = get_actual_app_id(app_id, path);
                match edge_app_command.promote_version(&actual_app_id, revision, channel) {
                    Ok(()) => {
                        println!("Edge app version successfully promoted.");
                    }
                    Err(e) => {
                        println!("Failed to promote edge app version: {e}.");
                        std::process::exit(1);
                    }
                }
            }
        },
        EdgeAppCommands::Setting(command) => match command {
            EdgeAppSettingsCommands::List { path, json, app_id } => {
                let actual_app_id = get_actual_app_id(app_id, path);
                handle_command_execution_result(
                    edge_app_command.list_settings(&actual_app_id),
                    json,
                );
            }
            EdgeAppSettingsCommands::Set {
                setting_pair,
                app_id,
                path,
            } => {
                let actual_app_id = get_actual_app_id(app_id, path);
                match edge_app_command.set_setting(&actual_app_id, &setting_pair.0, &setting_pair.1)
                {
                    Ok(()) => {
                        println!("Edge app setting successfully set.");
                    }
                    Err(e) => {
                        println!("Failed to set edge app setting: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        EdgeAppCommands::Secret(command) => match command {
            EdgeAppSecretsCommands::Set {
                secret_pair,
                app_id,
                path,
            } => {
                let actual_app_id = get_actual_app_id(app_id, path);

                match edge_app_command.set_secret(&actual_app_id, &secret_pair.0, &secret_pair.1) {
                    Ok(()) => {
                        println!("Edge app secret successfully set.");
                    }
                    Err(e) => {
                        println!("Failed to set edge app secret: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        EdgeAppCommands::Run { path, secrets } => {
            let secrets = if let Some(secret_pairs) = secrets {
                secret_pairs.secrets.clone()
            } else {
                Vec::new()
            };
            let path = match path {
                Some(path) => PathBuf::from(path),
                None => env::current_dir().unwrap(),
            };
            if !path.join(MOCK_DATA_FILENAME).exists() {
                println!("Error: No mock-data exist. Please run \"screenly edge-app generate-mock-data\" and try again.");
                std::process::exit(1);
            }

            edge_app_command.run(path.as_path(), secrets).unwrap();
        }
        EdgeAppCommands::GenerateMockData { path } => {
            let manifest_path = transform_edge_app_path_to_manifest(path);
            edge_app_command.generate_mock_data(&manifest_path).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {

    use httpmock::{Method::GET, MockServer};
    use tempfile::tempdir;

    use crate::authentication::Config;

    use super::*;

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

        let new_path = transform_edge_app_path_to_manifest(&path);

        assert_eq!(
            new_path,
            PathBuf::from(format!("{}/screenly.yml", dir_path))
        );
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_without_path_should_return_correct_path() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Change current directory to tempdir
        assert!(env::set_current_dir(dir_path).is_ok());

        let new_path = transform_edge_app_path_to_manifest(&None);

        assert_eq!(new_path, dir_path.join("screenly.yml"));
    }
}
