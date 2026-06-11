use std::time::Duration;

use log::debug;
use prettytable::{cell, Cell, Row};
use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::api::edge_app::app::EdgeApps;
use crate::api::edge_app::installation::EdgeAppInstances;
use crate::{Authentication, AuthenticationError};

pub mod asset;
pub mod edge_app;

mod ignorer;
pub(crate) mod playlist;
pub mod screen;
pub(crate) mod serde_utils;

pub enum OutputType {
    HumanReadable,
    Json,
    Csv,
}

pub trait Formatter {
    fn format(&self, output_type: OutputType) -> String;
    fn supports_csv() -> bool
    where
        Self: Sized,
    {
        false
    }
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
        OutputType::Csv => {
            let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
            wtr.write_record(&column_names).unwrap();
            if let Some(values) = value.value().as_array() {
                for v in values {
                    let row: Vec<String> = field_names
                        .iter()
                        .map(|field| {
                            let fv = &v[field];
                            if let Some(s) = fv.as_str() {
                                s.to_string()
                            } else if let Some(b) = fv.as_bool() {
                                b.to_string()
                            } else if let Some(n) = fv.as_u64() {
                                n.to_string()
                            } else if let Some(n) = fv.as_i64() {
                                n.to_string()
                            } else if let Some(n) = fv.as_f64() {
                                n.to_string()
                            } else if fv.is_null() {
                                String::new()
                            } else {
                                fv.to_string()
                            }
                        })
                        .collect();
                    wtr.write_record(&row).unwrap();
                }
            }
            String::from_utf8(wtr.into_inner().unwrap()).unwrap()
        }
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
    #[error("unexpected response status: {0}")]
    WrongResponseStatus(u16),
    #[error("{0}")]
    ApiError(String),
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
    #[error("App id is required in manifest.")]
    MissingAppId,
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
    #[error("Failed to open browser")]
    OpenBrowserError(String),
    #[error("Instance already exists")]
    InstanceAlreadyExists,
    #[error("Env var INSTANCE_FILE_NAME must hold only file name, not a path. {0}")]
    InstanceFilenameError(String),
    #[error("Env var MANIFEST_FILE_NAME must hold only file name, not a path. {0}")]
    ManifestFilenameError(String),
    #[error("Path is not a directory: {0}")]
    PathIsNotDirError(String),
    #[error("Missing installation id in the instance file")]
    MissingInstallationId,
    #[error("App not found: {0}")]
    AppNotFound(String),
}

fn api_error_from_body(body: &str, status: StatusCode) -> CommandError {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        for key in &["error", "message", "detail"] {
            if let Some(msg) = json.get(key).and_then(|v| v.as_str()) {
                return CommandError::ApiError(msg.to_string());
            }
        }
    }
    CommandError::WrongResponseStatus(status.as_u16())
}

