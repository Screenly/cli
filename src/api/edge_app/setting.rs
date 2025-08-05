use crate::commands;
use crate::commands::CommandError;
use crate::{api::Api, commands::EdgeAppSettings};

use log::debug;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::ops::Not;
use std::str::FromStr;

use serde::Deserializer;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::commands::serde_utils::{deserialize_string_field, serialize_non_empty_string_field};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SettingValue {
    name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub edge_app_setting_values: Vec<HashMap<String, String>>,
}

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Default, EnumString, Display, EnumIter)]
pub enum SettingType {
    #[default]
    #[strum(serialize = "string")]
    String,
    #[strum(serialize = "secret")]
    Secret,
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
            "Setting type should be one of the following:\n{valid_setting_types}"
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

impl Api {
    pub fn get_settings(&self, app_id: &str) -> Result<Vec<Setting>, CommandError> {
        Ok(deserialize_settings_from_array(commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?select=name,type,default_value,optional,title,help_text&app_id=eq.{app_id}&order=name.asc",
            ),
        )?)?)
    }

    pub fn is_setting_global(&self, app_id: &str, setting_key: &str) -> Result<bool, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?select=is_global&app_id=eq.{app_id}&name=eq.{setting_key}",
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct SettingIsGlobal {
            is_global: bool,
        }

        let setting_list = serde_json::from_value::<Vec<SettingIsGlobal>>(response)?;

        if setting_list.is_empty() {
            return Err(CommandError::SettingDoesNotExist(setting_key.to_string()));
        }

        let setting = &setting_list[0];

        Ok(setting.is_global)
    }

    pub fn list_settings(&self, app_id: &str) -> Result<EdgeAppSettings, CommandError> {
        // TODO: test values are returned properly when there are several installations. Most likely need to feed installation_id to the request.
        // installation_id=is.null or installation_id=eq.smth
        let app_settings: Vec<HashMap<String, serde_json::Value>> = serde_json::from_value(commands::get(&self.authentication,
            &format!("v4.1/edge-apps/settings?select=name,type,default_value,optional,title,help_text,edge_app_setting_values(value)&app_id=eq.{app_id}&order=name.asc",
            ))?)?;

        Ok(EdgeAppSettings::new(serde_json::to_value(app_settings)?))
    }

    pub fn get_global_setting(
        &self,
        app_id: &str,
        setting_key: &str,
    ) -> Result<Option<SettingValue>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?select=name,type,edge_app_setting_values(value)&app_id=eq.{app_id}&edge_app_setting_values.app_id=eq.{app_id}&name=eq.{setting_key}"
            ),
        )?;
        let settings = serde_json::from_value::<Vec<SettingValue>>(response)?;
        if settings.is_empty() {
            return Ok(None);
        }
        Ok(Some(settings[0].clone()))
    }

    pub fn get_local_setting(
        &self,
        app_id: &str,
        installation_id: &str,
        setting_key: &str,
    ) -> Result<Option<SettingValue>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?select=name,type,edge_app_setting_values(value)&edge_app_setting_values.installation_id=eq.{installation_id}&name=eq.{setting_key}&app_id=eq.{app_id}"
            ),
        )?;

        let settings: Vec<SettingValue> = serde_json::from_value(response)?;

        if settings.is_empty() {
            return Ok(None);
        }
        Ok(Some(settings[0].clone()))
    }

    pub fn create_setting(&self, app_id: &str, setting: &Setting) -> Result<Value, CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("app_id".to_owned(), json!(app_id));
        payload.insert("name".to_owned(), json!(setting.name));

        debug!("Creating setting: {:?}", &payload);
        commands::post(&self.authentication, "v4.1/edge-apps/settings", &payload)
    }

    pub fn update_setting(&self, app_id: &str, setting: &Setting) -> Result<Value, CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("name".to_owned(), json!(setting.name));

        debug!("Updating setting: {:?}", &payload);

        commands::patch(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?app_id=eq.{id}&name=eq.{name}",
                id = app_id,
                name = setting.name
            ),
            &payload,
        )
    }

    pub fn delete_setting(&self, app_id: &str, setting: &Setting) -> Result<(), CommandError> {
        commands::delete(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings?app_id=eq.{id}&name=eq.{name}",
                id = app_id,
                name = setting.name
            ),
        )?;
        Ok(())
    }

    pub fn create_global_setting_value(
        &self,
        app_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let settings_values_payload = json!(
            {
                "app_id": app_id,
                "name": setting_key,
                "value": setting_value,
            }
        );
        commands::post(
            &self.authentication,
            "v4.1/edge-apps/settings/values",
            &settings_values_payload,
        )?;

        Ok(())
    }

    pub fn create_local_setting_value(
        &self,
        installation_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let settings_values_payload = json!(
            {
                "installation_id": installation_id,
                "name": setting_key,
                "value": setting_value,
            }
        );
        commands::post(
            &self.authentication,
            "v4.1/edge-apps/settings/values",
            &settings_values_payload,
        )?;

        Ok(())
    }

    pub fn update_global_setting_value(
        &self,
        app_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings/values?app_id=eq.{app_id}&name=eq.{setting_key}&installation_id=is.null"
            ),
            &json!({
                "value": setting_value,
            }),
        )?;

        Ok(())
    }

    pub fn update_local_setting_value(
        &self,
        installation_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/settings/values?installation_id=eq.{installation_id}&name=eq.{setting_key}"
            ),
            &json!({
                "value": setting_value,
            }),
        )?;

        Ok(())
    }

    pub fn create_global_secret_value(
        &self,
        app_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let payload = json!(
            {
                "app_id": app_id,
                "name": setting_key,
                "value": setting_value,
            }
        );
        commands::post(
            &self.authentication,
            "v4.1/edge-apps/secrets/values",
            &payload,
        )?;

        Ok(())
    }

    pub fn create_local_secret_value(
        &self,
        installation_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let payload = json!(
            {
                "installation_id": installation_id,
                "name": setting_key,
                "value": setting_value,
            }
        );
        commands::post(
            &self.authentication,
            "v4.1/edge-apps/secrets/values",
            &payload,
        )?;

        Ok(())
    }
}
