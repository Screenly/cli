use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{
    CommandError, EdgeAppManifest, EdgeAppSettings, EdgeAppVersions, EdgeApps, Setting,
};
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use std::collections::HashMap;
use std::{str, thread};

use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use std::time::Duration;

use crate::commands::edge_app_utils::{
    collect_paths_for_upload, detect_changed_files, detect_changed_settings,
    ensure_edge_app_has_all_necessary_files, generate_file_tree, FileChanges, SettingChanges,
};

pub struct EdgeAppCommand {
    authentication: Authentication,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetSignature {
    pub(crate) signature: String,
}

impl EdgeAppCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn create(&self, name: &str, path: &Path) -> Result<(), CommandError> {
        let response = commands::post(
            &self.authentication,
            "v4/edge-apps?select=id,name",
            &json!({ "name": name }),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct EdgeAppCreationResponse {
            #[serde(default)]
            pub id: String,
            #[serde(default)]
            pub name: String,
        }

        let json_response = serde_json::from_value::<Vec<EdgeAppCreationResponse>>(response)?;
        let app_id = json_response[0].id.clone();
        if app_id.is_empty() {
            return Err(CommandError::MissingField);
        }

        let manifest = EdgeAppManifest {
            app_id,
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&manifest)?;
        let manifest_file = File::create(path)?;
        write!(&manifest_file, "{yaml}")?;

        let index_html_template = include_str!("../../data/index.html");
        let index_html_file = File::create(
            path.parent()
                .ok_or(CommandError::FileSystemError(
                    "Can not obtain edge app root directory.".to_owned(),
                ))?
                .join("index.html"),
        )?;
        write!(&index_html_file, "{index_html_template}")?;

        Ok(())
    }

    pub fn list(&self) -> Result<EdgeApps, CommandError> {
        Ok(EdgeApps::new(commands::get(
            &self.authentication,
            "v4/edge-apps?select=id,name",
        )?))
    }

