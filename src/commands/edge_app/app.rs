use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fs, io, str, thread};

use indicatif::ProgressBar;
use log::debug;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde_json::json;
use serde_yaml;

use crate::api::edge_app::setting::{Setting, SettingType};
use crate::api::version::EdgeAppVersion;
use crate::commands::edge_app::instance_manifest::InstanceManifest;
use crate::commands::edge_app::manifest::{
    EdgeAppManifest, Entrypoint, EntrypointType, MANIFEST_VERSION,
};
use crate::commands::edge_app::utils::{
    collect_paths_for_upload, detect_changed_files, detect_changed_settings,
    ensure_edge_app_has_all_necessary_files, generate_file_tree,
    transform_edge_app_path_to_manifest, transform_instance_path_to_instance_manifest, FileChanges,
    SettingChanges,
};
use crate::commands::edge_app::EdgeAppCommand;
use crate::commands::{CommandError, EdgeApps};

// Edge apps commands
impl EdgeAppCommand {
    pub fn create(&self, name: &str, path: &Path) -> Result<(), CommandError> {
        let parent_dir_path = path.parent().ok_or(CommandError::FileSystemError(
            "Cannot obtain Edge App root directory.".to_owned(),
        ))?;
        let index_html_path = parent_dir_path.join("index.html");

        if Path::new(&path).exists() || Path::new(&index_html_path).exists() {
            return Err(CommandError::FileSystemError(format!(
                "The directory {} already contains a screenly.yml or index.html file. Use --in-place if you want to create an Edge App in this directory",
                parent_dir_path.display()
            )));
        }

        let app_id = self.api.create_app(name.to_string())?;

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: Some(app_id),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings: vec![
                Setting {
                    name: "secret_word".to_string(),
                    title: Some("secret title".to_string()),
                    type_: SettingType::Secret,
                    default_value: None,
                    optional: true,
                    is_global: false,
                    help_text: "An example of a secret setting that is used in index.html"
                        .to_string(),
                },
                Setting {
                    name: "greeting".to_string(),
                    title: Some("greeting title".to_string()),
                    type_: SettingType::String,
                    default_value: Some("Unknown".to_string()),
                    optional: true,
                    is_global: false,
                    help_text: "An example of a string setting that is used in index.html"
                        .to_string(),
                },
            ],
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, path)?;

        let index_html_template =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/index.html"));
        let index_html_file = File::create(&index_html_path)?;
        write!(&index_html_file, "{index_html_template}")?;

