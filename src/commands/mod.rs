use crate::{Authentication, AuthenticationError};
use prettytable::{cell, Cell, Row};
use std::collections::HashMap;

use log::debug;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use thiserror::Error;

use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::StatusCode;

pub mod asset;
pub mod edge_app;
pub(crate) mod edge_app_server;
mod edge_app_utils;
mod ignorer;
pub(crate) mod playlist;
pub mod screen;

pub enum OutputType {
    HumanReadable,
    Json,
}

pub trait Formatter {
    fn format(&self, output_type: OutputType) -> String;
}

pub trait FormatterValue {
    fn value(&self) -> &serde_json::Value;
}

// Helper function to format a value returned from the API.
// Can be used if there is no need to make any transformation on the returned value.
fn format_value<T, F>(
    output_type: OutputType,
    column_names: Vec<&str>,
    field_names: Vec<&str>,
    value: &T,
    value_transformer: Option<F>,
) -> String
where
    T: FormatterValue,
    F: Fn(&str, &serde_json::Value) -> Cell, // Takes field name and field value and returns display representation
{
    match output_type {
        OutputType::HumanReadable => {
            let mut table = prettytable::Table::new();
            table.add_row(Row::from(column_names));

            if let Some(values) = value.value().as_array() {
                for v in values {
                    let mut row_content = Vec::new();
                    for field in &field_names {
                        let display_value = if let Some(transformer) = &value_transformer {
                            transformer(field, &v[field])
                        } else {
                            Cell::new(v[field].as_str().unwrap_or("N/A"))
                        };
                        row_content.push(display_value);
                    }
                    table.add_row(Row::new(row_content));
                }
            }
            table.to_string()
        }
        OutputType::Json => serde_json::to_string_pretty(&value.value()).unwrap(),
    }
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("auth error")]
    Authentication(#[from] AuthenticationError),
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    #[error("unknown error: {0}")]
    WrongResponseStatus(u16),
    #[error("Required field is missing in the response")]
    MissingField,
    #[error("Required file is missing in the edge app directory: {0}")]
    MissingRequiredFile(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Cannot upload a new version: {0}")]
    NoChangesToUpload(String),
    #[error("Strip prefix error: {0}")]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error("Filesystem error: {0}")]
    FileSystemError(String),
    #[error("Asset processing timeout")]
    AssetProcessingTimeout,
    #[error("Ignore error: {0}")]
    IgnoreError(String),
    #[error("Initialization Failed: {0}")]
    InitializationError(String),
    #[error("Asset processing error: {0}")]
    AssetProcessingError(String),
    #[error("Warning: these secrets are undefined: {0}.")]
    UndefinedSecrets(String),
    #[error("App id is required. Either in manifest or with --app-id .")]
    MissingAppId,
    #[error("Edge App Revision {0} not found")]
    RevisionNotFound(String),
}

pub fn get(
    authentication: &Authentication,
    endpoint: &str,
) -> Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let mut headers = HeaderMap::new();
    headers.insert("Prefer", "return=representation".parse()?);

    let response = authentication
        .build_client()?
        .get(url)
        .headers(headers)
        .send()?;

    let status = response.status();

    if status != StatusCode::OK {
        println!("Response: {:?}", &response.text());
        return Err(CommandError::WrongResponseStatus(status.as_u16()));
    }
    Ok(serde_json::from_str(&response.text()?)?)
}

pub fn post<T: Serialize + ?Sized>(
    authentication: &Authentication,
    endpoint: &str,
    payload: &T,
) -> Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let mut headers = HeaderMap::new();
    headers.insert("Prefer", "return=representation".parse()?);

    let response = authentication
        .build_client()?
        .post(url)
        .headers(headers)
        .timeout(Duration::from_secs(60))
        .json(&payload)
        .send()?;

    let status = response.status();

    // Ok, No_Content are acceptable because some of our RPC code returns that.
    if ![StatusCode::CREATED, StatusCode::OK, StatusCode::NO_CONTENT].contains(&status) {
        debug!("Response: {:?}", &response.text()?);
        return Err(CommandError::WrongResponseStatus(status.as_u16()));
    }
    if status == StatusCode::NO_CONTENT {
        return Ok(serde_json::Value::Null);
    }

    Ok(serde_json::from_str(&response.text()?)?)
}

pub fn delete(authentication: &Authentication, endpoint: &str) -> anyhow::Result<(), CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let response = authentication.build_client()?.delete(url).send()?;

    let status = response.status();

    if ![StatusCode::OK, StatusCode::NO_CONTENT].contains(&status) {
        debug!("Response: {:?}", &response.text()?);
        return Err(CommandError::WrongResponseStatus(status.as_u16()));
    }
    Ok(())
}

