use std::collections::HashMap;
use std::{env, fs};

use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_yaml;
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
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
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
    #[error("yaml error")]
    Yaml(#[from] serde_yaml::Error),
    #[error("unknown error")]
    Unknown,
}

#[derive(Serialize, Deserialize, Default)]
struct TokenStore {
    active: Option<String>,
    tokens: HashMap<String, String>,
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

fn screenly_path() -> Result<std::path::PathBuf, AuthenticationError> {
    dirs::home_dir()
        .map(|h| h.join(".screenly"))
        .ok_or(AuthenticationError::MissingHomeDir())
}

fn read_store() -> Result<TokenStore, AuthenticationError> {
    let path = screenly_path()?;
    if !path.exists() {
        return Ok(TokenStore::default());
    }
    let contents = fs::read_to_string(&path)?;
    if let Ok(store) = serde_yaml::from_str::<TokenStore>(&contents) {
        return Ok(store);
    }
    // Backward compat: plain text token → migrate to "default" profile
    let token = contents.trim().to_string();
    let mut store = TokenStore::default();
    store.tokens.insert("default".to_string(), token);
    store.active = Some("default".to_string());
    Ok(store)
}

fn write_store(store: &TokenStore) -> Result<(), AuthenticationError> {
    let path = screenly_path()?;
    fs::write(path, serde_yaml::to_string(store)?)?;
    Ok(())
}

impl Authentication {
    pub fn new() -> Result<Self, AuthenticationError> {
        Ok(Self {
            config: Config::default(),
            token: Self::read_token()?,
        })
    }

    fn read_token() -> Result<String, AuthenticationError> {
        if let Ok(token) = env::var("API_TOKEN") {
            return Ok(token);
        }
        let store = read_store()?;
        let active = store.active.ok_or(AuthenticationError::NoCredentials)?;
        store
            .tokens
            .get(&active)
            .cloned()
            .ok_or(AuthenticationError::NoCredentials)
    }

    pub fn remove_token(name: Option<&str>) -> Result<(), AuthenticationError> {
        let mut store = read_store()?;
        let target = match name {
            Some(n) => n.to_string(),
            None => store
                .active
                .clone()
                .ok_or(AuthenticationError::NoCredentials)?,
        };
        if !store.tokens.contains_key(&target) {
            return Err(AuthenticationError::ProfileNotFound(target));
        }
        store.tokens.remove(&target);
        if store.active.as_deref() == Some(&target) {
            store.active = store.tokens.keys().next().cloned();
        }
        write_store(&store)
    }

    pub fn list_profiles() -> Result<Vec<(String, String, bool)>, AuthenticationError> {
        let store = read_store()?;
        let mut profiles: Vec<(String, String, bool)> = store
            .tokens
            .iter()
            .map(|(name, token)| {
                let is_active = store.active.as_deref() == Some(name.as_str());
                (name.clone(), token.clone(), is_active)
            })
            .collect();
        profiles.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(profiles)
    }

