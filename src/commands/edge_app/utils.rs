use crate::commands::edge_app::app::AssetSignature;
use crate::commands::edge_app::instance_manifest::InstanceManifest;
use crate::commands::edge_app::manifest::EdgeAppManifest;
use crate::commands::edge_app::setting::{Setting, SettingType};
use crate::commands::CommandError;
use crate::signature::{generate_signature, sig_to_hex};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

use crate::commands::ignorer::Ignorer;

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

const INSTANCE_FILE_NAME_ENV: &str = "INSTANCE_FILE_NAME";
const MANIFEST_FILE_NAME_ENV: &str = "MANIFEST_FILE_NAME";

#[derive(Debug, Clone)]
pub struct EdgeAppFile {
    pub(crate) path: String,
    pub signature: String,
}

#[derive(Debug)]
pub struct SettingChanges {
    pub creates: Vec<Setting>,
    pub updates: Vec<Setting>,
    pub deleted: Vec<Setting>,
}

#[derive(Debug)]
pub struct FileChanges {
    pub local_files: Vec<EdgeAppFile>,
    changes_detected: bool,
}

impl FileChanges {
    pub fn new(local_files: &[EdgeAppFile], changes_detected: bool) -> Self {
        Self {
            local_files: local_files.to_vec(),
            changes_detected,
        }
    }

    pub fn has_changes(&self) -> bool {
        // not considering copies - copies are all assets from previous version anyhow
        self.changes_detected
    }

    pub fn get_local_signatures(&self) -> HashSet<String> {
        self.local_files
            .iter()
            .map(|f| f.signature.clone())
            .collect::<HashSet<String>>()
    }

    pub fn get_files_to_upload(&self, exclude_signatures: Vec<String>) -> Vec<&EdgeAppFile> {
        self.local_files
            .iter()
            .filter(|f| !exclude_signatures.contains(&f.signature))
            .collect::<Vec<&EdgeAppFile>>()
    }
}

fn is_included(entry: &DirEntry, ignore: &Ignorer) -> bool {
    let exclusion_list = ["screenly.js", "screenly.yml", ".ignore", "instance.yml"];
    if exclusion_list.contains(&entry.file_name().to_str().unwrap_or_default()) {
        return false;
    }

    return !ignore.is_ignored(entry.path());
}

pub fn transform_edge_app_path_to_manifest(path: &Option<String>) -> Result<PathBuf, CommandError> {
    let manifest_path = env::var(MANIFEST_FILE_NAME_ENV);

    let filename = match manifest_path {
        Ok(path) => {
            let path_obj = Path::new(&path);
            if path_obj.components().count() != 1 {
                return Err(CommandError::ManifestFilenameError(path));
            }
            path
        }
        Err(_) => "screenly.yml".to_string(),
    };

    let mut result = match path {
        Some(path) => {
            let path_buf_obj = PathBuf::from(path);
            if !path_buf_obj.is_dir() {
                return Err(CommandError::PathIsNotDirError(path.clone()));
            }
            path_buf_obj
        }
        None => env::current_dir().unwrap(),
    };

    result.push(filename);
    Ok(result)
}

pub fn transform_instance_path_to_instance_manifest(
    path: &Option<String>,
) -> Result<PathBuf, CommandError> {
    let instance_path = env::var(INSTANCE_FILE_NAME_ENV);

    let filename = match instance_path {
        Ok(path) => {
            let path_obj = Path::new(&path);
            if path_obj.components().count() != 1 {
                return Err(CommandError::InstanceFilenameError(path));
            }
            path
        }
        Err(_) => "instance.yml".to_string(),
    };

    let mut result = match path {
        Some(path) => {
            let path_buf_obj = PathBuf::from(path);
            if !path_buf_obj.is_dir() {
                return Err(CommandError::PathIsNotDirError(path.clone()));
            }
            path_buf_obj
        }
        None => env::current_dir().unwrap(),
    };

    result.push(filename);
    Ok(result)
}

