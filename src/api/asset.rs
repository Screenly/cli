use crate::api::Api;
use crate::commands::CommandError;
use crate::commands;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetSignature {
    pub(crate) signature: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetProcessingStatus {
    pub(crate) status: String,
    pub(crate) processing_error: String,
    pub(crate) title: String,
}

impl Api {
    pub fn get_version_asset_signatures(
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

    pub fn get_processing_statuses(
        &self,
        app_id: &str,
        revision: u32,
    ) -> Result<Vec<AssetProcessingStatus>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=status,processing_error,title&app_id=eq.{}&app_revision=eq.{}&status=neq.finished",
                app_id, revision
            ),
        )?;

        Ok(serde_json::from_value::<Vec<AssetProcessingStatus>>(response)?)
    }
}