    pub fn list_versions(&self, path: &Path) -> Result<EdgeAppVersions, CommandError> {
        let manifest = EdgeAppManifest::new(path)?;
        Ok(EdgeAppVersions::new(commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=revision,user_version,description,published&app_id=eq.{}",
                manifest.app_id
            ),
        )?))
    }

    pub fn list_settings(
        &self,
        manifest: &EdgeAppManifest,
    ) -> Result<EdgeAppSettings, CommandError> {
        Ok(EdgeAppSettings::new(commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&app_revision=eq.{}&order=title.asc",
                manifest.app_id,
                manifest.revision
            ),
        )?))
    }

    pub fn upload(self, path: &Path) -> Result<(), CommandError> {
        let data = fs::read_to_string(path)?;
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;
        let edge_app_dir = path.parent().ok_or(CommandError::MissingField)?;

        let local_files = collect_paths_for_upload(edge_app_dir)?;
        ensure_edge_app_has_all_necessary_files(&local_files)?;

        let remote_files = self.get_version_asset_signatures(&manifest)?;
        let changed_files = detect_changed_files(&local_files, &remote_files)?;
        debug!("Changed files: {:?}", &changed_files);

        let remote_settings =
            serde_json::from_value::<Vec<Setting>>(self.list_settings(&manifest)?.value)?;
        let changed_settings = detect_changed_settings(&manifest, &remote_settings)?;
        let file_tree = generate_file_tree(&local_files, edge_app_dir);
        let old_file_tree = self.get_file_tree(&manifest);

        let file_tree_changed = match old_file_tree {
            Ok(tree) => file_tree != tree,
            Err(_) => true,
        };

        debug!("File tree changed: {}", file_tree_changed);
        if !self.requires_upload(&changed_settings, &changed_files) && !file_tree_changed {
            return Err(CommandError::NoChangesToUpload(
                "No changes detected".to_owned(),
            ));
        }

        // now that we know we have changes, we can create a new version
        let revision =
            self.create_version(&manifest, generate_file_tree(&local_files, edge_app_dir))?;
        debug!("Created new version: {}", revision);
        let manifest = EdgeAppManifest {
            revision,
            ..manifest
        };
        EdgeAppManifest::save_to_file(&manifest, path)?;

        self.upload_changed_settings(&manifest, &changed_settings)?;

        self.upload_changed_files(edge_app_dir, &manifest, &changed_files)?;
        debug!("Files uploaded");

        self.ensure_assets_processing_finished(&manifest)?;
        // now we freeze it by publishing it
        self.publish(&manifest)?;
        debug!("Edge app published.");

        let root_asset_id = match self.get_root_edge_app_asset(&manifest) {
            Ok(asset_id) => {
                debug!("Found root edge app asset. No need to install.");
                asset_id
            }
            Err(_) => {
                debug!("No root edge app asset found. Installing...");
                self.install_edge_app(&manifest)?
            }
        };

        let manifest = EdgeAppManifest {
            root_asset_id,
            ..manifest
        };

        EdgeAppManifest::save_to_file(&manifest, path)?;

        Ok(())
    }

    pub fn promote(&self, manifest: &EdgeAppManifest, version: &Option<i32>) -> Result<i32, CommandError> {
        let payload = match version {
            Some(version_value) => json!(
                {
                    "revision": version_value,
                    "app_id": manifest.app_id.clone(),
                }
            ),
            None => json!(
                {
                    "app_id": manifest.app_id.clone(),
                }
            ),
        };
        let response = commands::post(
            &self.authentication,
            "v4/edge-apps/promote",
            &payload,
        )?;

        if let Some(dict) = response.as_object() {
            if let Some(revision) = dict.get("updated") {
                if let Some(revision_value) = revision.as_i64() {
                    return Ok(revision_value as i32);
                }
            }
        };

        Err(CommandError::MissingField)
    }

    fn create_version(
        &self,
        manifest: &EdgeAppManifest,
        file_tree: HashMap<String, String>,
    ) -> Result<u32, CommandError> {
        let json = json!({
           "app_id": manifest.app_id,
           "user_version": manifest.user_version,
           "description": manifest.description,
           "icon": manifest.icon,
           "author": manifest.author,
           "homepage_url": manifest.homepage_url,
           "file_tree": file_tree,
        });

        let response = commands::post(
            &self.authentication,
            "v4/edge-apps/versions?select=revision",
            &json,
        )?;

        if let Some(arr) = response.as_array() {
            if let Some(obj) = arr.get(0) {
                if let Some(revision) = obj["revision"].as_u64() {
                    debug!("New version revision: {}", revision);
                    return Ok(revision as u32);
                }
            }
        }

        Err(CommandError::MissingField)
    }

    fn get_file_tree(
        &self,
        manifest: &EdgeAppManifest,
    ) -> Result<HashMap<String, String>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=file_tree&app_id=eq.{}&revision=eq.{}",
                manifest.app_id, manifest.revision
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Deserialize)]
        struct FileTree {
            file_tree: HashMap<String, String>,
        }

        let file_tree = serde_json::from_value::<Vec<FileTree>>(response)?;
        if file_tree.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(file_tree[0].file_tree.clone())
    }

    fn requires_upload(
        &self,
        changed_settings: &SettingChanges,
        changed_files: &FileChanges,
    ) -> bool {
        changed_settings.has_changes() || changed_files.has_changes()
    }

    fn get_version_asset_signatures(
        &self,
        manifest: &EdgeAppManifest,
    ) -> Result<Vec<AssetSignature>, CommandError> {
        Ok(serde_json::from_value(commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=signature&app_id=eq.{}&app_revision=eq.{}&type=eq.edge-app-file",
                manifest.app_id, manifest.revision
            ),
        )?)?)
    }

    fn ensure_assets_processing_finished(
        &self,
        manifest: &EdgeAppManifest,
    ) -> Result<(), CommandError> {
        const SLEEP_TIME: u64 = 2;
        const MAX_WAIT_TIME: u64 = 20; // 20 seconds
        let mut total_duration = 0;

        loop {
            // TODO: we are not handling possible errors in asset processing here.
            // Which are unlikely to happen, because we upload assets as they are, but still
            if total_duration > MAX_WAIT_TIME {
                return Err(CommandError::AssetProcessingTimeout);
            }

            let value = commands::get(
                &self.authentication,
                &format!(
                    "v4/assets?select=status&app_id=eq.{}&app_revision=eq.{}&status=neq.finished&limit=1",
                    manifest.app_id, manifest.revision
                ),
            )?;
            debug!("ensure_assets_processing_finished: {:?}", &value);

            if let Some(array) = value.as_array() {
                if array.is_empty() {
                    break;
                }
            }
            thread::sleep(Duration::from_secs(SLEEP_TIME));
            total_duration += SLEEP_TIME;
        }
        Ok(())
    }

    fn get_root_edge_app_asset(&self, manifest: &EdgeAppManifest) -> Result<String, CommandError> {
        let v = commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=id&app_id=eq.{}&app_revision=eq.{}&type=eq.edge-app",
                manifest.app_id, manifest.revision
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Asset {
            id: String,
        }

        let asset = serde_json::from_value::<Vec<Asset>>(v)?;
        if asset.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(asset[0].id.clone())
    }

    fn upload_changed_settings(
        &self,
        manifest: &EdgeAppManifest,
        changed_settings: &SettingChanges,
    ) -> Result<(), CommandError> {
        for setting in &changed_settings.creates {
            self.create_setting(manifest, setting)?;
        }
        Ok(())
    }

    fn upload_changed_files(
        &self,
        edge_app_dir: &Path,
        manifest: &EdgeAppManifest,
        changed_files: &FileChanges,
    ) -> Result<(), CommandError> {
        debug!("Changed files: {:#?}", changed_files);
        if !changed_files.copies.is_empty() {
            self.copy_edge_app_assets(manifest, &changed_files.copies)?;
        }

        debug!("Uploading edge app assets");
        for file in &changed_files.uploads {
            self.upload_edge_app_asset(manifest, edge_app_dir.join(file.path.clone()).as_path())?;
        }

        Ok(())
    }

    fn create_setting(
        &self,
        manifest: &EdgeAppManifest,
        setting: &Setting,
    ) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("app_id".to_owned(), json!(manifest.app_id));
        payload.insert("app_revision".to_owned(), json!(manifest.revision));

        debug!("Creating setting: {:?}", &payload);

        let _response = commands::post(&self.authentication, "v4/edge-apps/settings", &payload);
        if _response.is_err() {
            let c = commands::get(
                &self.authentication,
                &format!(
                    "v4/edge-apps/settings?app_id=eq.{}&app_revision=eq.{}",
                    manifest.app_id, manifest.revision
                ),
            )?;
            debug!("Existing settings: {:?}", c);
            return Err(CommandError::NoChangesToUpload("".to_owned()));
        }

        Ok(())
    }

    fn copy_edge_app_assets(
        &self,
        manifest: &EdgeAppManifest,
        asset_signatures: &[String],
    ) -> Result<(), CommandError> {
        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);
        let payload = json!({
            "app_id": manifest.app_id,
            "revision": manifest.revision,
            "signatures": asset_signatures,
        });

        let _response = commands::post(&self.authentication, "v4/edge-apps/copy-assets", &payload)?;
        Ok(())
    }

    fn upload_edge_app_asset(
        &self,
        manifest: &EdgeAppManifest,
        path: &Path,
    ) -> Result<(), CommandError> {
        let url = format!("{}/v4/assets", &self.authentication.config.url);

        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

        let file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let pb = ProgressBar::new(file_size);

        if let Ok(template) = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:160.cyan/blue} {percent}% ETA: {eta}",
        ) {
            pb.set_style(template);
        }

        let part = reqwest::blocking::multipart::Part::reader(pb.wrap_read(file)).file_name("file");
        debug!("Uploading file: {:?}", path);
        let form = reqwest::blocking::multipart::Form::new()
            .text(
                "title",
                path.file_name()
                    .ok_or(CommandError::FileSystemError(
                        "Can't obtain file name".to_owned(),
                    ))?
                    .to_string_lossy()
                    .to_string(),
            )
            .text("app_id", manifest.app_id.clone())
            .text("app_revision", manifest.revision.to_string())
            .part("file", part);

        let response = self
            .authentication
            .build_client()?
            .post(url)
            .multipart(form)
            .headers(headers)
            .send()?;
        let status = response.status();
        if status != StatusCode::CREATED {
            debug!("Response: {:?}", &response.text());
            return Err(CommandError::WrongResponseStatus(status.as_u16()));
        }

        Ok(())
    }

    fn install_edge_app(&self, manifest: &EdgeAppManifest) -> Result<String, CommandError> {
        let mut payload = json!({
            "app_id": manifest.app_id,
            "revision": manifest.revision,
        });

        for setting in &manifest.settings {
            // if default value is not set we are fine for now with setting it to empty string
            payload[setting.title.clone()] = json!(setting.default_value);
        }

        let response = commands::post(&self.authentication, "v4/edge-apps/install", &payload)?;
        Ok(response.as_str().unwrap_or_default().to_string())
    }

    fn publish(&self, manifest: &EdgeAppManifest) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?app_id=eq.{}&revision=eq.{}",
                manifest.app_id, manifest.revision
            ),
            &json!({"published": true}),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authentication::Config;

    use httpmock::Method::{GET, PATCH, POST};
    use httpmock::MockServer;

    use tempdir::TempDir;

    #[test]
    fn test_edge_app_create_should_create_app_and_required_files() {
        let tmp_dir = TempDir::new("test").unwrap();

        let mock_server = MockServer::start();
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps")
                .header("Authorization", "Token token")
                .json_body(json!({
                    "name": "Best app ever"
                }));
            then.status(201)
                .json_body(json!([{"id": "test-id", "name": "Best app ever"}]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);

        let result = command.create(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        post_mock.assert();

        assert!(tmp_dir.path().join("screenly.yml").exists());
        assert!(tmp_dir.path().join("index.html").exists());

        let data = fs::read_to_string(tmp_dir.path().join("screenly.yml")).unwrap();
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(manifest.app_id, "test-id");

        let data_index_html = fs::read_to_string(tmp_dir.path().join("index.html")).unwrap();
        assert_eq!(data_index_html, include_str!("../../data/index.html"));

        assert!(result.is_ok());
    }

    #[test]
    fn test_list_edge_apps_should_send_correct_request() {
        let mock_server = MockServer::start();
        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let result = command.list();
        edge_apps_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_versions_should_send_correct_request() {
        let mock_server = MockServer::start();
        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            root_asset_id: "".to_string(),
            user_version: "1".to_string(),
            revision: 7,
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let tmp_dir = TempDir::new("test").unwrap();
        EdgeAppManifest::save_to_file(&manifest, tmp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let result = command.list_versions(tmp_dir.path().join("screenly.yml").as_path());
        edge_apps_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_settings_should_send_correct_request() {
        let mock_server = MockServer::start();
        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.7");
            then.status(200).json_body(json!([]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            root_asset_id: "".to_string(),
            user_version: "1".to_string(),
            revision: 7,
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.list_settings(&manifest);
        edge_apps_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_upload_should_send_correct_requests() {
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            root_asset_id: "".to_string(),
            user_version: "1".to_string(),
            revision: 7,
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let mock_server = MockServer::start();
        // "v4/assets?select=signature&app_id=eq.{}&app_revision=eq.{}&type=eq.edge-app-file",

        let assets_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "signature")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.7")
                .query_param("type", "eq.edge-app-file");
            then.status(200).json_body(json!([{"signature": "sig"}]));
        });

        let settings_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.7")
                .query_param("select", "type,default_value,optional,title,help_text");
            then.status(200).json_body(json!([           {
                    "type": "text".to_string(),
                    "default_value": "5".to_string(),
                    "title": "display_time".to_string(),
                    "optional": true,
                    "help_text": "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
                }]));
        });

        let file_tree_from_version_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("revision", "eq.7")
                .query_param("select", "file_tree");
            then.status(200).json_body(json!([{"index.html": "sig"}]));
        });

        let create_version_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(201).json_body(json!([{"revision": 8}]));
        });

        let upload_assets_mock = mock_server.mock(|when, then| {
            when.method(POST).path("/v4/assets");
            then.status(201).body("");
        });
        // "v4/assets?select=status&app_id=eq.{}&app_revision=eq.{}&status=neq.finished&limit=1",
        let finished_processing_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("select", "status")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.8")
                .query_param("status", "neq.finished")
                .query_param("limit", "1");
            then.status(200).json_body(json!([]));
        });

        //   "v4/edge-apps/versions?app_id=eq.{}&revision=eq.{}",
        let publish_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("revision", "eq.8")
                .json_body(json!({"published": true }));
            then.status(200);
        });

        // get root edge app asset
        //   "v4/assets?select=id&app_id=eq.{}&app_revision=eq.{}&type=eq.edge-app",
        let get_root_asset_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.8")
                .query_param("type", "eq.edge-app");

            then.status(200).json_body(json!([{"id": "ASSET_ID"}]));
        });

        let temp_dir = TempDir::new("test").unwrap();
        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let result = command.upload(temp_dir.path().join("screenly.yml").as_path());

        assets_mock.assert();
        settings_mock.assert();
        file_tree_from_version_mock.assert();
        create_version_mock.assert();
        upload_assets_mock.assert();
        finished_processing_mock.assert();
        publish_mock.assert();
        get_root_asset_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_promote_should_send_correct_request() {
        let mock_server = MockServer::start();
        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/promote")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "revision": 1,
                }));
            then.status(201)
                .json_body(json!({"updated": 2}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            root_asset_id: "".to_string(),
            user_version: "1".to_string(),
            revision: 7,
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.promote(&manifest, &Option::Some(1));
        edge_apps_mock.assert();

        assert!(&result.is_ok());
        assert_eq!(&result.unwrap(), &2);
    }

    #[test]
    fn test_promote_with_no_revision_should_send_correct_request() {
        let mock_server = MockServer::start();
        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/promote")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                }));
            then.status(201)
                .json_body(json!({"updated": 2}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            root_asset_id: "".to_string(),
            user_version: "1".to_string(),
            revision: 7,
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.promote(&manifest, &Option::None);
        edge_apps_mock.assert();

        assert!(&result.is_ok());
        assert_eq!(&result.unwrap(), &2);
    }
}