pub fn collect_paths_for_upload(path: &Path) -> Result<Vec<EdgeAppFile>, CommandError> {
    let mut files = Vec::new();

    let ignore = Ignorer::new(path).map_err(|e| {
        CommandError::IgnoreError(format!("Failed to initialize ignore module: {}", e))
    })?;

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| is_included(e, &ignore))
        .filter_map(|v| v.ok())
    {
        if entry.file_type().is_file() {
            let relative_path = entry.path().strip_prefix(path)?;
            let path = relative_path.to_str().unwrap_or_default();
            let signature = generate_signature(entry.path())?;
            files.push(EdgeAppFile {
                path: path.to_owned(),
                signature: sig_to_hex(&signature),
            });
        }
    }
    Ok(files)
}

pub fn ensure_edge_app_has_all_necessary_files(files: &[EdgeAppFile]) -> Result<(), CommandError> {
    let required_files = vec!["index.html"];
    for file in required_files {
        if !files.iter().any(|f| f.path == file) {
            return Err(CommandError::MissingRequiredFile(file.to_owned()));
        }
    }
    Ok(())
}

pub fn detect_changed_files(
    local_files: &[EdgeAppFile],
    remote_files: &[AssetSignature],
) -> Result<FileChanges, CommandError> {
    let mut signatures: HashSet<String> = HashSet::new();

    // Store remote file signatures in the hashmap
    for remote_file in remote_files {
        signatures.insert(remote_file.signature.clone());
    }

    let mut file_changes = FileChanges::new(local_files, false);

    let local_signatures = file_changes.get_local_signatures();
    file_changes.changes_detected = local_signatures != signatures;

    Ok(file_changes)
}

pub fn detect_changed_settings(
    manifest: &EdgeAppManifest,
    remote_settings: &[Setting],
) -> Result<SettingChanges, CommandError> {
    // Remote and local settings MUST be sorted.
    // This function compares remote and local settings
    // And returns if there are any new local settings missing from the remote
    // And changed settings to update

    let mut new_settings = manifest.settings.clone();

    if let Some(auth) = &manifest.auth {
        let auth_settings = auth.auth_type.generate_settings(auth.global);
        new_settings.extend(auth_settings);
    }

    if let Some(_entrypoint) = &manifest.entrypoint {
        match _entrypoint.entrypoint_type {
            crate::commands::edge_app::manifest::EntrypointType::RemoteGlobal => {
                new_settings.push(Setting::new(
                    SettingType::String,
                    "Entrypoint",
                    "screenly_entrypoint",
                    "The global entrypoint for the app.",
                    true,
                ));
            }
            crate::commands::edge_app::manifest::EntrypointType::RemoteLocal => {
                new_settings.push(Setting::new(
                    SettingType::String,
                    "Entrypoint",
                    "screenly_entrypoint",
                    "The entrypoint for the app.",
                    false,
                ));
            }
            crate::commands::edge_app::manifest::EntrypointType::File => {}
        }
    }

    new_settings.sort_by_key(|s| s.name.clone());

    let mut creates = Vec::new();
    let mut updates = Vec::new();
    let mut deleted: Vec<Setting> = Vec::new();

    let mut remote_iter = remote_settings.iter().peekable();
    let mut new_iter = new_settings.iter().peekable();

    while let (Some(&remote_setting), Some(&new_setting)) = (remote_iter.peek(), new_iter.peek()) {
        match remote_setting.name.cmp(&new_setting.name) {
            std::cmp::Ordering::Equal => {
                if remote_setting != new_setting {
                    updates.push(new_setting.clone());
                }
                remote_iter.next();
                new_iter.next();
            }
            std::cmp::Ordering::Less => {
                deleted.push(remote_setting.clone());
                remote_iter.next();
            }
            std::cmp::Ordering::Greater => {
                creates.push(new_setting.clone());
                new_iter.next();
            }
        }
    }

    creates.extend(new_iter.cloned());
    deleted.extend(remote_iter.cloned());

    Ok(SettingChanges {
        creates,
        updates,
        deleted,
    })
}

