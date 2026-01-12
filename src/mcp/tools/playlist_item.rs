//! Playlist item MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Position spacing for playlist items. Uses large gaps (100,000) between items
/// to allow inserting new items between existing ones without reordering.
/// This follows Screenly's API convention for position-based ordering.
const POSITION_MULTIPLIER: u64 = 100000;

/// Playlist item tools for the MCP server.
pub struct PlaylistItemTools;

impl PlaylistItemTools {
    /// List all items in a playlist.
    pub fn list(auth: &Authentication, playlist_uuid: &str) -> Result<String, String> {
        let endpoint = format!(
            "v4/playlist-items?playlist_id=eq.{}&order=position.asc",
            playlist_uuid
        );
        let result = commands::get(auth, &endpoint)
            .map_err(|e| format!("Failed to list playlist items: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Add an asset to a playlist.
    pub fn create(
        auth: &Authentication,
        playlist_uuid: &str,
        asset_uuid: &str,
        duration: u32,
        position: Option<u64>,
    ) -> Result<String, String> {
        // If position is not specified, get the highest position and add after it
        let final_position = if let Some(pos) = position {
            pos
        } else {
            // Get the highest position in the playlist
            let endpoint = format!(
                "v4/playlist-items?select=position&playlist_id=eq.{}&order=position.desc&limit=1",
                playlist_uuid
            );
            let result = commands::get(auth, &endpoint)
                .map_err(|e| format!("Failed to get playlist positions: {}", e))?;

            if let Some(items) = result.as_array() {
                if items.is_empty() {
                    POSITION_MULTIPLIER
                } else if let Some(pos) = items[0].get("position").and_then(|p| p.as_u64()) {
                    pos + POSITION_MULTIPLIER
                } else {
                    POSITION_MULTIPLIER
                }
            } else {
                POSITION_MULTIPLIER
            }
        };

        let payload = json!([{
            "playlist_id": playlist_uuid,
            "asset_id": asset_uuid,
            "duration": duration,
            "position": final_position
        }]);

        let result = commands::post(auth, "v4/playlist-items", &payload)
            .map_err(|e| format!("Failed to create playlist item: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Update a playlist item.
    pub fn update(
        auth: &Authentication,
        playlist_uuid: &str,
        item_uuid: &str,
        duration: Option<u32>,
        position: Option<u64>,
    ) -> Result<String, String> {
        let mut payload = serde_json::Map::new();

        if let Some(d) = duration {
            payload.insert("duration".to_string(), json!(d));
        }

        if let Some(p) = position {
            payload.insert("position".to_string(), json!(p));
        }

        if payload.is_empty() {
            return Err("No fields to update".to_string());
        }

        let endpoint = format!(
            "v4/playlist-items?playlist_id=eq.{}&id=eq.{}",
            playlist_uuid, item_uuid
        );
        let result = commands::patch(auth, &endpoint, &serde_json::Value::Object(payload))
            .map_err(|e| format!("Failed to update playlist item: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Remove an item from a playlist.
    pub fn delete(
        auth: &Authentication,
        playlist_uuid: &str,
        item_uuid: &str,
    ) -> Result<String, String> {
        let endpoint = format!(
            "v4/playlist-items?playlist_id=eq.{}&id=eq.{}",
            playlist_uuid, item_uuid
        );
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to delete playlist item: {}", e))?;

        Ok(
            json!({"status": "deleted", "playlist_id": playlist_uuid, "item_id": item_uuid})
                .to_string(),
        )
    }
}