pub fn patch<T: Serialize + ?Sized>(
    authentication: &Authentication,
    endpoint: &str,
    payload: &T,
) -> anyhow::Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let mut headers = HeaderMap::new();
    headers.insert("Prefer", "return=representation".parse()?);

    let response = authentication
        .build_client()?
        .patch(url)
        .json(&payload)
        .headers(headers)
        .send()?;

    let status = response.status();
    if status != StatusCode::OK {
        debug!("Response: {:?}", &response.text()?);
        return Err(CommandError::WrongResponseStatus(status.as_u16()));
    }

    if status == StatusCode::NO_CONTENT {
        return Ok(serde_json::Value::Null);
    }

    match serde_json::from_str(&response.text()?) {
        Ok(v) => Ok(v),
        Err(_) => Ok(serde_json::Value::Null),
    }
}

fn string_field_is_none_or_empty(opt: &Option<String>) -> bool {
    opt.as_ref().map_or(true, |s| s.is_empty())
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EdgeAppManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(
        deserialize_with = "deserialize_option_string_not_empty",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub user_version: Option<String>,
    #[serde(
        deserialize_with = "deserialize_option_string_not_empty",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub description: Option<String>,
    #[serde(
        deserialize_with = "deserialize_option_string_not_empty",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub icon: Option<String>,
    #[serde(
        deserialize_with = "deserialize_option_string_not_empty",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub author: Option<String>,
    #[serde(
        deserialize_with = "deserialize_option_string_not_empty",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub homepage_url: Option<String>,
    #[serde(
        serialize_with = "serialize_settings",
        deserialize_with = "deserialize_settings",
        default
    )]
    pub settings: Vec<Setting>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Default)]
pub enum SettingType {
    #[default]
    String,
    Secret,
}

impl std::fmt::Display for SettingType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let printable = match *self {
            SettingType::String => "string",
            SettingType::Secret => "secret",
        };
        write!(f, "{}", printable)
    }
}

impl std::str::FromStr for SettingType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(SettingType::String),
            "secret" => Ok(SettingType::Secret),
            _ => Err(()),
        }
    }
}

// maybe we can use a better name as we have EdgeAppSettings which is the same but serde_json::Value inside
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Setting {
    #[serde(
        rename = "type", 
        serialize_with = "serialize_setting_type",
        deserialize_with = "deserialize_setting_type"
    )]
    pub type_: SettingType,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub title: String,
    pub optional: bool,
    pub help_text: String,
}

fn deserialize_settings<'de, D>(deserializer: D) -> Result<Vec<Setting>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Setting> = serde::Deserialize::deserialize(deserializer)?;
    let mut settings: Vec<Setting> = map
        .into_iter()
        .map(|(title, mut setting)| {
            setting.title = title;
            setting
        })
        .collect();
    settings.sort_by_key(|s| s.title.clone());
    Ok(settings)
}

fn serialize_settings<S>(settings: &[Setting], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;

    let mut map = serializer.serialize_map(Some(settings.len()))?;
    for setting in settings {
        map.serialize_entry(&setting.title, &setting)?;
    }
    map.end()
}

fn deserialize_option_string_not_empty<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;

    match opt {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Err(serde::de::Error::custom("String cannot be empty")),
        _ => Ok(opt),
    }
}

fn serialize_setting_type<S>(setting_type: &SettingType, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&setting_type.to_string())
}

fn deserialize_setting_type<'de, D>(deserializer: D) -> Result<SettingType, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match SettingType::from_str(&s.to_lowercase()) {
        Ok(setting_type) => Ok(setting_type),
        Err(_) => Err(serde::de::Error::custom("Field must be either String or Secret")),
    }
}

impl EdgeAppManifest {
    pub fn new(path: &Path) -> Result<Self, CommandError> {
        let data = fs::read_to_string(path)?;
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;
        Ok(manifest)
    }

    pub fn save_to_file(manifest: &EdgeAppManifest, path: &Path) -> Result<(), CommandError> {
        let yaml = serde_yaml::to_string(&manifest)?;
        let manifest_file = File::create(path)?;
        write!(&manifest_file, "---\n{yaml}")?;
        Ok(())
    }

    pub fn validate_file(path: &Path) -> Result<bool, CommandError> {
        match EdgeAppManifest::new(path) {
            Ok(_) => Ok(true),
            Err(e) => {
                println!("Error: Validation failed with error: {}", e);
                Ok(false)
            }
        }
    }    
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub asset_id: String,
    #[serde(deserialize_with = "deserialize_float_to_u32")]
    pub duration: u32,
    #[serde(skip_serializing, default = "default_pos_value")]
    pub position: u64,
}

