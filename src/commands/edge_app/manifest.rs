use crate::commands::CommandError;
use std::collections::HashMap;

use std::fs;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::edge_app::setting::{Setting, serialize_settings, deserialize_settings};
use crate::commands::serde_utils::{
    deserialize_option_string_field, string_field_is_none_or_empty,
};

use super::manifest_auth::AuthType;

pub const MANIFEST_VERSION: &str = "manifest_v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Auth {
    #[serde(deserialize_with = "deserialize_auth_type")]
    pub auth_type: AuthType,
    pub global: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntrypointType {
    File,
    RemoteGlobal,
    RemoteLocal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Entrypoint {
    #[serde(rename = "type")]
    pub entrypoint_type: EntrypointType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeAppManifest {
    #[serde(deserialize_with = "deserialize_syntax")]
    pub syntax: String,

    #[serde(
        deserialize_with = "deserialize_app_id",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub id: Option<String>,
    #[serde(
        deserialize_with = "deserialize_user_version",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub user_version: Option<String>,
    #[serde(
        deserialize_with = "deserialize_description",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub description: Option<String>,
    #[serde(
        deserialize_with = "deserialize_icon",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub icon: Option<String>,

    #[serde(
        deserialize_with = "deserialize_author",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub author: Option<String>,

    #[serde(
        deserialize_with = "deserialize_homepage_url",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub homepage_url: Option<String>,

    #[serde(
        deserialize_with = "deserialize_entrypoint",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub entrypoint: Option<Entrypoint>,

    #[serde(
        deserialize_with = "deserialize_auth",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub auth: Option<Auth>,

    #[serde(
        deserialize_with = "deserialize_ready_signal",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub ready_signal: Option<bool>,

    #[serde(
        serialize_with = "serialize_settings",
        deserialize_with = "deserialize_settings",
        default
    )]
    pub settings: Vec<Setting>,
}

fn deserialize_auth<'de, D>(deserializer: D) -> Result<Option<Auth>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct AuthHelper {
        #[serde(rename = "type")]
        auth_type: AuthType,
        global: bool,
    }

    let auth = Option::deserialize(deserializer)?;
    Ok(auth.map(|AuthHelper { auth_type, global }| Auth { auth_type, global }))
}

fn deserialize_auth_type<'de, D>(deserializer: D) -> Result<AuthType, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "basic" => Ok(AuthType::Basic),
        "bearer" => Ok(AuthType::Bearer),
        _ => Err(serde::de::Error::custom(format!(
            "Invalid auth type: {}",
            s
        ))),
    }
}

fn deserialize_syntax<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        MANIFEST_VERSION => Ok(s),
        invalid => Err(serde::de::Error::custom(format!(
            "Invalid syntax: {}. Only 'manifest_v1' is accepted.",
            invalid
        ))),
    }
}

fn deserialize_app_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let maybe_app_id = deserialize_option_string_field("id", true, deserializer);

    maybe_app_id.map_err(|_e| {
        serde::de::Error::custom("Enter a valid ULID `id` parameter either in the maniphest file or as a command line parameter (e.g. `--app_id XXXXXXXXXXXXXXXX`). Field \"id\" cannot be empty in the maniphest file (screenly.yml)")
    })
}

fn deserialize_ready_signal<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    Option::<bool>::deserialize(deserializer)
}

fn deserialize_user_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("user_version", false, deserializer)
}

fn deserialize_description<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("description", false, deserializer)
}

fn deserialize_icon<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("icon", false, deserializer)
}

fn deserialize_author<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("author", false, deserializer)
}

fn deserialize_homepage_url<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("homepage_url", false, deserializer)
}

