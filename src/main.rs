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

    let _sentry_dsn = "https://891eb4b6f8ff4f959fd76a587d9ab302@o4505481987489792.ingest.sentry.io/4505482139140096";

    let _guard = sentry::init((_sentry_dsn, sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
