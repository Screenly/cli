//! Asset-related MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Asset tools for the MCP server.
pub struct AssetTools;

impl AssetTools {
    /// List all assets (excluding edge-app-file type).
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/assets?type=neq.edge-app-file")
            .map_err(|e| format!("Failed to list assets: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Get an asset by UUID.
    pub fn get(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/assets?id=eq.{}", uuid);
        let result =
            commands::get(auth, &endpoint).map_err(|e| format!("Failed to get asset: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Create a new asset from a URL.
    pub fn create(auth: &Authentication, title: &str, source_url: &str) -> Result<String, String> {
        let payload = json!({
            "title": title,
            "source_url": source_url,
        });

        let result = commands::post(auth, "v4/assets", &payload)
            .map_err(|e| format!("Failed to create asset: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Update an asset.
    pub fn update(
        auth: &Authentication,
        uuid: &str,
        title: Option<String>,
        js_injection: Option<String>,
        headers: Option<String>,
    ) -> Result<String, String> {
        let mut payload = serde_json::Map::new();

        if let Some(t) = title {
            payload.insert("title".to_string(), json!(t));
        }

        if let Some(js) = js_injection {
            payload.insert("js_injection".to_string(), json!(js));
        }

        if let Some(h) = headers {
            // Parse headers as JSON
            let headers_json: serde_json::Value =
                serde_json::from_str(&h).map_err(|e| format!("Invalid headers JSON: {}", e))?;
            payload.insert("headers".to_string(), headers_json);
        }

        if payload.is_empty() {
            return Err("No fields to update".to_string());
        }

        let endpoint = format!("v4/assets?id=eq.{}", uuid);
        let result = commands::patch(auth, &endpoint, &serde_json::Value::Object(payload))
            .map_err(|e| format!("Failed to update asset: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Delete an asset.
    pub fn delete(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/assets?id=eq.{}", uuid);
        commands::delete(auth, &endpoint).map_err(|e| format!("Failed to delete asset: {}", e))?;

        Ok(json!({"status": "deleted", "id": uuid}).to_string())
    }
}
