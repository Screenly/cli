use crate::commands;
use crate::commands::edge_app::EdgeAppCommand;
use crate::commands::{CommandError, EdgeAppSettings};
use log::debug;
use std::collections::HashMap;
use std::str;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::ops::Not;
use std::str::FromStr;

use serde::Deserializer;
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

impl EdgeAppCommand {
    pub fn list_settings(&self, installation_id: &str) -> Result<EdgeAppSettings, CommandError> {
        let app_id = self.get_app_id_by_installation(installation_id)?;

        let app_settings: Vec<HashMap<String, serde_json::Value>> = serde_json::from_value(commands::get(&self.api.authentication,
                                                                                                             &format!("v4.1/edge-apps/settings?select=name,type,default_value,optional,title,help_text,edge_app_setting_values(value)&app_id=eq.{}&order=name.asc",
                                                                                                                      app_id,
                                                                                                             ))?)?;

        Ok(EdgeAppSettings::new(serde_json::to_value(app_settings)?))
    }

    pub fn set_setting(
        &self,
        path: Option<String>,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let installation_id = match self.get_installation_id(path.clone()) {
            Ok(id) => Some(id),
            Err(_) => None,
        };
        let app_id: String = match self.get_app_id(path.clone()) {
            Ok(id) => id,
            Err(_) => return Err(CommandError::MissingAppId),
        };

        let _is_setting_global = self.is_setting_global(&app_id, setting_key)?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct SettingValue {
            name: String,
            #[serde(rename = "type")]
            type_field: String,
            edge_app_setting_values: Vec<HashMap<String, String>>,
        }

        let setting_url: String;
        let settings_values_payload: Value;
        let settings_values_patch_url: String;

        if _is_setting_global {
            setting_url = format!(
                "v4.1/edge-apps/settings?select=name,type,edge_app_setting_values(value)&app_id=eq.{}&edge_app_setting_values.app_id=eq.{}&name=eq.{}",
                app_id, app_id, setting_key,
            );
            settings_values_payload = json!(
                {
                    "app_id": app_id,
                    "name": setting_key,
                    "value": setting_value,
                }
            );
            settings_values_patch_url = format!(
                "v4.1/edge-apps/settings/values?app_id=eq.{}&name=eq.{}",
                app_id, setting_key,
            );
        } else {
            let actual_installation_id = match installation_id {
                Some(id) => id,
                None => return Err(CommandError::MissingInstallationId),
            };

            setting_url = format!(
                "v4.1/edge-apps/settings?select=name,type,edge_app_setting_values(value)&edge_app_setting_values.installation_id=eq.{}&name=eq.{}&app_id=eq.{}",
                actual_installation_id, setting_key, app_id
            );
            settings_values_payload = json!(
                {
                    "installation_id": actual_installation_id,
                    "name": setting_key,
                    "value": setting_value,
                }
            );
            settings_values_patch_url = format!(
                "v4.1/edge-apps/settings/values?installation_id=eq.{}&name=eq.{}",
                actual_installation_id, setting_key,
            );
        }

        let response = commands::get(&self.api.authentication, &setting_url)?;
        let setting_values = serde_json::from_value::<Vec<SettingValue>>(response)?;

        if setting_values.is_empty() {
            commands::post(
                &self.api.authentication,
                "v4.1/edge-apps/settings/values",
                &settings_values_payload,
            )?;
            return Ok(());
        }
        // we do know it is not empty - so it is safe to unwrap
        let setting = setting_values.first().unwrap();

        if setting.type_field == "secret" {
            commands::post(
                &self.api.authentication,
                "v4.1/edge-apps/secrets/values",
                &settings_values_payload,
            )?;
            return Ok(());
        }

        if setting.edge_app_setting_values.is_empty() {
            commands::post(
                &self.api.authentication,
                "v4.1/edge-apps/settings/values",
                &settings_values_payload,
            )?;

            return Ok(());
        }

        if setting.edge_app_setting_values.len() == 1
            && setting.edge_app_setting_values[0].get("value").unwrap() == setting_value
        {
            println!("Setting value is already set to {}", setting_value);
            return Ok(());
        }
        commands::patch(
            &self.api.authentication,
            &settings_values_patch_url,
            &json!(
                {
                    "value": setting_value,
                }
            ),
        )?;

        Ok(())
    }

    pub fn create_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("app_id".to_owned(), json!(app_id));
        payload.insert("name".to_owned(), json!(setting.name));

        debug!("Creating setting: {:?}", &payload);

