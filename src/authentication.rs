use std::collections::HashMap;
use std::{env, fs};

use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
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
    #[error("yaml error: {0}")]
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
    let contents = serde_yaml::to_string(store)?;

    // Write to a temp file and rename over the target so a concurrent reader
    // never observes a half-written store and a crash mid-write can't corrupt it.
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, contents)?;
    restrict_permissions(&tmp_path)?;
    fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// The token store holds every profile's credentials, so keep it readable
/// only by the owner. No-op on non-Unix platforms.
#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) -> Result<(), AuthenticationError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) -> Result<(), AuthenticationError> {
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
            .ok_or_else(|| AuthenticationError::ProfileNotFound(active))
    }

    /// Removes a profile. When `name` is `None` the active profile is removed.
    /// If the removed profile was active, the new active profile is chosen
    /// deterministically (the alphabetically first remaining profile).
    /// Returns the name of the profile that is active after removal, if any.
    pub fn remove_token(name: Option<&str>) -> Result<Option<String>, AuthenticationError> {
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
            let mut remaining: Vec<&String> = store.tokens.keys().collect();
            remaining.sort();
            store.active = remaining.first().map(|n| n.to_string());
        }
        write_store(&store)?;
        Ok(store.active)
    }

    /// Returns the stored profiles as `(name, is_active)` pairs sorted by name.
    /// Tokens are intentionally not exposed here to avoid accidental prints;
    /// use `fetch_profiles_with_info` when profile details are needed.
    pub fn list_profiles() -> Result<Vec<(String, bool)>, AuthenticationError> {
        let store = read_store()?;
        let mut profiles: Vec<(String, bool)> = store
            .tokens
            .keys()
            .map(|name| {
                let is_active = store.active.as_deref() == Some(name.as_str());
                (name.clone(), is_active)
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
        authenticated_client(&self.token)
    }
}

/// Builds a blocking client that sends the auth token and the standard
/// `screenly-cli {version}` User-Agent on every request.
fn authenticated_client(token: &str) -> Result<reqwest::blocking::Client, AuthenticationError> {
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

pub struct ProfileInfo {
    pub email: String,
    pub workspace: String,
}

pub struct ProfileEntry {
    pub name: String,
    pub is_active: bool,
    /// `None` when the profile's token could not be resolved against the API.
    pub info: Option<ProfileInfo>,
}

/// Returns every stored profile together with its email/workspace fetched
/// from the API. Tokens stay inside this module and are never returned.
/// The per-profile requests are issued in parallel so the total latency
/// does not grow linearly with the number of profiles.
pub fn fetch_profiles_with_info(api_url: &str) -> Result<Vec<ProfileEntry>, AuthenticationError> {
    use rayon::prelude::*;

    let store = read_store()?;
    let mut names: Vec<String> = store.tokens.keys().cloned().collect();
    names.sort();
    let entries = names
        .into_par_iter()
        .map(|name| {
            let is_active = store.active.as_deref() == Some(name.as_str());
            let info = fetch_profile_info(&store.tokens[&name], api_url).ok();
            ProfileEntry {
                name,
                is_active,
                info,
            }
        })
        .collect();
    Ok(entries)
}

pub fn fetch_profile_info(token: &str, api_url: &str) -> Result<ProfileInfo, AuthenticationError> {
    let client = authenticated_client(token)?;

    let user_response = client.get(format!("{api_url}/v4.1/users/me")).send()?;

    if user_response.status() == StatusCode::UNAUTHORIZED {
        return Err(AuthenticationError::WrongCredentials);
    }

    let user: serde_json::Value = user_response.json()?;

    // The endpoint may return either a single object or a one-element array.
    let user_obj = user.get(0).unwrap_or(&user);
    let email = user_obj["email"].as_str().unwrap_or("unknown").to_string();

    let teams: serde_json::Value = client.get(format!("{api_url}/v4.1/teams")).send()?.json()?;

    let workspace = teams
        .as_array()
        .and_then(|arr| arr.iter().find(|t| t["is_current"].as_bool() == Some(true)))
        .and_then(|t| t["name"].as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ProfileInfo { email, workspace })
}

pub fn active_profile_name() -> Option<String> {
    read_store().ok().and_then(|s| s.active)
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
    fn test_verify_and_store_token_preserves_existing_profiles() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());

        let existing = TokenStore {
            active: Some("prod".to_string()),
            tokens: [("prod".to_string(), "prod_token".to_string())]
                .into_iter()
                .collect(),
        };
        fs::write(
            tmp_dir.path().join(".screenly"),
            serde_yaml::to_string(&existing).unwrap(),
        )
        .unwrap();

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/groups/11CF9Z3GZR0005XXKH00F8V20R/");
            then.status(404);
        });

        let config = Config::new(mock_server.base_url());
        assert!(verify_and_store_token("stage_token", "stage", &config.url).is_ok());

        let store: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        // The pre-existing profile is retained and the new one becomes active.
        assert_eq!(store.tokens.get("prod").unwrap(), "prod_token");
        assert_eq!(store.tokens.get("stage").unwrap(), "stage_token");
        assert_eq!(store.active.as_deref(), Some("stage"));
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

        let new_active = Authentication::remove_token(None).unwrap();
        assert!(new_active.is_none());
        let store: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        assert!(store.tokens.is_empty());
        assert!(store.active.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_write_store_restricts_permissions_to_owner() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());

        let mut store = TokenStore::default();
        store
            .tokens
            .insert("default".to_string(), "token".to_string());
        store.active = Some("default".to_string());
        write_store(&store).unwrap();

        let mode = fs::metadata(tmp_dir.path().join(".screenly"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn test_remove_token_with_explicit_name_keeps_active() {
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

        // Removing a non-active profile by name leaves the active one intact.
        let new_active = Authentication::remove_token(Some("stage")).unwrap();
        assert_eq!(new_active.as_deref(), Some("prod"));
        let store: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        assert!(!store.tokens.contains_key("stage"));
        assert_eq!(store.active.as_deref(), Some("prod"));
    }

    #[test]
    fn test_remove_active_profile_picks_deterministic_new_active() {
        let tmp_dir = tempdir().unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let store = TokenStore {
            active: Some("prod".to_string()),
            tokens: [
                ("prod".to_string(), "prod_token".to_string()),
                ("alpha".to_string(), "alpha_token".to_string()),
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

        // Removing the active profile picks the alphabetically first remaining one.
        let new_active = Authentication::remove_token(None).unwrap();
        assert_eq!(new_active.as_deref(), Some("alpha"));
        let store: TokenStore =
            serde_yaml::from_str(&fs::read_to_string(tmp_dir.path().join(".screenly")).unwrap())
                .unwrap();
        assert_eq!(store.active.as_deref(), Some("alpha"));
    }

    #[test]
    fn test_remove_token_with_unknown_name_errors() {
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

        assert!(matches!(
            Authentication::remove_token(Some("ghost")),
            Err(AuthenticationError::ProfileNotFound(_))
        ));
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
        let prod = profiles.iter().find(|(n, _)| n == "prod").unwrap();
        let stage = profiles.iter().find(|(n, _)| n == "stage").unwrap();
        assert!(prod.1);
        assert!(!stage.1);
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

    #[test]
    fn test_fetch_profile_info_returns_email_and_workspace() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/users/me")
                .header("Authorization", "Token valid_token");
            then.status(200)
                .json_body(serde_json::json!([{"email": "user@example.com"}]));
        });
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/teams")
                .header("Authorization", "Token valid_token");
            then.status(200)
                .json_body(serde_json::json!([{"name": "My Team", "is_current": true}]));
        });

        let result = fetch_profile_info("valid_token", &mock_server.base_url());
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.email, "user@example.com");
        assert_eq!(info.workspace, "My Team");
    }

    #[test]
    fn test_fetch_profile_info_accepts_object_response() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/users/me")
                .header("Authorization", "Token valid_token");
            // A single object, not wrapped in an array.
            then.status(200)
                .json_body(serde_json::json!({"email": "user@example.com"}));
        });
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/teams")
                .header("Authorization", "Token valid_token");
            then.status(200)
                .json_body(serde_json::json!([{"name": "My Team", "is_current": true}]));
        });

        let info = fetch_profile_info("valid_token", &mock_server.base_url()).unwrap();
        assert_eq!(info.email, "user@example.com");
        assert_eq!(info.workspace, "My Team");
    }

    #[test]
    fn test_fetch_profile_info_returns_wrong_credentials_on_401() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET).path("/v4.1/users/me");
            then.status(401);
        });

        let result = fetch_profile_info("bad_token", &mock_server.base_url());
        assert!(matches!(result, Err(AuthenticationError::WrongCredentials)));
    }
}
