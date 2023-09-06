use crate::commands::edge_app::AssetSignature;
use crate::commands::{CommandError, EdgeAppManifest, Setting};
use crate::signature::{generate_signature, sig_to_hex};
use log::debug;
use std::collections::{HashMap, HashSet};

use crate::commands::ignorer::Ignorer;

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone)]
pub struct EdgeAppFile {
    pub(crate) path: String,
    signature: String,
}

#[derive(Debug)]
pub struct SettingChanges {
    pub creates: Vec<Setting>,
    pub updates: Vec<Setting>,
}

#[derive(Debug)]
pub struct FileChanges {
    pub uploads: Vec<EdgeAppFile>,
    pub copies: Vec<String>,
    changes_detected: bool,
}

impl FileChanges {
    pub fn new(uploads: &[EdgeAppFile], copies: &[String], changes_detected: bool) -> Self {
        Self {
            uploads: uploads.to_vec(),
            copies: copies.to_vec(),
            changes_detected,
        }
    }

    pub fn has_changes(&self) -> bool {
        // not considering copies - copies are all assets from previous version anyhow
        !self.uploads.is_empty() || self.changes_detected
    }
}

fn is_included(entry: &DirEntry, ignore: &Ignorer) -> bool {
    let exclusion_list = ["screenly.js", "screenly.yml", ".ignore"];
    if exclusion_list.contains(&entry.file_name().to_str().unwrap_or_default()) {
        return false;
    }

    return !ignore.is_ignored(entry.path());
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
    let mut uploads: Vec<EdgeAppFile> = Vec::new();
    let mut copies: Vec<String> = Vec::new();
    let mut signatures: HashSet<String> = HashSet::new();

    // Store remote file signatures in the hashmap
    for remote_file in remote_files {
        signatures.insert(remote_file.signature.clone());
    }

    for local_file in local_files {
        let local_signature = &local_file.signature;

        if signatures.contains(local_signature) {
            copies.push(local_signature.clone());
        } else {
            uploads.push(local_file.clone());
        }
    }

    Ok(FileChanges::new(&uploads, &copies, !uploads.is_empty()))
}

pub fn detect_changed_settings(
    manifest: &EdgeAppManifest,
    remote_settings: &[Setting],
) -> Result<SettingChanges, CommandError> {
    // This function compares remote and local settings
    // And returns if there are any new local settings missing from the remote
    // And changed settings to update
    let new_settings = &manifest.settings;

    let mut creates = Vec::new();
    let mut updates = Vec::new();

    let mut remote_iter = remote_settings.iter().peekable();
    let mut new_iter = new_settings.iter().peekable();

    while let (Some(&remote_setting), Some(&new_setting)) = (remote_iter.peek(), new_iter.peek()) {
        match remote_setting.title.cmp(&new_setting.title) {
            std::cmp::Ordering::Equal => {
                if remote_setting != new_setting {
                    updates.push(new_setting.clone());
                }
                remote_iter.next();
                new_iter.next();
            }
            std::cmp::Ordering::Less => {
                remote_iter.next();
            }
            std::cmp::Ordering::Greater => {
                creates.push(new_setting.clone());
                new_iter.next();
            }
        }
    }

    creates.extend(new_iter.cloned());

    Ok(SettingChanges { creates, updates })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use crate::commands::SettingType;

    fn create_manifest() -> EdgeAppManifest {
        EdgeAppManifest {
            app_id: "01H2QZ6Z8WXWNDC0KQ198XCZEW".to_string(),
            user_version: Some("1".to_string()),
            description: Some("asdf".to_string()),
            icon: Some("asdf".to_string()),
            author: Some("asdf".to_string()),
            homepage_url: Some("asdfasdf".to_string()),
            settings: vec![
                Setting {
                    type_: SettingType::String,
                    default_value: "5".to_string(),
                    title: "display_time".to_string(),
                    optional: true,
                    help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
                },
                Setting {
                    type_: SettingType::String,
                    default_value: "6".to_string(),
                    title: "google_maps_api_key".to_string(),
                    optional: true,
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
                type_: SettingType::String,
                default_value: "5".to_string(),
                title: "display_time".to_string(),
                optional: true,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                type_: SettingType::String,
                default_value: "6".to_string(),
                title: "google_maps_api_key".to_string(),
                optional: true,
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
    fn test_detect_changes_settings_when_setting_removed_should_not_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                type_: SettingType::String,
                default_value: "5".to_string(),
                title: "display_time".to_string(),
                optional: true,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                type_: SettingType::String,
                default_value: "6".to_string(),
                title: "google_maps_api_key".to_string(),
                optional: true,
                help_text: "Specify a commercial Google Maps API key. Required due to the app's map feature.".to_string(),
            },
            Setting {
                type_: SettingType::String,
                default_value: "10".to_string(),
                title: "new_setting".to_string(),
                optional: false,
                help_text: "New setting description".to_string(),
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
    fn test_detect_changes_settings_when_local_setting_added_should_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                type_: SettingType::String,
                default_value: "5".to_string(),
                title: "display_time".to_string(),
                optional: true,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
        ];

        // Act
        let result = detect_changed_settings(&manifest, &remote_settings);

        // Assert
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.creates.len(), 1);
        assert_eq!(changes.creates[0].title, "google_maps_api_key");
    }

    // TODO: Update test, when patching is implemented
    #[test]
    fn test_detect_changed_settings_when_setting_are_modified_should_detect_changes() {
        // Arrange
        let manifest = create_manifest();

        let remote_settings = vec![
            Setting {
                type_: SettingType::String,
                default_value: "5".to_string(),
                title: "display_time".to_string(),
                optional: true,
                help_text: "For how long to display the map overlay every time the rover has moved to a new position.".to_string(),
            },
            Setting {
                type_: SettingType::String,
                default_value: "7".to_string(), // Modified default value
                title: "google_maps_api_key".to_string(),
                optional: true,
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
        assert_eq!(changes.updates[0].title, "google_maps_api_key");
        assert_eq!(changes.updates[0].default_value, "6");
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
        assert_eq!(changes.uploads.len(), 0);
        assert_eq!(changes.copies.len(), 2);
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
        assert_eq!(changes.uploads.len(), 1);
        assert_eq!(changes.copies.len(), 1);
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
        assert_eq!(changes.uploads.len(), 2);
        assert_eq!(changes.copies.len(), 0);
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

        let result = collect_paths_for_upload(dir_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "file1.txt");
    }
}
