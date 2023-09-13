use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{
    CommandError, EdgeAppSecrets, EdgeAppSettings, EdgeAppVersions, EdgeApps
};
use crate::commands::edge_app_manifest::EdgeAppManifest;
use crate::commands::edge_app_settings::{SettingType, Setting};
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
use std::time::{Duration, Instant};

use crate::commands::edge_app_utils::{
    collect_paths_for_upload, detect_changed_files, detect_changed_settings,
    ensure_edge_app_has_all_necessary_files, generate_file_tree, FileChanges, SettingChanges,
};

use crate::commands::edge_app_server::{run_server, Metadata, MOCK_DATA_FILENAME};

pub struct EdgeAppCommand {
    authentication: Authentication,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetSignature {
    pub(crate) signature: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeAppCreationResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
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
                "The directory {} already contains a screenly.yml or index.html file. Use --in-place if you want to create an Edge App in this directory",
                parent_dir_path.display()
            )));
        }

        let response = commands::post(
            &self.authentication,
            "v4/edge-apps?select=id,name",
            &json!({ "name": name }),
        )?;

        let json_response = serde_json::from_value::<Vec<EdgeAppCreationResponse>>(response)?;
        let app_id = json_response[0].id.clone();
        if app_id.is_empty() {
            return Err(CommandError::MissingField);
        }

        let manifest = EdgeAppManifest {
            app_id: Some(app_id),
            settings: vec![Setting {
                title: "greeting".to_string(),
                type_: SettingType::Secret,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
            }],
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, path)?;

        let index_html_template = include_str!("../../data/index.html");
        let index_html_file = File::create(&index_html_path)?;
        write!(&index_html_file, "{index_html_template}")?;

        Ok(())
    }

    pub fn create_in_place(&self, name: &str, path: &Path) -> Result<(), CommandError> {
        let parent_dir_path = path.parent().ok_or(CommandError::FileSystemError(
            "Can not obtain edge app root directory.".to_owned(),
        ))?;
        let index_html_path = parent_dir_path.join("index.html");

        if !(Path::new(&path).exists() && Path::new(&index_html_path).exists()) {
            return Err(CommandError::FileSystemError(format!(
                "The directory {} should contain screenly.yml and index.html files",
                parent_dir_path.display()
            )));
        }

        let data = fs::read_to_string(path)?;
        let mut manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;

        if manifest.app_id.is_some() {
            return Err(CommandError::InitializationError("The operation can only proceed when 'app_id' is not set in the 'screenly.yml' configuration file".to_string()));
        }

        let response = commands::post(
            &self.authentication,
            "v4/edge-apps?select=id,name",
            &json!({ "name": name }),
        )?;

        let json_response = serde_json::from_value::<Vec<EdgeAppCreationResponse>>(response)?;
        let app_id = json_response[0].id.clone();
        if app_id.is_empty() {
            return Err(CommandError::MissingField);
        }

        manifest.app_id = Some(app_id);
        EdgeAppManifest::save_to_file(&manifest, path)?;

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
                "v4/edge-apps/versions?select=edge_app_channels(channel),revision,user_version,description,published&app_id=eq.{}",
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
                                                                                                             &format!("v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc&type=eq.string",
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

    pub fn list_secrets(&self, app_id: &str) -> Result<EdgeAppSecrets, CommandError> {
        let app_secrets: Vec<HashMap<String, serde_json::Value>> = serde_json::from_value(
            commands::get(
                &self.authentication,
                &format!("v4/edge-apps/settings?select=optional,title,help_text&app_id=eq.{}&order=title.asc&type=eq.secret", app_id,)
            )?
        )?;

        Ok(EdgeAppSecrets::new(serde_json::to_value(app_secrets)?))
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

    pub fn run(&self, path: &Path, secrets: Vec<(String, String)>) -> Result<(), anyhow::Error> {
        let address_shared = Arc::new(Mutex::new(None));
        let address_clone = address_shared.clone();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let path = path.to_path_buf();
        runtime.block_on(async {
            tokio::spawn(async move {
                let address = run_server(path.as_path(), secrets).await.unwrap();
                let mut locked_address = address_clone.lock().unwrap();
                *locked_address = Some(address);
            })
            .await
            .unwrap();

            while address_shared.lock().unwrap().is_none() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            println!(
                "Edge app emulator is running at {}/index.html",
                address_shared.lock().unwrap().as_ref().unwrap()
            );

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });

        Ok(())
    }

    pub fn upload(self, path: &Path, app_id: Option<String>) -> Result<u32, CommandError> {
        EdgeAppManifest::ensure_manifest_is_valid(path)?;
        let mut manifest = EdgeAppManifest::new(path)?;

        // override app_id if user passed it
        if let Some(id) = app_id {
            if id.is_empty() {
                return Err(CommandError::EmptyAppId);
            }
            manifest.app_id = Some(id);
        }
        let actual_app_id = match manifest.app_id {
            Some(ref id) => id,
            None => return Err(CommandError::MissingAppId),
        };

        let edge_app_dir = path.parent().ok_or(CommandError::MissingField)?;

        let local_files = collect_paths_for_upload(edge_app_dir)?;
        ensure_edge_app_has_all_necessary_files(&local_files)?;

        let revision = self.get_latest_revision(actual_app_id).unwrap_or(0);

        let remote_files = self.get_version_asset_signatures(actual_app_id, revision)?;
        let changed_files = detect_changed_files(&local_files, &remote_files)?;
        debug!("Changed files: {:?}", &changed_files);

        let remote_settings = serde_json::from_value::<Vec<Setting>>(commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc",
                actual_app_id,
            ),
        )?)?;

        let changed_settings = detect_changed_settings(&manifest, &remote_settings)?;
        self.upload_changed_settings(actual_app_id.clone(), &changed_settings)?;

        let file_tree = generate_file_tree(&local_files, edge_app_dir);

        let old_file_tree = self.get_file_tree(actual_app_id, revision);

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

        self.upload_changed_files(edge_app_dir, actual_app_id, revision, &changed_files)?;
        debug!("Files uploaded");

        self.ensure_assets_processing_finished(actual_app_id, revision)?;
        // now we freeze it by publishing it
        self.publish(actual_app_id, revision)?;
        debug!("Edge app published.");

        self.get_or_create_installation(actual_app_id)?;

        Ok(revision)
    }

    pub fn promote_version(
        &self,
        app_id: &str,
        revision: u32,
        channel: &String,
    ) -> Result<(), CommandError> {
        let secrets = self.get_undefined_secrets(app_id)?;
        if !secrets.is_empty() {
            return Err(CommandError::UndefinedSecrets(serde_json::to_string(
                &secrets,
            )?));
        }

        debug!("All secrets are defined.");

        let get_response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=revision&app_id=eq.{}&revision=eq.{}",
                app_id, revision
            ),
        )?;
        let version =
            serde_json::from_value::<Vec<HashMap<String, serde_json::Value>>>(get_response)?;
        if version.is_empty() {
            return Err(CommandError::RevisionNotFound(revision.to_string()));
        }

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
        if &channels[0].channel != channel || channels[0].app_revision != revision {
            return Err(CommandError::MissingField);
        }

        Ok(())
    }

    pub fn get_app_name(&self, app_id: &str) -> Result<String, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!("v4/edge-apps/edge-apps?select=name&id=eq.{}", app_id),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct App {
            name: String,
        }

        let apps = serde_json::from_value::<Vec<App>>(response)?;
        if apps.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(apps[0].name.clone())
    }

    pub fn delete_app(&self, app_id: &str) -> Result<(), CommandError> {
        commands::delete(
            &self.authentication,
            &format!("v4/edge-apps?id=eq.{}", app_id),
        )?;

        Ok(())
    }

    pub fn clear_app_id(&self, path: &Path) -> Result<(), CommandError> {
        let data = fs::read_to_string(path)?;
        let mut manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;

        manifest.app_id = None;
        EdgeAppManifest::save_to_file(&manifest, PathBuf::from(path).as_path())?;

        Ok(())
    }

    pub fn update_name(&self, app_id: &str, name: &str) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!("v4/edge-apps?select=name&id=eq.{}", app_id),
            &json!(
            {
                "name": name,
            }),
        )?;

        Ok(())
    }

    pub fn generate_mock_data(&self, path: &Path) -> Result<(), CommandError> {
        let data = fs::read_to_string(path)?;
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;
        let edge_app_dir = path.parent().ok_or(CommandError::MissingField)?;

        let default_metadata = Metadata::default();

        let mut settings: HashMap<String, serde_yaml::Value> = HashMap::new();
        for setting in &manifest.settings {
            if setting.type_ != SettingType::Secret {
                settings.insert(
                    setting.title.clone(),
                    serde_yaml::Value::String(setting.default_value.clone()),
                );
            }
        }

        let mut mock_data: HashMap<String, serde_yaml::Value> = HashMap::new();
        mock_data.insert(
            "metadata".to_string(),
            serde_yaml::to_value(default_metadata)?,
        );
        mock_data.insert("settings".to_string(), serde_yaml::to_value(settings)?);

        let mock_data_yaml = serde_yaml::to_string(&mock_data)?;

        fs::write(edge_app_dir.join(MOCK_DATA_FILENAME), mock_data_yaml)?;

        Ok(())
    }
    fn get_undefined_secrets(&self, app_id: &str) -> Result<Vec<String>, CommandError> {
        let installation_id = self.get_or_create_installation(app_id)?;

        let undefined_secrets_response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/secrets/undefined?installation_id={}",
                installation_id
            ),
        )?;

        let titles = serde_json::from_value::<Vec<String>>(undefined_secrets_response)?;

        Ok(titles)
    }

    fn create_version(
        &self,
        manifest: &EdgeAppManifest,
        file_tree: HashMap<String, String>,
    ) -> Result<u32, CommandError> {
        let mut json = EdgeAppManifest::prepare_payload(manifest);
        json.insert("file_tree", json!(file_tree));

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

    pub fn get_latest_revision(&self, app_id: &str) -> Result<u32, CommandError> {
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
        const MAX_WAIT_TIME: u64 = 1000; // 1000 seconds - it could take a while for assets to process

        let mut pb: Option<ProgressBar> = Option::None;
        let mut assets_to_process = 0;
        let start_time = Instant::now();

        loop {
            // TODO: we are not handling possible errors in asset processing here.
            // Which are unlikely to happen, because we upload assets as they are, but still
            if start_time.elapsed().as_secs() > MAX_WAIT_TIME {
                return Err(CommandError::AssetProcessingTimeout);
            }

            let value = commands::get(
                &self.authentication,
                &format!(
                    "v4/assets?select=status,processing_error,title&app_id=eq.{}&app_revision=eq.{}&status=neq.finished",
                    app_id, revision
                ),
            )?;
            debug!("ensure_assets_processing_finished: {:?}", &value);

            if let Some(array) = value.as_array() {
                for item in array {
                    if let Some(status) = item["status"].as_str() {
                        if status == "error" {
                            return Err(CommandError::AssetProcessingError(format!(
                                "Asset {}. Error: {}",
                                item["title"], item["processing_error"]
                            )));
                        }
                    }
                }

                if array.is_empty() {
                    if let Some(progress_bar) = pb.as_ref() {
                        progress_bar.finish_with_message("Assets processed");
                    }
                    break;
                }
                match &mut pb {
                    Some(ref mut progress_bar) => {
                        progress_bar.set_position(assets_to_process - (array.len() as u64));
                        progress_bar.set_message("Processing Items:");
                    }
                    None => {
                        pb = Some(ProgressBar::new(array.len() as u64));
                        assets_to_process = array.len() as u64;
                    }
                }
            }
            thread::sleep(Duration::from_secs(SLEEP_TIME));
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
        app_id: String,
        changed_settings: &SettingChanges,
    ) -> Result<(), CommandError> {
        for setting in &changed_settings.creates {
            self.create_setting(app_id.clone(), setting)?;
        }
        for setting in &changed_settings.updates {
            self.update_setting(app_id.clone(), setting)?;
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

    fn create_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let mut payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;
        payload.insert("app_id".to_owned(), json!(app_id));

        debug!("Creating setting: {:?}", &payload);

        let response = commands::post(&self.authentication, "v4/edge-apps/settings", &payload);
        if response.is_err() {
            let c = commands::get(
                &self.authentication,
                &format!("v4/edge-apps/settings?app_id=eq.{}", app_id),
            )?;
            debug!("Existing settings: {:?}", c);
            return Err(CommandError::NoChangesToUpload("".to_owned()));
        }

        Ok(())
    }

    fn update_setting(&self, app_id: String, setting: &Setting) -> Result<(), CommandError> {
        let value = serde_json::to_value(setting)?;
        let payload = serde_json::from_value::<HashMap<String, serde_json::Value>>(value)?;

        debug!("Updating setting: {:?}", &payload);

        let response = commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/settings?app_id=eq.{id}&title=eq.{title}",
                id = app_id,
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
            .timeout(Duration::from_secs(3600)) // timeout is equal to server timeout
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

    use httpmock::Method::{DELETE, GET, PATCH, POST};
    use httpmock::MockServer;

    use crate::commands::edge_app_server::MOCK_DATA_FILENAME;
    use tempfile::tempdir;

    fn create_edge_app_manifest_for_test(settings: Vec<Setting>) -> EdgeAppManifest {
        EdgeAppManifest {
            app_id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            settings,
        }
    }

    #[test]
    fn test_edge_app_create_should_create_app_and_required_files() {
        let tmp_dir = tempdir().unwrap();

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
        assert_eq!(manifest.app_id, Some("test-id".to_owned()));
        assert_eq!(
            manifest.settings,
            vec![Setting {
                title: "greeting".to_string(),
                type_: SettingType::Secret,
                default_value: "stranger".to_string(),
                optional: true,
                help_text: "An example of a setting that is used in index.html".to_string(),
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

        let tmp_dir = tempdir().unwrap();
        File::create(tmp_dir.path().join("screenly.yml")).unwrap();

        let result = command.create(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already contains a screenly.yml or index.html file. Use --in-place if you want to create an Edge App in this directory"));

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
            .contains("already contains a screenly.yml or index.html file. Use --in-place if you want to create an Edge App in this directory"));
    }

    #[test]
    fn test_create_in_place_edge_app_should_create_edge_app_using_existing_files() {
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

        // Prepare screenly.yml and index.html
        let tmp_dir = tempdir().unwrap();
        File::create(tmp_dir.path().join("index.html")).unwrap();
        EdgeAppManifest::save_to_file(
            &EdgeAppManifest {
                ..Default::default()
            },
            tmp_dir.path().join("screenly.yml").as_path(),
        )
        .unwrap();

        let result = command.create_in_place(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        post_mock.assert();

        let data = fs::read_to_string(tmp_dir.path().join("screenly.yml")).unwrap();
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(manifest.app_id, Some("test-id".to_owned()));

        assert!(result.is_ok());
    }

    #[test]
    fn test_create_in_place_edge_app_when_manifest_or_index_html_missed_should_return_error() {
        let command = EdgeAppCommand::new(Authentication::new_with_config(
            Config::new("http://localhost".to_string()),
            "token",
        ));

        let tmp_dir = tempdir().unwrap();
        File::create(tmp_dir.path().join("screenly.yml")).unwrap();

        let result = command.create_in_place(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("should contain screenly.yml and index.html files"));

        fs::remove_file(tmp_dir.path().join("screenly.yml")).unwrap();

        File::create(tmp_dir.path().join("index.html")).unwrap();

        let result = command.create_in_place(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("should contain screenly.yml and index.html files"));
    }

    #[test]
    fn test_create_in_place_edge_app_when_manifest_has_non_empty_app_id_should_return_error() {
        let command = EdgeAppCommand::new(Authentication::new_with_config(
            Config::new("http://localhost".to_string()),
            "token",
        ));

        let tmp_dir = tempdir().unwrap();

        File::create(tmp_dir.path().join("index.html")).unwrap();

        let manifest = EdgeAppManifest {
            app_id: Some("non-empty".to_string()),
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, tmp_dir.path().join("screenly.yml").as_path())
            .unwrap();

        let result = command.create_in_place(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Initialization Failed: The operation can only proceed when 'app_id' is not set in the 'screenly.yml' configuration file"
        );
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
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param(
                    "select",
                    "edge_app_channels(channel),revision,user_version,description,published",
                )
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([
                {
                    "edge_app_channels": [
                        {
                            "channel": "stable"
                        },
                        {
                            "channel": "candidate"
                        }
                    ],
                    "revision": 1,
                    "user_version": "1.0.0",
                    "description": "Initial release",
                    "published": true
                },
                {
                    "edge_app_channels": [],
                    "revision": 2,
                    "user_version": "1.0.1",
                    "description": "Bug fixes",
                    "published": true
                }
            ]));
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
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.list_settings(&manifest.app_id.unwrap());

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
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.set_setting(&manifest.app_id.unwrap(), "best_setting", "best_value");
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
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.set_setting(&manifest.app_id.unwrap(), "best_setting", "best_value1");
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
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.set_secret(
            &manifest.app_id.unwrap(),
            "best_secret_setting",
            "best_secret_value",
        );
        installation_mock.assert();
        installation_mock_create.assert();
        secrets_values_mock_post.assert();
        debug!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_upload_should_send_correct_requests() {
        let mut manifest = create_edge_app_manifest_for_test(
            vec![
                Setting {
                    type_: SettingType::String,
                    title: "asetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
                Setting {
                    type_: SettingType::String,
                    title: "nsetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
            ]
        );

        manifest.user_version = None;
        manifest.author = None;

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
                "type": SettingType::String,
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
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "description": "asdf",
                    "icon": "asdf",
                    "homepage_url": "asdfasdf",
                    "file_tree": {
                        "index.html": "0a209f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08122086cebd0c365d241e32d5b0972c07aae3a8d6499c2a9471aa85943a35577200021a180a14a94a8fe5ccb19ba61c4c0873d391e987982fbbd31000"
                    }
                }));
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
                    "help_text": "help text",
                }));
            then.status(201).json_body(json!(
            [{
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "asetting",
                "optional": false,
                "help_text": "help text",
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
                    "help_text": "help text",
                }));
            then.status(200).json_body(json!(
            [{
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "nsetting",
                "optional": false,
                "help_text": "help text",
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
                .query_param("select", "status,processing_error,title")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.8")
                .query_param("status", "neq.finished");
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

        let temp_dir = tempdir().unwrap();
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

        let get_version_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "revision")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("revision", "eq.7");

            then.status(200).json_body(json!([
                {
                    "revision": 7,
                }
            ]));
        });

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

        //  v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc
        let undefined_secrets_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/secrets/undefined")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("installation_id", "01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200).json_body(json!([]));
        });

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
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.promote_version(&manifest.app_id.unwrap(), 7, &"public".to_string());

        get_version_mock.assert();
        installation_mock.assert();
        installation_mock_create.assert();
        undefined_secrets_mock.assert();
        promote_mock.assert();

        assert!(&result.is_ok());
    }

    #[test]
    fn test_generate_mock_data_creates_file_with_expected_content() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_manifest.yml");

        // The EdgeAppManifest structure from your example
        let manifest = create_edge_app_manifest_for_test(
            vec![
                Setting {
                    type_: SettingType::String,
                    title: "asetting".to_string(),
                    optional: false,
                    default_value: "yes".to_string(),
                    help_text: "help text".to_string(),
                },
                Setting {
                    type_: SettingType::String,
                    title: "nsetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
            ]
        );

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();
        let config = Config::new("".to_owned());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        command.generate_mock_data(&file_path).unwrap();

        let mock_data_path = dir.path().join(MOCK_DATA_FILENAME);
        assert!(mock_data_path.exists());

        let _generated_content = fs::read_to_string(&mock_data_path).unwrap();
        let _expected_content = r#"metadata:
  coordinates:
    - "37.3861"
    - "-122.0839"
  hostname: "srly-t6kb0ta1jrd9o0w"
  location: "Code Cafe, Mountain View, California"
  screen_name: "Code Cafe Display"
  tags:
    - "All Screens"
settings:
  asetting: "yes"
  nsetting: ""
"#;
    }

    #[test]
    fn test_generate_mock_data_excludes_secret_settings() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_manifest_with_varied_settings.yml");

        let manifest = create_edge_app_manifest_for_test( 
            vec![
                Setting {
                    type_: SettingType::Secret,
                    title: "excluded_setting".to_string(),
                    optional: false,
                    default_value: "0".to_string(),
                    help_text: "help text".to_string(),
                },
                Setting {
                    type_: SettingType::String,
                    title: "included_setting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
            ]
        );

        EdgeAppManifest::save_to_file(&manifest, &file_path).unwrap();
        let config = Config::new("".to_owned());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        command.generate_mock_data(&file_path).unwrap();

        let mock_data_path = dir.path().join(MOCK_DATA_FILENAME);
        let content = fs::read_to_string(mock_data_path).unwrap();

        assert!(!content.contains("excluded_setting"));
        assert!(content.contains("included_setting"));
    }

    #[test]
    fn test_ensure_assets_processing_finished_when_processing_failed_should_return_error() {
        let manifest = create_edge_app_manifest_for_test(
            vec![
                Setting {
                    type_: SettingType::String,
                    title: "asetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
                Setting {
                    type_: SettingType::String,
                    title: "nsetting".to_string(),
                    optional: false,
                    default_value: "".to_string(),
                    help_text: "help text".to_string(),
                },
            ]
        );

        let mock_server = MockServer::start();

        // "v4/assets?select=status&app_id=eq.{}&app_revision=eq.{}&status=neq.finished&limit=1",
        let finished_processing_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("select", "status,processing_error,title")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("app_revision", "eq.8")
                .query_param("status", "neq.finished");
            then.status(200).json_body(json!([
                {
                    "status": "error",
                    "title": "wrong_file.ext",
                    "processing_error": "File type not supported."
                }
            ]));
        });

        let temp_dir = tempdir().unwrap();
        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let result = command.ensure_assets_processing_finished("01H2QZ6Z8WXWNDC0KQ198XCZEW", 8);

        finished_processing_mock.assert();

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Asset processing error: Asset \"wrong_file.ext\". Error: \"File type not supported.\""
                .to_string()
        );
    }

    #[test]
    fn test_list_secrets_should_send_correct_request() {
        let mock_server = MockServer::start();

        let secrets_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "optional,title,help_text")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("type", "eq.secret")
                .query_param("order", "title.asc");

            then.status(200).json_body(json!([
                {
                    "optional": true,
                    "title": "Example secret1",
                    "help_text": "An example of a secret that is used in index.html"
                },
                {
                    "optional": true,
                    "title": "Example secret2",
                    "help_text": "An example of a secret that is used in index.html"
                },
                {
                    "optional": false,
                    "title": "Example secret3",
                    "help_text": "An example of a secret that is used in index.html"
                }
            ]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.list_secrets(&manifest.app_id.unwrap());

        secrets_mock.assert();

        assert!(result.is_ok());
        let secrets = result.unwrap();
        let secrets_json: Value = serde_json::from_value(secrets.value).unwrap();
        assert_eq!(
            secrets_json,
            json!([
                {
                    "optional": true,
                    "title": "Example secret1",
                    "help_text": "An example of a secret that is used in index.html",
                },
                {
                    "optional": true,
                    "title": "Example secret2",
                    "help_text": "An example of a secret that is used in index.html",
                },
                {
                    "optional": false,
                    "title": "Example secret3",
                    "help_text": "An example of a secret that is used in index.html"
                }
            ])
        );
    }

    #[test]
    fn test_promote_when_there_are_undefined_secrets_should_fail() {
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

        //  v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc
        let undefined_secrets_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/secrets/undefined")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("installation_id", "01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200)
                .json_body(json!(["undefined_secret", "another_undefined_secret"]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.promote_version(&manifest.app_id.unwrap(), 7, &"public".to_string());

        installation_mock.assert();
        installation_mock_create.assert();
        undefined_secrets_mock.assert();

        assert!(!&result.is_ok());
        assert!(result.unwrap_err().to_string().contains("Warning: these secrets are undefined: [\"undefined_secret\",\"another_undefined_secret\"]."));
    }

    #[test]
    fn test_promote_when_version_doesnt_exist_should_fail() {
        let mock_server = MockServer::start();

        let get_version_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("select", "revision")
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("revision", "eq.7");

            then.status(200).json_body(json!([]));
        });

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

        //  v4/edge-apps/settings?select=type,default_value,optional,title,help_text&app_id=eq.{}&order=title.asc
        let undefined_secrets_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps/secrets/undefined")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("installation_id", "01H2QZ6Z8WXWNDC0KQ198XCZEB");
            then.status(200).json_body(json!([]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.promote_version(&manifest.app_id.unwrap(), 7, &"public".to_string());

        get_version_mock.assert();
        installation_mock.assert();
        installation_mock_create.assert();
        undefined_secrets_mock.assert();

        assert!(!&result.is_ok());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Edge App Revision 7 not found"));
    }

    #[test]
    fn test_update_name_should_send_correct_request() {
        let mock_server = MockServer::start();

        let update_name_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/edge-apps")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("select", "name")
                .json_body(json!({
                    "name": "New name",
                }));

            then.status(200).json_body(json!([
                {
                    "name": "New name",
                }
            ]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = EdgeAppCommand::new(authentication);
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let result = command.update_name(&manifest.app_id.unwrap(), "New name");
        update_name_mock.assert();
        debug!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_app_should_send_correct_request() {
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4/edge-apps")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .header("Authorization", "Token token")
                .query_param("id", "eq.test-id");
            then.status(204);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let edge_app_command = EdgeAppCommand::new(authentication);
        assert!(edge_app_command.delete_app("test-id").is_ok());
    }

    #[test]
    fn test_clear_app_id_should_remove_app_id_from_manifest() {
        let mock_server = MockServer::start();
        let manifest = create_edge_app_manifest_for_test(vec![]);

        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().join("screenly.yml");
        let manifest_path = temp_path.as_path();
        EdgeAppManifest::save_to_file(&manifest, manifest_path).unwrap();

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let edge_app_command = EdgeAppCommand::new(authentication);
        assert!(edge_app_command.clear_app_id(manifest_path).is_ok());

        let data = fs::read_to_string(manifest_path).unwrap();
        let new_manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();

        let expected_manifest = EdgeAppManifest {
            app_id: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            settings: vec![],
        };

        assert_eq!(new_manifest, expected_manifest);
    }
}
