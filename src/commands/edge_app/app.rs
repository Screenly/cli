use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, thread};

use indicatif::ProgressBar;
use log::debug;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde_yaml;

use crate::api::edge_app::deploy::{describe_failed_files, DeployPayload, FailedFile};
use crate::api::edge_app::setting::{Setting, SettingType};
use crate::commands::edge_app::instance_manifest::InstanceManifest;
use crate::commands::edge_app::manifest::{
    EdgeAppManifest, Entrypoint, EntrypointType, MANIFEST_VERSION,
};
use crate::commands::edge_app::utils::{
    collect_paths_for_upload, generate_file_tree, transform_edge_app_path_to_manifest,
    transform_instance_path_to_instance_manifest,
};
use crate::commands::edge_app::EdgeAppCommand;
use crate::commands::{CommandError, EdgeApps};

pub const INJECT_JS_FILE_NAME: &str = "screenly_inject.js";

#[derive(Debug)]
pub struct DeployOutcome {
    pub revision: Option<u32>,
    pub created: bool,
}

// Edge apps commands
impl EdgeAppCommand {
    pub fn create(
        &self,
        name: &str,
        path: &Path,
        entrypoint: Option<String>,
    ) -> Result<(), CommandError> {
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

        let entrypoint_value = match entrypoint {
            Some(url) => {
                match reqwest::Url::parse(&url) {
                    Ok(parsed) if parsed.scheme() == "http" || parsed.scheme() == "https" => {}
                    _ => {
                        return Err(CommandError::InitializationError(format!(
                            "Invalid --entrypoint URL: {url}. Must be a valid http or https URL."
                        )));
                    }
                }
                Some(Entrypoint {
                    entrypoint_type: EntrypointType::RemoteGlobal,
                    uri: Some(url),
                })
            }
            None => Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
        };

        let app_id = self.api.create_app(name.to_string())?;

        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: Some(app_id),
            entrypoint: entrypoint_value,
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

        let is_remote = matches!(
            manifest.entrypoint,
            Some(Entrypoint {
                entrypoint_type: EntrypointType::RemoteGlobal,
                ..
            })
        );

        if !is_remote {
            let index_html_template =
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/index.html"));
            let index_html_file = File::create(&index_html_path)?;
            write!(&index_html_file, "{index_html_template}")?;
        }

        if is_remote {
            let inject_js_path = parent_dir_path.join(INJECT_JS_FILE_NAME);
            if !inject_js_path.exists() {
                let template = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/data/screenly_inject.js"
                ));
                fs::write(&inject_js_path, template)?;
            }
        }

        Ok(())
    }
}

