mod authentication;
mod commands;

extern crate prettytable;

use crate::authentication::{Authentication, AuthenticationError};
use crate::commands::{CommandError, Formatter, OutputType, Screens};
use clap::{command, Parser, Subcommand};
use simple_logger::SimpleLogger;
use std::io;
use std::io::Write;

#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = "Command line interface is intended for quick interaction with Screenly through terminal. Moreover, this CLI is built such that it can be used for automating tasks."
)]
#[command(propagate_version = true)]
struct Cli {
    /// Enables json output
    #[arg(short, long)]
    json: Option<u8>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Logins with the token and stores it for further use if it's valid. You can set API_TOKEN environment variable to override used API token.
    Login { token: String },
    /// Screen related commands
    #[command(subcommand)]
    Screen(ScreenCommands),
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ScreenCommands {
    /// Lists your screens
    List {
        /// Enables json output
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
    },
    /// Gets a single screen by id
    Get {
        /// Enables json output
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// UUID of the screen
        uuid: String,
    },
    /// Adds a new screen
    Add {
        /// Enables json output
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        json: Option<bool>,
        /// Pin code created with registrations endpoint
        pin: String,
        /// Optional name of the new screen.
        name: Option<String>,
    },
    /// Deletes a screen. This cannot be undone.
    Delete {
        /// UUID of the screen to be deleted
        uuid: String,
    },
}

fn handle_command_execution_result(
    result: anyhow::Result<Screens, CommandError>,
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
            eprintln!("Error occurred: {:?}", e);
            std::process::exit(1);
        }
    }
}

fn get_screen_name(
    id: &str,
    screen_command: &commands::ScreenCommand,
) -> Result<String, CommandError> {
    let target_screen = screen_command.get(id)?;

    if let Some(screens) = target_screen.value.as_array() {
        if screens.is_empty() {
            eprintln!("Screen could not be found.");
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

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();
    let cli = Cli::parse();
    let authentication = Authentication::new();
    match &cli.command {
        Commands::Login { token } => match authentication.verify_and_store_token(token) {
            Ok(()) => {
                println!("Login credentials have been saved.");
                std::process::exit(0);
            }

            Err(e) => match e {
                AuthenticationError::WrongCredentialsError => {
                    eprintln!("Token verification failed.");
                    std::process::exit(1);
                }
                _ => {
                    eprintln!("Error occurred: {:?}", e);
                    std::process::exit(1);
                }
            },
        },
        Commands::Screen(command) => match command {
            ScreenCommands::List { json } => {
                let screen_command = commands::ScreenCommand::new(authentication);
                handle_command_execution_result(screen_command.list(), json);
            }
            ScreenCommands::Get { uuid, json } => {
                let screen_command = commands::ScreenCommand::new(authentication);
                handle_command_execution_result(screen_command.get(uuid), json);
            }
            ScreenCommands::Add { pin, name, json } => {
                let screen_command = commands::ScreenCommand::new(authentication);
                handle_command_execution_result(screen_command.add(pin, name.clone()), json);
            }
            ScreenCommands::Delete { uuid } => {
                let screen_command = commands::ScreenCommand::new(authentication);
                match get_screen_name(uuid, &screen_command) {
                    Ok(name) => {
                        println!("You are about to delete the screen named \"{}\".  This operation cannot be reversed.", name);
                        print!("Enter the screen name to confirm the screen deletion: ");
                        io::stdout().flush().unwrap();

                        let stdin = io::stdin();
                        let mut user_input = String::new();
                        match stdin.read_line(&mut user_input) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error occurred: {}", e);
                                std::process::exit(1);
                            }
                        }

                        if name != user_input.trim() {
                            eprintln!("The name you entered is incorrect. Aborting.");
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error occurred: {}", e);
                        std::process::exit(1);
                    }
                }

                match screen_command.delete(uuid) {
                    Ok(()) => {
                        println!("Screen deleted successfully.");
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("Error occurred: {:?}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::authentication::Config;
    use crate::{get_screen_name, Authentication};
    use httpmock::{Method::GET, MockServer};

    use envtestkit::lock::lock_test;
    use envtestkit::set_env;

    use std::ffi::OsString;
    use std::fs;

    use crate::commands::ScreenCommand;

    use tempdir::TempDir;

    #[test]
    fn test_list_screens_should_return_correct_screen_list() {
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
                .header("user-agent", "screenly-cli 0.1.0")
                .header("Authorization", "Token token");
            then
                .status(200)
                .body(b"[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Test name\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let name =
            get_screen_name("017a5104-524b-33d8-8026-9087b59e7eb5", &screen_command).unwrap();
        assert_eq!(name, "Test name");
    }
}