        Ok(())
    }

    pub fn create_in_place(&self, name: &str, path: &Path) -> Result<(), CommandError> {
        let parent_dir_path = path.parent().ok_or(CommandError::FileSystemError(
            "Cannot obtain Edge App root directory.".to_owned(),
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

        if manifest.id.is_some() {
            return Err(CommandError::InitializationError("The operation can only proceed when 'id' is not set in the 'screenly.yml' configuration file".to_string()));
        }

        let app_id = self.api.create_app(name.to_string())?;

        manifest.id = Some(app_id);

        EdgeAppManifest::save_to_file(&manifest, path)?;

        Ok(())
    }

    pub fn list(&self) -> Result<EdgeApps, CommandError> {
        self.api.list_apps()
    }

    pub fn deploy(
        self,
        path: Option<String>,
        delete_missing_settings: Option<bool>,
    ) -> Result<u32, CommandError> {
        let manifest_path = transform_edge_app_path_to_manifest(&path)?;

        EdgeAppManifest::ensure_manifest_is_valid(&manifest_path)?;
        let manifest = EdgeAppManifest::new(&manifest_path)?;

        let actual_app_id = match self.get_app_id(path.clone()) {
            Ok(id) => id,
            Err(_) => return Err(CommandError::MissingAppId),
        };

        let version_metadata_changed =
            self.detect_version_metadata_changes(&actual_app_id, &manifest)?;

        let edge_app_dir = manifest_path.parent().ok_or(CommandError::MissingField)?;

        let local_files = collect_paths_for_upload(edge_app_dir)?;
        ensure_edge_app_has_all_necessary_files(&local_files)?;

        let revision = match self.api.get_latest_revision(&actual_app_id)? {
            Some(revision) => revision.revision,
            None => 0,
        };

        let remote_files = self
            .api
            .get_version_asset_signatures(&actual_app_id, revision)?;
        let changed_files = detect_changed_files(&local_files, &remote_files)?;
        debug!("Changed files: {:?}", &changed_files);

        let remote_settings = self.api.get_settings(&actual_app_id)?;

        let changed_settings = detect_changed_settings(&manifest, &remote_settings)?;
        self.upload_changed_settings(actual_app_id.clone(), &changed_settings)?;

        self.maybe_delete_missing_settings(
            delete_missing_settings,
            actual_app_id.clone(),
            changed_settings,
        )?;

        self.update_entrypoint_value(path)?;

        let file_tree = generate_file_tree(&local_files, edge_app_dir);

        let old_file_tree = self.api.get_file_tree(&actual_app_id, revision);

        let file_tree_changed = match old_file_tree {
            Ok(tree) => file_tree != tree,
            Err(_) => true,
        };

        debug!("File tree changed: {file_tree_changed}");
        if !self.requires_upload(&changed_files) && !file_tree_changed && !version_metadata_changed
        {
            return Err(CommandError::NoChangesToUpload(
                "No changes detected".to_owned(),
            ));
        }

        // now that we know we have changes, we can create a new version
        let revision =
            self.create_version(&manifest, generate_file_tree(&local_files, edge_app_dir))?;

        self.upload_changed_files(edge_app_dir, &actual_app_id, revision, &changed_files)?;
        debug!("Files uploaded");

        self.ensure_assets_processing_finished(&actual_app_id, revision)?;
        // now we freeze it by publishing it
        self.api.publish_version(&actual_app_id, revision)?;
        debug!("Edge App published.");

        self.promote_version(&actual_app_id, revision, "stable")?;

        Ok(revision)
    }

    fn promote_version(
        &self,
        app_id: &str,
        revision: u32,
        channel: &str,
    ) -> Result<(), CommandError> {
        let version_exists = self.api.version_exists(app_id, revision)?;
        if !version_exists {
            return Err(CommandError::RevisionNotFound(revision.to_string()));
        }

        self.api.update_channel(channel, app_id, revision)?;

        Ok(())
    }

    pub fn delete_app(&self, app_id: &str) -> Result<(), CommandError> {
        self.api.delete_app(app_id)?;

        Ok(())
    }

    pub fn update_name(&self, app_id: &str, name: &str) -> Result<(), CommandError> {
        self.api.update_app(app_id, name)?;

        Ok(())
    }

    fn maybe_delete_missing_settings(
        &self,
        delete_missing_settings: Option<bool>,
        actual_app_id: String,
        changed_settings: SettingChanges,
    ) -> Result<(), CommandError> {
        match delete_missing_settings {
            Some(delete) => {
                if delete {
                    self.delete_deleted_settings(
                        actual_app_id.clone(),
                        &changed_settings.deleted,
                        false,
                    )?;
                }
            }
            None => {
                if let Ok(_ci) = std::env::var("CI") {
                    return Ok(());
                }
                self.delete_deleted_settings(
                    actual_app_id.clone(),
                    &changed_settings.deleted,
                    true,
                )?;
            }
        }

        Ok(())
    }

    pub fn update_entrypoint_value(&self, path: Option<String>) -> Result<(), CommandError> {
        let manifest = EdgeAppManifest::new(&transform_edge_app_path_to_manifest(&path)?)?;
        let setting_key = "screenly_entrypoint";

        if let Some(entrypoint) = &manifest.entrypoint {
            match entrypoint.entrypoint_type {
                EntrypointType::RemoteGlobal => {
                    let setting_value = match entrypoint.uri {
                        Some(ref uri) => uri.clone(),
                        None => "".to_owned(),
                    };
                    self.set_setting(path, setting_key, &setting_value)?;
                }
                EntrypointType::RemoteLocal => {
                    let instance_manifest = InstanceManifest::new(
                        &transform_instance_path_to_instance_manifest(&path)?,
                    )?;
                    let setting_value: String = match instance_manifest.entrypoint_uri {
                        Some(ref uri) => uri.clone(),
                        None => "".to_owned(),
                    };
                    self.set_setting(path, setting_key, &setting_value)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn ensure_assets_processing_finished(
        &self,
        app_id: &str,
        revision: u32,
    ) -> Result<(), CommandError> {
        const SLEEP_TIME: u64 = 2;
        const MAX_WAIT_TIME: u64 = 1000; // 1000 seconds - it could take a while for assets to process

        let mut pb: Option<ProgressBar> = None;
        let mut assets_to_process = 0;
        let start_time = Instant::now();

        loop {
            // TODO: we are not handling possible errors in asset processing here.
            // Which are unlikely to happen, because we upload assets as they are, but still
            if start_time.elapsed().as_secs() > MAX_WAIT_TIME {
                return Err(CommandError::AssetProcessingTimeout);
            }

            let asset_processing_statuses = self.api.get_processing_statuses(app_id, revision)?;
            if asset_processing_statuses.is_empty() {
                if let Some(progress_bar) = pb.as_ref() {
                    progress_bar.finish_with_message("Assets processed");
                }
                break;
            }
            debug!(
                "ensure_assets_processing_finished: {:?}",
                &asset_processing_statuses
            );

            for asset_processing_status in &asset_processing_statuses {
                if asset_processing_status.status == "error" {
                    return Err(CommandError::AssetProcessingError(format!(
                        "Asset {}. Error: {}",
                        asset_processing_status.title, asset_processing_status.processing_error
                    )));
                }
            }

            let unprocessed_asset_count = asset_processing_statuses.len() as u64;

            match &mut pb {
                Some(ref mut progress_bar) => {
                    progress_bar.set_position(assets_to_process - unprocessed_asset_count);
                    progress_bar.set_message("Processing Items:");
                }
                None => {
                    pb = Some(ProgressBar::new(unprocessed_asset_count));
                    assets_to_process = unprocessed_asset_count;
                }
            }

            thread::sleep(Duration::from_secs(SLEEP_TIME));
        }
        Ok(())
    }

    // TODO: remove
    fn requires_upload(&self, changed_files: &FileChanges) -> bool {
        changed_files.has_changes()
    }
}

impl EdgeAppCommand {
    pub fn get_app_name(&self, app_id: &str) -> Result<String, CommandError> {
        let app = self.api.get_app(app_id)?;

        Ok(app.name.clone())
    }

    pub fn clear_app_id(&self, path: &Path) -> Result<(), CommandError> {
        let data = fs::read_to_string(path)?;
        let mut manifest: EdgeAppManifest = serde_yaml::from_str(&data)?;

        manifest.id = None;
        EdgeAppManifest::save_to_file(&manifest, PathBuf::from(path).as_path())?;

        Ok(())
    }

    fn create_version(
        &self,
        manifest: &EdgeAppManifest,
        file_tree: HashMap<String, String>,
    ) -> Result<u32, CommandError> {
        let mut json = EdgeAppManifest::prepare_payload(manifest);
        json.insert("file_tree", json!(file_tree));

        self.api.create_version(json)
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

    fn delete_deleted_settings(
        &self,
        app_id: String,
        deleted: &Vec<Setting>,
        prompt_user: bool,
    ) -> Result<(), CommandError> {
        for setting in deleted {
            self.try_delete_setting(app_id.clone(), setting, prompt_user)?;
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
        debug!("Changed files: {changed_files:#?}");

        let copied_signatures = self.copy_edge_app_assets(
            app_id,
            revision,
            changed_files
                .get_local_signatures()
                .iter()
                .cloned()
                .collect(),
        )?;

        debug!("Uploading Edge App assets");
        let files_to_upload = changed_files.get_files_to_upload(copied_signatures);
        if files_to_upload.is_empty() {
            debug!("No files to upload");
            return Ok(());
        }

        debug!("Uploading Edge App files: {files_to_upload:#?}");
        let file_paths: Vec<PathBuf> = files_to_upload
            .iter()
            .map(|file| edge_app_dir.join(&file.path))
            .collect();

        self.upload_edge_app_assets(app_id, revision, &file_paths)?;

        Ok(())
    }

    fn try_delete_setting(
        &self,
        app_id: String,
        setting: &Setting,
        prompt_user: bool,
    ) -> Result<(), CommandError> {
        debug!("Deleting setting: {:?}", &setting.name);

        let mut input_name = String::new();

        if !prompt_user {
            return self.delete_setting(app_id, setting);
        }

        let prompt = format!("It seems like the setting \"{}\" is absent in the YAML file, but it exists on the server. If you wish to skip deletion, you can leave the input blank. Warning, deleting the setting will drop all the associated values. To proceed with deletion, please confirm the setting name by writing it down: ", setting.name);
        println!("{prompt}");
        io::stdin()
            .read_line(&mut input_name)
            .expect("Failed to read input");

        if input_name.trim() == "" {
            return Ok(());
        }

        if input_name.trim() != setting.name {
            // Should we ask for confirmation again if user input is wrong?
            return Err(CommandError::WrongSettingName(setting.name.to_string()));
        }

        self.delete_setting(app_id, setting)
    }

    fn copy_edge_app_assets(
        &self,
        app_id: &str,
        revision: u32,
        mut asset_signatures: Vec<String>,
    ) -> Result<Vec<String>, CommandError> {
        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

        asset_signatures.sort();
        let payload = json!({
            "app_id": app_id,
            "revision": revision,
            "signatures": asset_signatures,
        });

        let copied_assets = self.api.copy_assets(payload)?;
        Ok(copied_assets)
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
        let url = format!("{}/v4/assets", &self.api.authentication.config.url);

        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

        debug!("Uploading file: {path:?}");
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
            .api
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

    pub fn detect_version_metadata_changes(
        &self,
        app_id: &str,
        manifest: &EdgeAppManifest,
    ) -> Result<bool, CommandError> {
        let version = self.api.get_latest_revision(app_id)?;
        // TODO: implement entrypoint changes on the backend
        match version {
            Some(_version) => Ok(_version
                != EdgeAppVersion {
                    ready_signal: manifest.ready_signal.unwrap_or(false),
                    user_version: manifest.user_version.clone(),
                    description: manifest.description.clone(),
                    icon: manifest.icon.clone(),
                    author: manifest.author.clone(),
                    homepage_url: manifest.homepage_url.clone(),
                    revision: _version.revision,
                }),
            None => Ok(false),
        }
    }

    pub fn get_installation_id(&self, path: Option<String>) -> Result<String, CommandError> {
        let instance_manifest =
            InstanceManifest::new(&transform_instance_path_to_instance_manifest(&path)?)?;
        match instance_manifest.id {
            Some(id) if !id.is_empty() => Ok(id),
            _ => Err(CommandError::MissingInstallationId),
        }
    }

    pub fn get_app_id(&self, path: Option<String>) -> Result<String, CommandError> {
        let edge_app_manifest = EdgeAppManifest::new(&transform_edge_app_path_to_manifest(&path)?)?;
        match edge_app_manifest.id {
            Some(id) if !id.is_empty() => Ok(id),
            _ => Err(CommandError::MissingAppId),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use httpmock::Method::{DELETE, GET, PATCH, POST};
    use tempfile::tempdir;

    use super::*;
    use crate::commands::edge_app::manifest::MANIFEST_VERSION;
    use crate::commands::edge_app::test_utils::tests::{
        create_edge_app_manifest_for_test, create_instance_manifest_for_test,
        prepare_edge_apps_test,
    };
    use crate::commands::edge_app::utils::EdgeAppFile;

    #[test]
    fn test_edge_app_create_should_create_app_and_required_files() {
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

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

        let result = command.create(
            "Best app ever",
            tmp_dir.path().join("screenly.yml").as_path(),
        );

        post_mock.assert();

        assert!(tmp_dir.path().join("screenly.yml").exists());
        assert!(tmp_dir.path().join("index.html").exists());

        let data = fs::read_to_string(tmp_dir.path().join("screenly.yml")).unwrap();
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(manifest.id, Some("test-id".to_owned()));
        assert_eq!(
            manifest.settings,
            vec![
                Setting {
                    name: "greeting".to_string(),
                    title: Some("greeting title".to_string()),
                    type_: SettingType::String,
                    default_value: Some("Unknown".to_string()),
                    optional: true,
                    is_global: false,
                    help_text: "An example of a string setting that is used in index.html"
                        .to_string(),
                },
                Setting {
                    name: "secret_word".to_string(),
                    title: Some("secret title".to_string()),
                    type_: SettingType::Secret,
                    default_value: None,
                    optional: true,
                    is_global: false,
                    help_text: "An example of a secret setting that is used in index.html"
                        .to_string(),
                }
            ]
        );
        assert_eq!(
            manifest.entrypoint,
            Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None
            })
        );

        let data_index_html = fs::read_to_string(tmp_dir.path().join("index.html")).unwrap();
        assert_eq!(
            data_index_html,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/index.html"))
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_app_create_when_manifest_or_index_html_exist_should_return_error() {
        let (tmp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

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
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

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

        // Prepare index.html
        File::create(tmp_dir.path().join("index.html")).unwrap();
        EdgeAppManifest::save_to_file(
            &EdgeAppManifest {
                syntax: MANIFEST_VERSION.to_owned(),
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
        assert_eq!(manifest.id, Some("test-id".to_owned()));

        assert!(result.is_ok());
    }

    #[test]
    fn test_create_in_place_edge_app_when_manifest_or_index_html_missed_should_return_error() {
        let (tmp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

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
        let (tmp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        File::create(tmp_dir.path().join("index.html")).unwrap();

        let manifest = EdgeAppManifest {
            id: Some("non-empty".to_string()),
            syntax: MANIFEST_VERSION.to_owned(),
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
                "Initialization Failed: The operation can only proceed when 'id' is not set in the 'screenly.yml' configuration file"
            );
    }

    #[test]
    fn test_list_edge_apps_should_send_correct_request() {
        let (_tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let edge_apps_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/edge-apps")
                .query_param("select", "id,name")
                .query_param("deleted", "eq.false")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([]));
        });

        let result = command.list();
        edge_apps_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_deploy_should_send_correct_requests() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let mut manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        manifest.user_version = None;
        manifest.author = None;
        manifest.entrypoint = None;

        // let get_entrypoint_mock = mock_server.mock(|when, then| {
        //     when.method(GET)
        //         .path("/v4.1/edge-apps/installations")
        //         .header("Authorization", "Token token")
        //         .header(
        //             "user-agent",
        //             format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
        //         )
        //         .query_param("id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEB")
        //         .query_param("select", "entrypoint");
        //     then.status(200).json_body(json!([{"entrypoint": null}]));
        // });
        // "v4.1/edge-apps/versions?select=user_version,description,icon,author,entrypoint&app_id=eq.{}&order=revision.desc&limit=1",
        let last_versions_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param(
                    "select",
                    "user_version,description,icon,author,homepage_url,revision,ready_signal",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("order", "revision.desc")
                .query_param("limit", "1");
            then.status(200).json_body(json!([
                {
                    "user_version": "1",
                    "description": "desc",
                    "icon": "icon",
                    "author": "author",
                    "homepage_url": "homepage_url",
                    "ready_signal": false,
                    "revision": 7,
                }
            ]));
        });

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
                    .path("/v4.1/edge-apps/settings")
                    .header("Authorization", "Token token")
                    .header(
                        "user-agent",
                        format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                    )
                    .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                    .query_param("select", "name,type,default_value,optional,title,help_text")
                    .query_param("order", "name.asc");
                then.status(200).json_body(json!([{
                    "name": "nsetting".to_string(),
                    "type": SettingType::String,
                    "default_value": "5".to_string(),
                    "title": "ntitle".to_string(),
                    "optional": true,
                    "help_text": "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
                    "is_global": false,
                }, {
                    "name": "isetting".to_string(),
                    "type": SettingType::String,
                    "default_value": "5".to_string(),
                    "title": null,
                    "optional": true,
                    "help_text": "Some text".to_string(),
                    "is_global": false,
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
                        },
                        "ready_signal": false,
                    }));
                then.status(201).json_body(json!([{"revision": 8}]));
            });

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
                    "name": "asetting",
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "type": "string",
                    "default_value": "",
                    "title": "atitle",
                    "optional": false,
                    "help_text": "help text",
                }));
            then.status(201).json_body(json!(
            [{
                "name": "asetting",
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "atitle",
                "optional": false,
                "help_text": "help text",
            }]));
        });

        let settings_mock_patch = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.nsetting")
                .json_body(json!({
                    "name": "nsetting",
                    "type": "string",
                    "default_value": "",
                    "title": "ntitle",
                    "optional": false,
                    "help_text": "help text",
                }));
            then.status(200).json_body(json!(
            [{
                "name": "nsetting",
                "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                "type": "string",
                "default_value": "",
                "title": "ntitle",
                "optional": false,
                "help_text": "help text",
            }]));
        });

        let settings_mock_delete = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4.1/edge-apps/settings")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("name", "eq.isetting");
            then.status(204).json_body(json!({}));
        });

        let copy_assets_mock = mock_server.mock(|when, then| {
                when.method(POST)
                    .path("/v4/edge-apps/copy-assets")
                    .header("Authorization", "Token token")
                    .header(
                        "user-agent",
                        format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                    ).json_body(json!({
                        "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                        "revision": 8,
                        "signatures": ["0a209f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08122086cebd0c365d241e32d5b0972c07aae3a8d6499c2a9471aa85943a35577200021a180a14a94a8fe5ccb19ba61c4c0873d391e987982fbbd31000"]
                    }));
                then.status(201).json_body(json!([]));
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
                .query_param("revision", "eq.8");

            then.status(200).json_body(json!([
                {
                    "revision": 8,
                }
            ]));
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
                .query_param("channel", "eq.stable")
                .query_param("select", "channel,app_revision")
                .json_body(json!({
                    "app_revision": 8,
                }));
            then.status(200).json_body(json!([
                {
                    "channel": "stable",
                    "app_revision": 8
                }
            ]));
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        let result = command.deploy(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            Some(true),
        );

        // get_entrypoint_mock.assert();
        last_versions_mock.assert_calls(2);
        assets_mock.assert();
        file_tree_from_version_mock.assert();
        settings_mock.assert();
        create_version_mock.assert();
        settings_mock_create.assert();
        settings_mock_patch.assert();
        settings_mock_delete.assert();
        upload_assets_mock.assert();
        finished_processing_mock.assert();
        publish_mock.assert();
        copy_assets_mock.assert();
        get_version_mock.assert();
        promote_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_version_metadata_changes_when_no_changes_should_return_false() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        let last_versions_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param(
                    "select",
                    "user_version,description,icon,author,homepage_url,revision,ready_signal",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("order", "revision.desc")
                .query_param("limit", "1");
            then.status(200).json_body(json!([
                {
                    "user_version": "1",
                    "description": "asdf",
                    "icon": "asdf",
                    "author": "asdf",
                    "homepage_url": "asdfasdf",
                    "ready_signal": false,
                    "revision": 1
                }
            ]));
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();

        let manifest =
            EdgeAppManifest::new(temp_dir.path().join("screenly.yml").as_path()).unwrap();
        let result =
            command.detect_version_metadata_changes(&manifest.id.clone().unwrap(), &manifest);

        assert!(result.is_ok());
        assert!(!result.unwrap());
        last_versions_mock.assert();
    }

    #[test]
    fn test_detect_version_metadata_changes_when_has_changes_should_return_true() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        let last_versions_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param(
                    "select",
                    "user_version,description,icon,author,homepage_url,revision,ready_signal",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("order", "revision.desc")
                .query_param("limit", "1");
            then.status(200).json_body(json!([
                {
                    "user_version": "new_version",
                    "description": "description",
                    "icon": "another_icon",
                    "author": "asdf",
                    "homepage_url": "asdfasdf",
                    "ready_signal": false,
                    "revision": 1,
                }
            ]));
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();

        let manifest =
            EdgeAppManifest::new(temp_dir.path().join("screenly.yml").as_path()).unwrap();
        let result =
            command.detect_version_metadata_changes(&manifest.id.clone().unwrap(), &manifest);

        assert!(result.is_ok());
        assert!(result.unwrap());
        last_versions_mock.assert();
    }

    #[test]
    fn test_detect_version_metadata_changes_when_no_version_exist_should_return_false() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        let last_versions_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4.1/edge-apps/versions")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .query_param(
                    "select",
                    "user_version,description,icon,author,homepage_url,revision,ready_signal",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .query_param("order", "revision.desc")
                .query_param("limit", "1");
            then.status(200).json_body(json!([]));
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();

        let manifest =
            EdgeAppManifest::new(temp_dir.path().join("screenly.yml").as_path()).unwrap();
        let result =
            command.detect_version_metadata_changes(&manifest.id.clone().unwrap(), &manifest);

        assert!(result.is_ok());
        assert!(!result.unwrap());
        last_versions_mock.assert();
    }

    #[test]
    fn test_ensure_assets_processing_finished_when_processing_failed_should_return_error() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

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

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        let result = command.ensure_assets_processing_finished("01H2QZ6Z8WXWNDC0KQ198XCZEW", 8);

        finished_processing_mock.assert();

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Asset processing error: Asset wrong_file.ext. Error: File type not supported."
                .to_string()
        );
    }

    #[test]
    fn test_update_name_should_send_correct_request() {
        let (_temp_dir, command, mock_server, manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

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

        let result = command.update_name(&manifest.unwrap().id.unwrap(), "New name");
        update_name_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_app_should_send_correct_request() {
        let (_temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

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

        assert!(command.delete_app("test-id").is_ok());
    }

    #[test]
    fn test_clear_app_id_should_remove_app_id_from_manifest() {
        let (temp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

        let manifest_path = temp_dir.path().join("screenly.yml");
        assert!(command.clear_app_id(&manifest_path).is_ok());

        let data = fs::read_to_string(manifest_path).unwrap();
        let new_manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();

        let expected_manifest = EdgeAppManifest {
            id: None,
            syntax: MANIFEST_VERSION.to_owned(),
            auth: None,
            ready_signal: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            settings: vec![],
        };

        assert_eq!(new_manifest, expected_manifest);
    }

    #[test]
    fn test_deploy_without_app_id_should_fail() {
        let (temp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let mut manifest = create_edge_app_manifest_for_test(vec![
            Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
            Setting {
                name: "nsetting".to_string(),
                type_: SettingType::String,
                title: Some("ntitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            },
        ]);

        manifest.id = None;
        manifest.entrypoint = None;

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        let result = command.deploy(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            Some(true),
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "App id is required in manifest."
        );
    }

    #[test]
    fn test_changed_files_when_not_all_files_are_copied_should_upload_missed_ones() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            auth: None,
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: None,
            settings: vec![
                Setting {
                    name: "asetting".to_string(),
                    type_: SettingType::String,
                    title: Some("atitle".to_string()),
                    optional: false,
                    default_value: Some("".to_string()),
                    is_global: false,
                    help_text: "asdf".to_string(),
                },
                Setting {
                    name: "nsetting".to_string(),
                    type_: SettingType::String,
                    title: Some("ntitle".to_string()),
                    optional: false,
                    default_value: Some("".to_string()),
                    is_global: false,
                    help_text: "asdf".to_string(),
                },
            ],
        };

        let copy_assets_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/copy-assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "revision": 7,
                    "signatures": ["somesig", "somesig1", "somesig2"]
                }));
            then.status(201).json_body(json!(["somesig"]));
        });

        let upload_assets_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/assets")
                .body_includes("test222");
            then.status(201).body("");
        });
        let upload_assets_mock2 = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/assets")
                .body_includes("test333");
            then.status(201).body("");
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        let screenly_path = temp_dir.path().join("screenly.yml");
        let path = screenly_path.as_path();
        let edge_app_dir = path.parent().ok_or(CommandError::MissingField).unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test111").unwrap();
        let mut file1 = File::create(temp_dir.path().join("index1.html")).unwrap();
        write!(file1, "test222").unwrap();
        let mut file2 = File::create(temp_dir.path().join("index2.html")).unwrap();
        write!(file2, "test333").unwrap();

        let changed_files = FileChanges::new(
            &[
                EdgeAppFile {
                    path: "index.html".to_owned(),
                    signature: "somesig".to_owned(),
                },
                EdgeAppFile {
                    path: "index1.html".to_owned(),
                    signature: "somesig1".to_owned(),
                },
                EdgeAppFile {
                    path: "index2.html".to_owned(),
                    signature: "somesig2".to_owned(),
                },
            ],
            true,
        );

        let result = command.upload_changed_files(
            edge_app_dir,
            "01H2QZ6Z8WXWNDC0KQ198XCZEW",
            7,
            &changed_files,
        );

        // Twice for somesig1 and somesig2
        upload_assets_mock.assert();
        upload_assets_mock2.assert();
        copy_assets_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_changed_files_when_all_files_are_copied_should_not_upload() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            auth: None,
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: None,
            settings: vec![
                Setting {
                    name: "asetting".to_string(),
                    type_: SettingType::String,
                    title: Some("atitle".to_string()),
                    optional: false,
                    default_value: Some("".to_string()),
                    is_global: false,
                    help_text: "sdfg".to_string(),
                },
                Setting {
                    name: "nsetting".to_string(),
                    type_: SettingType::String,
                    title: Some("ntitle".to_string()),
                    optional: false,
                    default_value: Some("".to_string()),
                    is_global: false,
                    help_text: "asdf".to_string(),
                },
            ],
        };

        let copy_assets_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps/copy-assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({
                    "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    "revision": 7,
                    "signatures": ["somesig", "somesig1", "somesig2"]
                }));
            then.status(201)
                .json_body(json!(["somesig", "somesig1", "somesig2"]));
        });

        let upload_assets_mock = mock_server.mock(|when, then| {
            when.method(POST).path("/v4/assets");
            then.status(201).body("");
        });

        EdgeAppManifest::save_to_file(&manifest, temp_dir.path().join("screenly.yml").as_path())
            .unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test").unwrap();

        let screenly_path = temp_dir.path().join("screenly.yml");
        let path = screenly_path.as_path();
        let edge_app_dir = path.parent().ok_or(CommandError::MissingField).unwrap();
        let mut file = File::create(temp_dir.path().join("index.html")).unwrap();
        write!(file, "test111").unwrap();
        let mut file1 = File::create(temp_dir.path().join("index1.html")).unwrap();
        write!(file1, "test222").unwrap();
        let mut file2 = File::create(temp_dir.path().join("index2.html")).unwrap();
        write!(file2, "test333").unwrap();

        let changed_files = FileChanges::new(
            &[
                EdgeAppFile {
                    path: "index.html".to_owned(),
                    signature: "somesig".to_owned(),
                },
                EdgeAppFile {
                    path: "index1.html".to_owned(),
                    signature: "somesig1".to_owned(),
                },
                EdgeAppFile {
                    path: "index2.html".to_owned(),
                    signature: "somesig2".to_owned(),
                },
            ],
            true,
        );

        let result = command.upload_changed_files(
            edge_app_dir,
            "01H2QZ6Z8WXWNDC0KQ198XCZEW",
            7,
            &changed_files,
        );

        upload_assets_mock.assert_calls(0);
        copy_assets_mock.assert();

        assert!(result.is_ok());
    }

    #[test]
    fn test_maybe_delete_missing_settings_when_ci_is_1_and_no_arg_provided_should_ignore_deleting_settings(
    ) {
        let (_temp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, false);

        let changed_settings: SettingChanges = SettingChanges {
            creates: vec![],
            updates: vec![],
            deleted: vec![Setting {
                name: "asetting".to_string(),
                type_: SettingType::String,
                title: Some("atitle".to_string()),
                optional: false,
                default_value: Some("".to_string()),
                is_global: false,
                help_text: "help text".to_string(),
            }],
        };

        temp_env::with_var("CI", Some("true"), || {
            let result = command.maybe_delete_missing_settings(
                None,
                "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
                changed_settings,
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_get_installation_id_when_manifest_has_id_should_return_id() {
        let (temp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(true, true);

        let result =
            command.get_installation_id(Some(temp_dir.path().to_str().unwrap().to_string()));

        println!("{result:?}");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "01H2QZ6Z8WXWNDC0KQ198XCZEB");
    }

    #[test]
    fn test_update_entrypoint_value_when_entrypoint_is_global_and_it_is_not_set_should_post_value()
    {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, true);

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
                .query_param("name", "eq.screenly_entrypoint");

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
                .query_param("name", "eq.screenly_entrypoint")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.app_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "screenly_entrypoint",
                    "type": "string",
                    "edge_app_setting_values": [],
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
                        "value": "https://global-entrypoint.com",
                        "name": "screenly_entrypoint",
                        "app_id": "01H2QZ6Z8WXWNDC0KQ198XCZEW",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let mut edge_app_manifest = create_edge_app_manifest_for_test(vec![]);
        edge_app_manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteGlobal,
            uri: Some("https://global-entrypoint.com".to_string()),
        });

        EdgeAppManifest::save_to_file(
            &edge_app_manifest,
            temp_dir.path().join("screenly.yml").as_path(),
        )
        .unwrap();

        let result =
            command.update_entrypoint_value(Some(temp_dir.path().to_str().unwrap().to_string()));

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_entrypoint_value_when_entrypoint_is_global_and_setting_is_set_should_patch_it() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, true);

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
                .query_param("name", "eq.screenly_entrypoint");

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
                .query_param("name", "eq.screenly_entrypoint")
                .query_param("select", "name,type,edge_app_setting_values(value)")
                .query_param(
                    "edge_app_setting_values.app_id",
                    "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW",
                )
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW");
            then.status(200).json_body(json!([
                {
                    "name": "screenly_entrypoint",
                    "type": "string",
                    "edge_app_setting_values": [
                        {
                            "value": "https://global-entrypoint.com",
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
                .query_param("app_id", "eq.01H2QZ6Z8WXWNDC0KQ198XCZEW")
                .json_body(json!(
                    {
                        "value": "https://new-global-entrypoint.com",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let mut edge_app_manifest = create_edge_app_manifest_for_test(vec![]);
        edge_app_manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteGlobal,
            uri: Some("https://new-global-entrypoint.com".to_string()),
        });

        EdgeAppManifest::save_to_file(
            &edge_app_manifest,
            temp_dir.path().join("screenly.yml").as_path(),
        )
        .unwrap();

        let result =
            command.update_entrypoint_value(Some(temp_dir.path().to_str().unwrap().to_string()));

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_patch.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_entrypoint_value_when_entrypoint_is_local_and_it_is_not_set_should_post_value() {
        let (_temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

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
                .query_param("name", "eq.screenly_entrypoint");

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
                    "edge_app_setting_values": [],
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
                        "value": "https://local-entrypoint.com",
                        "name": "screenly_entrypoint",
                        "installation_id": "01H2QZ6Z8WXWNDC0KQ198XCZEB",
                    }
                ));
            then.status(200).json_body(json!({}));
        });

        let mut edge_app_manifest = create_edge_app_manifest_for_test(vec![]);
        edge_app_manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteLocal,
            uri: None,
        });

        let mut instance_manifest = create_instance_manifest_for_test();
        instance_manifest.entrypoint_uri = Some("https://local-entrypoint.com".to_string());

        let temp_dir = tempdir().unwrap();
        EdgeAppManifest::save_to_file(
            &edge_app_manifest,
            temp_dir.path().join("screenly.yml").as_path(),
        )
        .unwrap();
        InstanceManifest::save_to_file(
            &instance_manifest,
            temp_dir.path().join("instance.yml").as_path(),
        )
        .unwrap();

        let result =
            command.update_entrypoint_value(Some(temp_dir.path().to_str().unwrap().to_string()));

        setting_is_global_get_mock.assert();
        setting_mock_get.assert();
        setting_values_mock_post.assert();
        assert!(result.is_ok());
    }
}