    pub fn switch_profile(name: &str) -> Result<(), AuthenticationError> {
        let mut store = read_store()?;
        if !store.tokens.contains_key(name) {
            return Err(AuthenticationError::ProfileNotFound(name.to_string()));
        }
        store.active = Some(name.to_string());
        write_store(&store)
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

pub struct ProfileInfo {
    pub email: String,
    pub workspace: String,
}

pub fn fetch_profile_info(token: &str, api_url: &str) -> Result<ProfileInfo, AuthenticationError> {
    let secret = format!("Token {token}");
    let client = reqwest::blocking::Client::builder().build()?;

    let user: serde_json::Value = client
        .get(format!("{api_url}/v4.1/users/me"))
        .header(header::AUTHORIZATION, &secret)
        .send()?
        .json()?;

    let email = user
        .get(0)
        .and_then(|u| u["email"].as_str())
        .unwrap_or("unknown")
        .to_string();

    let teams: serde_json::Value = client
        .get(format!("{api_url}/v4.1/teams"))
        .header(header::AUTHORIZATION, &secret)
        .send()?
        .json()?;

    let workspace = teams
        .as_array()
        .and_then(|arr| arr.iter().find(|t| t["is_current"].as_bool() == Some(true)))
        .and_then(|t| t["name"].as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ProfileInfo { email, workspace })
}

pub fn verify_and_store_token(
    token: &str,
    name: &str,
    api_url: &str,
) -> anyhow::Result<(), AuthenticationError> {
    verify_token(token, api_url)?;

    let mut store = read_store()?;
    store.tokens.insert(name.to_string(), token.to_string());
    store.active = Some(name.to_string());
    write_store(&store)
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
    use httpmock::Method::GET;
    use httpmock::MockServer;
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
        assert!(
            verify_and_store_token("correct_token", "default", &authentication.config.url).is_ok()
        );
        let path = tmp_dir.path().join(".screenly");
        assert!(path.exists());
        let store: TokenStore = serde_yaml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(store.tokens.get("default").unwrap(), "correct_token");
        assert_eq!(store.active.unwrap(), "default");
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
        assert!(verify_and_store_token("wrong_token", "default", &config.url).is_err());
        let path = tmp_dir.path().join(".screenly");

        assert!(!path.exists());
    }

    #[test]
    fn test_read_token_when_token_is_overridden_with_env_variable_correct_token_is_returned() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _token = set_env(OsString::from("API_TOKEN"), "env_token");
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("default".to_string()),
            tokens: [("default".to_string(), "token".to_string())]
                .into_iter()
                .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();
        assert_eq!(Authentication::read_token().unwrap(), "env_token");
    }

    #[test]
    fn test_read_token_correct_token_is_returned() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("default".to_string()),
            tokens: [("default".to_string(), "token".to_string())]
                .into_iter()
                .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();
        assert_eq!(Authentication::read_token().unwrap(), "token");
    }

    #[test]
    fn test_read_token_backward_compat_plain_text() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly"), "legacy_token").unwrap();
        assert_eq!(Authentication::read_token().unwrap(), "legacy_token");
    }

    #[test]
    fn test_remove_token_should_remove_active_profile() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("default".to_string()),
            tokens: [("default".to_string(), "token".to_string())]
                .into_iter()
                .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();

        Authentication::remove_token(None).unwrap();
        let store: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        assert!(store.tokens.is_empty());
        assert!(store.active.is_none());
    }

    #[test]
    fn test_switch_profile_should_change_active() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("prod".to_string()),
            tokens: [
                ("prod".to_string(), "prod_token".to_string()),
                ("stage".to_string(), "stage_token".to_string()),
            ]
            .into_iter()
            .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();

        Authentication::switch_profile("stage").unwrap();
        let updated: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        assert_eq!(updated.active.unwrap(), "stage");
    }

    #[test]
    fn test_switch_profile_to_nonexistent_should_fail() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("prod".to_string()),
            tokens: [("prod".to_string(), "prod_token".to_string())]
                .into_iter()
                .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();

        assert!(Authentication::switch_profile("ghost").is_err());
    }

    #[test]
    fn test_list_profiles_should_return_profiles_with_active_marked() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("prod".to_string()),
            tokens: [
                ("prod".to_string(), "prod_token".to_string()),
                ("stage".to_string(), "stage_token".to_string()),
            ]
            .into_iter()
            .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&store).unwrap(),
        )
        .unwrap();

        let profiles = Authentication::list_profiles().unwrap();
        assert_eq!(profiles.len(), 2);
        let prod = profiles.iter().find(|(n, _, _)| n == "prod").unwrap();
        let stage = profiles.iter().find(|(n, _, _)| n == "stage").unwrap();
        assert!(prod.2);
        assert!(!stage.2);
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
        assert!(
            verify_and_store_token("correct_token", "default", &authentication.config.url).is_ok()
        );
        let path = tmp_dir.path().join(".screenly");
        assert!(path.exists());
        let store: TokenStore = serde_yaml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        group_call_mock.assert();
        assert_eq!(store.tokens.get("default").unwrap(), "correct_token");
    }
}
