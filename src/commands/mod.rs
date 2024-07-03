use crate::{Authentication, AuthenticationError};
use prettytable::{cell, Cell, Row};

use log::debug;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use thiserror::Error;

use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::StatusCode;

#[allow(unused_imports)]
pub use edge_app_settings::SettingType;

pub mod asset;
pub mod edge_app;
pub mod edge_app_manifest;
pub(crate) mod edge_app_server;
pub(crate) mod edge_app_settings;
pub mod edge_app_utils;
mod ignorer;
pub(crate) mod playlist;
pub mod screen;
pub(crate) mod serde_utils;

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
    #[error("App id is required. Either in manifest or with --app-id.")]
    MissingAppId,
    #[error("App id cannot be empty. Provide it either in manifest or with --app-id.")]
    EmptyAppId,
    #[error("Edge App Revision {0} not found")]
    RevisionNotFound(String),
    #[error("Manifest file validation failed with error: {0}")]
    InvalidManifest(String),
    #[error("Edge App Manifest (screenly.yml) doesn't exist under provided path: {0}. Enter a valid command line --path parameter or invoke command in a directory containing Edge App Manifest")]
    MisingManifest(String),
    #[error("Setting does not exist: {0}.")]
    SettingDoesNotExist(String),
    #[error("Wrong setting name: {0}.")]
    WrongSettingName(String),
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
                "Name",
                "Title",
                "Value",
                "Default value",
                "Optional",
                "Type",
                "Help text",
            ],
            vec![
                "name",
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
            vec!["Name", "Title", "Optional", "Help text"],
            vec!["name", "title", "optional", "help_text"],
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
pub struct EdgeAppInstances {
    pub value: serde_json::Value,
}

impl EdgeAppInstances {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl FormatterValue for EdgeAppInstances {
    fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Formatter for EdgeAppInstances {
    fn format(&self, output_type: OutputType) -> String {
        format_value(
            output_type,
            vec!["Id", "Name"],
            vec!["id", "name"],
            self,
            Some(
                |_field_name: &str, field_value: &serde_json::Value| -> Cell {
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_edge_app_versions_formatter_format_output_properly() {
//         let data = r#"[{
//             "edge_app_channels": [
//                 {
//                     "channel": "stable"
//                 },
//                 {
//                     "channel": "candidate"
//                 }
//             ],
//             "revision": 1,
//             "user_version": "1.0.0",
//             "description": "Initial release",
//             "published": true
//         },
//         {
//             "edge_app_channels": [],
//             "revision": 2,
//             "user_version": "1.0.1",
//             "description": "Bug fixes",
//             "published": true
//         }]"#;
//         let edge_app_versions = EdgeAppVersions::new(serde_json::from_str(data).unwrap());
//
//         let output = edge_app_versions.format(OutputType::HumanReadable);
//         assert_eq!(
//             output,
//             r#"+----------+-----------------+-----------+-------------------+
// | Revision | Description     | Published | Channels          |
// +----------+-----------------+-----------+-------------------+
// | 1        | Initial release | ✅        | stable, candidate |
// +----------+-----------------+-----------+-------------------+
// | 2        | Bug fixes       | ✅        |                   |
// +----------+-----------------+-----------+-------------------+
// "#
//         );
//     }
// }
