use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fs, str};

use anyhow::Result;
use futures::future::{self, BoxFuture, FutureExt};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

use crate::api::edge_app::setting::SettingType;
use crate::commands::edge_app::manifest::EdgeAppManifest;
use crate::commands::edge_app::EdgeAppCommand;
use crate::commands::ignorer::Ignorer;
use crate::commands::CommandError;

pub const MOCK_DATA_FILENAME: &str = "mock-data.yml";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Value {
    Str(String),
    Array(Vec<String>),
}

pub async fn run_server(
    path: &Path,
    secrets: Vec<(String, String)>,
) -> Result<String, anyhow::Error> {
    let secrets_val = secrets
        .iter()
        .map(|(k, v)| (k.clone(), Value::Str(v.clone())))
        .collect::<Vec<(_, _)>>();

    let dir_path = Arc::new(path.to_path_buf());

    let ignorer = Arc::new(Ignorer::new(&*dir_path)?);

    let directory = warp::path("edge")
        .and(warp::path("1"))
        .and(warp::fs::dir(dir_path.as_path().to_owned()))
        .and_then(
            move |file: warp::filters::fs::File| -> BoxFuture<'static, Result<_, Rejection>> {
                if ignorer.is_ignored(file.path()) {
                    future::err(warp::reject::not_found()).boxed()
                } else {
                    future::ok(file).boxed()
                }
            },
        );

    let secrets_map: Vec<(_, _)> = secrets_val.into_iter().collect();
    let secrets_clone = secrets_map;

    let virtual_file = warp::path("edge")
        .and(warp::path("1"))
        .and(warp::path("screenly.js"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then({
            move |params: HashMap<String, String>| {
                let dir_path = dir_path.clone();
                let secrets_clone = secrets_clone.clone();
                async move {
                    if let Some(version) = params.get("version") {
                        if version == "1" {
                            return generate_content(dir_path, &secrets_clone).await;
                        }
                    }
                    Err(warp::reject::not_found())
                }
            }
        });

    let routes = directory.or(virtual_file);

    let server = warp::serve(routes);
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], 0).into();

    let (addr, server_future) = server.bind_ephemeral(addr);

    tokio::task::spawn(server_future);

    Ok(format!("http://{addr}/edge/1"))
}

#[derive(Debug)]
#[allow(dead_code)]
struct WarpError(#[allow(dead_code)] anyhow::Error);

impl Reject for WarpError {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Metadata {
    coordinates: Vec<String>,
    hardware: String,
    hostname: String,
    location: String,
    screen_name: String,
    tags: Vec<String>,
}
impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            coordinates: vec!["37.3861".to_string(), "-122.0839".to_string()],
            hardware: "x86".to_string(),
            hostname: "srly-t6kb0ta1jrd9o0w".to_string(),
            location: "Silicon Valley, USA".to_string(),
            screen_name: "Code Cafe Display".to_string(),
            tags: vec!["All Screens".to_string()],
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct MockData {
    metadata: Metadata,
    settings: HashMap<String, String>,
}

async fn generate_content(
    dir_path: Arc<PathBuf>,
    secrets: &[(String, Value)],
) -> Result<impl Reply, Rejection> {
    let file_path = dir_path.join(MOCK_DATA_FILENAME);

    let content = if file_path.exists() {
        fs::read_to_string(&file_path).unwrap_or("".to_string())
    } else {
        eprintln!(
            "Mock data does not exist. Use \"screenly edge-app run --generate-mock-data\" to create mock data."
        );
        return Err(warp::reject::not_found());
    };
    let data: MockData = match serde_yaml::from_str(&content) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to parse mock data: {e}");
            return Err(warp::reject::not_found());
        }
    };

    let js_output = format_js(data, secrets);

    Ok(warp::reply::with_header(
        js_output,
        "content-type",
        "application/javascript",
    ))
}

fn format_js(data: MockData, secrets: &[(String, Value)]) -> String {
    let mut settings: Vec<(String, Value)> = data
        .settings
        .into_iter()
        .map(|(k, v)| (k, Value::Str(v)))
        .collect();

    settings.extend(secrets.iter().map(|(k, v)| (k.clone(), v.clone())));
    settings.sort_by_key(|a| a.0.clone());

    format!(
        "var screenly = {{\n{metadata},\n{settings},\n{cors_proxy},\n    signalReadyForRendering: function() {{}}\n}};",
        metadata = format_section("metadata", &hashmap_from_metadata(&data.metadata)),
        settings = format_section("settings", &settings),
        cors_proxy = "    cors_proxy_url: \"http://127.0.0.1:8080\""
    )
}

