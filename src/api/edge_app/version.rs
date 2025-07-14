use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

use log::debug;
use serde_json::Value;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::json;

impl Api {
    pub fn version_exists(&self, app_id: &str, revision: u32) -> Result<bool, CommandError> {
        let get_response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=revision&app_id=eq.{app_id}&revision=eq.{revision}"
            ),
        )?;
        let version =
            serde_json::from_value::<Vec<HashMap<String, serde_json::Value>>>(get_response)?;

        if version.is_empty() {
            return Ok(false);
        }

        Ok(true)
    }

    pub fn create_version(&self, json: HashMap<&str, Value>) -> Result<u32, CommandError> {
        let response = commands::post(
            &self.authentication,
            "v4/edge-apps/versions?select=revision",
            &json,
        )?;
        if let Some(arr) = response.as_array() {
            if let Some(obj) = arr.first() {
                if let Some(revision) = obj["revision"].as_u64() {
                    debug!("New version revision: {}", revision);
                    return Ok(revision as u32);
                }
            }
        }

        Err(CommandError::MissingField)
    }

    pub fn get_file_tree(
        &self,
        app_id: &str,
        revision: u32,
    ) -> Result<HashMap<String, String>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?select=file_tree&app_id=eq.{app_id}&revision=eq.{revision}"
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

    pub fn publish_version(&self, app_id: &str, revision: u32) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/versions?app_id=eq.{app_id}&revision=eq.{revision}"
            ),
            &json!({"published": true}),
        )?;
        Ok(())
    }
}