fn deserialize_entrypoint<'de, D>(deserializer: D) -> Result<Option<Entrypoint>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct EntrypointHelper {
        #[serde(rename = "type")]
        entrypoint_type: EntrypointType,
        uri: Option<String>,
    }

    let entrypoint = Option::deserialize(deserializer)?;
    entrypoint
        .map(
            |EntrypointHelper {
                 entrypoint_type,
                 uri,
             }| {
                match (entrypoint_type, uri) {
                    (EntrypointType::RemoteGlobal, None) => Err(serde::de::Error::custom(
                        "URI is required for remote-global type",
                    )),
                    (EntrypointType::RemoteGlobal, Some(uri)) => Ok(Entrypoint {
                        entrypoint_type,
                        uri: Some(uri),
                    }),
                    (EntrypointType::RemoteLocal, Some(_)) => Err(serde::de::Error::custom(
                        "URI should not be provided for remote-local type",
                    )),
                    (EntrypointType::File, Some(_)) => Err(serde::de::Error::custom(
                        "URI should not be provided for file type",
                    )),
                    (_, None) => Ok(Entrypoint {
                        entrypoint_type,
                        uri: None,
                    }),
                }
            },
        )
        .transpose()
}
impl EdgeAppManifest {
    pub fn new(path: &Path) -> Result<EdgeAppManifest, CommandError> {
        match fs::read_to_string(path) {
            Ok(data) => {
                let manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;
                Ok(manifest)
            }
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    if let Some(2) = e.raw_os_error() {
                        return Err(CommandError::MisingManifest(format!("{}", path.display())));
                    }
                }
                Err(CommandError::InvalidManifest(e.to_string()))
            }
        }
    }

    pub fn save_to_file(manifest: &EdgeAppManifest, path: &Path) -> Result<(), CommandError> {
        let yaml = serde_yaml::to_string(&manifest)?;
        let manifest_file = File::create(path)?;
        write!(&manifest_file, "---\n{yaml}")?;
        Ok(())
    }

    pub fn prepare_payload(manifest: &EdgeAppManifest) -> HashMap<&str, serde_json::Value> {
        let entrypoint_uri = match &manifest.entrypoint {
            Some(entrypoint) => entrypoint.uri.clone(),
            None => None,
        };

        let mut payload: HashMap<&str, serde_json::Value> = [
            ("app_id", &manifest.id),
            ("user_version", &manifest.user_version),
            ("description", &manifest.description),
            ("icon", &manifest.icon),
            ("author", &manifest.author),
            ("homepage_url", &manifest.homepage_url),
            ("entrypoint", &entrypoint_uri),
        ]
        .iter()
        .filter_map(|(key, value)| value.as_ref().map(|v| (*key, json!(v))))
        .collect();

        payload.insert(
            "ready_signal",
            json!(manifest.ready_signal.unwrap_or(false)),
        );

        payload
    }

    pub fn ensure_manifest_is_valid(path: &Path) -> Result<(), CommandError> {
        match EdgeAppManifest::new(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(CommandError::InvalidManifest(beautify_error_message(
                &e.to_string(),
            ))),
        }
    }
}

pub fn beautify_error_message(error: &str) -> String {
    let prefix = "parse error: ";

    let mut stripped = error;

    if let Some(s) = error.strip_prefix(prefix) {
        stripped = s;
    }

    stripped.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::edge_app::setting::{Setting, SettingType};
    use tempfile::tempdir;

    fn create_test_manifest() -> EdgeAppManifest {
        EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: Some("test_app".to_string()),
            ready_signal: Some(true),
            auth: None,
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings: vec![create_test_setting()],
        }
    }

    fn create_test_setting() -> Setting {
        Setting {
            name: "username".to_string(),
            title: Some("username title".to_string()),
            type_: SettingType::String,
            default_value: Some("stranger".to_string()),
            optional: true,
            is_global: false,
            help_text: "An example of a setting that is used in index.html".to_string(),
        }
    }

    fn write_to_tempfile(
        dir: &tempfile::TempDir,
        file_name: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let file_path = dir.path().join(file_name);
        std::fs::write(&file_path, content).unwrap();
        file_path
    }

    fn serialize_deserialize_cycle(
        manifest: EdgeAppManifest,
    ) -> Result<EdgeAppManifest, CommandError> {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        EdgeAppManifest::save_to_file(&manifest, &file_path)?;
        EdgeAppManifest::new(&file_path)
    }

    #[test]
    fn test_save_manifest_to_file_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");
        let manifest = create_test_manifest();

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: manifest_v1
id: test_app
user_version: test_version
description: test_description
icon: test_icon
author: test_author
homepage_url: test_url
entrypoint:
  type: file
ready_signal: true
settings:
  username:
    type: string
    default_value: stranger
    title: username title
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_manifest_to_file_should_skip_none_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");
        let mut manifest = create_test_manifest();
        manifest.description = None;
        manifest.author = None;
        manifest.ready_signal = None;

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: manifest_v1
id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
entrypoint:
  type: file
settings:
  username:
    type: string
    default_value: stranger
    title: username title
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_manifest_to_file_should_skip_empty_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");
        let mut manifest = create_test_manifest();
        manifest.description = Some("".to_string());
        manifest.author = Some("".to_string());

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: manifest_v1
id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
entrypoint:
  type: file