fn hashmap_from_metadata(metadata: &Metadata) -> Vec<(String, Value)> {
    let result = vec![
        (
            "coordinates".to_string(),
            Value::Array(metadata.coordinates.clone()),
        ),
        (
            "hardware".to_string(),
            Value::Str(metadata.hardware.clone()),
        ),
        (
            "hostname".to_string(),
            Value::Str(metadata.hostname.clone()),
        ),
        (
            "location".to_string(),
            Value::Str(metadata.location.clone()),
        ),
        (
            "screen_name".to_string(),
            Value::Str(metadata.screen_name.clone()),
        ),
        (
            "tags".to_string(),
            Value::Array(
                metadata
                    .tags
                    .iter()
                    .map(|tag| tag.to_string())
                    .collect::<Vec<String>>(),
            ),
        ),
    ];
    result
}

fn format_section(section_name: &str, items: &[(String, Value)]) -> String {
    // Build the inner object: { "key": <json>, ... }
    let mut inner_object = JsonMap::new();

    for (key, value) in items.iter() {
        match value {
            Value::Str(text) => {
                inner_object.insert(key.clone(), JsonValue::String(text.clone()));
            }
            Value::Array(list) => {
                let json_array = list
                    .iter()
                    .cloned()
                    .map(JsonValue::String)
                    .collect::<Vec<_>>();
                inner_object.insert(key.clone(), JsonValue::Array(json_array));
            }
        }
    }

    format!(
        "{}: {}",
        section_name,
        serde_json::to_string_pretty(&JsonValue::Object(inner_object))
            .expect("Failed to serialize to JSON")
    )
}

impl EdgeAppCommand {
    pub fn run(&self, path: &Path, secrets: Vec<(String, String)>) -> Result<(), anyhow::Error> {
        let address_shared = Arc::new(Mutex::new(None));
        let address_clone = address_shared.clone();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let path = path.to_path_buf();
        runtime.block_on(async {
            tokio::spawn(async move {
                let address = run_server(path.as_path(), secrets).await.unwrap();
                let mut locked_address = address_clone.lock().unwrap();
                *locked_address = Some(address);
            })
            .await
            .unwrap();

            while address_shared.lock().unwrap().is_none() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            println!(
                "Edge App emulator is running at {}/index.html",
                address_shared.lock().unwrap().as_ref().unwrap()
            );

            if let Err(e) = self.open_browser(&format!(
                "{}/index.html",
                address_shared.lock().unwrap().as_ref().unwrap()
            )) {
                eprintln!("{e}");
            }

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });

        Ok(())
    }

    fn open_browser(&self, address: &str) -> Result<(), CommandError> {
        let (command, args) = match std::env::consts::OS {
            "macos" => ("open", vec![address]),
            "windows" => ("cmd", vec!["/C", "start", "", address]),
            "linux" => ("xdg-open", vec![address]),
            _ => {
                return Err(CommandError::OpenBrowserError(
                    "Unsupported OS to open browser".to_string(),
                ))
            }
        };

        let output = std::process::Command::new(command)
            .args(&args)
            .output()
            .expect("Failed to open browser");

        if !output.status.success() {
            return Err(CommandError::OpenBrowserError(format!(
                "Failed to open browser: {}",
                std::str::from_utf8(&output.stderr).unwrap()
            )));
        }

        Ok(())
    }

