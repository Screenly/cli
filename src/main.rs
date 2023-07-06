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

    let _sentry_dns = "https://cf4fbc3a05024138ad8ef56f218fbb0e@o1402263.ingest.sentry.io/4505391836233728";

    let _guard = sentry::init((_sentry_dns, sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