pub fn get(
    authentication: &Authentication,
    endpoint: &str,
) -> Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    debug!("GET {url}");
    let mut headers = HeaderMap::new();
    headers.insert("Prefer", "return=representation".parse()?);

    let response = authentication
        .build_client()?
        .get(&url)
        .headers(headers)
        .send()?;

    let status = response.status();
    debug!("GET {url} -> {status}");

    if status != StatusCode::OK {
        let body = response.text().unwrap_or_default();
        debug!("Response: {:?}", &body);
        return Err(api_error_from_body(&body, status));
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
        let body = response.text().unwrap_or_default();
        debug!("Response: {:?}", &body);
        return Err(api_error_from_body(&body, status));
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
        let body = response.text().unwrap_or_default();
        debug!("Response: {:?}", &body);
        return Err(api_error_from_body(&body, status));
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
        let body = response.text().unwrap_or_default();
        debug!("Response: {:?}", &body);
        return Err(api_error_from_body(&body, status));
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

impl Formatter for PlaylistFile {
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            OutputType::Json => serde_json::to_string_pretty(self).unwrap(),
            OutputType::HumanReadable => {
                let mut table = prettytable::Table::new();
                table.add_row(Row::from(vec!["Asset Id", "Duration"]));
                for item in &self.items {
                    table.add_row(Row::new(vec![
                        Cell::new(&item.asset_id),
                        Cell::new(
                            &indicatif::HumanDuration(Duration::from_secs(item.duration as u64))
                                .to_string(),
                        ),
                    ]));
                }
                table.to_string()
            }
            OutputType::Csv => {
                let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
                wtr.write_record(["asset_id", "duration"]).unwrap();
                for item in &self.items {
                    wtr.write_record([item.asset_id.as_str(), &item.duration.to_string()])
                        .unwrap();
                }
                String::from_utf8(wtr.into_inner().unwrap()).unwrap()
            }
        }
    }
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

    fn supports_csv() -> bool {
        true
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
                "edge_app_setting_values",
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
                    if field_name.eq("edge_app_setting_values") {
                        let default_array = &vec![];
                        let value = field_value.as_array().unwrap_or(default_array);
                        if value.len() == 1 {
                            return Cell::new(value[0]["value"].as_str().unwrap_or_default());
                        }
                        return Cell::new("");
                    }
                    debug!("field_name: {field_name}, field_value: {field_value:?}");
                    Cell::new(field_value.as_str().unwrap_or_default())
                },
            ),
        )
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

    fn supports_csv() -> bool {
        true
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

    fn supports_csv() -> bool {
        true
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
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        fn format_boolean_field(value: &serde_json::Value) -> Cell {
            if value.as_bool().unwrap_or(false) {
                cell!(c -> "✅")
            } else {
                cell!(c -> "❌")
            }
        }

        format_value(
            output_type,
            vec![
                "Id",
                "Name",
                "Enabled",
                "Priority",
                "Hardware Version",
                "In Sync",
                "Last Ping",
                "Uptime",
            ],
            vec![
                "id",
                "name",
                "is_enabled",
                "priority",
                "hardware_version",
                "in_sync",
                "last_ping",
                "uptime",
            ],
            self,
            Some(|field: &str, value: &serde_json::Value| {
                if field.eq("is_enabled") || field.eq("priority") || field.eq("in_sync") {
                    format_boolean_field(value)
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
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        fn format_boolean_field(value: &serde_json::Value) -> Cell {
            if value.as_bool().unwrap_or(false) {
                cell!(c -> "✅")
            } else {
                cell!(c -> "❌")
            }
        }

        format_value(
            output_type,
            vec!["Id", "Title", "Enabled", "Priority"],
            vec!["id", "title", "is_enabled", "priority"],
            self,
            Some(|field: &str, value: &serde_json::Value| {
                if field.eq("is_enabled") || field.eq("priority") {
                    format_boolean_field(value)
                } else {
                    Cell::new(value.as_str().unwrap_or("N/A"))
                }
            }),
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
    fn supports_csv() -> bool {
        true
    }

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

    #[test]
    fn test_assets_csv_round_trip() {
        let data = r#"[
            {"id": "abc-123", "title": "Plain Title", "type": "video/mp4", "status": "active"},
            {"id": "def-456", "title": "Comma, Title", "type": "image/png", "status": "active"},
            {"id": "ghi-789", "title": "Quote \"Title\"", "type": "image/jpeg", "status": "active"}
        ]"#;
        let assets = Assets::new(serde_json::from_str(data).unwrap());
        let output = assets.format(OutputType::Csv);
        let mut lines = output.lines();
        assert_eq!(lines.next().unwrap(), "Id,Title,Type,Status");
        assert_eq!(
            lines.next().unwrap(),
            "abc-123,Plain Title,video/mp4,active"
        );
        assert_eq!(
            lines.next().unwrap(),
            r#"def-456,"Comma, Title",image/png,active"#
        );
        assert_eq!(
            lines.next().unwrap(),
            r#"ghi-789,"Quote ""Title""",image/jpeg,active"#
        );
    }

    #[test]
    fn test_edge_app_settings_does_not_support_csv() {
        assert!(!EdgeAppSettings::supports_csv());
    }

    #[test]
    fn test_edge_app_instance_formatter_format_output_properly() {
        let data = r#"[{
            "id": "01J1SNE1GMGG8R0ZXZ183ZGN6T",
            "name": "Test App"
        },
        {
            "id": "01J1SNE1GMGG8R0ZXZ183ZGN7T",
            "name": "Test App 2"
        }]"#;
        let edge_app_instances = EdgeAppInstances::new(serde_json::from_str(data).unwrap());

        let output = edge_app_instances.format(OutputType::HumanReadable);
        assert_eq!(
            output,
            r#"+----------------------------+------------+
| Id                         | Name       |
+----------------------------+------------+
| 01J1SNE1GMGG8R0ZXZ183ZGN6T | Test App   |
+----------------------------+------------+
| 01J1SNE1GMGG8R0ZXZ183ZGN7T | Test App 2 |
+----------------------------+------------+
"#
        );
    }
}
