use std::collections::{HashMap, HashSet};
use std::ops::Not;
use std::str::FromStr;

use log::debug;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::api::Api;
use crate::commands;
use crate::commands::{CommandError, EdgeAppSettings};

const SETTING_HELP_TEXT_SCHEMA_VERSION: u32 = 1;

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
    struct SettingsVisitor;

    impl<'de> serde::de::Visitor<'de> for SettingsVisitor {
        type Value = Vec<Setting>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map of settings")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut settings = Vec::new();
            while let Some((name, mut setting)) = map.next_entry::<String, Setting>()? {
                setting.name = name;
                settings.push(setting);
            }
            Ok(settings)
        }
    }

    let settings = deserializer.deserialize_map(SettingsVisitor)?;

    let mut seen_names: HashSet<&str> = HashSet::new();

    for setting in &settings {
        if !seen_names.insert(setting.name.as_str()) {
            return Err(serde::de::Error::custom(format!(
                "Setting \"{}\" is declared more than once. Each setting name must be unique.",
                setting.name
            )));
        }
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
                        setting.help_text = match value {
                            Value::String(help_text) => help_text,
                            other => other.to_string(),
                        };
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
    if value.trim().is_empty() {
        return Err(serde::ser::Error::custom(
            "Field \"help_text\" cannot be empty",
        ));
    }

    match serde_json::from_str::<Value>(value) {
        Ok(json_value) if json_value.is_object() => json_value.serialize(serializer),
        _ => serializer.serialize_str(value),
    }
}

fn deserialize_help_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HelpTextHelper {
        Plain(String),
        Structured(Value),
    }

    match HelpTextHelper::deserialize(deserializer)? {
        HelpTextHelper::Plain(value) => {
            if value.trim().is_empty() {
                Err(serde::de::Error::custom(
                    "Field \"help_text\" cannot be empty",
                ))
            } else {
                Ok(value)
            }
        }
        HelpTextHelper::Structured(value) => {
            if !value.is_object() {
                return Err(serde::de::Error::custom(
                    "Field \"help_text\" must be either a string or an object",
                ));
            }

            serde_json::to_string(&value).map_err(|err| {
                serde::de::Error::custom(format!("Failed to serialize help_text: {err}"))
            })
        }
    }
}

const HELP_TEXT_NAME_OVERRIDES: &[&str] = &[
    "message_body",
    "rss_url",
    "bypass_cors",
    "cache_interval",
    "limit",
    "override_coordinates",
    "override_locale",
    "override_timezone",
    "target_timestamp",
    "stop_id",
    "azure_ad_scope",
    "azure_ad_resource",
    "theme",
];

fn is_structured_help_text(help_text: &str) -> bool {
    serde_json::from_str::<Value>(help_text).is_ok_and(|value| value.is_object())
}

fn has_malformed_properties(help_text: &str) -> bool {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(help_text) else {
        return false;
    };
    matches!(object.get("properties"), Some(value) if !value.is_object())
}

pub fn help_text_with_display_order(name: &str, help_text: &str, display_order: usize) -> String {
    if HELP_TEXT_NAME_OVERRIDES.contains(&name) {
        return help_text.to_string();
    }

    match serde_json::from_str::<Value>(help_text) {
        Ok(Value::Object(mut object)) => {
            match object.get_mut("properties") {
                Some(Value::Object(properties)) => {
                    properties
                        .entry("display_order")
                        .or_insert_with(|| json!(display_order));
                }
                Some(_) => return help_text.to_string(),
                None => {
                    object.insert(
                        "properties".to_string(),
                        json!({ "display_order": display_order }),
                    );
                }
            }
            object
                .entry("schema_version")
                .or_insert_with(|| json!(SETTING_HELP_TEXT_SCHEMA_VERSION));
            serde_json::to_string(&Value::Object(object)).unwrap_or_else(|_| help_text.to_string())
        }
        _ => json!({
            "schema_version": SETTING_HELP_TEXT_SCHEMA_VERSION,
            "properties": {
                "help_text": help_text,
                "display_order": display_order,
            }
        })
        .to_string(),
    }
}

pub fn assign_setting_display_orders(settings: &mut [Setting]) {
    let mut skipped: Vec<String> = Vec::new();
    let mut malformed: Vec<String> = Vec::new();

    for (display_order, setting) in settings.iter_mut().enumerate() {
        if setting.name.starts_with("screenly_") {
            continue;
        }

        if has_malformed_properties(&setting.help_text) {
            malformed.push(setting.name.clone());
            continue;
        }

        if setting.type_ == SettingType::Secret && !is_structured_help_text(&setting.help_text) {
            continue;
        }

        if HELP_TEXT_NAME_OVERRIDES.contains(&setting.name.as_str()) {
            skipped.push(setting.name.clone());
            continue;
        }

        setting.help_text =
            help_text_with_display_order(&setting.name, &setting.help_text, display_order);
    }

    if !malformed.is_empty() {
        eprintln!(
            "Warning: the following settings have a malformed help_text schema (\"properties\" is not an object) and will render as a plain field with the raw JSON as their help text: {}.",
            malformed.join(", ")
        );
    }

    if !skipped.is_empty() {
        eprintln!(
            "Warning: no display order was assigned to the following settings, so the UI decides where they render: {}.",
            skipped.join(", ")
        );
    }
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

        debug!("Creating setting: {:?}", payload);
        commands::post(&self.authentication, "v4.1/edge-apps/settings", &payload)
    }

    pub fn update_setting(&self, app_id: &str, setting: &Setting) -> Result<Value, CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("name".to_owned(), json!(setting.name));

        debug!("Updating setting: {:?}", payload);

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