        let response = commands::post(&self.api.authentication, "v4.1/edge-apps/settings", &payload);
        if response.is_err() {
            let c = commands::get(
                &self.api.authentication,
                &format!("v4.1/edge-apps/settings?app_id=eq.{}", app_id),
            )?;
            debug!("Existing settings: {:?}", c);
            return Err(CommandError::NoChangesToUpload("".to_owned()));
        }

        Ok(())
    }

    pub fn update_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("name".to_owned(), json!(setting.name));

        debug!("Updating setting: {:?}", &payload);

        let response = commands::patch(
            &self.api.authentication,
            &format!(
                "v4.1/edge-apps/settings?app_id=eq.{id}&name=eq.{name}",
                id = app_id,
                name = setting.name
            ),
            &payload,
        );

        if let Err(error) = response {
            debug!("Failed to update setting: {}", setting.name);
            return Err(error);
        }

        Ok(())
    }

    pub fn delete_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let response = commands::delete(
            &self.api.authentication,
            &format!(
                "v4.1/edge-apps/settings?app_id=eq.{id}&name=eq.{name}",
                id = app_id,
                name = setting.name
            ),
        );

        if let Err(error) = response {
            debug!("Failed to delete setting: {}", setting.name);
            return Err(error);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use crate::commands::edge_app::test_utils::tests::prepare_edge_apps_test;
    use httpmock::Method::{GET, PATCH, POST};

    #[test]
    fn test_list_settings_should_send_correct_request() {
        let (_tmp_dir, command, mock_server, _manifest, instance_manifest) =
            prepare_edge_apps_test(false, true);

        let installations_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "app_id")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200).json_body(json!([
                {
                    "app_id": "02H2QZ6Z8WXWNDC0KQ198XCZEW"
                }
            ]));
        });

        // &format!("v4.1/edge-apps/settings?select=name,type,default_value,optional,title,help_text,edge_app_setting_values(value)&app_id=eq.{}&order=name.asc",
        let settings_mock = mock_server.mock(|when, then| {
                when.method(GET)
                    .path("/v4.1/edge-apps/settings")
                    .header("Authorization", "Token token")
                    .header(
                        "user-agent",
                        format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                    )
                    .query_param("select", "name,type,default_value,optional,title,help_text,edge_app_setting_values(value)")
                    .query_param("app_id", "eq.02H2QZ6Z8WXWNDC0KQ198XCZEW")
                    .query_param("order", "name.asc");

                then.status(200).json_body(json!([
                    {
                        "name": "Example setting1",
                        "type": "string",
                        "default_value": "stranger",
                        "optional": true,
                        "title": "Example title1",
                        "help_text": "An example of a setting that is used in index.html",
                        "edge_app_setting_values": [
                            {
                                "value": "stranger1"
                            }
                        ]
                    },
                    {
                        "name": "Example setting2",
                        "type": "string",
                        "default_value": "stranger",
                        "optional": true,
                        "title": "Example title2",
                        "help_text": "An example of a setting that is used in index.html",
                        "edge_app_setting_values": [
                            {
                                "value": "stranger2"
                            }
                        ]
                    },
                    {
                        "name": "Example setting3",
                        "type": "string",
                        "default_value": "stranger",
                        "optional": true,
                        "title": "Example title3",
                        "help_text": "An example of a setting that is used in index.html",
                        "edge_app_setting_values": []
                    },
                    {
                        "name": "Example secret",
                        "type": "secret",
                        "default_value": "stranger",
                        "optional": true,
                        "title": "Example title4",
                        "help_text": "An example of a secret that is used in index.html",
                        "edge_app_setting_values": []
                    }
                ]));
            });

        let result = command.list_settings(&instance_manifest.unwrap().id.unwrap());

        installations_get_mock.assert();
        settings_mock.assert();

        assert!(result.is_ok());
        let settings = result.unwrap();
        let settings_json: Value = serde_json::from_value(settings.value).unwrap();
        assert_eq!(
            settings_json,
            json!([
                {
                    "name": "Example setting1",
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example title1",
                    "help_text": "An example of a setting that is used in index.html",
                    "edge_app_setting_values": [
                        {
                            "value": "stranger1"
                        }
                    ]
                },
                {
                    "name": "Example setting2",
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example title2",
                    "help_text": "An example of a setting that is used in index.html",
                    "edge_app_setting_values": [
                        {
                            "value": "stranger2"
                        }
                    ]
                },
                {
                    "name": "Example setting3",
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example title3",
                    "help_text": "An example of a setting that is used in index.html",
                    "edge_app_setting_values": []
                },
                {
                    "name": "Example secret",
                    "type": "secret",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example title4",
                    "help_text": "An example of a secret that is used in index.html",
                    "edge_app_setting_values": []
                }
            ])
        );
    }

    #[test]
    fn test_set_setting_should_send_correct_request() {
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_get_is_global_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": false,
                }
            ]));
        });

        // "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}"
        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.installation_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([]));
        });

        let setting_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "name": "best_setting",
                        "value": "best_value",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(tmp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value",
        );

        setting_get_is_global_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_setting_when_setting_value_exists_should_send_correct_update_request() {
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_get_is_global_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": false,
                }
            ]));
        });

        // "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}"
        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param(
                    "edge_app_setting_values.installation_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB",
                );
            then.status(200).json_body(json!([
                {
                    "name": "best_setting",
                    "type": "string",
                    "edge_app_setting_values": [
                        {
                            "value": "best_value"
                        }
                    ]

                }
            ]));
        });

        let setting_values_mock_patch = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4.1/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .json_body(json!(
                    {
                        "value": "best_value1",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(tmp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value1",
        );

        setting_get_is_global_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_patch.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_global_setting_when_setting_value_exists_should_send_correct_update_request() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_is_global_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": true,
                }
            ]));
        });

        // "v4.1/edge-apps/settings?select=name,type,edge_app_setting_values(value)&edge_app_setting_values.app_id=eq.{}&name=eq.{}",
        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.app_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "best_setting",
                    "type": "string",
                    "edge_app_setting_values": [
                        {
                            "value": "best_value"
                        }
                    ]
                }
            ]));
        });

        let setting_values_mock_patch = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4.1/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .json_body(json!(
                    {
                        "value": "best_value1",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value1",
        );

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_patch.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_global_setting_when_setting_value_not_exists_should_send_correct_create_request() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_is_global_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": true,
                }
            ]));
        });

        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.app_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([]));
        });

        let setting_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "value": "best_value1",
                        "name": "best_setting",
                        "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value1",
        );

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_setting_when_setting_doesnt_exist_should_fail() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([]));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value1",
        );

        setting_get_mock.assert();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "Setting does not exist: best_setting.".to_string()
        );
    }

    #[test]
    fn test_set_setting_with_secret_should_send_correct_request() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_is_global_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_secret_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": false,
                }
            ]));
        });

        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_secret_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.installation_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "best_secret_setting",
                    "type": "secret",
                    "edge_app_setting_values": []
                }
            ]));
        });

        // "v4/edge-apps/secrets/values"
        let secrets_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/secrets/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "name": "best_secret_setting",
                        "value": "best_secret_value",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_secret_setting",
            "best_secret_value",
        );

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        secrets_values_mock_post.assert();
        debug!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_global_secrets_should_send_correct_request() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_is_global_get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_secret_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": true,
                }
            ]));
        });

        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_secret_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.app_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "best_secret_setting",
                    "type": "secret",
                    "edge_app_setting_values": []
                }
            ]));
        });

        // "v4/edge-apps/secrets/values"

        let secrets_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/secrets/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "name": "best_secret_setting",
                        "value": "best_secret_value",
                        "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_secret_setting",
            "best_secret_value",
        );

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        secrets_values_mock_post.assert();
        debug!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_setting_when_value_has_not_changed_should_not_update_it() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let setting_get_is_global_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "is_global")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.best_setting");

            then.status(200).json_body(json!([
                {
                    "is_global": false,
                }
            ]));
        });

        // "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}"
        let setting_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("name", "eq.best_setting")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param(
                    "edge_app_setting_values.installation_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB",
                );
            then.status(200).json_body(json!([
                {
                    "name": "best_setting",
                    "type": "string",
                    "edge_app_setting_values": [
                        {
                            "value": "best_value"
                        }
                    ]

                }
            ]));
        });

        let result = command.set_setting(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            "best_setting",
            "best_value",
        );

        setting_get_is_global_mock.assert();
        setting_mock_get.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_is_global_setting_should_pass_is_global_property() {
        let (_temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

        //  v4/edge-apps/settings?app_id=eq.{}
        let settings_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "name": "ssetting",
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "type": "secret",
                    "default_value": "",
                    "title": "stitle",
                    "optional": false,
                    "help_text": "help text",
                    "is_global": true
                }));
            then.status(201).json_body(json!(
            [{
                "name": "ssetting",
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "secret",
                "default_value": "",
                "title": "stitle",
                "optional": false,
                "help_text": "help text",
                "is_global": true,
            }]));
        });

        let setting = Setting {
            name: "ssetting".to_string(),
            type_: SettingType::Secret,
            title: Some("stitle".to_string()),
            optional: false,
            default_value: Some("".to_string()),
            is_global: true,
            help_text: "help text".to_string(),
        };
        command
            .create_setting("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(), &setting)
            .unwrap();

        settings_mock_create.assert();
    }
}
