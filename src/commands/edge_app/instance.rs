use super::EdgeAppCommand;
use crate::commands;

use crate::commands::edge_app::instance_manifest::{InstanceManifest, INSTANCE_MANIFEST_VERSION};
use crate::commands::edge_app::utils::transform_instance_path_to_instance_manifest;
use crate::commands::{CommandError, EdgeAppInstances};
use std::str;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

impl EdgeAppCommand {
    fn get_instance_name(&self, installation_id: &str) -> Result<String, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/installations?select=name&id=eq.{}",
                installation_id
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Instance {
            name: String,
        }

        let instances = serde_json::from_value::<Vec<Instance>>(response)?;
        if instances.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(instances[0].name.clone())
    }
    pub fn list_instances(&self, app_id: &str) -> Result<EdgeAppInstances, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/installations?select=id,name&app_id=eq.{}",
                app_id
            ),
        )?;

        let instances = EdgeAppInstances::new(response);

        Ok(instances)
    }

    pub fn create_instance(
        &self,
        path: &Path,
        app_id: &str,
        name: &str,
    ) -> Result<String, CommandError> {
        // Though we could either allow --force to re-create it or --new to create a new instance w/o writing to instance.yml
        if let Ok(manifest) = InstanceManifest::new(path) {
            if manifest.id.is_some() {
                return Err(CommandError::InstanceAlreadyExists);
            }
        }

        let installation_id = self.install_edge_app(app_id, name, None)?;

        let instance_manifest = InstanceManifest {
            id: Some(installation_id.clone()),
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            name: name.to_owned(),
            entrypoint_uri: None,
        };

        InstanceManifest::save_to_file(&instance_manifest, path)?;

        Ok(installation_id)
    }

    pub fn delete_instance(
        &self,
        installation_id: &str,
        manifest_path: String,
    ) -> Result<(), CommandError> {
        commands::delete(
            &self.authentication,
            &format!("v4.1/edge-apps/installations?id=eq.{}", installation_id),
        )?;
        match fs::remove_file(manifest_path) {
            Ok(_) => {
                println!("Instance manifest file removed.")
            }
            Err(_) => {
                println!("Failed to remove instance manifest file.")
            }
        };
        Ok(())
    }

    pub fn update_instance(&self, path: Option<String>) -> Result<(), CommandError> {
        let instance_manifest =
            InstanceManifest::new(&transform_instance_path_to_instance_manifest(&path)?)?;
        let installation_id = match instance_manifest.id {
            Some(ref id) => id.clone(),
            None => return Err(CommandError::MissingInstallationId),
        };

        let server_instance_name = self.get_instance_name(&installation_id)?;

        if instance_manifest.name != server_instance_name {
            let payload = json!({
                "name": instance_manifest.name,
            });
            commands::patch(
                &self.authentication,
                &format!("v4.1/edge-apps/installations?id=eq.{}", installation_id),
                &payload,
            )?;
        }

        self.update_entrypoint_value(path)?;

        Ok(())
    }

    pub fn install_edge_app(
        &self,
        app_id: &str,
        name: &str,
        entrypoint: Option<String>,
    ) -> Result<String, CommandError> {
        let mut payload = json!({
            "app_id": app_id,
            "name": name,
        });

        if let Some(_entrypoint) = entrypoint {
            payload["entrypoint"] = json!(_entrypoint);
        }

        let response = commands::post(
            &self.authentication,
            "v4.1/edge-apps/installations?select=id",
            &payload,
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Installation {
            id: String,
        }

        let installation = serde_json::from_value::<Vec<Installation>>(response)?;
        if installation.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(installation[0].id.clone())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::commands::edge_app::instance_manifest::InstanceManifest;
    use crate::commands::edge_app::manifest::EdgeAppManifest;
    use crate::commands::edge_app::manifest::{Entrypoint, EntrypointType};
    use crate::commands::edge_app::test_utils::tests::prepare_edge_apps_test;
    use httpmock::Method::{DELETE, GET, PATCH, POST};

    use serde_json::Value;

    #[test]
    fn test_instance_list_should_list_instances() {
        let (_temp_dir, command, mock_server, manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

        let installations_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .query_param("select", "id,name")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                    "name": "Edge app cli installation",
                },
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEC",
                    "name": "Edge app cli installation 2",
                }
            ]));
        });

        let result = command.list_instances(&manifest.unwrap().id.unwrap());

        installations_mock.assert();

        assert!(result.is_ok());
        let installations = result.unwrap();
        let installations_json: Value = serde_json::from_value(installations.value).unwrap();
        assert_eq!(
            installations_json,
            json!(
                [
                    {
                        "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                        "name": "Edge app cli installation",
                    },
                    {
                        "id": "01H2QZ6Z8WXWNDC0KQ198XCZEC",
                        "name": "Edge app cli installation 2",
                    }
                ]
            )
        );
    }

    #[test]
    fn test_create_instance_should_create_instance() {
        let (temp_dir, command, mock_server, manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

        let instance_manifest_path = temp_dir.path().join("instance.yml");

        let create_instance_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4.1/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "name": "Edge app cli installation",
                }));
            then.status(201)
                .json_body(json!([{"id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"}]));
        });

        let result = command.create_instance(
            &instance_manifest_path,
            &manifest.unwrap().id.unwrap(),
            "Edge app cli installation",
        );

        create_instance_mock.assert();
        assert!(result.is_ok());

        assert_eq!(result.unwrap(), "01H2QZ6Z8WXWNDC0KQ198XCZEB");

        let instance_manifest =
            InstanceManifest::new(&temp_dir.path().join("instance.yml")).unwrap();
        assert_eq!(
            instance_manifest.id,
            Some("01H2QZ6Z8WXWNDC0KQ198XCZEB".to_string())
        );
    }

    #[test]
    fn test_create_instance_when_instance_exist_should_fail() {
        let (temp_dir, command, _mock_server, manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let instance_manifest_path = temp_dir.path().join("instance.yml");
        let result = command.create_instance(
            &instance_manifest_path,
            &manifest.unwrap().id.unwrap(),
            "Edge app cli installation",
        );

        assert!(result.is_err());

        assert_eq!(result.unwrap_err().to_string(), "Instance already exists");
    }

    #[test]
    fn test_update_instance_when_name_changed_should_update_instance() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let get_instance_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .query_param("select", "name")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200)
                .json_body(json!([{"name": "Edge app cli installation"}]));
        });

        let update_instance_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "name": "test",
                }));
            then.status(200)
                .json_body(json!([{"id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"}]));
        });

        let result = command.update_instance(Some(temp_dir.path().to_str().unwrap().to_string()));

        get_instance_mock.assert();
        update_instance_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_instance_when_name_not_changed_should_not_update_instance() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let get_instance_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .query_param("select", "name")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([{"name": "test"}]));
        });

        let result = command.update_instance(Some(temp_dir.path().to_str().unwrap().to_string()));

        get_instance_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_instance_when_entrypoint_uri_added_should_create_entrypoint_setting_value() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let mut manifest = _manifest.unwrap();
        let mut instance_manifest = _instance_manifest.unwrap();

        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteLocal,
            uri: None,
        });
        instance_manifest.entrypoint_uri = Some("https://local-entrypoint.com".to_string());
        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        InstanceManifest::save_to_file(
            &instance_manifest,
            temp_dir.path().join("instance.yml").as_path(),
        )
        .unwrap();

        let get_instance_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .query_param("select", "name")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([{"name": "test"}]));
        });

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
                .query_param("name", "eq.screenly_entrypoint");

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
                .query_param("name", "eq.screenly_entrypoint")
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
                        "name": "screenly_entrypoint",
                        "value": "https://local-entrypoint.com",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let result = command.update_instance(Some(temp_dir.path().to_str().unwrap().to_string()));

        get_instance_mock.assert();
        setting_get_is_global_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_instance_when_entrypoint_uri_updated_should_update_entrypoint_setting_value() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let mut manifest = _manifest.unwrap();
        let mut instance_manifest = _instance_manifest.unwrap();

        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteLocal,
            uri: None,
        });
        instance_manifest.entrypoint_uri = Some("https://local-entrypoint2.com".to_string());
        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        InstanceManifest::save_to_file(
            &instance_manifest,
            temp_dir.path().join("instance.yml").as_path(),
        )
        .unwrap();

        let get_instance_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .query_param("select", "name")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([{"name": "test"}]));
        });

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
                .query_param("name", "eq.screenly_entrypoint");

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
                .query_param("name", "eq.screenly_entrypoint")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.installation_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "screenly_entrypoint",
                    "type": "string",
                    "edge_app_setting_values": [
                        {
                            "value": "https://local-entrypoint.com",
                            "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
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
                .query_param("name", "eq.screenly_entrypoint")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .json_body(json!(
                    {
                        "value": "https://local-entrypoint2.com",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let result = command.update_instance(Some(temp_dir.path().to_str().unwrap().to_string()));

        get_instance_mock.assert();
        setting_get_is_global_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_patch.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_instance_should_delete_instance() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let instance_manifest_path = temp_dir.path().join("instance.yml");

        let delete_instance_mock = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4.1/edge-apps/installations")
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(204).body("");
        });

        let result = command.delete_instance(
            "01H2QZ6Z8WXWNDC0KQ198XCZEB",
            instance_manifest_path.to_str().unwrap().to_string(),
        );

        delete_instance_mock.assert();
        assert!(result.is_ok());

        assert!(!instance_manifest_path.as_path().exists());
    }
}
