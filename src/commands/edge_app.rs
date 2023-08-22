use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{
    CommandError, EdgeAppManifest, EdgeAppSettings, EdgeAppVersions, EdgeApps, Setting,
};
use indicatif::ProgressBar;
use log::debug;
use std::collections::HashMap;
use std::{str, thread};

use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serde_yaml;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
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
        let parent_dir_path = path.parent().ok_or(CommandError::FileSystemError(
            "Can not obtain edge app root directory.".to_owned(),
        ))?;
        let index_html_path = parent_dir_path.join("index.html");

        if Path::new(&path).exists() || Path::new(&index_html_path).exists() {
            return Err(CommandError::FileSystemError(format!(
                "The directory {} already contains a screenly.yml or index.html file",
                parent_dir_path.display()
            )));
        }

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
            settings: vec![Setting {
                title: "username".to_string(),
                type_: "string".to_string(),
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }],
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&manifest)?;
        let manifest_file = File::create(path)?;
        write!(&manifest_file, "{yaml}")?;

        let index_html_template = include_str!("../../data/index.html");
        let index_html_file = File::create(&index_html_path)?;
        write!(&index_html_file, "{index_html_template}")?;

        Ok(())
    }

    pub fn list(&self) -> Result<EdgeApps, CommandError> {
        Ok(EdgeApps::new(commands::get(
            &self.authentication,
            "v4/edge-apps?select=id,name",
        )?))
    }

    pub fn list_versions(&self, app_id: &str) -> Result<EdgeAppVersions, CommandError> {
        Ok(EdgeAppVersions::new(commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=revision,user_version,description,published&app_id=eq.{}",
                app_id
            ),
        )?))
    }

    pub fn list_settings(&self, app_id: &str) -> Result<EdgeAppSettings, CommandError> {
        let installation_id = self.get_or_create_installation(app_id)?;
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings/values?select=title,value&installation_id=eq.{}",
                installation_id
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct SettingValue {
            title: String,
            value: String,
        }
        let settings: HashMap<String, String> =
            serde_json::from_value::<Vec<SettingValue>>(response)?
                .into_iter()
                .map(|setting| (setting.title, setting.value))
                .collect();

        let mut app_settings: Vec<HashMap<String, serde_json::Value>> = serde_json::from_value(commands::get(&self.authentication,
                                                                                                             &format!("v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc&type=eq.text",
                                                                                                                      app_id,
                                                                                                             ))?)?;

        // Combine settings and values into one object
        for setting in app_settings.iter_mut() {
            let title = setting
                .get("title")
                .and_then(|t| t.as_str())
                .ok_or_else(|| {
                    eprintln!("Title field not found in the setting.");
                    CommandError::MissingField
                })?;

            let value = match settings.get(title) {
                Some(v) => v,
                None => continue,
            };

            setting.insert("value".to_string(), Value::String(value.to_string()));
        }

        Ok(EdgeAppSettings::new(serde_json::to_value(app_settings)?))
    }

    pub fn set_setting(
        &self,
        app_id: &str,
        setting_key: &str,
        setting_value: &str,
    ) -> Result<(), CommandError> {
        let installation_id = self.get_or_create_installation(app_id)?;

        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}",
                installation_id, setting_key,
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct SettingValue {
            title: String,
        }

        let setting_values = serde_json::from_value::<Vec<SettingValue>>(response)?;
        if setting_values.is_empty() {
            commands::post(
                &self.authentication,
                "v4/edge-apps/settings/values",
                &json!(
                    {
                        "installation_id": installation_id,
                        "title": setting_key,
                        "value": setting_value,
                    }
                ),
            )?;
        } else {
            commands::patch(
                &self.authentication,
                &format!(
                    "v4/edge-apps/settings/values?installation_id=eq.{}&title=eq.{}",
                    installation_id, setting_key,
                ),
                &json!(
                    {
                        "value": setting_value,
                    }
                ),
            )?;
        }

        Ok(())
    }

    pub fn set_secret(
        &self,
        app_id: &str,
        secret_key: &str,
        secret_value: &str,
    ) -> Result<(), CommandError> {
        let installation_id = self.get_or_create_installation(app_id)?;

        commands::post(
            &self.authentication,
            "v4/edge-apps/secrets/values",
            &json!(
                {
                    "installation_id": installation_id,
                    "title": secret_key,
                    "value": secret_value,
                }
            ),
        )?;

        Ok(())
    }

    pub fn upload(self, path: &Path, app_id: Option<String>) -> Result<u32, CommandError> {
        let data = fs::read_to_string(path)?;
        let mut manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;

        // override app_id if user passed it
        if let Some(id) = app_id {
            manifest.app_id = id;
        }

        let edge_app_dir = path.parent().ok_or(CommandError::MissingField)?;

        let local_files = collect_paths_for_upload(edge_app_dir)?;
        ensure_edge_app_has_all_necessary_files(&local_files)?;

        let revision = self.get_latest_revision(&manifest.app_id).unwrap_or(0);

        let remote_files = self.get_version_asset_signatures(&manifest.app_id, revision)?;
        let changed_files = detect_changed_files(&local_files, &remote_files)?;
        debug!("Changed files: {:?}", &changed_files);

        let remote_settings = serde_json::from_value::<Vec<Setting>>(commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc",
                manifest.app_id
            ),
        )?)?;

        let changed_settings = detect_changed_settings(&manifest, &remote_settings)?;
        self.upload_changed_settings(&manifest, &changed_settings)?;

        let file_tree = generate_file_tree(&local_files, edge_app_dir);

        let old_file_tree = self.get_file_tree(&manifest.app_id, revision);

        let file_tree_changed = match old_file_tree {
            Ok(tree) => file_tree != tree,
            Err(_) => true,
        };

        debug!("File tree changed: {}", file_tree_changed);
        if !self.requires_upload(&changed_files) && !file_tree_changed {
            return Err(CommandError::NoChangesToUpload(
                "No changes detected".to_owned(),
            ));
        }

        // now that we know we have changes, we can create a new version
        let revision =
            self.create_version(&manifest, generate_file_tree(&local_files, edge_app_dir))?;

        self.upload_changed_files(edge_app_dir, &manifest.app_id, revision, &changed_files)?;
        debug!("Files uploaded");

        self.ensure_assets_processing_finished(&manifest.app_id, revision)?;
        // now we freeze it by publishing it
        self.publish(&manifest.app_id, revision)?;
        debug!("Edge app published.");

        self.get_or_create_installation(&manifest.app_id)?;

        Ok(revision)
    }

    pub fn promote_version(
        &self,
        app_id: &str,
        revision: &u32,
        channel: &String,
    ) -> Result<(), CommandError> {
        let response = commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/channels?select=channel,app_revision&channel=eq.{}&app_id=eq.{}",
                channel, app_id
            ),
            &json!(
            {
                "app_revision": revision,
            }),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Deserialize)]
        struct Channel {
            app_revision: u32,
            channel: String,
        }

        let channels = serde_json::from_value::<Vec<Channel>>(response)?;
        if channels.is_empty() {
            return Err(CommandError::MissingField);
        }
        if &channels[0].channel != channel || &channels[0].app_revision != revision {
            return Err(CommandError::MissingField);
        }

        Ok(())
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

    fn get_latest_revision(&self, app_id: &str) -> Result<u32, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=revision&order=revision.desc&limit=1&app_id=eq.{}",
                app_id
            ),
        )?;

        #[derive(Deserialize)]
        struct EdgeAppVersion {
            revision: u32,
        }

        let versions: Vec<EdgeAppVersion> = serde_json::from_value(response)?;
        if let Some(version) = versions.get(0) {
            Ok(version.revision)
        } else {
            Err(CommandError::MissingField)
        }
    }

    fn get_file_tree(
        &self,
        app_id: &str,
        revision: u32,
    ) -> Result<HashMap<String, String>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=file_tree&app_id=eq.{}&revision=eq.{}",
                app_id, revision
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

    fn requires_upload(&self, changed_files: &FileChanges) -> bool {
        changed_files.has_changes()
    }

    fn get_version_asset_signatures(
        &self,
        app_id: &str,
        revision: u32,
    ) -> Result<Vec<AssetSignature>, CommandError> {
        Ok(serde_json::from_value(commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=signature&app_id=eq.{}&app_revision=eq.{}&type=eq.edge-app-file",
                app_id, revision
            ),
        )?)?)
    }

    fn ensure_assets_processing_finished(
        &self,
        app_id: &str,
        revision: u32,
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
                    app_id, revision
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

    fn get_or_create_installation(&self, app_id: &str) -> Result<String, CommandError> {
        let installation_id = match self.get_installation(app_id) {
            Ok(installation) => {
                debug!("Found installation. No need to install.");
                installation
            }
            Err(_) => {
                debug!("No installation found. Installing...");
                self.install_edge_app(app_id)?
            }
        };

        Ok(installation_id)
    }

    fn get_installation(&self, app_id: &str) -> Result<String, CommandError> {
        let v = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/installations?select=id&app_id=eq.{}&name=eq.Edge app cli installation",
                app_id
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Installation {
            id: String,
        }

        let installation = serde_json::from_value::<Vec<Installation>>(v)?;
        if installation.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(installation[0].id.clone())
    }

    fn upload_changed_settings(
        &self,
        manifest: &EdgeAppManifest,
        changed_settings: &SettingChanges,
    ) -> Result<(), CommandError> {
        for setting in &changed_settings.creates {
            self.create_setting(manifest, setting)?;
        }
        for setting in &changed_settings.updates {
            self.update_setting(manifest, setting)?;
        }
        Ok(())
    }

    fn upload_changed_files(
        &self,
        edge_app_dir: &Path,
        app_id: &str,
        revision: u32,
        changed_files: &FileChanges,
    ) -> Result<(), CommandError> {
        debug!("Changed files: {:#?}", changed_files);

        if !changed_files.copies.is_empty() {
            self.copy_edge_app_assets(app_id, revision, &changed_files.copies)?;
        }

        debug!("Uploading edge app assets");

        let file_paths: Vec<PathBuf> = changed_files
            .uploads
            .iter()
            .map(|file| edge_app_dir.join(&file.path))
            .collect();

        self.upload_edge_app_assets(app_id, revision, &file_paths)?;

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

        debug!("Creating setting: {:?}", &payload);

        let response = commands::post(&self.authentication, "v4/edge-apps/settings", &payload);
        if response.is_err() {
            let c = commands::get(
                &self.authentication,
                &format!("v4/edge-apps/settings?app_id=eq.{}", manifest.app_id),
            )?;
            debug!("Existing settings: {:?}", c);
            return Err(CommandError::NoChangesToUpload("".to_owned()));
        }

        Ok(())
    }

    fn update_setting(
        &self,
        manifest: &EdgeAppManifest,
        setting: &Setting,
    ) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;

        debug!("Updating setting: {:?}", &payload);

        let response = commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings?app_id=eq.{id}&title=eq.{title}",
                id = manifest.app_id,
                title = setting.title
            ),
            &payload,
        );

        if let Err(error) = response {
            debug!("Failed to update setting: {}", setting.title);
            return Err(error);
        }

        Ok(())
    }

    fn copy_edge_app_assets(
        &self,
        app_id: &str,
        revision: u32,
        asset_signatures: &[String],
    ) -> Result<(), CommandError> {
        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);
        let payload = json!({
            "app_id": app_id,
            "revision": revision,
            "signatures": asset_signatures,
        });

        let _response = commands::post(&self.authentication, "v4/edge-apps/copy-assets", &payload)?;
        Ok(())
    }

    fn upload_edge_app_assets(
        &self,
        app_id: &str,
        revision: u32,
        paths: &[PathBuf],
    ) -> Result<(), CommandError> {
        let pb = ProgressBar::new(paths.len() as u64);
        pb.set_message("Files uploaded:");
        let shared_pb = Arc::new(Mutex::new(pb));

        paths.par_iter().try_for_each(|path| {
            let result = self.upload_single_asset(app_id, revision, path, &shared_pb);
            if result.is_ok() {
                let locked_pb = shared_pb.lock().unwrap();
                locked_pb.inc(1);
            }
            result
        })
    }

    fn upload_single_asset(
        &self,
        app_id: &str,
        revision: u32,
        path: &Path,
        _pb: &Arc<Mutex<ProgressBar>>,
    ) -> Result<(), CommandError> {
        let url = format!("{}/v4/assets", &self.authentication.config.url);

        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

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
            .text("app_id", app_id.to_string())
            .text("app_revision", revision.to_string())
            .file("file", path)?;

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

    fn install_edge_app(&self, app_id: &str) -> Result<String, CommandError> {
        let payload = json!({
            "app_id": app_id,
            "name": "Edge app cli installation",
        });

        let response = commands::post(
            &self.authentication,
            "v4/edge-apps/installations?select=id",
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

    fn publish(&self, app_id: &str, revision: u32) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?app_id=eq.{}&revision=eq.{}",
                app_id, revision
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
        assert_eq!(
            manifest.settings,
            vec![Setting {
                title: "username".to_string(),
                type_: "string".to_string(),
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string()
            }]
        );

        let data_index_html = fs::read_to_string(tmp_dir.path().join("index.html")).unwrap();
        assert_eq!(data_index_html, include_str!("../../data/index.html"));

        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_app_create_when_manifest_or_index_html_exist_should_return_error() {
        let command = EdgeAppCommand::new(Authentication::new_with_config(
            Config::new("http://localhost".to_string()),
            "token",
        ));

        let tmp_dir = TempDir::new("test").unwrap();
        File::create(tmp_dir.path().join("screenly.yml")).unwrap();

        let result = command.create(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already contains a screenly.yml or index.html file"));

        fs::remove_file(tmp_dir.path().join("screenly.yml")).unwrap();

        File::create(tmp_dir.path().join("index.html")).unwrap();

        let result = command.create(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already contains a screenly.yml or index.html file"));
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

        let result = command.list_versions("01H2QZ6Z8WXWNDC0KQ198XCZEW");
        edge_apps_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_settings_should_send_correct_request() {
        let mock_server = MockServer::start();

        let installation_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.Edge app cli installation");

            then.status(200).json_body(json!([]));
        });

        let installation_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "name": "Edge app cli installation",
                }));

            then.status(201).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                }
            ]));
        });

        let settings_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "type,default_value,optional,title,help_text")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("order", "title.asc");

            then.status(200).json_body(json!([
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting1",
                    "help_text": "An example of a setting that is used in index.html"
                },
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting2",
                    "help_text": "An example of a setting that is used in index.html"
                },
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting3",
                    "help_text": "An example of a setting that is used in index.html"
                }
            ]));
        });

        let setting_values_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "title,value")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB");

            then.status(200).json_body(json!([
                {
                    "title": "Example setting1",
                    "value": "stranger"
                },
                {
                    "title": "Example setting2",
                    "value": "stranger"
                }
            ]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.list_settings(&manifest.app_id);

        installation_mock.assert();
        installation_mock_create.assert();
        settings_mock.assert();
        setting_values_mock.assert();
        assert!(result.is_ok());
        let settings = result.unwrap();
        let settings_json: Value = serde_json::from_value(settings.value).unwrap();
        assert_eq!(
            settings_json,
            json!([
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting1",
                    "help_text": "An example of a setting that is used in index.html",
                    "value": "stranger",
                },
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting2",
                    "help_text": "An example of a setting that is used in index.html",
                    "value": "stranger"
                },
                {
                    "type": "string",
                    "default_value": "stranger",
                    "optional": true,
                    "title": "Example setting3",
                    "help_text": "An example of a setting that is used in index.html"
                }
            ])
        );
    }

    #[test]
    fn test_set_setting_should_send_correct_request() {
        let mock_server = MockServer::start();

        let installation_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.Edge app cli installation");

            then.status(200).json_body(json!([]));
        });

        let installation_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "name": "Edge app cli installation",
                }));

            then.status(201).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                }
            ]));
        });

        // "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}"
        let setting_values_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("title", "eq.best_setting")
                .query_param("select", "title")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200).json_body(json!([]));
        });

        let setting_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "title": "best_setting",
                        "value": "best_value",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.set_setting(&manifest.app_id, "best_setting", "best_value");
        installation_mock.assert();
        installation_mock_create.assert();
        setting_values_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_setting_when_setting_value_exists_should_send_correct_update_request() {
        let mock_server = MockServer::start();

        let installation_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.Edge app cli installation");

            then.status(200).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                }
            ]));
        });

        // "v4/edge-apps/settings/values?select=title&installation_id=eq.{}&title=eq.{}"
        let setting_values_mock_get = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("title", "eq.best_setting")
                .query_param("select", "title")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200).json_body(json!([
                {
                    "title": "best_setting",
                    "value": "best_value",
                }
            ]));
        });

        let setting_values_mock_patch = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/edge-apps/settings/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("title", "eq.best_setting")
                .query_param("installation_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
                .json_body(json!(
                    {
                        "value": "best_value1",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.set_setting(&manifest.app_id, "best_setting", "best_value1");
        installation_mock.assert();
        setting_values_mock_get.assert();
        setting_values_mock_patch.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_secrets_should_send_correct_request() {
        let mock_server = MockServer::start();

        let installation_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.Edge app cli installation");

            then.status(200).json_body(json!([]));
        });

        let installation_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "name": "Edge app cli installation",
                }));

            then.status(201).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                }
            ]));
        });

        // "v4/edge-apps/secrets/values"

        let secrets_values_mock_post = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/secrets/values")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!(
                    {
                        "title": "best_secret_setting",
                        "value": "best_secret_value",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB"
                    }
                ));
            then.status(204).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result =
            command.set_secret(&manifest.app_id, "best_secret_setting", "best_secret_value");
        installation_mock.assert();
        installation_mock_create.assert();
        secrets_values_mock_post.assert();
        debug!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_upload_should_send_correct_requests() {
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![
                Setting {
                    type_: "string".to_string(),
                    title: "asetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "".to_string(),
                },
                Setting {
                    type_: "string".to_string(),
                    title: "nsetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "".to_string(),
                },
            ],
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

        // v4/edge-apps/versions?select=revision&order=revision.desc&limit=1&app_id=eq.{}
        let revision_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "revision")
                .query_param("order", "revision.desc")
                .query_param("limit", "1")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([{"revision": 7}]));
        });

        // v4/edge-apps/versions?select=file_tree&app_id=eq.{}&revision=eq.{}
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

        //  v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc
        let settings_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("select", "type,default_value,optional,title,help_text")
                .query_param("order", "title.asc");
            then.status(200).json_body(json!([{
                "type": "string".to_string(),
                "default_value": "5".to_string(),
                "title": "nsetting".to_string(),
                "optional": true,
                "help_text": "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            }]));
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

        //  v4/edge-apps/settings?app_id=eq.{}
        let settings_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "type": "string",
                    "default_value": "",
                    "title": "asetting",
                    "optional": false,
                    "help_text": "",
                }));
            then.status(201).json_body(json!(
            [{
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "asetting",
                "optional": false,
                "help_text": "",
            }]));
        });

        let settings_mock_patch = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("title", "eq.nsetting")
                .json_body(json!({
                    "type": "string",
                    "default_value": "",
                    "title": "nsetting",
                    "optional": false,
                    "help_text": "",
                }));
            then.status(200).json_body(json!(
            [{
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "nsetting",
                "optional": false,
                "help_text": "",
            }]));
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
        let installation_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.Edge app cli installation");

            then.status(200).json_body(json!([]));
        });

        let installation_mock_create = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/installations")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "id")
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "name": "Edge app cli installation",
                }));

            then.status(201).json_body(json!([
                {
                    "id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                }
            ]));
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
        let result = command.upload(temp_dir.path().join("screenly.yml").as_path(), None);

        assets_mock.assert();
        file_tree_from_version_mock.assert();
        settings_mock.assert();
        create_version_mock.assert();
        settings_mock_create.assert();
        settings_mock_patch.assert();
        upload_assets_mock.assert();
        finished_processing_mock.assert();
        publish_mock.assert();
        installation_mock.assert();
        installation_mock_create.assert();
        revision_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_promote_should_send_correct_request() {
        let mock_server = MockServer::start();
        let promote_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/edge-apps/channels")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("channel", "eq.public")
                .query_param("select", "channel,app_revision")
                .json_body(json!({
                    "app_revision": 7,
                }));
            then.status(200).json_body(json!([
                {
                    "channel": "public",
                    "app_revision": 7
                }
            ]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: "1".to_string(),
            description: "asdf".to_string(),
            icon: "asdf".to_string(),
            author: "asdf".to_string(),
            homepage_url: "asdfasdf".to_string(),
            settings: vec![],
        };

        let result = command.promote_version(&manifest.app_id, &7, &"public".to_string());
        promote_mock.assert();

        assert!(&result.is_ok());
    }
}