fn default_pos_value() -> u64 {
    0
}

fn deserialize_float_to_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let float_value: f64 = Deserialize::deserialize(deserializer)?;
    Ok(float_value as u32)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaylistFile {
    predicate: String,
    playlist_id: String,
    items: Vec<PlaylistItem>,
}

impl PlaylistFile {
    pub fn new(
        predicate: String,
        playlist_id: String,
        items: serde_json::Value,
    ) -> Result<Self, CommandError> {
        Ok(Self {
            predicate,
            playlist_id,
            items: serde_json::from_value(items)?,
        })
    }
}

#[derive(Debug)]
pub struct EdgeApps {
    pub value: serde_json::Value,
}

impl EdgeApps {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}
impl FormatterValue for EdgeApps {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for EdgeApps {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Id", "Title"],
            vec!["id", "name"],
            self,
            None::<fn(&str, &serde_json::Value) -> Cell>,
        )
    }
}

#[derive(Debug)]
pub struct EdgeAppVersions {
    pub value: serde_json::Value,
}

impl EdgeAppVersions {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for EdgeAppVersions {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}
impl Formatter for EdgeAppVersions {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Revision", "Description", "Published", "Channels"],
            vec!["revision", "description", "published", "edge_app_channels"],
            self,
            Some(|field_name: &str, field_value: &serde_json::Value| {
                if field_name.eq("revision") {
                    let version = field_value.as_u64().unwrap_or(0);
                    let str_version = version.to_string();
                    Cell::new(if version > 0 { &str_version } else { "N/A" })
                } else if field_name.eq("published") {
                    let published = field_value.as_bool().unwrap_or(false);
                    Cell::new(if published { "✅" } else { "❌" })
                } else if field_name.eq("edge_app_channels") {
                    // list of maps to string with comma separator
                    // [{"channel":"stable"},{"channel":"beta"}] -> "stable, beta"
                    let channels = field_value
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|channel| channel["channel"].as_str().unwrap_or(""))
                        .collect::<Vec<&str>>()
                        .join(", ");
                    Cell::new(channels.as_str())
                } else {
                    Cell::new(field_value.as_str().unwrap_or("N/A"))
                }
            }),
        )
    }
}

#[derive(Debug)]
pub struct EdgeAppSettings {
    pub value: serde_json::Value,
}

impl EdgeAppSettings {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for EdgeAppSettings {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for EdgeAppSettings {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec![
                "Title",
                "Value",
                "Default value",
                "Optional",
                "Type",
                "Help text",
            ],
            vec![
                "title",
                "value",
                "default_value",
                "optional",
                "type",
                "help_text",
            ],
            self,
            Some(
                |field_name: &str, field_value: &serde_json::Value| -> Cell {
                    if field_name.eq("optional") {
                        let value = field_value.as_bool().unwrap_or(false);
                        return Cell::new(if value { "Yes" } else { "No" });
                    }
                    Cell::new(field_value.as_str().unwrap_or_default())
                },
            ),
        )
    }
}

#[derive(Debug)]
pub struct EdgeAppSecrets {
    pub value: serde_json::Value,
}

impl EdgeAppSecrets {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for EdgeAppSecrets {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for EdgeAppSecrets {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Title", "Optional", "Help text"],
            vec!["title", "optional", "help_text"],
            self,
            Some(
                |field_name: &str, field_value: &serde_json::Value| -> Cell {
                    if field_name.eq("optional") {
                        let value = field_value.as_bool().unwrap_or(false);
                        return Cell::new(if value { "Yes" } else { "No" });
                    }
                    Cell::new(field_value.as_str().unwrap_or_default())
                },
            ),
        )
    }
}

#[derive(Debug)]
pub struct Assets {
    pub value: serde_json::Value,
}

impl Assets {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for Assets {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for Assets {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Id", "Title", "Type", "Status"],
            vec!["id", "title", "type", "status"],
            self,
            None::<fn(&str, &serde_json::Value) -> Cell>,
        )
    }
}

#[derive(Debug)]
pub struct Screens {
    pub value: serde_json::Value,
}

