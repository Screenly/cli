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
