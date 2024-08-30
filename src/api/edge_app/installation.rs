use crate::api::Api;
use crate::commands::CommandError;
use crate::commands;

use std::collections::HashMap;
use std::ops::Not;
use std::str::FromStr;
use serde_json::Value;
use log::debug;

use serde::Deserializer;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::commands::serde_utils::{deserialize_string_field, serialize_non_empty_string_field};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug)]
pub struct EdgeAppInstances {
    pub value: serde_json::Value,
}

impl EdgeAppInstances {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Api {
    pub fn get_instance_name(&self, installation_id: &str) -> Result<String, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4.1/edge-apps/installations?select=name&id=eq.{}",
                installation_id
            ),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Instance {
            name: String,
        }

        let instances = serde_json::from_value::<Vec<Instance>>(response)?;
        if instances.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(instances[0].name.clone())
    }

    pub fn list_installations(&self, app_id: &str) -> Result<EdgeAppInstances, CommandError> {
        let response = commands::get(
            &self.authentication,
            &format!(
                "v4/edge-apps/installations?select=id,name&app_id=eq.{}",
                app_id
            ),
        )?;

        let instances = EdgeAppInstances::new(response);

        Ok(instances)
    }

    pub fn delete_installation(&self, installation_id: &str) -> Result<(), CommandError> {
        commands::delete(
            &self.authentication,
            &format!("v4.1/edge-apps/installations?id=eq.{}", installation_id),
        )?;
        Ok(())
    }

    pub fn update_installation_name(&self, installation_id: &str, name: &str) -> Result<(), CommandError> {
        let payload = json!({
            "name": name,
        });
        commands::patch(
            &self.authentication,
            &format!("v4.1/edge-apps/installations?id=eq.{}", installation_id),
            &payload,
        )?;
        Ok(())
    }

    pub fn create_installation(&self, app_id: &str, name: &str) -> Result<String, CommandError> {
        let payload = json!({
            "app_id": app_id,
            "name": name,
        });

        let response = commands::post(
            &self.authentication,
            "v4.1/edge-apps/installations?select=id",
            &payload,
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        struct Installation {
            id: String,
        }

        let installation = serde_json::from_value::<Vec<Installation>>(response)?;
        if installation.is_empty() {
            return Err(CommandError::MissingField);
        }

        Ok(installation[0].id.clone())
    }
}