impl Screens {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for Screens {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for Screens {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec![
                "Id",
                "Name",
                "Hardware Version",
                "In Sync",
                "Last Ping",
                "Uptime",
            ],
            vec![
                "id",
                "name",
                "hardware_version",
                "in_sync",
                "last_ping",
                "uptime",
            ],
            self,
            Some(|field: &str, value: &serde_json::Value| {
                if field.eq("in_sync") {
                    if value.as_bool().unwrap_or(false) {
                        cell!(c -> "✅")
                    } else {
                        cell!(c -> "❌")
                    }
                } else if field.eq("uptime") {
                    let uptime = if let Some(uptime) = value.as_u64() {
                        indicatif::HumanDuration(Duration::new(uptime, 0)).to_string()
                    } else {
                        "N/A".to_owned()
                    };
                    Cell::new(&uptime).style_spec("r")
                } else {
                    Cell::new(value.as_str().unwrap_or("N/A"))
                }
            }),
        )
    }
}

#[derive(Debug)]
pub struct Playlists {
    pub value: serde_json::Value,
}

impl Playlists {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for Playlists {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for Playlists {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Id", "Title"],
            vec!["id", "title"],
            self,
            None::<fn(&str, &serde_json::Value) -> Cell>,
        )
    }
}

#[derive(Debug)]
pub struct PlaylistItems {
    pub value: serde_json::Value,
}

impl PlaylistItems {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for PlaylistItems {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for PlaylistItems {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Asset Id", "Duration"],
            vec!["asset_id", "duration"],
            self,
            Some(|field: &str, value: &serde_json::Value| {
                if field.eq("duration") {
                    cell!(indicatif::HumanDuration(Duration::from_secs(
                        value.as_f64().unwrap_or(0.0) as u64
                    ))
                    .to_string())
                } else {
                    Cell::new(value.as_str().unwrap_or("N/A"))
                }
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_to_tempfile(dir: &tempfile::TempDir, file_name: &str, content: &str) -> std::path::PathBuf {
        let file_path = dir.path().join(file_name);
        std::fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn test_save_to_file_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            settings: vec![Setting {
                title: "username".to_string(),
                type_: SettingType::String,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }]
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
description: test_description
icon: test_icon
author: test_author
homepage_url: test_url
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_to_file_should_skip_none_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: None,
            icon: Some("test_icon".to_string()),
            author: None,
            homepage_url: Some("test_url".to_string()),
            settings: vec![Setting {
                title: "username".to_string(),
                type_: SettingType::String,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }]
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_to_file_should_skip_empty_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("".to_string()),
            homepage_url: Some("test_url".to_string()),
            settings: vec![Setting {
                title: "username".to_string(),
                type_: SettingType::String,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }]
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_to_file_should_skip_default_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            settings: vec![Setting {
                title: "username".to_string(),
                type_: SettingType::String,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }],
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_validate_file_when_file_non_existent_should_return_error() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");
    
        let result = EdgeAppManifest::validate_file(&file_path);
        assert!(result.is_err(), "Expected an error for non-existent file");
    }

    #[test]
    fn test_validate_file_when_file_valid_should_return_true() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);        
        assert_eq!(EdgeAppManifest::validate_file(&file_path).unwrap(), true);
    }

    #[test]
    fn test_validate_file_when_missing_field_should_return_false() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);        
        assert_eq!(EdgeAppManifest::validate_file(&file_path).unwrap(), false);
    }

    #[test]
    fn test_validate_file_when_empty_field_should_return_false() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
homepage_url: ''
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);        
        assert_eq!(EdgeAppManifest::validate_file(&file_path).unwrap(), false);
    }

    #[test]
    fn test_validate_file_when_invaild_type_should_return_false() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
settings:
  username:
    type: bool
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);        
        assert_eq!(EdgeAppManifest::validate_file(&file_path).unwrap(), false);
    }

    #[test]
    fn test_edge_app_versions_formatter_format_output_properly() {
        let data = r#"[{
            "edge_app_channels": [
                {
                    "channel": "stable"
                },
                {
                    "channel": "candidate"
                }
            ],
            "revision": 1,
            "user_version": "1.0.0",
            "description": "Initial release",
            "published": true
        },
        {
            "edge_app_channels": [],
            "revision": 2,
            "user_version": "1.0.1",
            "description": "Bug fixes",
            "published": true
        }]"#;
        let edge_app_versions = EdgeAppVersions::new(serde_json::from_str(data).unwrap());
        let output = edge_app_versions.format(OutputType::HumanReadable);
        assert_eq!(
            output,
            r#"+----------+-----------------+-----------+-------------------+
| Revision | Description     | Published | Channels          |
+----------+-----------------+-----------+-------------------+
| 1        | Initial release | ✅        | stable, candidate |
+----------+-----------------+-----------+-------------------+
| 2        | Bug fixes       | ✅        |                   |
+----------+-----------------+-----------+-------------------+
"#
        );
    }
}
