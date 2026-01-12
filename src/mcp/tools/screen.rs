//! Screen-related MCP tools.

use crate::authentication::Authentication;
use crate::commands;

/// Screen tools for the MCP server.
pub struct ScreenTools;

impl ScreenTools {
    /// List all screens.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/screens")
            .map_err(|e| format!("Failed to list screens: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Get a screen by UUID.
    pub fn get(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/screens?id=eq.{}", uuid);
        let result =
            commands::get(auth, &endpoint).map_err(|e| format!("Failed to get screen: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }
}
