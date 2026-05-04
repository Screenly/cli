use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

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
                "v4/assets?select=signature&app_id=eq.{app_id}&app_revision=eq.{revision}&type=eq.edge-app-file"
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
                "v4/assets?select=status,processing_error,title&app_id=eq.{app_id}&app_revision=eq.{revision}&status=neq.finished"
            ),
        )?;

        Ok(serde_json::from_value::<Vec<AssetProcessingStatus>>(
            response,
        )?)
    }

    pub fn set_asset_js_injection(
        &self,
        asset_id: &str,
        js_code: &str,
    ) -> Result<(), CommandError> {
        // TODO: Switch to a dedicated revision-scoped js_injection endpoint
        // when the backend ships it. Patching `js_injection` directly on the
        // asset currently returns 400 ({"code":"22P02","error":"Invalid input
        // value: nil"}) on the live API, so the deploy errors at this final
        // step. Versioning + entrypoint setup + asset lookup are fine.
        let endpoint = format!("api/v4.1/assets?id=eq.{asset_id}");
        commands::patch(
            &self.authentication,
            &endpoint,
            &serde_json::json!({ "js_injection": js_code }),
        )?;
        Ok(())
    }

    pub fn get_installation_stable_asset(
        &self,
        installation_id: &str,
    ) -> Result<Option<(String, String)>, CommandError> {
        // Assets have `app_installation_id` as a direct column, so a flat
        // filter is enough — no PostgREST embed needed.
        let endpoint = format!(
            "api/v4.1/assets?select=id,js_injection\
             &app_installation_id=eq.{installation_id}&app_channel=eq.stable"
        );
        let response = commands::get(&self.authentication, &endpoint)?;

        let array = response.as_array().ok_or(CommandError::MissingField)?;
        match array.first() {
            Some(asset) => {
                let id = asset
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or(CommandError::MissingField)?
                    .to_owned();
                let js_injection = asset
                    .get("js_injection")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                Ok(Some((id, js_injection)))
            }
            None => Ok(None),
        }
    }
}
