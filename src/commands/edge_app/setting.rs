use std::collections::HashMap;

use std::ops::Not;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::commands::serde_utils::{deserialize_string_field, serialize_non_empty_string_field};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Default, EnumString, Display, EnumIter)]
pub enum SettingType {
    #[default]
    #[strum(serialize = "string")]
    String,
    #[strum(serialize = "secret")]
    Secret,
}

// maybe we can use a better name as we have EdgeAppSettings which is the same but serde_json::Value inside
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    #[serde(
        rename = "type",
        serialize_with = "serialize_setting_type",
        deserialize_with = "deserialize_setting_type"
    )]
    pub type_: SettingType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(skip)]
    pub name: String,
    pub optional: bool,

    #[serde(
        serialize_with = "serialize_help_text",
        deserialize_with = "deserialize_help_text"
    )]
    pub help_text: String,

    #[serde(default = "bool::default", skip_serializing_if = "<&bool>::not")]
    pub is_global: bool,
}

pub fn serialize_settings<S>(settings: &[Setting], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;

    let mut map = serializer.serialize_map(Some(settings.len()))?;
    for setting in settings {
        map.serialize_entry(&setting.name, &setting)?;
    }
    map.end()
}

pub fn deserialize_settings<'de, D>(deserializer: D) -> Result<Vec<Setting>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, Setting> = serde::Deserialize::deserialize(deserializer)?;
    let mut settings: Vec<Setting> = map
        .into_iter()
        .map(|(name, mut setting)| {
            setting.name = name;
            setting
        })
        .collect();

    for setting in &settings {
        if setting.type_ == SettingType::Secret && setting.default_value.is_some() {
            return Err(serde::de::Error::custom(format!(
                "Setting \"{}\" is of type \"secret\" and cannot have a default value",
                setting.name
            )));
        }
        if setting.name.starts_with("screenly_") {
            return Err(serde::de::Error::custom(format!(
                "Setting \"{}\" cannot start with \"screenly_\" as this prefix is preserved.",
                setting.name
            )));
        }
    }

    settings.sort_by_key(|s| s.name.clone());
    Ok(settings)
}

pub fn deserialize_settings_from_array<'de, D>(deserializer: D) -> Result<Vec<Setting>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: Vec<HashMap<String, Value>> = serde::Deserialize::deserialize(deserializer)?;
    let mut settings: Vec<Setting> = map
        .into_iter()
        .map(|setting_data| {
            let mut setting = Setting::default();
            for (key, value) in setting_data {
                match key.as_str() {
                    "type" => {
                        setting.type_ =
                            deserialize_setting_type(value).expect("Failed to parse setting type.");
                    }
                    "default_value" => {
                        setting.default_value = value.as_str().map(|s| s.to_string());
                    }
                    "title" => {
                        setting.title = value.as_str().map(|s| s.to_string());
                    }
                    "optional" => {
                        setting.optional = value.as_bool().expect("Failed to parse optional.")
                    }
                    "help_text" => {
                        setting.help_text = value
                            .as_str()
                            .expect("Failed to parse help_text.")
                            .to_string();
                    }
                    "is_global" => {
                        setting.is_global = value.as_bool().expect("Failed to parse is_global.");
                    }
                    "name" => {
                        setting.name = value.as_str().expect("Failed to parse name.").to_string();
                    }
                    _ => {}
                }
            }
            setting
        })
        .collect();

    settings.sort_by_key(|s| s.name.clone());
    Ok(settings)
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

    let valid_setting_types = SettingType::iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    match SettingType::from_str(&s.to_lowercase()) {
        Ok(setting_type) => Ok(setting_type),
        Err(_) => Err(serde::de::Error::custom(format!(
            "Setting type should be one of the following:\n{}",
            valid_setting_types
        ))),
    }
}

fn serialize_help_text<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serialize_non_empty_string_field("help_text", value, serializer)
}

fn deserialize_help_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    deserialize_string_field("help_text", true, deserializer)
}

impl Setting {
    pub fn new(type_: SettingType, title: &str, name: &str, help_text: &str, global: bool) -> Self {
        Setting {
            type_,
            default_value: None,
            title: Some(title.to_string()),
            name: name.to_string(),
            optional: false,
            help_text: help_text.to_string(),
            is_global: global,
        }
    }
}
