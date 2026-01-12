//! Shared playlist MCP tools.

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;

/// Shared playlist tools for the MCP server.
pub struct SharedPlaylistTools;

impl SharedPlaylistTools {
    /// List all shared playlists.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/playlists/shared")
            .map_err(|e| format!("Failed to list shared playlists: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Share a playlist with a team.
    pub fn create(
        auth: &Authentication,
        playlist_uuid: &str,
        team_uuid: &str,
    ) -> Result<String, String> {
        let payload = json!({
            "playlist_id": playlist_uuid,
            "team_id": team_uuid,
        });

        let result = commands::post(auth, "v4/playlists/shared", &payload)
            .map_err(|e| format!("Failed to share playlist: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Unshare a playlist from a team.
    pub fn delete(
        auth: &Authentication,
        playlist_uuid: &str,
        team_uuid: &str,
    ) -> Result<String, String> {
        let endpoint = format!(
            "v4/playlists/shared?playlist_id=eq.{}&team_id=eq.{}",
            playlist_uuid, team_uuid
        );
        commands::delete(auth, &endpoint)
            .map_err(|e| format!("Failed to unshare playlist: {}", e))?;

        Ok(json!({
            "status": "unshared",
            "playlist_id": playlist_uuid,
            "team_id": team_uuid
        })
        .to_string())
    }
}
