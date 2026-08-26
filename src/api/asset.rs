use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetProcessingStatus {
    pub(crate) status: String,
    pub(crate) processing_error: String,
    pub(crate) title: String,
}

impl Api {
    pub fn get_processing_statuses(
        &self,
        asset_ids: &[String],
    ) -> Result<Vec<AssetProcessingStatus>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=status,processing_error,title&id=in.({})&status=neq.finished",
                asset_ids.join(",")
            ),
        )?;

        Ok(serde_json::from_value::<Vec<AssetProcessingStatus>>(
            response,
        )?)
    }
}