#[cfg(test)]
mod display_order_tests {
    use super::*;

    fn setting(name: &str, type_: SettingType, help_text: &str) -> Setting {
        Setting {
            type_,
            default_value: None,
            title: Some(name.to_string()),
            name: name.to_string(),
            optional: false,
            help_text: help_text.to_string(),
            is_global: false,
        }
    }

    fn properties(help_text: &str) -> serde_json::Map<String, Value> {
        serde_json::from_str::<Value>(help_text)
            .unwrap()
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn plain_help_text_is_wrapped_with_the_positional_display_order() {
        let result = help_text_with_display_order("greeting", "Say hello", 3);
        let properties = properties(&result);

        assert_eq!(properties["help_text"], json!("Say hello"));
        assert_eq!(properties["display_order"], json!(3));
    }

    #[test]
    fn explicitly_authored_display_order_is_not_overwritten() {
        let authored = json!({
            "schema_version": 1,
            "properties": { "help_text": "x", "type": "number", "display_order": 2 }
        })
        .to_string();

        let properties = properties(&help_text_with_display_order("number_field", &authored, 0));

        assert_eq!(properties["display_order"], json!(2));
        assert_eq!(properties["type"], json!("number"));
    }

    #[test]
    fn structured_help_text_without_display_order_gets_the_positional_one() {
        let authored = json!({
            "schema_version": 1,
            "properties": { "help_text": "x", "type": "url" }
        })
        .to_string();

        let properties = properties(&help_text_with_display_order("url_field", &authored, 5));

        assert_eq!(properties["display_order"], json!(5));
        assert_eq!(properties["type"], json!("url"));
    }

    #[test]
    fn json_object_with_a_malformed_properties_key_is_left_untouched() {
        let authored = json!({ "schema_version": 1, "properties": "nope" }).to_string();

        assert_eq!(
            help_text_with_display_order("weird", &authored, 0),
            authored,
            "a malformed object must not be stringified into its own help text"
        );
    }

    #[test]
    fn json_object_without_properties_keeps_its_other_keys() {
        let authored = json!({ "schema_version": 1, "depends_on": "other" }).to_string();

        let value: Value =
            serde_json::from_str(&help_text_with_display_order("field", &authored, 1)).unwrap();

        assert_eq!(value["depends_on"], json!("other"));
        assert_eq!(value["properties"]["display_order"], json!(1));
    }

    #[test]
    fn overridden_names_are_left_untouched() {
        assert_eq!(
            help_text_with_display_order("theme", "Pick a theme", 0),
            "Pick a theme"
        );
    }

    #[test]
    fn internal_and_excluded_settings_are_skipped() {
        let mut settings = vec![
            setting("greeting", SettingType::String, "Say hello"),
            setting("theme", SettingType::String, "Pick a theme"),
            setting("api_key", SettingType::Secret, "Your API key"),
            setting(
                "screenly_entrypoint",
                SettingType::String,
                "The entrypoint.",
            ),
        ];

        assign_setting_display_orders(&mut settings);

        assert_eq!(
            properties(&settings[0].help_text)["display_order"],
            json!(0)
        );
        assert_eq!(settings[1].help_text, "Pick a theme");
        assert_eq!(settings[2].help_text, "Your API key");
        assert_eq!(settings[3].help_text, "The entrypoint.");
    }

    #[test]
    fn malformed_properties_are_left_untouched_and_do_not_consume_a_display_order() {
        let malformed = json!({ "schema_version": 1, "properties": "nope" }).to_string();
        let mut settings = vec![
            setting("greeting", SettingType::String, "Say hello"),
            setting("weird", SettingType::String, &malformed),
            setting("farewell", SettingType::String, "Say bye"),
        ];

        assign_setting_display_orders(&mut settings);

        assert_eq!(
            properties(&settings[0].help_text)["display_order"],
            json!(0)
        );
        assert_eq!(settings[1].help_text, malformed);
        assert_eq!(
            properties(&settings[2].help_text)["display_order"],
            json!(2)
        );
    }

    #[test]
    fn secrets_that_opt_into_structured_help_text_are_still_ordered() {
        let authored = json!({
            "schema_version": 1,
            "properties": { "help_text": "Your API key" }
        })
        .to_string();
        let mut settings = vec![setting("api_key", SettingType::Secret, &authored)];

        assign_setting_display_orders(&mut settings);

        assert_eq!(
            properties(&settings[0].help_text)["display_order"],
            json!(0)
        );
    }

    #[test]
    fn duplicate_setting_names_are_rejected() {
        let yaml = "\
greeting:
  type: string
  optional: true
  help_text: first
greeting:
  type: string
  optional: true
  help_text: second
";
        let error = deserialize_settings(serde_yaml::Deserializer::from_str(yaml))
            .expect_err("duplicate setting names must be rejected");

        assert!(
            error.to_string().contains("declared more than once"),
            "unexpected error: {error}"
        );
    }
}
