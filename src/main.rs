mod authentication;
mod commands;

use crate::authentication::{Authentication, AuthenticationError};
use clap::{command, Parser, Subcommand};
use log::{warn};
use simple_logger::SimpleLogger;


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Logins with the token and stores it for further use if it's valid
    Login { token: String },
    /// Screen related commands
    #[command(subcommand)]
    Screen(ScreenCommands),
}

#[derive(Subcommand, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ScreenCommands {
    /// Lists your screens
    List,
    /// Gets a single screen by id
    Get { id: String },
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
                    println!("Token verification failed.");
                    std::process::exit(1);
                }
                _ => {
                    println!("Unknown error");
                    std::process::exit(2);
                }
            },
        },
        Commands::Screen(command) => match command {
            ScreenCommands::List => {
                warn!("List: to be implemented");
            }
            ScreenCommands::Get { id: _ } => {
                warn!("Get: to be implemented");
            }
        },
    }
}