pub fn generate_file_tree(files: &[EdgeAppFile], root_path: &Path) -> HashMap<String, String> {
    let mut tree = HashMap::new();
    let prefix = root_path.as_os_str().to_string_lossy().to_string();
    for file in files {
        let relative_path = file.path.strip_prefix(&prefix).unwrap_or(&file.path);
        tree.insert(relative_path.to_owned(), file.signature.clone());
    }

    debug!("File tree: {:?}", &tree);

    tree
}

pub fn validate_manifests_dependacies(
    manifest: &EdgeAppManifest,
    instance_manifest: &InstanceManifest,
) -> Result<(), CommandError> {
    if let Some(entrypoint) = &manifest.entrypoint {
        match entrypoint.entrypoint_type {
            crate::commands::edge_app::manifest::EntrypointType::RemoteLocal => {
                if instance_manifest.entrypoint_uri.is_none() {
                    return Err(CommandError::InvalidManifest(
                        "entrypoint_uri must be set for remote local entrypoint".to_owned(),
                    ));
                }
            }
            _ => {
                if instance_manifest.entrypoint_uri.is_some() {
                    return Err(CommandError::InvalidManifest(
                        "entrypoint_uri must not be set when entrypoint is not remote local"
                            .to_owned(),
                    ));
                }
            }
        }
    } else if instance_manifest.entrypoint_uri.is_some() {
        return Err(CommandError::InvalidManifest(
            "entrypoint_uri must not be set when entrypoint is not set".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::edge_app::instance_manifest::INSTANCE_MANIFEST_VERSION;
    use crate::commands::edge_app::manifest::{Auth, Entrypoint, EntrypointType, MANIFEST_VERSION};
    use crate::commands::edge_app::manifest_auth::AuthType;
    use crate::commands::edge_app::setting::SettingType;
    use std::fs::File;
    use std::io::Write;
    use temp_env;
    use tempfile::tempdir;

    fn create_manifest() -> EdgeAppManifest {
        EdgeAppManifest {
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: Some("entrypoint.html".to_string()),
            }),
            settings: vec![
                Setting {
                    name: "display_time".to_string(),
                    type_: SettingType::String,
                    default_value: Some("5".to_string()),
                    title: Some("display time title".to_string()),
                    optional: true,
                    is_global: false,
                    help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
                },
                Setting {
                    name: "google_maps_api_key".to_string(),
                    type_: SettingType::String,
                    default_value: Some("6".to_string()),
                    title: Some("Google maps title".to_string()),
                    optional: true,
                    is_global: false,
                    help_text: "Specify a commercial Google Maps API key. Required due to the app's map feature.".to_string(),
                },
            ],
        }
    }

    #[test]
    fn test_detect_changed_settings_when_no_changes_should_detect_no_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                name: "display_time".to_string(),
                type_: SettingType::String,
                default_value: Some("5".to_string()),
                title: Some("display time title".to_string()),
                optional: true,
                is_global: false,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                name: "google_maps_api_key".to_string(),
                type_: SettingType::String,
                default_value: Some("6".to_string()),
                title: Some("Google maps title".to_string()),
                optional: true,
                is_global: false,
                help_text: "Specify a commercial Google Maps API key. Required due to the app's map feature.".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 0);
    }

    #[test]
    fn test_detect_changes_settings_when_setting_removed_should_detect_deleted_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                name: "display_time".to_string(),
                type_: SettingType::String,
                default_value: Some("5".to_string()),
                title: Some("display time title".to_string()),
                optional: true,
                is_global: false,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                name: "google_maps_api_key".to_string(),
                type_: SettingType::String,
                default_value: Some("6".to_string()),
                title: Some("Google maps title".to_string()),
                optional: true,
                is_global: false,
                help_text: "Specify a commercial Google Maps API key. Required due to the app's map feature.".to_string(),
            },
            Setting {
                name: "new_setting".to_string(),
                type_: SettingType::String,
                default_value: Some("10".to_string()),
                title: Some("new setting title".to_string()),
                optional: false,
                is_global: false,
                help_text: "New setting description".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.deleted.len(), 1);
        assert_eq!(changes.deleted[0].name, "new_setting");
    }

    #[test]
    fn test_detect_changes_settings_when_local_setting_added_should_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                name: "display_time".to_string(),
                type_: SettingType::String,
                default_value: Some("5".to_string()),
                title: Some("display time title".to_string()),
                optional: true,
                is_global: false,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert_eq!(changes.creates[0].name, "google_maps_api_key");
    }

    // TODO: Update test, when patching is implemented
    #[test]
    fn test_detect_changed_settings_when_setting_are_modified_should_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                name: "display_time".to_string(),
                type_: SettingType::String,
                default_value: Some("5".to_string()),
                title: Some("display time title".to_string()),
                optional: true,
                is_global: false,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                name: "google_maps_api_key".to_string(),
                type_: SettingType::String,
                default_value: Some("7".to_string()), // Modified default value
                title: Some("Google maps title".to_string()),
                optional: true,
                is_global: false,
                help_text: "Specify a commercial Google Maps API key. Required due to the app's map feature.".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 0);
        assert_eq!(changes.updates.len(), 1);
        assert_eq!(changes.updates[0].name, "google_maps_api_key");
        assert_eq!(changes.updates[0].default_value, Some("6".to_owned()));
    }

    #[test]
    fn test_detect_changed_settings_when_no_remote_settings_should_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = Vec::new();

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 2);
    }

    #[test]
    fn test_detect_changed_settings_when_is_global_changed_on_setting_should_detect_changes() {
        // Arrange
        let manifest = EdgeAppManifest {
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: Some("entrypoint.html".to_string()),
            }),
            settings: vec![
                Setting {
                    name: "display_time".to_string(),
                    type_: SettingType::String,
                    default_value: Some("5".to_string()),
                    title: Some("display time title".to_string()),
                    optional: true,
                    is_global: true,
                    help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
                },
            ],
        };

        let remote_settings = vec![
            Setting {
                name: "display_time".to_string(),
                type_: SettingType::String,
                default_value: Some("5".to_string()),
                title: Some("display time title".to_string()),
                optional: true,
                is_global: false,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 0);
        assert_eq!(changes.updates.len(), 1);
    }

    #[test]
    fn test_detect_changed_files_no_changes() {
        // Arrange
        let local_files = vec![
            EdgeAppFile {
                path: "file1".to_string(),
                signature: "signature1".to_string(),
            },
            EdgeAppFile {
                path: "file2".to_string(),
                signature: "signature2".to_string(),
            },
        ];

        let remote_files = vec![
            AssetSignature {
                signature: "signature1".to_string(),
            },
            AssetSignature {
                signature: "signature2".to_string(),
            },
        ];

        // Act
        let result = detect_changed_files(&local_files, &remote_files);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.local_files.len(), 2);
        assert!(!changes.changes_detected);
    }

    #[test]
    fn test_detect_changed_files_changes_detected() {
        // Arrange
        let local_files = vec![
            EdgeAppFile {
                path: "file1".to_string(),
                signature: "signature1".to_string(),
            },
            EdgeAppFile {
                path: "file2".to_string(),
                signature: "signature2".to_string(),
            },
        ];

        let remote_files = vec![
            AssetSignature {
                signature: "signature3".to_string(),
            },
            AssetSignature {
                signature: "signature2".to_string(),
            },
        ];

        // Act
        let result = detect_changed_files(&local_files, &remote_files);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.local_files.len(), 2);
        assert!(changes.changes_detected);
    }

    #[test]
    fn test_detect_changed_files_remote_files_empty() {
        // Arrange
        let local_files = vec![
            EdgeAppFile {
                path: "file1".to_string(),
                signature: "signature1".to_string(),
            },
            EdgeAppFile {
                path: "file2".to_string(),
                signature: "signature2".to_string(),
            },
        ];

        let remote_files = Vec::new();

        // Act
        let result = detect_changed_files(&local_files, &remote_files);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.local_files.len(), 2);
        assert!(changes.changes_detected);
    }

    #[test]
    fn test_detect_changed_when_files_local_deleted_should_detect_changes() {
        // Arrange
        let local_files = vec![EdgeAppFile {
            path: "file1".to_string(),
            signature: "signature1".to_string(),
        }];

        let remote_files = vec![
            AssetSignature {
                signature: "signature1".to_string(),
            },
            AssetSignature {
                signature: "signature2".to_string(),
            },
        ];

        // Act
        let result = detect_changed_files(&local_files, &remote_files);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.local_files.len(), 1);
        assert!(changes.changes_detected);
    }

    #[test]
    fn test_ignore_functionality() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        File::create(dir_path.join("file1.txt"))
            .unwrap()
            .write_all(b"Hello, world!")
            .unwrap();
        File::create(dir_path.join("file2.txt"))
            .unwrap()
            .write_all(b"Hello, again!")
            .unwrap();
        File::create(dir_path.join(".ignore"))
            .unwrap()
            .write_all(b"file2.txt")
            .unwrap();
        File::create(dir_path.join("instance.yml"))
            .unwrap()
            .write_all(b"id: 01H2QZ6Z8WXWNDC0KQ198XCZEB\nname: test\n")
            .unwrap();

        let result = collect_paths_for_upload(dir_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "file1.txt");
    }

    #[test]
    fn test_detect_changed_settings_when_basic_auth_added_should_detect_changes() {
        // Arrange
        let mut manifest = create_manifest();
        manifest.auth = Some(Auth {
            auth_type: AuthType::Basic,
            global: false,
        });

        let remote_settings = manifest.settings.clone();

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 2);
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_username" && !s.is_global));
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_password" && !s.is_global));
    }

    #[test]
    fn test_detect_changed_settings_when_bearer_auth_added_should_detect_changes() {
        // Arrange
        let mut manifest = create_manifest();
        manifest.auth = Some(Auth {
            auth_type: AuthType::Bearer,
            global: false,
        });

        let remote_settings = manifest.settings.clone();

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert_eq!(changes.creates[0].name, "screenly_bearer_token");
        assert!(!changes.creates[0].is_global);
    }

    #[test]
    fn test_detect_changed_settings_when_switching_from_basic_to_bearer_auth() {
        // Arrange
        let mut manifest = create_manifest();
        manifest.auth = Some(Auth {
            auth_type: AuthType::Bearer,
            global: false,
        });

        let mut remote_settings = manifest.settings.clone();
        remote_settings.extend(vec![
            Setting::new(
                SettingType::String,
                "Username",
                "screenly_basic_auth_username",
                "Basic auth username",
                false,
            ),
            Setting::new(
                SettingType::Secret,
                "Password",
                "screenly_basic_auth_password",
                "Basic auth password",
                false,
            ),
        ]);

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert_eq!(changes.creates[0].name, "screenly_bearer_token");
        assert!(!changes.creates[0].is_global);
        assert_eq!(changes.deleted.len(), 2);
        assert!(changes
            .deleted
            .iter()
            .any(|s| s.name == "screenly_basic_auth_username"));
        assert!(changes
            .deleted
            .iter()
            .any(|s| s.name == "screenly_basic_auth_password"));
    }

    #[test]
    fn test_detect_changed_settings_when_switching_from_bearer_to_basic_auth() {
        // Arrange
        let mut manifest = create_manifest();
        manifest.auth = Some(Auth {
            auth_type: AuthType::Basic,
            global: false,
        });

        let mut remote_settings = manifest.settings.clone();
        remote_settings.push(Setting::new(
            SettingType::String,
            "Token",
            "screenly_bearer_token",
            "Bearer token",
            false,
        ));

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 2);
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_username" && !s.is_global));
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_password" && !s.is_global));
        assert_eq!(changes.deleted.len(), 1);
        assert_eq!(changes.deleted[0].name, "screenly_bearer_token");
    }

    #[test]
    fn test_detect_changed_settings_when_auth_is_global() {
        // Arrange
        let mut manifest = create_manifest();
        manifest.auth = Some(Auth {
            auth_type: AuthType::Basic,
            global: true,
        });

        let remote_settings = manifest.settings.clone();

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 2);
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_username" && s.is_global));
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_basic_auth_password" && s.is_global));
    }

    #[test]
    fn test_detect_changed_settings_when_entrypoint_is_remote_should_create_global_setting() {
        let mut manifest = create_manifest();
        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteGlobal,
            uri: Some("https://global_entrypoint.html".to_string()),
        });

        let remote_settings = manifest.settings.clone();

        let result = detect_changed_settings(&manifest, &remote_settings);

        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_entrypoint"
                && s.is_global
                && s.type_ == SettingType::String));
    }

    #[test]
    fn test_detect_changed_settings_when_entrypoint_setting_exist_in_remote_should_not_create_global_setting(
    ) {
        let mut manifest = create_manifest();
        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteGlobal,
            uri: Some("https://global_entrypoint.html".to_string()),
        });

        manifest.settings.push(Setting::new(
            SettingType::String,
            "SortedTest",
            "t_sorted_after_entrypoint",
            "Sorted after entrypoint setting.",
            true,
        ));

        let mut remote_settings = manifest.settings.clone();
        remote_settings.push(Setting::new(
            SettingType::String,
            "screenly_entrypoint",
            "screenly_entrypoint",
            "The global entrypoint for the app.",
            true,
        ));
        remote_settings.sort_by_key(|s| s.name.clone());

        let result = detect_changed_settings(&manifest, &remote_settings);

        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 0);
    }

    #[test]
    fn test_detect_changed_settings_when_entrypoint_is_local_should_create_local_setting() {
        let mut manifest = create_manifest();
        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteLocal,
            uri: Some("https://local_entrypoint.html".to_string()),
        });

        let remote_settings = manifest.settings.clone();

        let result = detect_changed_settings(&manifest, &remote_settings);

        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert!(changes
            .creates
            .iter()
            .any(|s| s.name == "screenly_entrypoint"
                && !s.is_global
                && s.type_ == SettingType::String));
    }

    #[test]
    fn test_detect_changed_settings_when_entrypoint_is_file_should_not_create_setting() {
        let mut manifest = create_manifest();
        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::File,
            uri: Some("entrypoint.html".to_string()),
        });

        let remote_settings = manifest.settings.clone();

        let result = detect_changed_settings(&manifest, &remote_settings);

        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 0);
    }

    #[test]
    fn test_transform_edge_app_instance_path_to_instance_manifest_should_return_current_dir_with_()
    {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let result = transform_instance_path_to_instance_manifest(&None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path.join("instance.yml"));
    }

    #[test]
    fn test_transform_edge_app_instance_path_to_instance_manifest_when_path_provided_should_return_path_with_instance_manifest(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let dir2 = tempdir().unwrap();
        let dir_path2 = dir2.path();

        let result = transform_instance_path_to_instance_manifest(&Some(
            dir_path2.to_str().unwrap().to_string(),
        ));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path2.join("instance.yml"));
    }

    #[test]
    fn test_transform_edge_app_instance_path_to_instance_manifest_when_path_provided_is_not_a_dir_should_fail(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let result =
            transform_instance_path_to_instance_manifest(&Some("instance2.yml".to_string()));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Path is not a directory: instance2.yml"
        );
    }

    #[test]
    fn test_transform_edge_app_instance_path_to_instance_manifest_with_env_instance_override_should_return_overrided_manifest_path(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());
        temp_env::with_var(INSTANCE_FILE_NAME_ENV, Some("instance2.yml"), || {
            let result = transform_instance_path_to_instance_manifest(&None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), dir_path.join("instance2.yml"));
        });
    }

    #[test]
    fn test_transform_edge_app_instance_path_to_instance_manifest_with_env_path_instead_of_file_should_fail(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        temp_env::with_var(INSTANCE_FILE_NAME_ENV, Some("folder/instance2.yml"), || {
            let result = transform_instance_path_to_instance_manifest(&None);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().to_string(), "Env var INSTANCE_FILE_NAME must hold only file name, not a path. folder/instance2.yml");
        });
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_should_return_current_dir_with_() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let result = transform_edge_app_path_to_manifest(&None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path.join("screenly.yml"));
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_when_path_provided_should_return_path_with_instance_manifest(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let dir2 = tempdir().unwrap();
        let dir_path2 = dir2.path();

        let result =
            transform_edge_app_path_to_manifest(&Some(dir_path2.to_str().unwrap().to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path2.join("screenly.yml"));
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_when_path_provided_is_not_a_dir_should_fail() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let result = transform_edge_app_path_to_manifest(&Some("screenly2.yml".to_string()));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Path is not a directory: screenly2.yml"
        );
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_with_env_instance_override_should_return_overrided_manifest_path(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());
        temp_env::with_var(MANIFEST_FILE_NAME_ENV, Some("screenly2.yml"), || {
            let result = transform_edge_app_path_to_manifest(&None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), dir_path.join("screenly2.yml"));
        });
    }

    #[test]
    fn test_transform_edge_app_path_to_manifest_with_env_path_instead_of_file_should_fail() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        temp_env::with_var(MANIFEST_FILE_NAME_ENV, Some("folder/screenly2.yml"), || {
            let result = transform_edge_app_path_to_manifest(&None);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().to_string(), "Env var MANIFEST_FILE_NAME must hold only file name, not a path. folder/screenly2.yml");
        });
    }

    #[test]
    fn test_validate_manifests_dependacies_when_entrypoint_type_is_not_remote_local_and_entrypoint_uri_is_set_should_fail(
    ) {
        let mut manifest = EdgeAppManifest {
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
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

        let instance_manifest = InstanceManifest {
            entrypoint_uri: Some("entrypoint.html".to_string()),
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: Some("01B2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            name: "instance".to_string(),
        };

        let result = validate_manifests_dependacies(&manifest, &instance_manifest);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Manifest file validation failed with error: entrypoint_uri must not be set when entrypoint is not remote local");

        manifest.entrypoint = Some(Entrypoint {
            entrypoint_type: EntrypointType::RemoteGlobal,
            uri: None,
        });

        let result = validate_manifests_dependacies(&manifest, &instance_manifest);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Manifest file validation failed with error: entrypoint_uri must not be set when entrypoint is not remote local");
    }

    #[test]
    fn test_validate_manifests_dependacies_when_entrypoint_type_is_remote_local_and_entrypoint_uri_is_not_set_should_fail(
    ) {
        let manifest = EdgeAppManifest {
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::RemoteLocal,
                uri: None,
            }),
            settings: vec![],
        };

        let instance_manifest = InstanceManifest {
            entrypoint_uri: None,
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: Some("01B2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            name: "instance".to_string(),
        };

        let result = validate_manifests_dependacies(&manifest, &instance_manifest);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Manifest file validation failed with error: entrypoint_uri must be set for remote local entrypoint");
    }

    #[test]
    fn test_validate_manifests_dependacies_when_entrypoint_type_is_remote_local_and_entrypoint_uri_is_set_should_succeed(
    ) {
        let manifest = EdgeAppManifest {
            id: Some("01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            auth: None,
            syntax: MANIFEST_VERSION.to_owned(),
            ready_signal: None,
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::RemoteLocal,
                uri: None,
            }),
            settings: vec![],
        };

        let instance_manifest = InstanceManifest {
            entrypoint_uri: Some("https://remote-local.com".to_string()),
            syntax: INSTANCE_MANIFEST_VERSION.to_owned(),
            id: Some("01B2QZ6Z8WXWNDC0KQ198XCZEW".to_string()),
            name: "instance".to_string(),
        };

        let result = validate_manifests_dependacies(&manifest, &instance_manifest);
        assert!(result.is_ok());
    }
}
