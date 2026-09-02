use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use log::debug;
use walkdir::{DirEntry, WalkDir};

use crate::commands::edge_app::instance_manifest::InstanceManifest;
use crate::commands::edge_app::manifest::EdgeAppManifest;
use crate::commands::ignorer::Ignorer;
use crate::commands::CommandError;
use crate::signature::{generate_signature, sig_to_hex};

const INSTANCE_FILE_NAME_ENV: &str = "INSTANCE_FILE_NAME";
const MANIFEST_FILE_NAME_ENV: &str = "MANIFEST_FILE_NAME";

#[derive(Debug, Clone)]
pub struct EdgeAppFile {
    pub(crate) path: String,
    pub signature: String,
}

fn is_included(entry: &DirEntry, ignore: &Ignorer) -> bool {
    let exclusion_list = ["screenly.js", "screenly.yml", ".ignore", "instance.yml"];
    if exclusion_list.contains(&entry.file_name().to_str().unwrap_or_default()) {
        return false;
    }

    !ignore.is_ignored(entry.path())
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
        CommandError::IgnoreError(format!("Failed to initialize ignore module: {e}"))
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
    use std::fs::File;
    use std::io::Write;

    use temp_env;
    use tempfile::tempdir;

    use super::*;
    use crate::commands::edge_app::instance_manifest::INSTANCE_MANIFEST_VERSION;
    use crate::commands::edge_app::manifest::{Entrypoint, EntrypointType, MANIFEST_VERSION};

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
    #[cfg_attr(target_os = "macos", ignore)]
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
    #[cfg_attr(target_os = "macos", ignore)]
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
    #[cfg_attr(target_os = "macos", ignore)]
    fn test_transform_edge_app_instance_path_to_instance_manifest_with_env_instance_override_should_return_overrided_manifest_path(
    ) {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());
        temp_env::with_var(INSTANCE_FILE_NAME_ENV, Some("instance2.yml"), || {
            let result = transform_instance_path_to_instance_manifest(&None);
            assert!(result.is_ok());
            let expected_path = dir_path.join("instance2.yml");
            // Compare only the file names to avoid issues with temp dir path differences
            assert_eq!(
                result.unwrap().file_name(),
                expected_path.file_name(),
                "Expected filename did not match"
            );
        });
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)]
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
    #[cfg_attr(target_os = "macos", ignore)]
    fn test_transform_edge_app_path_to_manifest_should_return_current_dir_with_() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        assert!(env::set_current_dir(dir_path).is_ok());

        let result = transform_edge_app_path_to_manifest(&None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path.join("screenly.yml"));
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)]
    fn test_transform_edge_app_path_to_manifest_when_path_provided_should_return_path_with_manifest(
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
    #[cfg_attr(target_os = "macos", ignore)]
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
    #[cfg_attr(target_os = "macos", ignore)]
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
    #[cfg_attr(target_os = "macos", ignore)]
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
            categories: vec!["Utilities".to_string(), "Dashboards".to_string()],
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
            categories: vec!["Utilities".to_string(), "Dashboards".to_string()],
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
            categories: vec!["Utilities".to_string(), "Dashboards".to_string()],
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
