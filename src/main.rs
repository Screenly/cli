mod authentication;
mod cli;
mod commands;

extern crate prettytable;

use crate::authentication::{Authentication, AuthenticationError};
use clap::Parser;
use simple_logger::SimpleLogger;

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
