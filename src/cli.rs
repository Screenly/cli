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
    /// Playlist related commands.
    #[command(subcommand)]
    Playlist(PlaylistCommands),
    /// Edge App related commands.
    #[command(subcommand)]
    EdgeApp(EdgeAppCommands),
    /// For generating `docs/CommandLineHelp.md`.
    #[clap(hide = true)]
    PrintHelpMarkdown {},
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
    /// Creates Edge App in the store.
    Create {
        /// Edge App name
        #[arg(short, long)]
        name: String,
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
        /// Use an existing Edge App directory with the manifest and index.html.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        in_place: Option<bool>,
    },

    /// Lists your Edge Apps.
    List {
        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Renames Edge App
    Rename {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Edge App name
        #[arg(short, long)]
        name: String,
    },

    /// Runs Edge App emulator.
    Run {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Secrets to be passed to the Edge App in the form KEY=VALUE. Can be specified multiple times.
        #[arg(short, long, value_parser = parse_key_values::<Secrets>)]
        secrets: Option<Secrets>,

        /// Generates mock data to be used with Edge App run
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        generate_mock_data: Option<bool>,
    },

    /// Settings commands.
    #[command(subcommand)]
    Setting(EdgeAppSettingsCommands),

    /// Instance commands.
    #[command(subcommand)]
    Instance(EdgeAppInstanceCommands),

    /// Deploys assets and settings of the Edge App and release it.
    Deploy {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        #[arg(short, long)]
        delete_missing_settings: Option<bool>,
    },
    /// Deletes an Edge App. This cannot be undone.
    Delete {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Validates Edge App manifest file
    Validate {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppSettingsCommands {
    /// Lists Edge App settings.
    List {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Sets Edge App setting.
    Set {
        /// Key value pair of the setting to be set in the form of `key=value`.
        #[arg(value_parser = parse_key_val)]
        setting_pair: (String, String),

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppInstanceCommands {
    /// Lists Edge App instances.
    List {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,

        /// Enables JSON output.
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Creates Edge App instance.
    Create {
        /// Name of the Edge App instance.
        #[arg(short, long)]
        name: Option<String>,

        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Deletes Edge App instance.
    Delete {
        /// Path to the directory with the manifest. If not specified CLI will use the current working directory.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Update Edge App instance based on changes in the instance.yml.
    Update {
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
        Commands::PrintHelpMarkdown {} => {
            clap_markdown::print_help_markdown::<Cli>();
        }
    }
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
                    eprintln!("Error occurred when getting playlist: {e:?}")
                }
            }
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
                    eprintln!("Error occurred when updating playlist: {e:?}")
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
        EdgeAppCommands::Create {
            name,
            path,
            in_place,
        } => {
            let create_func = if in_place.unwrap_or(false) {
                commands::edge_app::EdgeAppCommand::create_in_place
            } else {
                commands::edge_app::EdgeAppCommand::create
            };

            let manifest_path = match transform_edge_app_path_to_manifest(path) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to create edge app: {e}.");
                    std::process::exit(1);
                }
            };

            match create_func(&edge_app_command, name, manifest_path.as_path()) {
                Ok(()) => {
                    println!("Edge app successfully created.");
                }
                Err(e) => {
                    eprintln!("Failed to publish edge app manifest: {e}.");
                    std::process::exit(1);
                }
            }
        }

        EdgeAppCommands::List { json } => {
            handle_command_execution_result(edge_app_command.list(), json);
        }
        EdgeAppCommands::Deploy {
            path,
            delete_missing_settings,
        } => match edge_app_command.deploy(path.clone(), *delete_missing_settings) {
            Ok(revision) => {
                println!(
                    "Edge app successfully deployed. Revision: {revision}.",
                    revision = revision
                );
            }
            Err(e) => {
                eprintln!("Failed to upload edge app: {e}.");
                std::process::exit(1);
            }
        },
        EdgeAppCommands::Setting(command) => match command {
            EdgeAppSettingsCommands::List { path, json } => {
                handle_command_execution_result(edge_app_command.list_settings(path.clone()), json);
            }
            EdgeAppSettingsCommands::Set { setting_pair, path } => {
                match edge_app_command.set_setting(path.clone(), &setting_pair.0, &setting_pair.1) {
                    Ok(()) => {
                        println!("Edge app setting successfully set.");
                    }
                    Err(e) => {
                        eprintln!("Failed to set edge app setting: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        EdgeAppCommands::Delete { path } => {
            let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                Ok(id) => id,
                Err(e) => {
                    error!("Error calling delete Edge App: {}", e);
                    std::process::exit(1);
                }
            };
            match edge_app_command.get_app_name(&actual_app_id) {
                Ok(name) => {
                    info!("You are about to delete the Edge App named \"{}\".  This operation cannot be reversed.", name);
                    info!("Enter the Edge App name to confirm the app deletion: ");
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

            match edge_app_command.delete_app(&actual_app_id) {
                Ok(()) => {
                    println!("Edge App Deletion in Progress.\nRequest to delete the Edge App has been received and is now being processed. The deletion is marked for asynchronous handling, so it won't happen instantly.");

                    let manifest_path = match transform_edge_app_path_to_manifest(path) {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Failed to delete edge app: {e}.");
                            std::process::exit(1);
                        }
                    };

                    // If the user didn't specify an app id, we need to clear it from the manifest
                    match edge_app_command.clear_app_id(manifest_path.as_path()) {
                        Ok(()) => {
                            println!("App id cleared from manifest.");
                        }
                        Err(e) => {
                            error!("Error occurred while clearing manifest: {}", e);
                            std::process::exit(1);
                        }
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error occurred: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Rename { path, name } => {
            let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                Ok(id) => id,
                Err(e) => {
                    error!("Error calling delete Edge App: {}", e);
                    std::process::exit(1);
                }
            };
            match edge_app_command.update_name(&actual_app_id, name) {
                Ok(()) => {
                    println!("Edge app successfully updated.");
                }
                Err(e) => {
                    eprintln!("Failed to update edge app: {e}.");
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
                    eprintln!("Failed to validate edge app manifest file: {e}.");
                    std::process::exit(1);
                }
            };
            let instance_manifest = match InstanceManifest::new(&instance_manifest_path) {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!("Failed to validate edge app instance manifest file: {e}.");
                    std::process::exit(1);
                }
            };

            match validate_manifests_dependacies(&manifest, &instance_manifest) {
                Ok(()) => {
                    println!("Manifests dependancies are valid.");
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
        EdgeAppCommands::Instance(command) => match command {
            EdgeAppInstanceCommands::List { path, json } => {
                let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Error calling list instances: {}", e);
                        std::process::exit(1);
                    }
                };
                handle_command_execution_result(
                    edge_app_command.list_instances(&actual_app_id),
                    json,
                );
            }
            EdgeAppInstanceCommands::Create { path, name } => {
                let actual_app_id = match edge_app_command.get_app_id(path.clone()) {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Error calling create instance: {}", e);
                        std::process::exit(1);
                    }
                };
                let new_name = match name {
                    Some(name) => name,
                    None => "Edge App instance created by Screenly CLI",
                };

                let instance_manifest_path =
                    match transform_instance_path_to_instance_manifest(path) {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Failed to create edge app instance: {e}.");
                            std::process::exit(1);
                        }
                    };

                match edge_app_command.create_instance(
                    &instance_manifest_path,
                    &actual_app_id,
                    new_name,
                ) {
                    Ok(_some_id) => {
                        println!("Edge app instance successfully created.");
                    }
                    Err(e) => {
                        eprintln!("Failed to create edge app instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
            EdgeAppInstanceCommands::Delete { path } => {
                let actual_installation_id =
                    match edge_app_command.get_installation_id(path.clone()) {
                        Ok(_installation_id) => _installation_id,
                        Err(e) => {
                            error!("Error calling delete setting: {}", e);
                            std::process::exit(1);
                        }
                    };

                let instance_manifest_path =
                    match transform_instance_path_to_instance_manifest(path) {
                        Ok(path) => match path.to_str() {
                            Some(path) => path.to_string(),
                            None => {
                                eprintln!("Failed to delete edge app instance. Invalid path.");
                                std::process::exit(1);
                            }
                        },
                        Err(e) => {
                            eprintln!("Failed to delete edge app instance. {:?}", e);
                            std::process::exit(1);
                        }
                    };

                match edge_app_command
                    .delete_instance(&actual_installation_id, instance_manifest_path)
                {
                    Ok(()) => {
                        println!("Edge app instance successfully deleted.");
                    }
                    Err(e) => {
                        eprintln!("Failed to delete edge app instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
            EdgeAppInstanceCommands::Update { path } => {
                match edge_app_command.update_instance(path.clone()) {
                    Ok(()) => {
                        println!("Edge app instance successfully updated.");
                    }
                    Err(e) => {
                        eprintln!("Failed to update edge app instance: {e}.");
                        std::process::exit(1);
                    }
                }
            }
        },
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

        let new_path = transform_edge_app_path_to_manifest(&path).unwrap();

        assert_eq!(
            new_path,
            PathBuf::from(format!("{}/screenly.yml", dir_path))
        );
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
}
