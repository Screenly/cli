use std::collections::HashMap;

use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, Display};

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

pub fn serialize_settings<S>(settings: &[Setting], serializer: S) -> Result<S::Ok, S::Error>
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

pub fn deserialize_settings<'de, D>(deserializer: D) -> Result<Vec<Setting>, D::Error>
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
        .join("\n") + "\n";

    match SettingType::from_str(&s.to_lowercase()) {
        Ok(setting_type) => Ok(setting_type),
        Err(_) => Err(serde::de::Error::custom(format!(
            "Setting type should be one of the following:\n{}",
            valid_setting_types
        ))),
    }
}