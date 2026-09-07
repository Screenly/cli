use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

/// Keeps the `id=in.(...)` query string clear of the 8 KB request-line limit servers impose.
const MAX_ASSET_IDS_PER_STATUS_REQUEST: usize = 100;

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
        let mut statuses = Vec::new();

        for chunk in asset_ids.chunks(MAX_ASSET_IDS_PER_STATUS_REQUEST) {
            let response = commands::get(
                &self.authentication,
                &format!(
                    "v4/assets?select=status,processing_error,title&id=in.({})&status=neq.finished",
                    chunk.join(",")
                ),
            )?;

            statuses.extend(serde_json::from_value::<Vec<AssetProcessingStatus>>(
                response,
            )?);
        }

        Ok(statuses)
    }
}
