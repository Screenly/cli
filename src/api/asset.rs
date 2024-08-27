use crate::api::Api;
use crate::commands::CommandError;
use crate::commands;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetSignature {
    pub(crate) signature: String,
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
}