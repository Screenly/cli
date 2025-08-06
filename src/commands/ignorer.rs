use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::RegexSet;

pub struct Ignorer {
    base_path: PathBuf,
    ignores: RegexSet,
}

impl Ignorer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        let ignore_path = base_path.join(".ignore");
        let mut patterns = Vec::new();

        if ignore_path.exists() {
            let content = std::fs::read_to_string(&ignore_path)?;

            for line in content.lines() {
                let pattern = line.trim();
                if pattern.ends_with('/') {
                    patterns.push(format!("^{}.*$", regex::escape(pattern)));
                } else if pattern.contains('*') {
                    // Convert wildcard '*' to regex '.*'
                    let converted = pattern.replace('.', r"\.").replace('*', r".*");
                    patterns.push(format!("^{converted}$"));
                } else {
                    patterns.push(format!("^{}$", regex::escape(pattern)));
                }
            }
        }

        let regex_set = RegexSet::new(&patterns)?;

        Ok(Self {
            base_path,
            ignores: regex_set,
        })
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        let relative_path = path.strip_prefix(&self.base_path).unwrap_or(path);
        self.ignores
            .is_match(relative_path.to_string_lossy().as_ref())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use tempfile::tempdir;

    use super::Ignorer;

    #[test]
    fn test_when_no_ignore_file_should_ignore_nothing() {
        let dir = tempdir().unwrap();
        let ignorer = Ignorer::new(dir.path()).unwrap();
        assert!(!ignorer.is_ignored(Path::new("some_file.txt")));
    }

    #[test]
    fn test_ignore_when_specific_file_in_ignore_list_should_ignore_it() {
        let dir = tempdir().unwrap();

        File::create(dir.path().join(".ignore"))
            .unwrap()
            .write_all(b"file_to_ignore.txt")
            .unwrap();

        let ignorer = Ignorer::new(dir.path()).unwrap();

        assert!(ignorer.is_ignored(Path::new("file_to_ignore.txt")));
        assert!(!ignorer.is_ignored(Path::new("other_file.txt")));
    }

    #[test]
    fn test_ignore_when_pattern_specified_should_ignore_files_matching_that_pattern() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join(".ignore"))
            .unwrap()
            .write_all(b"*.log")
            .unwrap();

        let ignorer = Ignorer::new(dir.path()).unwrap();
        assert!(ignorer.is_ignored(Path::new("error.log")));
        assert!(ignorer.is_ignored(Path::new("info.log")));
        assert!(!ignorer.is_ignored(Path::new("file.txt")));
    }

    #[test]
    fn test_ignore_when_directory_specified_should_ignore_that_directory() {
        let dir = tempdir().unwrap();

        // Create .ignore file in temp dir
        File::create(dir.path().join(".ignore"))
            .unwrap()
            .write_all(b"ignored_dir/")
            .unwrap();

        // Create the directories and files you want to check
        let ignored_directory_path = dir.path().join("ignored_dir");
        std::fs::create_dir(&ignored_directory_path).unwrap();
        File::create(ignored_directory_path.join("some_file.txt")).unwrap();

        let other_directory_path = dir.path().join("other_dir");
        std::fs::create_dir(&other_directory_path).unwrap();
        File::create(other_directory_path.join("some_file.txt")).unwrap();

        let ignorer = Ignorer::new(dir.path()).unwrap();

        assert!(ignorer.is_ignored(&ignored_directory_path.join("some_file.txt")));
        assert!(!ignorer.is_ignored(&other_directory_path.join("some_file.txt")));
    }

    #[test]
    fn test_ignore_when_top_level_directory_ignored_should_also_ignore_nested_subdirs() {
        let dir = tempdir().unwrap();

        File::create(dir.path().join(".ignore"))
            .unwrap()
            .write_all(b"top_ignored_dir/")
            .unwrap();

        let top_ignored_directory_path = dir.path().join("top_ignored_dir");
        std::fs::create_dir(&top_ignored_directory_path).unwrap();
        File::create(top_ignored_directory_path.join("some_file.txt")).unwrap();

        let nested_subdirectory_path = top_ignored_directory_path.join("nested_subdir");
        std::fs::create_dir(&nested_subdirectory_path).unwrap();
        File::create(nested_subdirectory_path.join("nested_file.txt")).unwrap();

        let other_directory_path = dir.path().join("other_dir");
        std::fs::create_dir(&other_directory_path).unwrap();
        File::create(other_directory_path.join("some_file.txt")).unwrap();

        let ignorer = Ignorer::new(dir.path()).unwrap();

        // Check if files in top ignored directory are ignored
        assert!(ignorer.is_ignored(&top_ignored_directory_path.join("some_file.txt")));

        // Check if files in nested subdirectory of top ignored directory are ignored
        assert!(ignorer.is_ignored(&nested_subdirectory_path.join("nested_file.txt")));

        // Check if files in other directory are not ignored
        assert!(!ignorer.is_ignored(&other_directory_path.join("some_file.txt")));
    }
}
