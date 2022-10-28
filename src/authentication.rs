use reqwest::header;
use std::{env, fs};
use thiserror::Error;

const API_BASE_URL: &str = "https://api.screenlyapp.com/api";

pub struct Config {
    url: String,
}

#[derive(Error, Debug)]
pub enum AuthenticationError {
    #[error("wrong credentials error")]
    WrongCredentialsError,
    #[error("request error")]
    RequestError(#[from] reqwest::Error),
    #[error("i/o error")]
    IoError(#[from] std::io::Error),
    #[error("env error")]
    EnvError(#[from] env::VarError),
    #[error("unknown error")]
    Unknown,
}

pub struct Authentication {
    config: Config,
}

impl Config {
    pub fn default() -> Self {
        Self {
            url: API_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub fn new(url: String) -> Self {
        Self {
            url: url.to_string(),
        }
    }
}

impl Authentication {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    #[cfg(test)]
    pub fn new_with_config(config: Config) -> Self {
        Self { config }
    }

    pub fn verify_and_store_token(&self, token: &str) -> anyhow::Result<(), AuthenticationError> {
        self.verify_token(token)?;

        match std::env::var("HOME") {
            Ok(home) => {
                fs::write(home + "/.screenly", &token)?;
                Ok(())
            }
            Err(e) => Err(AuthenticationError::EnvError(e)),
        }
    }

    fn verify_token(&self, token: &str) -> anyhow::Result<(), AuthenticationError> {
        // Using uuid of non existing playlist. If we get 404 it means we authenticated successfully.
        let url = self.config.url.clone() + "/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/";
        let secret = "Token ".to_owned() + token;
        let client = reqwest::blocking::Client::builder().build()?;

        let res = client
            .get(url)
            .header(header::AUTHORIZATION, &secret)
            .send()?;

        return match res.status().as_u16() {
            401 => Err(AuthenticationError::WrongCredentialsError),
            404 => Ok(()),
            _ => Err(AuthenticationError::Unknown),
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::authentication::Config;
    use crate::Authentication;
    use httpmock::{Method::GET, MockServer};
    use simple_logger::SimpleLogger;

    use std::{env, fs};
    use tempdir::TempDir;

    #[test]
    fn test_verify_and_store_token_when_token_is_valid() {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Debug)
            .init()
            .unwrap();
        let tmp_dir = TempDir::new("test").unwrap();
        env::set_var("HOME", tmp_dir.path().to_str().unwrap());
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/");
            then.status(404);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        assert!(authentication
            .verify_and_store_token("correct_token")
            .is_ok());
        let path = tmp_dir.path().join(".screenly");
        assert!(path.exists());
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.eq("correct_token"));
        tmp_dir.close().unwrap();
    }

    #[test]
    fn test_verify_and_store_token_when_token_is_invalid() {
        let tmp_dir = TempDir::new("invalid").unwrap();
        env::set_var("HOME", tmp_dir.path().to_str().unwrap());
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/");
            then.status(401);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        assert!(authentication
            .verify_and_store_token("wrong_token")
            .is_err());
        let path = tmp_dir.path().join(".screenly");
        assert!(!path.exists());
    }
}