impl EdgeAppCommand {
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
    ) -> Result<DeployOutcome, CommandError> {
        let manifest_path = transform_edge_app_path_to_manifest(&path)?;

        EdgeAppManifest::ensure_manifest_is_valid(&manifest_path)?;
        let manifest = EdgeAppManifest::new(&manifest_path)?;

        let actual_app_id = self
            .get_app_id(path)
            .map_err(|_| CommandError::MissingAppId)?;

        let edge_app_dir = manifest_path.parent().ok_or(CommandError::MissingField)?;
        let local_files = collect_paths_for_upload(edge_app_dir)?;

        let delete_missing_settings = delete_missing_settings == Some(true);
        let payload = DeployPayload {
            manifest: serde_json::to_value(&manifest)?,
            file_tree: generate_file_tree(&local_files, edge_app_dir),
            delete_missing_settings,
        };

        let preview = self.api.deploy_preview(&actual_app_id, &payload)?;
        debug!("Deploy preview: {preview:?}");

        let settings_to_delete = &preview.diff.settings.delete;
        if !delete_missing_settings && !settings_to_delete.is_empty() {
            println!(
                "Settings not in manifest: {}. Re-run with --delete-missing-settings to remove.",
                settings_to_delete.join(", ")
            );
        }

        if !preview.deploy_needed {
            debug!("Nothing to deploy.");
            return Ok(DeployOutcome {
                revision: None,
                created: false,
            });
        }

        let files_to_upload: Vec<PathBuf> = preview
            .outstanding
            .missing
            .iter()
            .map(|file| edge_app_dir.join(file))
            .collect();
        self.upload_edge_app_assets(&actual_app_id, &files_to_upload)?;

        self.wait_for_assets_processing(&actual_app_id)?;

        let result = self.api.deploy(&actual_app_id, &payload)?;
        debug!(
            "Deployed revision {} (created: {}, published: {}, channel: {})",
            result.revision, result.created, result.published, result.channel
        );

        Ok(DeployOutcome {
            revision: Some(result.revision),
            created: result.created,
        })
    }

    pub fn delete_app(&self, app_id: &str) -> Result<(), CommandError> {
        self.api.delete_app(app_id)?;

        Ok(())
    }

    pub fn update_name(&self, app_id: &str, name: &str) -> Result<(), CommandError> {
        self.api.update_app(app_id, name)?;

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

    fn wait_for_assets_processing(&self, app_id: &str) -> Result<(), CommandError> {
        const POLL_INTERVAL_SECONDS: u64 = 2;
        const MAX_WAIT_SECONDS: u64 = 1000;

        let mut progress_bar: Option<ProgressBar> = None;
        let mut assets_to_process: u64 = 0;
        let start_time = Instant::now();

        loop {
            let statuses = self.api.get_staged_processing_statuses(app_id)?;
            debug!("Staged assets still processing: {statuses:?}");

            let failed: Vec<FailedFile> = statuses
                .iter()
                .filter(|status| status.status == "error")
                .map(|status| FailedFile {
                    path: status.title.clone(),
                    error: status.processing_error.clone(),
                })
                .collect();
            if !failed.is_empty() {
                return Err(CommandError::AssetProcessingError(describe_failed_files(
                    &failed,
                )));
            }

            let pending_count = statuses.len() as u64;
            if pending_count == 0 {
                progress_bar
                    .as_ref()
                    .inspect(|bar| bar.finish_with_message("Assets processed"));
                return Ok(());
            }

            if start_time.elapsed().as_secs() > MAX_WAIT_SECONDS {
                return Err(CommandError::AssetProcessingTimeout);
            }

            if progress_bar.is_none() {
                assets_to_process = pending_count;
            }
            let bar = progress_bar.get_or_insert_with(|| ProgressBar::new(pending_count));
            bar.set_position(assets_to_process.saturating_sub(pending_count));
            bar.set_message("Processing Items:");

            thread::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS));
        }
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

    fn upload_edge_app_assets(&self, app_id: &str, paths: &[PathBuf]) -> Result<(), CommandError> {
        if paths.is_empty() {
            debug!("No files to upload");
            return Ok(());
        }

        debug!("Uploading Edge App files: {paths:#?}");
        let pb = ProgressBar::new(paths.len() as u64);
        pb.set_message("Files uploaded:");

        paths.par_iter().try_for_each(|path| {
            let result = self.upload_single_asset(app_id, path);
            if result.is_ok() {
                pb.inc(1);
            }
            result
        })
    }

    fn upload_single_asset(&self, app_id: &str, path: &Path) -> Result<(), CommandError> {
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
    use httpmock::Method::{DELETE, GET, PATCH, POST};
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::commands::edge_app::manifest::MANIFEST_VERSION;
    use crate::commands::edge_app::test_utils::tests::{
        create_edge_app_manifest_for_test, create_instance_manifest_for_test,
        prepare_edge_apps_test,
    };

    const APP_ID: &str = "01H2QZ6Z8WXWNDC0KQ198XCZEW";
    const INDEX_HTML_SIGNATURE: &str = "0a209f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08122086cebd0c365d241e32d5b0972c07aae3a8d6499c2a9471aa85943a35577200021a180a14a94a8fe5ccb19ba61c4c0873d391e987982fbbd31000";

    fn deploy_test_settings() -> Vec<Setting> {
        vec![
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
        ]
    }

    fn write_deployable_edge_app(dir: &Path) -> EdgeAppManifest {
        let mut manifest = create_edge_app_manifest_for_test(deploy_test_settings());
        manifest.user_version = None;
        manifest.author = None;
        manifest.entrypoint = None;

        let manifest_path = dir.join("screenly.yml");
        EdgeAppManifest::save_to_file(&manifest, manifest_path.as_path()).unwrap();
        let mut file = File::create(dir.join("index.html")).unwrap();
        write!(file, "test").unwrap();

        EdgeAppManifest::new(manifest_path.as_path()).unwrap()
    }

    fn no_outstanding_preview() -> serde_json::Value {
        json!({
            "deploy_needed": true,
            "outstanding": {"missing": [], "pending": [], "failed": []},
            "diff": {
                "settings": {"create": [], "update": [], "delete": []},
                "revision": {"update": []},
                "files": {"create": [], "update": [], "delete": []}
            }
        })
    }

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
            None,
        );

        post_mock.assert();

        assert!(tmp_dir.path().join("screenly.yml").exists());
        assert!(tmp_dir.path().join("index.html").exists());
        assert!(!tmp_dir.path().join(".ignore").exists());

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
            None,
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
            None,
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
    fn test_edge_app_create_with_remote_entrypoint_should_set_remote_global_and_write_inject_template(
    ) {
        let (tmp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/edge-apps")
                .header("Authorization", "Token token")
                .json_body(json!({ "name": "Remote app" }));
            then.status(201)
                .json_body(json!([{"id": "test-id", "name": "Remote app"}]));
        });

        let result = command.create(
            "Remote app",
            tmp_dir.path().join("screenly.yml").as_path(),
            Some("https://example.com/app".to_string()),
        );

        post_mock.assert();
        assert!(result.is_ok());

        let data = fs::read_to_string(tmp_dir.path().join("screenly.yml")).unwrap();
        let manifest: EdgeAppManifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(
            manifest.entrypoint,
            Some(Entrypoint {
                entrypoint_type: EntrypointType::RemoteGlobal,
                uri: Some("https://example.com/app".to_string()),
            })
        );

        assert!(!tmp_dir.path().join("index.html").exists());
        assert!(!tmp_dir.path().join(".ignore").exists());

        let inject_path = tmp_dir.path().join(INJECT_JS_FILE_NAME);
        assert!(inject_path.exists());
        let inject_content = fs::read_to_string(&inject_path).unwrap();
        assert!(inject_content.contains("screenly_settings"));
    }

    #[test]
    fn test_edge_app_create_with_invalid_entrypoint_url_should_fail() {
        let (tmp_dir, command, _mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        let result = command.create(
            "Bad app",
            tmp_dir.path().join("screenly.yml").as_path(),
            Some("not-a-url".to_string()),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid --entrypoint URL"));
        assert!(!tmp_dir.path().join("screenly.yml").exists());
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

        let manifest = write_deployable_edge_app(temp_dir.path());
        let expected_payload = json!({
            "manifest": serde_json::to_value(&manifest).unwrap(),
            "file_tree": { "index.html": INDEX_HTML_SIGNATURE },
            "delete_missing_settings": true,
        });

        let preview_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy/preview"))
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(expected_payload.clone());
            then.status(200).json_body(json!({
                "deploy_needed": true,
                "outstanding": {"missing": ["index.html"], "pending": [], "failed": []},
                "diff": {
                    "settings": {"create": ["asetting"], "update": ["nsetting"], "delete": ["isetting"]},
                    "revision": {"update": []},
                    "files": {"create": ["index.html"], "update": [], "delete": []}
                }
            }));
        });

        let upload_assets_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .body_includes("name=\"app_id\"")
                .body_includes(APP_ID)
                .body_includes("test");
            then.status(201).body("");
        });

        let staged_status_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("app_id", format!("eq.{APP_ID}"))
                .query_param("app_revision", "is.null");
            then.status(200).json_body(json!([]));
        });

        let deploy_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy"))
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(expected_payload.clone());
            then.status(200).json_body(json!({
                "revision": 8,
                "created": true,
                "published": true,
                "channel": "stable"
            }));
        });

        let result = command.deploy(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            Some(true),
        );

        preview_mock.assert();
        staged_status_mock.assert();
        upload_assets_mock.assert();
        deploy_mock.assert();

        assert_eq!(result.unwrap().revision, Some(8));
    }

    #[test]
    fn test_deploy_when_server_reports_outstanding_files_should_return_error() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        write_deployable_edge_app(temp_dir.path());

        let preview_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy/preview"))
                .body_includes("\"delete_missing_settings\":false");
            then.status(200).json_body(no_outstanding_preview());
        });

        let staged_status_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("app_revision", "is.null");
            then.status(200).json_body(json!([]));
        });

        let deploy_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy"));
            then.status(409).json_body(json!({
                "outstanding": {"missing": ["logo.png"], "pending": ["app.js"], "failed": []}
            }));
        });

        let result = command.deploy(Some(temp_dir.path().to_str().unwrap().to_string()), None);

        preview_mock.assert();
        staged_status_mock.assert();
        deploy_mock.assert();

        assert_eq!(
            result.unwrap_err().to_string(),
            "Deploy rejected: not uploaded: logo.png; still processing: app.js"
        );
    }

    #[test]
    fn test_deploy_when_asset_processing_failed_should_return_error() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        write_deployable_edge_app(temp_dir.path());

        let preview_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy/preview"));
            then.status(200).json_body(no_outstanding_preview());
        });

        let staged_status_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("app_revision", "is.null");
            then.status(200).json_body(json!([{
                "status": "error",
                "processing_error": "File type not supported.",
                "title": "wrong_file.ext"
            }]));
        });

        let deploy_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy"));
            then.status(200).json_body(json!({"revision": 8}));
        });

        let result = command.deploy(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            Some(true),
        );

        preview_mock.assert();
        staged_status_mock.assert();
        deploy_mock.assert_calls(0);

        assert_eq!(
            result.unwrap_err().to_string(),
            "Asset processing error: wrong_file.ext: File type not supported."
        );
    }

    #[test]
    fn test_deploy_when_app_does_not_exist_should_return_error() {
        let (temp_dir, command, mock_server, _manifest, _instance_manifest) =
            prepare_edge_apps_test(false, false);

        write_deployable_edge_app(temp_dir.path());

        let preview_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v3/edge-apps/{APP_ID}/deploy/preview"));
            then.status(404)
                .json_body(json!({"detail": "App not found"}));
        });

        let result = command.deploy(
            Some(temp_dir.path().to_str().unwrap().to_string()),
            Some(true),
        );

        preview_mock.assert();

        assert_eq!(
            result.unwrap_err().to_string(),
            format!("App not found: Edge App with ID '{APP_ID}' not found.")
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
            categories: vec!["Utilities".to_string(), "Dashboards".to_string()],
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

        let mut manifest = create_edge_app_manifest_for_test(deploy_test_settings());

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