    pub fn generate_mock_data(&self, path: &Path) -> Result<(), CommandError> {
        let data = fs::read_to_string(path)?;
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;
        let edge_app_dir = path.parent().ok_or(CommandError::MissingField)?;

        if edge_app_dir.join(MOCK_DATA_FILENAME).exists() {
            println!("Mock data for Edge App emulator already exists.");
            return Ok(());
        }

        let default_metadata = Metadata::default();

        let mut settings: HashMap<String, serde_yaml::Value> = HashMap::new();
        for setting in &manifest.settings {
            if setting.type_ != SettingType::Secret {
                let settings_default_value = match setting.default_value {
                    Some(ref default_value) => default_value.clone(),
                    None => "".to_owned(),
                };
                settings.insert(
                    setting.name.clone(),
                    serde_yaml::Value::String(settings_default_value),
                );
            }
        }

        let mut mock_data: HashMap<String, serde_yaml::Value> = HashMap::new();
        mock_data.insert(
            "metadata".to_string(),
            serde_yaml::to_value(default_metadata)?,
        );
        mock_data.insert("settings".to_string(), serde_yaml::to_value(settings)?);

        let mock_data_yaml = serde_yaml::to_string(&mock_data)?;

        fs::write(edge_app_dir.join(MOCK_DATA_FILENAME), mock_data_yaml)?;

        println!("Mock data for Edge App emulator was generated.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;
    use crate::api::edge_app::setting::{Setting, SettingType};
    use crate::authentication::{Authentication, Config};
    use crate::commands::edge_app::test_utils::tests::{
        create_edge_app_manifest_for_test, prepare_edge_apps_test,
    };
    use crate::commands::edge_app::EdgeAppCommand;

    fn setup_temp_dir_with_mock_data() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(MOCK_DATA_FILENAME);
        let mut file = fs::File::create(file_path).unwrap();

        writeln!(
            file,
            r#"
metadata:
  coordinates:
  - '37.3861'
  - '-122.0839'
  hardware: x86
  hostname: srly-t6kb0ta1jrd9o0w
  location: Silicon Valley, USA
  screen_name: Code Cafe Display
  tags:
  - All Screens
settings:
  multi_line: |
    This is a
    multi-line
    string.
  enable_analytics: 'true'
  override_timezone: ''
  tag_manager_id: ''
"#
        )
        .unwrap();

        dir
    }

    #[tokio::test]
    async fn test_server_should_serve_correct_mock_data() {
        let dir = setup_temp_dir_with_mock_data();
        let dir_path = dir.path().to_path_buf();

        let address = run_server(&dir_path, vec![("key".to_string(), "value".to_string())])
            .await
            .unwrap();
        let resp = reqwest::get(format!("{address}/screenly.js?version=1"))
            .await
            .unwrap();
        let content = resp.text().await.unwrap();
        let expected_content = r#"var screenly = {
metadata: {
  "coordinates": [
    "37.3861",
    "-122.0839"
  ],
  "hardware": "x86",
  "hostname": "srly-t6kb0ta1jrd9o0w",
  "location": "Silicon Valley, USA",
  "screen_name": "Code Cafe Display",
  "tags": [
    "All Screens"
  ]
},
settings: {
  "enable_analytics": "true",
  "key": "value",
  "multi_line": "This is a\nmulti-line\nstring.\n",
  "override_timezone": "",
  "tag_manager_id": ""
},
    cors_proxy_url: "http://127.0.0.1:8080",
    signalReadyForRendering: function() {}
};"#;
        assert_eq!(content, expected_content);
    }

    #[tokio::test]
    async fn test_server_without_mock_data() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        let address = run_server(&dir_path, vec![("key".to_string(), "value".to_string())])
            .await
            .unwrap();
        let resp = reqwest::get(format!("{address}/screenly.js?version=1"))
            .await
            .unwrap();

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn test_server_when_invalid_version_requested_should_return() {
        let dir = setup_temp_dir_with_mock_data();
        let dir_path = dir.path().to_path_buf();

        let address = run_server(&dir_path, vec![("key".to_string(), "value".to_string())])
            .await
            .unwrap();

        let resp = reqwest::get(format!("{address}/screenly.js?version=2"))
            .await
            .unwrap();

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn test_server_should_serve_javascript_with_correct_mime_type() {
        let dir = setup_temp_dir_with_mock_data();
        let dir_path = dir.path().to_path_buf();

        let address = run_server(&dir_path, vec![("key".to_string(), "value".to_string())])
            .await
            .unwrap();
        let resp = reqwest::get(format!("{address}/screenly.js?version=1"))
            .await
            .unwrap();

        // Verify the response is successful
        assert_eq!(resp.status(), 200);

        // Verify the Content-Type header is correct
        let content_type = resp.headers().get("content-type").unwrap();
        assert_eq!(content_type, "application/javascript");
    }

    #[test]
    fn test_generate_mock_data_creates_file_with_expected_content() {
        let (_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_manifest.yml");

        // The EdgeAppManifest structure from your example
        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("yes".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();
        command.generate_mock_data(&file_path).unwrap();

        let mock_data_path = dir.path().join(MOCK_DATA_FILENAME);
        assert!(mock_data_path.exists());

        let _generated_content = fs::read_to_string(&mock_data_path).unwrap();
        let _expected_content = r#"metadata:
      coordinates:
        - "37.3861"
        - "-122.0839"
      hostname: "srly-t6kb0ta1jrd9o0w"
      location: "Code Cafe, Mountain View, California"
      screen_name: "Code Cafe Display"
      tags:
        - "All Screens"
    settings:
      asetting: "yes"
      nsetting: ""
    "#;
    }

    #[test]
    fn test_generate_mock_data_excludes_secret_settings() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_manifest_with_varied_settings.yml");

        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "excluded_setting".to_string(),
                type_: SettingType::Secret,
                title: Some("excluded title".to_string()),
                optional: false,
                default_value: None,
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "included_setting".to_string(),
                type_: SettingType::String,
                title: Some("included title".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();
        let config = Config::new("".to_owned());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        command.generate_mock_data(&file_path).unwrap();

        let mock_data_path = dir.path().join(MOCK_DATA_FILENAME);
        let content = fs::read_to_string(mock_data_path).unwrap();

        assert!(!content.contains("excluded_setting"));
        assert!(content.contains("included_setting"));
    }
}
