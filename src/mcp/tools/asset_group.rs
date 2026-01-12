//! Asset group (folder) MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Asset group tools for the MCP server.
pub struct AssetGroupTools;

impl AssetGroupTools {
    /// List all asset groups.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/asset-groups")
            .map_err(|e| format!("Failed to list asset groups: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Create a new asset group.
    pub fn create(auth: &Authentication, title: &str) -> Result<String, String> {
        let payload = json!({
            "title": title,
        });

        let result = commands::post(auth, "v4/asset-groups", &payload)
            .map_err(|e| format!("Failed to create asset group: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Update an asset group.
    pub fn update(auth: &Authentication, uuid: &str, title: &str) -> Result<String, String> {
        let payload = json!({
            "title": title,
        });

        let endpoint = format!("v4/asset-groups?id=eq.{}", uuid);
        let result = commands::patch(auth, &endpoint, &payload)
            .map_err(|e| format!("Failed to update asset group: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Delete an asset group (and all assets within it).
    pub fn delete(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/asset-groups?id=eq.{}", uuid);
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to delete asset group: {}", e))?;

        Ok(json!({"status": "deleted", "id": uuid}).to_string())
    }
}
