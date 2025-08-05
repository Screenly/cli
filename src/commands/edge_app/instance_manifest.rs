use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::commands::edge_app::manifest::beautify_error_message;
use crate::commands::serde_utils::{
    deserialize_option_string_field, string_field_is_none_or_empty,
};
use crate::commands::CommandError;

pub const INSTANCE_MANIFEST_VERSION: &str = "instance_v1";

#[serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceManifest {
    /// The syntax version of the instance manifest.
    #[serde(deserialize_with = "deserialize_syntax")]
    pub syntax: String,

    /// The unique identifier for this instance.
    #[serde(
        deserialize_with = "deserialize_instance_id",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub id: Option<String>,

    /// The name of the instance.
    #[serde(deserialize_with = "deserialize_name")]
    pub name: String,

    /// The entrypoint URI for the instance. Only valid when the app has remote-local entrypoint
    /// type.
    #[serde(
        deserialize_with = "deserialize_entrypoint_uri",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub entrypoint_uri: Option<String>,
}

fn deserialize_instance_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let maybe_instance_id = deserialize_option_string_field("id", true, deserializer);

    maybe_instance_id.map_err(|_e| {
        serde::de::Error::custom("Enter a valid ULID `id` parameter either in the maniphest file or as a command line parameter (e.g. `--instance_id XXXXXXXXXXXXXXXX`). Field \"id\" cannot be empty in the maniphest file (instance.yml)")
    })
}

fn deserialize_syntax<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        INSTANCE_MANIFEST_VERSION => Ok(s),
        invalid => Err(serde::de::Error::custom(format!(
            "Invalid syntax: {invalid}. Only 'instance_v1' is accepted."
        ))),
    }
}

fn deserialize_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Err(serde::de::Error::custom(
            "The 'name' field is mandatory and cannot be empty",
        ))
    } else {
        Ok(s)
    }
}

fn deserialize_entrypoint_uri<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let maybe_uri = deserialize_option_string_field("entrypoint_uri", true, deserializer)?;

    if let Some(uri) = maybe_uri {
        if uri.trim().is_empty() {
            return Ok(None);
        }

        match Url::parse(&uri) {
            Ok(url) if url.scheme() == "http" || url.scheme() == "https" => Ok(Some(uri)),
            _ => Err(serde::de::Error::custom(
                "The 'entrypoint_uri' must be a valid URL with http or https schema",
            )),
        }
    } else {
        Ok(None)
    }
}

impl InstanceManifest {
    pub fn new(path: &Path) -> Result<InstanceManifest, CommandError> {
        match fs::read_to_string(path) {
            Ok(data) => {
                let manifest: InstanceManifest = serde_yaml::from_str(&data)?;
                Ok(manifest)
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    if let Some(2) = e.raw_os_error() {
                        return Err(CommandError::MisingManifest(format!("{}", path.display())));
                    }
                }
                Err(CommandError::InvalidManifest(e.to_string()))
            }
        }
    }

    pub fn save_to_file(manifest: &InstanceManifest, path: &Path) -> Result<(), CommandError> {
        let yaml = serde_yaml::to_string(&manifest)?;
        let manifest_file = File::create(path)?;
        write!(&manifest_file, "---\n{yaml}")?;
        Ok(())
    }

    pub fn ensure_manifest_is_valid(path: &Path) -> Result<(), CommandError> {
        match InstanceManifest::new(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(CommandError::InvalidManifest(beautify_error_message(
                &e.to_string(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    const INSTANCE_MANIFEST_FILENAME: &str = "instance.yml";

    fn create_valid_manifest() -> InstanceManifest {
        InstanceManifest {
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: Some("01H7YRXN7XMH2ALPC5FMTC6ZY4".to_string()),
            name: "Test Instance".to_string(),
            entrypoint_uri: Some("https://example.com/app".to_string()),
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
        manifest: InstanceManifest,
    ) -> Result<InstanceManifest, CommandError> {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(INSTANCE_MANIFEST_FILENAME);

        InstanceManifest::save_to_file(&manifest, &file_path)?;
        InstanceManifest::new(&file_path)
    }

    #[test]
    fn test_save_manifest_to_file_should_save_yaml_correctly() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(INSTANCE_MANIFEST_FILENAME);

        let manifest = create_valid_manifest();

        InstanceManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: Test Instance
entrypoint_uri: https://example.com/app
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_save_manifest_to_file_should_skip_none_optional_fields() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(INSTANCE_MANIFEST_FILENAME);

        let manifest = InstanceManifest {
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: None,
            name: "Test Instance".to_string(),
            entrypoint_uri: None,
        };

        InstanceManifest::save_to_file(&manifest, &file_path).unwrap();

        let contents = fs::read_to_string(file_path).unwrap();

        let expected_contents = r#"---
syntax: instance_v1
name: Test Instance
"#;

        assert_eq!(contents, expected_contents);
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct() {
        let manifest = create_valid_manifest();

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();

        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_serialize_deserialize_cycle_should_pass_on_valid_struct_missing_optional_fields() {
        let manifest = InstanceManifest {
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: None,
            name: "Test Instance".to_string(),
            entrypoint_uri: None,
        };

        let deserialized_manifest = serialize_deserialize_cycle(manifest.clone()).unwrap();

        assert_eq!(manifest, deserialized_manifest);
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_file_non_existent_should_return_error() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(INSTANCE_MANIFEST_FILENAME);

        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_file_valid_should_return_ok() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: Test Instance
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_ok());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_missing_required_field_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invalid_syntax_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: invalid_syntax
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: Test Instance
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_invalid_entrypoint_uri_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: Test Instance
entrypoint_uri: invalid-url
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_misspelled_name_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
naem: Test Instance
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_extra_field_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: Test Instance
entrypoint_uri: https://example.com/app
extra_field: This should not be here
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_id_is_empty_string_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: ""
name: Test Instance
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }

    #[test]
    fn test_ensure_manifest_is_valid_when_name_is_empty_string_should_return_error() {
        let dir = tempdir().unwrap();
        let content = r#"---
syntax: instance_v1
id: 01H7YRXN7XMH2ALPC5FMTC6ZY4
name: ""
entrypoint_uri: https://example.com/app
"#;

        let file_path = write_to_tempfile(&dir, INSTANCE_MANIFEST_FILENAME, content);
        assert!(InstanceManifest::ensure_manifest_is_valid(&file_path).is_err());
    }
}
