use std::{env, fs};

use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::{header, StatusCode};
use thiserror::Error;

// For compatability reasons - let's leave build env as well.
include!(concat!(env!("OUT_DIR"), "/config.rs"));
// for local development
// also uncomment unsafe certificate lines "danger_accept_invalid_certs(true)".

pub struct Config {
    pub url: String,
}

#[derive(Error, Debug)]
pub enum AuthenticationError {
    #[error("wrong credentials error")]
    WrongCredentials,
    #[error("no credentials error")]
    NoCredentials,
    #[error("request error")]
    Request(#[from] reqwest::Error),
    #[error("i/o error")]
    Io(#[from] std::io::Error),
    #[error("env error")]
    Env(#[from] env::VarError),
    #[error("missing home dir error")]
    MissingHomeDir(),
    #[error("invalid header error")]
    InvalidHeader(#[from] InvalidHeaderValue),
    #[error("unknown error")]
    Unknown,
}

pub struct Authentication {
    pub config: Config,
    pub token: String,
}

impl Config {
    pub fn default() -> Self {
        Self {
            url: {
                if let Ok(url) = env::var("API_BASE_URL") {
                    url
                } else {
                    API_BASE_URL.to_string()
                }
            },
        }
    }

    #[cfg(test)]
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl Authentication {
    pub fn new() -> Result<Self, AuthenticationError> {
        Ok(Self {
            config: Config::default(),
            token: Self::read_token()?,
        })
    }

    pub fn remove_token() -> Result<(), AuthenticationError> {
        match dirs::home_dir() {
            Some(home) => {
                fs::remove_file(home.join(".screenly"))?;
                Ok(())
            }
            None => Err(AuthenticationError::MissingHomeDir()),
        }
    }

    fn read_token() -> Result<String, AuthenticationError> {
        if let Ok(token) = env::var("API_TOKEN") {
            return Ok(token);
        }

        match dirs::home_dir() {
            Some(path) => {
                fs::read_to_string(path.join(".screenly")).map_err(AuthenticationError::Io)
            }
            None => Err(AuthenticationError::NoCredentials),
        }
    }

    #[cfg(test)]
    pub fn new_with_config(config: Config, token: &str) -> Self {
        Self {
            config,
            token: token.to_string(),
        }
    }

    pub fn build_client(&self) -> Result<reqwest::blocking::Client, AuthenticationError> {
        let token = self.token.clone();
        let secret = format!("Token {token}");
        let mut default_headers = HeaderMap::new();
        default_headers.insert(header::AUTHORIZATION, secret.parse()?);
        default_headers.insert(
            header::USER_AGENT,
            format!("screenly-cli {}", env!("CARGO_PKG_VERSION")).parse()?,
        );

        reqwest::blocking::Client::builder()
            .default_headers(default_headers)
            .build()
            .map_err(AuthenticationError::Request)
    }
}

pub fn verify_and_store_token(
    token: &str,
    api_url: &str,
) -> anyhow::Result<(), AuthenticationError> {
    verify_token(token, api_url)?;

    match dirs::home_dir() {
        Some(home) => {
            fs::write(home.join(".screenly"), token)?;
            Ok(())
        }
        None => Err(AuthenticationError::MissingHomeDir()),
    }
}

fn verify_token(token: &str, api_url: &str) -> anyhow::Result<(), AuthenticationError> {
    // Using uuid of non existing playlist. If we get 404 it means we authenticated successfully.
    let url = format!("{api_url}/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/");
    let secret = format!("Token {token}");
    let client = reqwest::blocking::Client::builder().build()?;

    let res = client
        .get(url)
        .header(header::AUTHORIZATION, &secret)
        .send()?;

    match res.status() {
        StatusCode::UNAUTHORIZED => Err(AuthenticationError::WrongCredentials),
        StatusCode::NOT_FOUND => Ok(()),
        _ => Err(AuthenticationError::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use envtestkit::lock::lock_test;
    use envtestkit::set_env;
    use httpmock::{Method::GET, MockServer};
    use simple_logger::SimpleLogger;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_verify_and_store_token_when_token_is_valid() {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Debug)
            .init()
            .unwrap();
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/")
                .header("Authorization", "Token token");
            then.status(404);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "");
        assert!(verify_and_store_token("correct_token", &authentication.config.url).is_ok());
        let path = tmp_dir.path().join(".screenly");
        assert!(path.exists());
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.eq("correct_token"));
    }

    #[test]
    fn test_verify_and_store_token_when_token_is_invalid() {
        let tmp_dir = tempdir().unwrap();

        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/");
            then.status(401);
        });

        let config = Config::new(mock_server.base_url());
        assert!(verify_and_store_token("wrong_token", &config.url).is_err());
        let path = tmp_dir.path().join(".screenly");

        assert!(!path.exists());
    }

    #[test]
    fn test_read_token_when_token_is_overridden_with_env_variable_correct_token_is_returned() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _token = set_env(OsString::from("API_TOKEN"), "env_token");
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        println!("{}", tmp_dir.path().join(".screenly").to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        assert_eq!(Authentication::read_token().unwrap(), "env_token");
    }

    #[test]
    fn test_read_token_correct_token_is_returned() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();

        assert_eq!(Authentication::read_token().unwrap(), "token");
    }

    #[test]
    fn test_remove_token_should_remove_token_from_storage() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();

        Authentication::remove_token().unwrap();
        assert!(!tmp_dir.path().join(".screenly").exists());
    }

    #[test]
    fn test_verify_and_store_token_when_base_url_is_overdriven() {
        env::set_var("API_BASE_URL", "https://login.screenly.local");
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());

        let mock_server = MockServer::start();
        let group_call_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/")
                .header("Authorization", "Token correct_token");
            then.status(404);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "");
        assert!(verify_and_store_token("correct_token", &authentication.config.url).is_ok());
        let path = tmp_dir.path().join(".screenly");
        assert!(path.exists());
        let contents = fs::read_to_string(path).unwrap();
        group_call_mock.assert();
        assert!(contents.eq("correct_token"));
    }
}