ready_signal: true
settings:
  username:
    type: string
    default_value: stranger
    title: username title
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_manifest_to_file_should_skip_default_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: Some("test_app".to_string()),
            settings: vec![create_test_setting()],
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username title
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_manifest_to_file_should_fail_on_empty_help_text_in_setting() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");
        let mut manifest = create_test_manifest();
        manifest.settings[0].help_text = "".to_string();

        let result = EdgeAppManifest::save_to_file(&manifest, &file_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("help_text"));
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct() {
        let manifest = create_test_manifest();
        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();
        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct_missing_optional_fields() {
        let manifest = EdgeAppManifest {
            id: Some("test_app".to_string()),
            ready_signal: Some(true),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: None,
            author: Some("test_author".to_string()),
            homepage_url: None,
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings: vec![create_test_setting()],
        };

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();
        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_file_non_existent_should_return_error() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        let error_string = result.unwrap_err().to_string();
        assert!(error_string
            .contains("Edge App Manifest (screenly.yml) doesn't exist under provided path"));
        assert!(error_string.contains("Enter a valid command line --path parameter or invoke command in a directory containing Edge App Manifest"));
    }
    #[test]
    fn test_ensure_manifest_is_valid_when_file_valid_should_return_ok() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_ok());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_missing_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    is_global: false
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing field `optional`"));
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invalid_type_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: bool
    default_value: stranger
    title: username
    optional: true
    is_global: false
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        let error_string = result.unwrap_err().to_string();
        assert!(error_string.contains("Setting type should be one of the following:"));
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invalid_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
invalid_field: test value
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    is_global: false
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown field `invalid_field`"));
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_empty_required_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: ''
settings:
  username:
    type: string
    default_value: stranger
    title: username
    optional: true
    is_global: false
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Field \"id\" cannot be empty"));
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_secret_field_has_default_value_should_fail() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: secret
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(
            "Setting \"username\" is of type \"secret\" and cannot have a default value"
        ));
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_required_string_field_has_no_default_value_should_succeed(
    ) {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  username:
    type: string
    title: username
    optional: false
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_setting_starts_with_predefined_string_should_fail() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
syntax: manifest_v1
id: test_app
settings:
  screenly_setting:
    type: string
    default_value: stranger
    title: some_setting
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        let file_path = write_to_tempfile(&dir, file_name, content);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(
            "Setting \"screenly_setting\" cannot start with \"screenly_\" as this prefix is preserved."
        ));
    }

    #[test]
    fn test_save_manifest_to_file_with_is_global_true_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: Some("test_app".to_string()),
            ready_signal: None,
            auth: None,
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings: vec![Setting {
                name: "username".to_string(),
                title: Some("username title".to_string()),
                type_: SettingType::String,
                default_value: Some("stranger".to_string()),
                optional: true,
                is_global: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }],
        };

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: manifest_v1
id: test_app
user_version: test_version
description: test_description
icon: test_icon
author: test_author
homepage_url: test_url
entrypoint:
  type: file
settings:
  username:
    type: string
    default_value: stranger
    title: username title
    optional: true
    help_text: An example of a setting that is used in index.html
    is_global: true
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_prepare_manifest_payload_includes_some_fields() {
        let manifest = EdgeAppManifest {
            id: Some("test_app".to_string()),
            ready_signal: Some(false), // Changed to false
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: Some("entrypoint.html".to_string()),
            }),
            settings: vec![Setting {
                name: "username".to_string(),
                title: Some("username title".to_string()),
                type_: SettingType::String,
                default_value: Some("stranger".to_string()),
                optional: true,
                is_global: false,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }],
        };
        let result = EdgeAppManifest::prepare_payload(&manifest);
        assert_eq!(result["app_id"], json!("test_app"));
        assert_eq!(result["user_version"], json!("test_version"));
        assert_eq!(result["description"], json!("test_description"));
        assert_eq!(result["icon"], json!("test_icon"));
        assert_eq!(result["author"], json!("test_author"));
        assert_eq!(result["homepage_url"], json!("test_url"));
        assert_eq!(result["entrypoint"], json!("entrypoint.html"));
        assert_eq!(result["ready_signal"], json!(false)); // Added assertion for ready_signal
    }

    #[test]
    fn test_prepare_manifest_payload_omits_none_fields() {
        let manifest = EdgeAppManifest {
            id: Some("test_app".to_string()),
            user_version: None,
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: None,
            homepage_url: Some("test_url".to_string()),
            ready_signal: Some(false), // Added ready_signal
            ..Default::default()
        };
        let result = EdgeAppManifest::prepare_payload(&manifest);
        assert_eq!(result["app_id"], json!("test_app"));
        assert!(!result.contains_key("user_version"));
        assert_eq!(result["description"], json!("test_description"));
        assert_eq!(result["icon"], json!("test_icon"));
        assert!(!result.contains_key("author"));
        assert_eq!(result["homepage_url"], json!("test_url"));
        assert!(!result.contains_key("entrypoint"));
        assert_eq!(result["ready_signal"], json!(false)); // Added assertion for ready_signal
    }

    #[test]
    fn test_prepare_manifest_payload_with_ready_signal_true() {
        let manifest = EdgeAppManifest {
            id: Some("test_app".to_string()),
            ready_signal: Some(true),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: Some("entrypoint.html".to_string()),
            }),
            ..Default::default()
        };
        let result = EdgeAppManifest::prepare_payload(&manifest);
        assert_eq!(result["app_id"], json!("test_app"));
        assert_eq!(result["user_version"], json!("test_version"));
        assert_eq!(result["description"], json!("test_description"));
        assert_eq!(result["icon"], json!("test_icon"));
        assert_eq!(result["author"], json!("test_author"));
        assert_eq!(result["homepage_url"], json!("test_url"));
        assert_eq!(result["entrypoint"], json!("entrypoint.html"));
        assert_eq!(result["ready_signal"], json!(true)); // Assert ready_signal is true
    }
}
