mod authentication;
mod cli;
mod commands;

extern crate prettytable;

use crate::authentication::{Authentication, AuthenticationError};
use crate::commands::{CommandError, Formatter, OutputType};
use clap::{command, Parser, Subcommand};

use http_auth_basic::Credentials;
use log::{error, info};
use rpassword::read_password;
use simple_logger::SimpleLogger;
use std::io::Write;
use std::{fs, io};
use thiserror::Error;

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
