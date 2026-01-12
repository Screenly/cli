use std::str;

use log::debug;

use crate::api::edge_app::setting::Setting;
use crate::commands::edge_app::EdgeAppCommand;
use crate::commands::{CommandError, EdgeAppSettings};

impl EdgeAppCommand {
    pub fn list_settings(&self, path: Option<String>) -> Result<EdgeAppSettings, CommandError> {
        let app_id = self.get_app_id(path)?;
        self.api.list_settings(&app_id)
    }

    pub fn set_setting(
        &self,
        path: Option<String>,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let installation_id = self.get_installation_id(path.clone()).ok();
        let actual_installation_id = match installation_id {
            Some(id) => id.clone(),
            None => "".to_string(),
        };
        let app_id: String = match self.get_app_id(path.clone()) {
            Ok(id) => id,
            Err(_) => return Err(CommandError::MissingAppId),
        };

        let _is_setting_global = self.api.is_setting_global(&app_id, setting_key)?;

        let server_setting_value = {
            if _is_setting_global {
                self.api.get_global_setting(&app_id, setting_key)?
            } else {
                if actual_installation_id.is_empty() {
                    return Err(CommandError::MissingInstallationId);
                }
                self.api
                    .get_local_setting(&app_id, &actual_installation_id, setting_key)?
            }
        };

        if server_setting_value.is_none() {
            return Err(CommandError::SettingDoesNotExist(setting_key.to_string()));
        }

        // we do know it is not empty - so it is safe to unwrap
        let setting = server_setting_value.unwrap();

        if setting.type_field == "secret" {
            if _is_setting_global {
                self.api
                    .create_global_secret_value(&app_id, setting_key, setting_value)?;
            } else {
                if actual_installation_id.is_empty() {
                    return Err(CommandError::MissingInstallationId);
                }
                self.api.create_local_secret_value(
                    &actual_installation_id,
                    setting_key,
                    setting_value,
                )?;
            }

            return Ok(());
        }

        if setting.edge_app_setting_values.is_empty() {
            if _is_setting_global {
                self.api
                    .create_global_setting_value(&app_id, setting_key, setting_value)?;
            } else {
                if actual_installation_id.is_empty() {
                    return Err(CommandError::MissingInstallationId);
                }
                self.api.create_local_setting_value(
                    &actual_installation_id,
                    setting_key,
                    setting_value,
                )?;
            }

            return Ok(());
        }

        if setting.edge_app_setting_values.len() == 1
            && setting.edge_app_setting_values[0].get("value").unwrap() == setting_value
        {
            println!("Setting value is already set to {setting_value}");
            return Ok(());
        }

        if _is_setting_global {
            self.api
                .update_global_setting_value(&app_id, setting_key, setting_value)?;
        } else {
            if actual_installation_id.is_empty() {
                return Err(CommandError::MissingInstallationId);
            }
            self.api.update_local_setting_value(
                &actual_installation_id,
                setting_key,
                setting_value,
            )?;
        }

        Ok(())
    }

    pub fn create_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let response = self.api.create_setting(&app_id, setting);
        if response.is_err() {
            let c = self.api.get_settings(&app_id)?;
            debug!("Existing settings: {c:?}");
            return Err(CommandError::NoChangesToUpload("".to_owned()));
        }

        Ok(())
    }

    pub fn update_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let response = self.api.update_setting(&app_id, setting);

        if let Err(error) = response {
            debug!("Failed to update setting: {}", setting.name);
            return Err(error);
        }

        Ok(())
    }

    pub fn delete_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let response = self.api.delete_setting(&app_id, setting);

        if let Err(error) = response {
            debug!("Failed to delete setting: {}", setting.name);
            return Err(error);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use httpmock::Method::{GET, PATCH, POST};
    use serde_json::{json, Value};

    use super::*;
    use crate::api::edge_app::setting::SettingType;
    use crate::commands::edge_app::test_utils::tests::prepare_edge_apps_test;

    #[test]
    fn test_list_settings_should_send_correct_request() {
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

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
                    .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
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

        let result = command.list_settings(Some(tmp_dir.path().to_str().unwrap().to_string()));

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
            then.status(200).json_body(json!([
                {
                    "name": "best_setting",
                    "type": "string",
                    "edge_app_setting_values": []
                }
            ]));
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
                .query_param("installation_id", "is.null")
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
            then.status(200).json_body(json!([
                {
                    "name": "best_setting",
                    "type": "string",
                    "edge_app_setting_values": []
                }
            ]));
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
        debug!("result: {result:?}");
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
        debug!("result: {result:?}");
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
