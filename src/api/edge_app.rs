use crate::api::Api;
use crate::commands::CommandError;
use crate::commands;

use serde_json::json;
use serde::{Deserialize, Serialize};


#[derive(Debug)]
pub struct EdgeApps {
    pub value: serde_json::Value,
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
            "v4/edge-apps?select=id,name",
        )?))
    }
}
