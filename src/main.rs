mod authentication;
mod cli;
mod commands;
mod pb_signature;
mod signature;

extern crate prettytable;

use crate::authentication::{Authentication, AuthenticationError};
use clap::Parser;
use simple_logger::SimpleLogger;

fn main() {
    SimpleLogger::new().init().unwrap();

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
