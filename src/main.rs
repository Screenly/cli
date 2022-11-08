mod authentication;
mod commands;

extern crate prettytable;
use crate::authentication::{Authentication, AuthenticationError};
use clap::{command, Parser, Subcommand};

use crate::commands::{Formatter, OutputType};
use simple_logger::SimpleLogger;

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
        id: String,
    },
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
                match screen_command.list() {
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
            ScreenCommands::Get { id, json } => {
                let screen_command = commands::ScreenCommand::new(authentication);
                match screen_command.get(id) {
                    Ok(screen) => {
                        let output_type = if json.unwrap_or(false) {
                            OutputType::Json
                        } else {
                            OutputType::HumanReadable
                        };

                        println!("{}", screen.format(output_type));
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
