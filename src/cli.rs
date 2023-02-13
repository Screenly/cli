use crate::authentication::{Authentication, AuthenticationError};
use crate::commands;

use crate::commands::{CommandError, Formatter, OutputType};
use clap::{Parser, Subcommand};
use http_auth_basic::Credentials;
use log::{error, info};
use rpassword::read_password;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

#[derive(Error, Debug)]
enum ParseError {
    #[error("missing \"=\" symbol")]
    MissingSymbol(),
}

fn parse_key_val(s: &str) -> Result<(String, String), ParseError> {
    let pos = s.find('=').ok_or(ParseError::MissingSymbol())?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn parse_headers(s: &str) -> Result<Headers, ParseError> {
    if s.is_empty() {
        return Ok(Headers {
            headers: Vec::new(),
        });
    }

    let mut headers = Vec::new();
    let header_pairs = s.split(',');
    for header_pair in header_pairs {
        headers.push(parse_key_val(header_pair)?);
    }
    Ok(Headers { headers })
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
    /// Screen related commands.
    #[command(subcommand)]
    Screen(ScreenCommands),
    /// Asset related commands.
    #[command(subcommand)]
    Asset(AssetCommands),
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Headers {
    // this struct is only needed because I was getting panic from clap when trying to directly use Vec<(String, String)> and parse it.
    // it really did not want to deal with vector when argaction was not set to Append.
    headers: Vec<(String, String)>,
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
        #[arg(value_parser = parse_headers)]
        headers: Headers,
    },

    UpdateHeaders {
        /// UUID of the web asset to set http headers.
        uuid: String,

        /// HTTP headers in the following form `header1=value1[,header2=value2[,...]]`. This command updates only the given headers (adding them if new), leaving any other headers unchanged.
        #[arg(value_parser=parse_headers)]
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
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeAppCommands {
    /// Initializes edge app manifest.
    Init {
        /// Path to the directory to initialize manifest in. If not specified CLI initializes screenly.yml in the current working directory.
        path: Option<String>,
    },
    /// Publishes edge-app to the store.
    Publish {
        /// Path to the directory with edge app manifest. If not specified CLI looks for screenly.yml in the current working directory.
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
    let authentication = Authentication::new();
    match &cli.command {
        Commands::Login {} => {
            print!("Enter your API Token: ");
            std::io::stdout().flush().unwrap();
            let token = read_password().unwrap();
            match authentication.verify_and_store_token(&token) {
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
        Commands::EdgeApp(edge_app_command) => match edge_app_command {
            EdgeAppCommands::Init { path } => {
                let authentication = Authentication::new();
                let command = commands::edge_app::EdgeAppCommand::new(authentication);

                match command.init(transform_edge_app_path_to_manifest(path).as_path()) {
                    Ok(_) => {
                        println!("screenly.yml successfully initialized.");
                    }
                    Err(e) => {
                        println!("Failed to initialize screenly.yml: {e}.");
                    }
                }
            }
            EdgeAppCommands::Publish { path } => {
                let authentication = Authentication::new();
                let command = commands::edge_app::EdgeAppCommand::new(authentication);
                match command.publish(transform_edge_app_path_to_manifest(path).as_path()) {
                    Ok(manifest) => {
                        println!("Edge app successfully published. Your manifest: {manifest:?}");
                    }
                    Err(e) => {
                        println!("Failed to publish edge app manifest: {e}.");
                    }
                }
            }
        },
    }
}

fn transform_edge_app_path_to_manifest(path: &Option<String>) -> PathBuf {
    let mut p = "screenly.yml".to_owned();
    if let Some(path_input) = path {
        let mut buf = PathBuf::from(path_input);
        buf.push("screenly.yml");
        p = buf.to_str().unwrap_or("screenly.yml").to_owned();
    }

    PathBuf::from(p)
}

pub fn handle_cli_screen_command(command: &ScreenCommands) {
    let authentication = Authentication::new();
    match command {
        ScreenCommands::List { json } => {
            let screen_command = commands::screen::ScreenCommand::new(authentication);
            handle_command_execution_result(screen_command.list(), json);
        }
        ScreenCommands::Get { uuid, json } => {
            let screen_command = commands::screen::ScreenCommand::new(authentication);
            handle_command_execution_result(screen_command.get(uuid), json);
        }
        ScreenCommands::Add { pin, name, json } => {
            let screen_command = commands::screen::ScreenCommand::new(authentication);
            handle_command_execution_result(screen_command.add(pin, name.clone()), json);
        }
        ScreenCommands::Delete { uuid } => {
            let screen_command = commands::screen::ScreenCommand::new(authentication);
            match get_screen_name(uuid, &screen_command) {
                Ok(name) => {
                    info!("You are about to delete the screen named \"{}\".  This operation cannot be reversed.", name);
                    info!("Enter the screen name to confirm the screen deletion: ");
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

                    if name != user_input.trim() {
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

pub fn handle_cli_asset_command(command: &AssetCommands) {
    let authentication = Authentication::new();
    match command {
        AssetCommands::List { json } => {
            let asset_command = commands::asset::AssetCommand::new(authentication);
            handle_command_execution_result(asset_command.list(), json);
        }
        AssetCommands::Get { uuid, json } => {
            let asset_command = commands::asset::AssetCommand::new(authentication);
            handle_command_execution_result(asset_command.get(uuid), json);
        }
        AssetCommands::Add { path, title, json } => {
            let asset_command = commands::asset::AssetCommand::new(authentication);
            handle_command_execution_result(asset_command.add(path, title), json);
        }
        AssetCommands::Delete { uuid } => {
            let asset_command = commands::asset::AssetCommand::new(authentication);
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
            let asset_command = commands::asset::AssetCommand::new(authentication);
            let js_code = if path.starts_with("http://") || path.starts_with("https://") {
                match reqwest::blocking::get(path) {
                    Ok(response) => {
                        match response.status().as_u16() {
                            200 => response.text().unwrap_or_default(),
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
            let asset_command = commands::asset::AssetCommand::new(authentication);
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
            let asset_command = commands::asset::AssetCommand::new(authentication);
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
            let asset_command = commands::asset::AssetCommand::new(authentication);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authentication::Config;
    use httpmock::{Method::GET, MockServer};

    use envtestkit::lock::lock_test;
    use envtestkit::set_env;

    use std::ffi::OsString;
    use std::fs;

    use tempdir::TempDir;

    #[test]
    fn test_get_screen_name_should_return_correct_screen_name() {
        let _tmp_dir = TempDir::new("test").unwrap();
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
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
        let authentication = Authentication::new_with_config(config);
        let screen_command = commands::screen::ScreenCommand::new(authentication);
        let name =
            get_screen_name("017a5104-524b-33d8-8026-9087b59e7eb5", &screen_command).unwrap();
        assert_eq!(name, "Test name");
    }
}
