//! Playlist-related MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Playlist tools for the MCP server.
pub struct PlaylistTools;

impl PlaylistTools {
    /// List all playlists.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/playlists")
            .map_err(|e| format!("Failed to list playlists: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Create a new playlist.
    pub fn create(
        auth: &Authentication,
        title: &str,
        predicate: Option<String>,
        priority: Option<bool>,
        is_enabled: Option<bool>,
    ) -> Result<String, String> {
        let payload = json!({
            "title": title,
            "predicate": predicate.unwrap_or_else(|| "TRUE".to_string()),
            "priority": priority.unwrap_or(false),
            "is_enabled": is_enabled.unwrap_or(true),
            "transitions": true
        });

        let result = commands::post(auth, "v4/playlists", &payload)
            .map_err(|e| format!("Failed to create playlist: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Update a playlist.
    pub fn update(
        auth: &Authentication,
        uuid: &str,
        title: Option<String>,
        predicate: Option<String>,
        priority: Option<bool>,
        is_enabled: Option<bool>,
    ) -> Result<String, String> {
        let mut payload = serde_json::Map::new();

        if let Some(t) = title {
            payload.insert("title".to_string(), json!(t));
        }

        if let Some(p) = predicate {
            payload.insert("predicate".to_string(), json!(p));
        }

        if let Some(pr) = priority {
            payload.insert("priority".to_string(), json!(pr));
        }

        if let Some(e) = is_enabled {
            payload.insert("is_enabled".to_string(), json!(e));
        }

        if payload.is_empty() {
            return Err("No fields to update".to_string());
        }

        let endpoint = format!("v4/playlists?id=eq.{}", uuid);
        let result = commands::patch(auth, &endpoint, &serde_json::Value::Object(payload))
            .map_err(|e| format!("Failed to update playlist: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Delete a playlist.
    pub fn delete(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/playlists?id=eq.{}", uuid);
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to delete playlist: {}", e))?;

        Ok(json!({"status": "deleted", "id": uuid}).to_string())
    }
}
