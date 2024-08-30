#[cfg(test)]
pub mod tests {
    use crate::authentication::Config;
    use tempfile::TempDir;

    use crate::authentication::Authentication;
    use crate::commands::edge_app::instance_manifest::{
        InstanceManifest, INSTANCE_MANIFEST_VERSION,
    };
    use crate::commands::edge_app::manifest::{
        EdgeAppManifest, Entrypoint, EntrypointType, MANIFEST_VERSION,
    };
    use crate::api::edge_app::setting::Setting;
    use crate::commands::edge_app::EdgeAppCommand;

    use httpmock::MockServer;

    use tempfile::tempdir;

    pub fn create_edge_app_manifest_for_test(settings: Vec<Setting>) -> EdgeAppManifest {
        EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            auth: None,
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings,
            ready_signal: None,
        }
    }

    pub fn create_instance_manifest_for_test() -> InstanceManifest {
        InstanceManifest {
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEB".to_string()),
            name: "test".to_string(),
            entrypoint_uri: None,
        }
    }

    pub fn prepare_edge_apps_test(
        create_manifest: bool,
        create_instance_manifest: bool,
    ) -> (
        TempDir,
        EdgeAppCommand,
        MockServer,
        Option<EdgeAppManifest>,
        Option<InstanceManifest>,
    ) {
        let tmp_dir = tempdir().unwrap();
        let mock_server = MockServer::start();
        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);

        let edge_app_manifest = if create_manifest {
            let edge_app_manifest = create_edge_app_manifest_for_test(vec![]);
            EdgeAppManifest::save_to_file(
                &edge_app_manifest,
                tmp_dir.path().join("screenly.yml").as_path(),
            )
            .unwrap();
            Some(edge_app_manifest)
        } else {
            None
        };

        let instance_manifest = if create_instance_manifest {
            let instance_manifest = create_instance_manifest_for_test();
            InstanceManifest::save_to_file(
                &instance_manifest,
                tmp_dir.path().join("instance.yml").as_path(),
            )
            .unwrap();
            Some(instance_manifest)
        } else {
            None
        };

        (
            tmp_dir,
            command,
            mock_server,
            edge_app_manifest,
            instance_manifest,
        )
    }
}
