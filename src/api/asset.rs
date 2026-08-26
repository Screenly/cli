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
    /// Staged assets carry no revision until a deploy claims them, and no signature until the
    /// processor has run.
    pub fn get_staged_processing_statuses(
        &self,
        app_id: &str,
    ) -> Result<Vec<AssetProcessingStatus>, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/assets?select=status,processing_error,title&app_id=eq.{app_id}&app_revision=is.null&status=neq.finished"
            ),
        )?;

        Ok(serde_json::from_value::<Vec<AssetProcessingStatus>>(
            response,
        )?)
    }
}
