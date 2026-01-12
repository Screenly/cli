//! Label-related MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Label tools for the MCP server.
pub struct LabelTools;

impl LabelTools {
    /// List all labels.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/labels")
            .map_err(|e| format!("Failed to list labels: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Create a new label.
    pub fn create(auth: &Authentication, name: &str) -> Result<String, String> {
        let payload = json!({
            "name": name,
        });

        let result = commands::post(auth, "v4/labels", &payload)
            .map_err(|e| format!("Failed to create label: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Update a label.
    pub fn update(auth: &Authentication, uuid: &str, name: &str) -> Result<String, String> {
        let payload = json!({
            "name": name,
        });

        let endpoint = format!("v4/labels?id=eq.{}", uuid);
        let result = commands::patch(auth, &endpoint, &payload)
            .map_err(|e| format!("Failed to update label: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Delete a label.
    pub fn delete(auth: &Authentication, uuid: &str) -> Result<String, String> {
        let endpoint = format!("v4/labels?id=eq.{}", uuid);
        commands::delete(auth, &endpoint).map_err(|e| format!("Failed to delete label: {}", e))?;

        Ok(json!({"status": "deleted", "id": uuid}).to_string())
    }

    /// Attach a label to a screen.
    pub fn link_screen(
        auth: &Authentication,
        label_uuid: &str,
        screen_uuid: &str,
    ) -> Result<String, String> {
        let payload = json!({
            "label_id": label_uuid,
            "screen_id": screen_uuid,
        });

        let result = commands::post(auth, "v4/labels/screens", &payload)
            .map_err(|e| format!("Failed to link label to screen: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Remove a label from a screen.
    pub fn unlink_screen(
        auth: &Authentication,
        label_uuid: &str,
        screen_uuid: &str,
    ) -> Result<String, String> {
        let endpoint = format!(
            "v4/labels/screens?label_id=eq.{}&screen_id=eq.{}",
            label_uuid, screen_uuid
        );
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to unlink label from screen: {}", e))?;

        Ok(json!({
            "status": "unlinked",
            "label_id": label_uuid,
            "screen_id": screen_uuid
        })
        .to_string())
    }

    /// Attach a label to a playlist.
    pub fn link_playlist(
        auth: &Authentication,
        label_uuid: &str,
        playlist_uuid: &str,
    ) -> Result<String, String> {
        let payload = json!({
            "label_id": label_uuid,
            "playlist_id": playlist_uuid,
        });

        let result = commands::post(auth, "v4/labels/playlists", &payload)
            .map_err(|e| format!("Failed to link label to playlist: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Remove a label from a playlist.
    pub fn unlink_playlist(
        auth: &Authentication,
        label_uuid: &str,
        playlist_uuid: &str,
    ) -> Result<String, String> {
        let endpoint = format!(
            "v4/labels/playlists?label_id=eq.{}&playlist_id=eq.{}",
            label_uuid, playlist_uuid
        );
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to unlink label from playlist: {}", e))?;

        Ok(json!({
            "status": "unlinked",
            "label_id": label_uuid,
            "playlist_id": playlist_uuid
        })
        .to_string())
    }
}
