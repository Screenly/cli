//! Edge App MCP tools.

use crate::authentication::Authentication;
use crate::commands;

/// Edge App tools for the MCP server.
pub struct EdgeAppTools;

impl EdgeAppTools {
    /// List all Edge Apps.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/edge-apps?select=id,name&deleted=eq.false")
            .map_err(|e| format!("Failed to list Edge Apps: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// List settings for an Edge App.
    pub fn list_settings(auth: &Authentication, app_uuid: &str) -> Result<String, String> {
        let endpoint = format!(
            "v4.1/edge-apps/settings?app_id=eq.{}&select=name,type,default_value,optional,title,help_text&order=name.asc",
            app_uuid
        );
        let result = commands::get(auth, &endpoint)
            .map_err(|e| format!("Failed to list Edge App settings: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// List instances of an Edge App.
    pub fn list_instances(auth: &Authentication, app_uuid: &str) -> Result<String, String> {
        let endpoint = format!(
            "v4.1/edge-apps/installations?select=id,name&app_id=eq.{}",
            app_uuid
        );
        let result = commands::get(auth, &endpoint)
            .map_err(|e| format!("Failed to list Edge App instances: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }
}
