//! Screen-related MCP tools.

use crate::authentication::Authentication;
use crate::commands;
use crate::commands::screen::SCREEN_SELECT;

/// Screen tools for the MCP server.
pub struct ScreenTools;

impl ScreenTools {
    /// List all screens.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let endpoint = format!("v4.1/screens?select={SCREEN_SELECT}");
        let result =
            commands::get(auth, &endpoint).map_err(|e| format!("Failed to list screens: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Get a screen by UUID.
    pub fn get(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4.1/screens?id=eq.{uuid}&select={SCREEN_SELECT}");
        let result =
            commands::get(auth, &endpoint).map_err(|e| format!("Failed to get screen: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }
}
