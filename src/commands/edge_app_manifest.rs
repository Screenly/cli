use crate::commands::CommandError;
use std::collections::HashMap;

use std::fs;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::edge_app_settings::{deserialize_settings, serialize_settings, Setting};
use crate::commands::serde_utils::{
    deserialize_option_string_field, string_field_is_none_or_empty,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeAppManifest {
    #[serde(
        deserialize_with = "deserialize_app_id",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub app_id: Option<String>,
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
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub entrypoint: Option<String>,
    #[serde(
        serialize_with = "serialize_settings",
        deserialize_with = "deserialize_settings",
        default
    )]
    pub settings: Vec<Setting>,
}

fn deserialize_app_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let maybe_app_id = deserialize_option_string_field("app_id", true, deserializer);

    maybe_app_id.map_err(|_e| {
        serde::de::Error::custom("Enter a valid ULID `app_id` parameter either in the maniphest file or as a command line parameter (e.g. `--app_id XXXXXXXXXXXXXXXX`). Field \"app_id\" cannot be empty in the maniphest file (screenly.yml)")
    })
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

fn deserialize_entrypoint<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_option_string_field("entrypoint", true, deserializer)
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
        [
            ("app_id", &manifest.app_id),
            ("user_version", &manifest.user_version),
            ("description", &manifest.description),
            ("icon", &manifest.icon),
            ("author", &manifest.author),
            ("homepage_url", &manifest.homepage_url),
            ("entrypoint", &manifest.entrypoint),
        ]
        .iter()
        .filter_map(|(key, value)| value.as_ref().map(|v| (*key, json!(v))))
        .collect()
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

fn beautify_error_message(error: &str) -> String {
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
    use crate::commands::edge_app_settings::SettingType;
    use tempfile::tempdir;

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
        let new_manifest = EdgeAppManifest::new(&file_path)?;

        Ok(new_manifest)
    }

    #[test]
    fn test_save_manifest_to_file_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
description: test_description
icon: test_icon
author: test_author
homepage_url: test_url
entrypoint: entrypoint.html
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

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: None,
            icon: Some("test_icon".to_string()),
            author: None,
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
entrypoint: entrypoint.html
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

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
app_id: test_app
user_version: test_version
icon: test_icon
homepage_url: test_url
entrypoint: entrypoint.html
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
            app_id: Some("test_app".to_string()),
            settings: vec![Setting {
                name: "username".to_string(),
                title: Some("username title".to_string()),
                type_: SettingType::String,
                default_value: Some("stranger".to_string()),
                optional: true,
                is_global: false,
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

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            settings: vec![Setting {
                name: "username".to_string(),
                title: Some("username title".to_string()),
                type_: SettingType::String,
                default_value: Some("stranger".to_string()),
                optional: true,
                is_global: false,
                help_text: "".to_string(),
            }],
            ..Default::default()
        };

        assert!(EdgeAppManifest::save_to_file(&manifest, &file_path).is_err());
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct() {
        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();

        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_serialize_deserialize_cycle_with_is_global_setting_should_pass() {
        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();

        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct_missing_optional_fields() {
        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: None,
            author: Some("test_author".to_string()),
            homepage_url: None,
            entrypoint: Some("entrypoint.html".to_owned()),
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

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();

        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_file_non_existent_should_return_error() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_file_valid_should_return_ok() {
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
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_ok());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_missing_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
settings:
  username:
    type: string
    default_value: stranger
    title: username
    is_global: false,
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_required_empty_field_should_return_error() {
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
    is_global: false,
    help_text: ''
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invaild_type_should_return_error() {
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
    is_global: false,
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invalid_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
asdqweuser_version: test version
settings:
  username:
    type: bool
    default_value: stranger
    title: username
    optional: true
    is_global: false,
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_empty_required_field_should_return_error() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: ''
settings:
  username:
    type: bool
    default_value: stranger
    title: username
    optional: true
    is_global: false
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        assert!(EdgeAppManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_secret_field_has_default_value_should_fail() {
        let dir = tempdir().unwrap();
        let file_name = "screenly.yml";
        let content = r#"---
app_id: test_app
settings:
  username:
    type: secret
    default_value: stranger
    title: username
    optional: true
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
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
app_id: test_app
settings:
  username:
    type: string
    title: username
    optional: false
    help_text: An example of a setting that is used in index.html
"#;

        write_to_tempfile(&dir, file_name, content);
        let file_path = dir.path().join(file_name);
        let result = EdgeAppManifest::ensure_manifest_is_valid(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_manifest_to_file_with_is_global_true_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("screenly.yml");

        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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
app_id: test_app
user_version: test_version
description: test_description
icon: test_icon
author: test_author
homepage_url: test_url
entrypoint: entrypoint.html
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
            app_id: Some("test_app".to_string()),
            user_version: Some("test_version".to_string()),
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: Some("test_author".to_string()),
            homepage_url: Some("test_url".to_string()),
            entrypoint: Some("entrypoint.html".to_owned()),
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
    }

    #[test]
    fn test_prepare_manifest_payload_omits_none_fields() {
        let manifest = EdgeAppManifest {
            app_id: Some("test_app".to_string()),
            user_version: None,
            description: Some("test_description".to_string()),
            icon: Some("test_icon".to_string()),
            author: None,
            homepage_url: Some("test_url".to_string()),
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
    }
}
