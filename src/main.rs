mod api;
mod authentication;
mod cli;
mod commands;
mod mcp;
mod pb_signature;
mod signature;

extern crate prettytable;

use std::env;

use clap::Parser;
use simple_logger::{init_with_env, SimpleLogger};

use crate::authentication::{Authentication, AuthenticationError};

fn main() {
    if env::var("RUST_LOG").is_ok() {
        init_with_env().unwrap();
    } else {
        let log_level = {
            if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            }
        };
        SimpleLogger::new().with_level(log_level).init().unwrap();
    }

    let _sentry_dsn = "https://891eb4b6f8ff4f959fd76a587d9ab302@o4505481987489792.ingest.sentry.io/4505482139140096";

    let _guard = sentry::init((
        _sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let cli = cli::Cli::parse();
    cli::handle_cli(&cli);
}
