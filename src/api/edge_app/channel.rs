use serde::Deserialize;
use serde_json::json;

use crate::api::Api;
use crate::commands;
use crate::commands::CommandError;

impl Api {
    pub fn update_channel(
        &self,
        channel: &str,
        app_id: &str,
        revision: u32,
    ) -> Result<(), CommandError> {
        let response = commands::patch(
            &self.authentication,
            &format!(
                "v4/edge-apps/channels?select=channel,app_revision&channel=eq.{channel}&app_id=eq.{app_id}"
            ),
            &json!(
            {
                "app_revision": revision,
            }),
        )?;

        #[derive(Clone, Debug, Default, PartialEq, Deserialize)]
        struct Channel {
            app_revision: u32,
            channel: String,
        }

        let channels = serde_json::from_value::<Vec<Channel>>(response)?;
        if channels.is_empty() {
            return Err(CommandError::MissingField);
        }
        if channels[0].channel != channel || channels[0].app_revision != revision {
            return Err(CommandError::MissingField);
        }

        Ok(())
    }
}
