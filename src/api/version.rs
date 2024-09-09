use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeAppVersion {
    #[serde(default)]
    pub user_version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage_url: Option<String>,
    #[serde(default)]
    pub ready_signal: bool,
    #[serde(default)]
    pub revision: u32,
}

impl Api {
    pub fn get_latest_revision(
        &self,
        app_id: &str,
    ) -> Result<Option<EdgeAppVersion>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/versions?select=user_version,description,icon,author,homepage_url,revision,ready_signal&app_id=eq.{}&order=revision.desc&limit=1",
                app_id
            ),
        )?;

        let versions: Vec<EdgeAppVersion> =
            serde_json::from_value::<Vec<EdgeAppVersion>>(response)?;

        if versions.is_empty() {
            return Ok(None);
        }
        Ok(versions.first().cloned())
    }
}
