use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

#[derive(Debug)]
pub struct EdgeApps {
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeApp {
    pub name: String,
}

impl Api {
    pub fn create_app(&self, name: String) -> Result<String, CommandError> {
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ApiResponse {
            #[serde(default)]
            pub id: String,
            #[serde(default)]
            pub name: String,
        }

        let response = commands::post(
            &self.authentication,
            "v4/edge-apps?select=id,name",
            &json!({ "name": name }),
        )?;

        let json_response = serde_json::from_value::<Vec<ApiResponse>>(response)?;
        let app_id = json_response[0].id.clone();

        if app_id.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(app_id)
    }

    pub fn list_apps(&self) -> Result<EdgeApps, CommandError> {
        Ok(EdgeApps::new(commands::get(
            &self.authentication,
            "v4/edge-apps?select=id,name&deleted=eq.false",
        )?))
    }

    pub fn delete_app(&self, app_id: &str) -> Result<(), CommandError> {
        commands::delete(
            &self.authentication,
            &format!("v4/edge-apps?id=eq.{app_id}"),
        )?;
        Ok(())
    }

    pub fn update_app(&self, app_id: &str, name: &str) -> Result<(), CommandError> {
        commands::patch(
            &self.authentication,
            &format!("v4/edge-apps?select=name&id=eq.{app_id}"),
            &json!({ "name": name }),
        )?;
        Ok(())
    }

    pub fn get_app(&self, app_id: &str) -> Result<EdgeApp, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!("v4/edge-apps?select=name&id=eq.{app_id}"),
        )?;

        let apps = serde_json::from_value::<Vec<EdgeApp>>(response)?;
        if apps.is_empty() {
            Err(CommandError::AppNotFound(format!(
                "Edge App with ID '{app_id}' not found."
            )))
        } else {
            Ok(apps[0].clone())
        }
    }

    pub fn copy_assets(&self, payload: Value) -> Result<Vec<String>, CommandError> {
        let response = commands::post(&self.authentication, "v4/edge-apps/copy-assets", &payload)?;
        let copied_assets = serde_json::from_value::<Vec<String>>(response)?;

        debug!("Copied assets: {copied_assets:?}");
        Ok(copied_assets)
    }
}
