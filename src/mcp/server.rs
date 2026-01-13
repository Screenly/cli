//! MCP server handler implementation.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{schemars, tool, tool_handler, tool_router, ServiceExt};
use serde::Deserialize;
use serde_json::json;

use crate::authentication::Authentication;
use crate::mcp::tools::asset::AssetTools;
use crate::mcp::tools::asset_group::AssetGroupTools;
use crate::mcp::tools::edge_app::EdgeAppTools;
use crate::mcp::tools::label::LabelTools;
use crate::mcp::tools::playlist::PlaylistTools;
use crate::mcp::tools::playlist_item::PlaylistItemTools;
use crate::mcp::tools::screen::ScreenTools;
use crate::mcp::tools::shared_playlist::SharedPlaylistTools;

// ============ PARAMETER STRUCTS ============

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UuidParam {
    #[schemars(description = "UUID of the resource")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssetCreateParam {
    #[schemars(description = "Title of the asset")]
    pub title: String,
    #[schemars(description = "Source URL of the asset (web page, image, or video URL)")]
    pub source_url: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssetUpdateParam {
    #[schemars(description = "UUID of the asset to update")]
    pub uuid: String,
    #[schemars(description = "New title for the asset")]
    pub title: Option<String>,
    #[schemars(description = "JavaScript code to inject into web assets")]
    pub js_injection: Option<String>,
    #[schemars(description = "HTTP headers as JSON object")]
    pub headers: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TitleParam {
    #[schemars(description = "Title")]
    pub title: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssetGroupUpdateParam {
    #[schemars(description = "UUID of the asset group")]
    pub uuid: String,
    #[schemars(description = "New title")]
    pub title: String,
}

/// Predicate DSL documentation for playlist scheduling.
///
/// Predicates are boolean expressions that control when a playlist is shown.
/// They use three context variables:
/// - `$DATE`: Current date as Unix timestamp in milliseconds
/// - `$TIME`: Time of day in milliseconds since midnight (0-86400000)
/// - `$WEEKDAY`: Day of week (0=Sunday, 1=Monday, ..., 6=Saturday)
///
/// Operators: `=`, `<=`, `>=`, `<`, `>`, `AND`, `OR`, `NOT`
/// Special: `BETWEEN {min, max}`, `IN {val1, val2, ...}`
///
/// Examples:
/// - `TRUE` - Always show
/// - `$WEEKDAY IN {1, 2, 3, 4, 5}` - Weekdays only
/// - `$TIME BETWEEN {32400000, 61200000}` - 9 AM to 5 PM
/// - `$TIME >= 32400000 AND $TIME <= 61200000 AND NOT $WEEKDAY IN {0, 6}` - Business hours
const _PREDICATE_DSL_DOCS: () = ();

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistCreateParam {
    #[schemars(description = "Title of the playlist")]
    pub title: String,
    #[schemars(
        description = "Predicate expression for when to show the playlist. Uses DSL with $DATE (ms timestamp), $TIME (ms since midnight, 0-86400000), $WEEKDAY (0=Sun to 6=Sat). Examples: 'TRUE' (always), '$WEEKDAY IN {1,2,3,4,5}' (weekdays), '$TIME BETWEEN {32400000, 61200000}' (9AM-5PM). Operators: =, <=, >=, <, >, AND, OR, NOT, BETWEEN {min,max}, IN {values}. Default: TRUE"
    )]
    pub predicate: Option<String>,
    #[schemars(description = "Whether this is a priority playlist")]
    pub priority: Option<bool>,
    #[schemars(description = "Whether the playlist is enabled")]
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistUpdateParam {
    #[schemars(description = "UUID of the playlist")]
    pub uuid: String,
    #[schemars(description = "New title")]
    pub title: Option<String>,
    #[schemars(
        description = "New predicate expression. Uses DSL with $DATE (ms timestamp), $TIME (ms since midnight, 0-86400000), $WEEKDAY (0=Sun to 6=Sat). Examples: 'TRUE' (always), '$WEEKDAY IN {1,2,3,4,5}' (weekdays), '$TIME BETWEEN {32400000, 61200000}' (9AM-5PM). Operators: =, <=, >=, <, >, AND, OR, NOT, BETWEEN {min,max}, IN {values}"
    )]
    pub predicate: Option<String>,
    #[schemars(description = "Set as priority playlist")]
    pub priority: Option<bool>,
    #[schemars(description = "Enable or disable")]
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistItemCreateParam {
    #[schemars(description = "UUID of the playlist")]
    pub playlist_uuid: String,
    #[schemars(description = "UUID of the asset to add")]
    pub asset_uuid: String,
    #[schemars(description = "Duration in seconds")]
    pub duration: u32,
    #[schemars(description = "Position in the playlist")]
    pub position: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistItemUpdateParam {
    #[schemars(description = "UUID of the playlist")]
    pub playlist_uuid: String,
    #[schemars(description = "UUID of the playlist item")]
    pub item_uuid: String,
    #[schemars(description = "New duration in seconds")]
    pub duration: Option<u32>,
    #[schemars(description = "New position")]
    pub position: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistItemDeleteParam {
    #[schemars(description = "UUID of the playlist")]
    pub playlist_uuid: String,
    #[schemars(description = "UUID of the playlist item")]
    pub item_uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NameParam {
    #[schemars(description = "Name")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LabelUpdateParam {
    #[schemars(description = "UUID of the label")]
    pub uuid: String,
    #[schemars(description = "New name")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LabelScreenParam {
    #[schemars(description = "UUID of the label")]
    pub label_uuid: String,
    #[schemars(description = "UUID of the screen")]
    pub screen_uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LabelPlaylistParam {
    #[schemars(description = "UUID of the label")]
    pub label_uuid: String,
    #[schemars(description = "UUID of the playlist")]
    pub playlist_uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SharedPlaylistParam {
    #[schemars(description = "UUID of the playlist")]
    pub playlist_uuid: String,
    #[schemars(description = "UUID of the team")]
    pub team_uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AppUuidParam {
    #[schemars(description = "UUID of the Edge App")]
    pub app_uuid: String,
}

// ============ SERVER STRUCT ============

/// MCP Server for Screenly API
#[derive(Clone)]
pub struct ScreenlyMcpServer {
    auth: Arc<Authentication>,
    tool_router: ToolRouter<Self>,
}

impl ScreenlyMcpServer {
    /// Create a new ScreenlyMcpServer instance.
    pub fn new() -> Result<Self, crate::authentication::AuthenticationError> {
        let auth = Authentication::new()?;
        Ok(Self {
            auth: Arc::new(auth),
            tool_router: Self::tool_router(),
        })
    }

    /// Run the MCP server on stdio transport.
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let service = self
            .serve(rmcp::transport::stdio())
            .await
            .inspect_err(|e| {
                log::error!("Failed to start MCP server: {}", e);
            })?;
        service.waiting().await?;
        Ok(())
    }
}

// ============ TOOL IMPLEMENTATIONS ============

#[tool_router]
impl ScreenlyMcpServer {
    // ============ SCREEN TOOLS ============

    #[tool(description = "List all screens with their status, hardware info, and sync state.")]
    fn screen_list(&self) -> String {
        match ScreenTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Get a screen by UUID.")]
    fn screen_get(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match ScreenTools::get(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ ASSET TOOLS ============

    #[tool(description = "List all assets with their type, status, and metadata.")]
    fn asset_list(&self) -> String {
        match AssetTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Get an asset by UUID.")]
    fn asset_get(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match AssetTools::get(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Create a new asset from a URL. Supports web pages, images, and videos.")]
    fn asset_create(
        &self,
        Parameters(AssetCreateParam { title, source_url }): Parameters<AssetCreateParam>,
    ) -> String {
        match AssetTools::create(&self.auth, &title, &source_url) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Update an asset's properties (title, js_injection, headers).")]
    fn asset_update(
        &self,
        Parameters(AssetUpdateParam {
            uuid,
            title,
            js_injection,
            headers,
        }): Parameters<AssetUpdateParam>,
    ) -> String {
        match AssetTools::update(&self.auth, &uuid, title, js_injection, headers) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Delete an asset by UUID.")]
    fn asset_delete(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match AssetTools::delete(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ ASSET GROUP TOOLS ============

    #[tool(description = "List all asset groups (folders for organizing assets).")]
    fn asset_group_list(&self) -> String {
        match AssetGroupTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Create a new asset group.")]
    fn asset_group_create(
        &self,
        Parameters(TitleParam { title }): Parameters<TitleParam>,
    ) -> String {
        match AssetGroupTools::create(&self.auth, &title) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Update an asset group.")]
    fn asset_group_update(
        &self,
        Parameters(AssetGroupUpdateParam { uuid, title }): Parameters<AssetGroupUpdateParam>,
    ) -> String {
        match AssetGroupTools::update(&self.auth, &uuid, &title) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Delete an asset group. WARNING: Also deletes all assets in the group.")]
    fn asset_group_delete(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match AssetGroupTools::delete(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ PLAYLIST TOOLS ============

    #[tool(description = "List all playlists.")]
    fn playlist_list(&self) -> String {
        match PlaylistTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Create a new playlist.")]
    fn playlist_create(
        &self,
        Parameters(PlaylistCreateParam {
            title,
            predicate,
            priority,
            is_enabled,
        }): Parameters<PlaylistCreateParam>,
    ) -> String {
        match PlaylistTools::create(&self.auth, &title, predicate, priority, is_enabled) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Update a playlist.")]
    fn playlist_update(
        &self,
        Parameters(PlaylistUpdateParam {
            uuid,
            title,
            predicate,
            priority,
            is_enabled,
        }): Parameters<PlaylistUpdateParam>,
    ) -> String {
        match PlaylistTools::update(&self.auth, &uuid, title, predicate, priority, is_enabled) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Delete a playlist by UUID.")]
    fn playlist_delete(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match PlaylistTools::delete(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ PLAYLIST ITEM TOOLS ============

    #[tool(description = "List all items in a playlist.")]
    fn playlist_item_list(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match PlaylistItemTools::list(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Add an asset to a playlist.")]
    fn playlist_item_create(
        &self,
        Parameters(PlaylistItemCreateParam {
            playlist_uuid,
            asset_uuid,
            duration,
            position,
        }): Parameters<PlaylistItemCreateParam>,
    ) -> String {
        match PlaylistItemTools::create(&self.auth, &playlist_uuid, &asset_uuid, duration, position)
        {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Update a playlist item (duration, position).")]
    fn playlist_item_update(
        &self,
        Parameters(PlaylistItemUpdateParam {
            playlist_uuid,
            item_uuid,
            duration,
            position,
        }): Parameters<PlaylistItemUpdateParam>,
    ) -> String {
        match PlaylistItemTools::update(&self.auth, &playlist_uuid, &item_uuid, duration, position)
        {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Remove an item from a playlist.")]
    fn playlist_item_delete(
        &self,
        Parameters(PlaylistItemDeleteParam {
            playlist_uuid,
            item_uuid,
        }): Parameters<PlaylistItemDeleteParam>,
    ) -> String {
        match PlaylistItemTools::delete(&self.auth, &playlist_uuid, &item_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ LABEL TOOLS ============

    #[tool(description = "List all labels. Labels group screens and target playlists.")]
    fn label_list(&self) -> String {
        match LabelTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Create a new label.")]
    fn label_create(&self, Parameters(NameParam { name }): Parameters<NameParam>) -> String {
        match LabelTools::create(&self.auth, &name) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Update a label.")]
    fn label_update(
        &self,
        Parameters(LabelUpdateParam { uuid, name }): Parameters<LabelUpdateParam>,
    ) -> String {
        match LabelTools::update(&self.auth, &uuid, &name) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Delete a label.")]
    fn label_delete(&self, Parameters(UuidParam { uuid }): Parameters<UuidParam>) -> String {
        match LabelTools::delete(&self.auth, &uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Attach a label to a screen.")]
    fn label_link_screen(
        &self,
        Parameters(LabelScreenParam {
            label_uuid,
            screen_uuid,
        }): Parameters<LabelScreenParam>,
    ) -> String {
        match LabelTools::link_screen(&self.auth, &label_uuid, &screen_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Remove a label from a screen.")]
    fn label_unlink_screen(
        &self,
        Parameters(LabelScreenParam {
            label_uuid,
            screen_uuid,
        }): Parameters<LabelScreenParam>,
    ) -> String {
        match LabelTools::unlink_screen(&self.auth, &label_uuid, &screen_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Attach a label to a playlist.")]
    fn label_link_playlist(
        &self,
        Parameters(LabelPlaylistParam {
            label_uuid,
            playlist_uuid,
        }): Parameters<LabelPlaylistParam>,
    ) -> String {
        match LabelTools::link_playlist(&self.auth, &label_uuid, &playlist_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Remove a label from a playlist.")]
    fn label_unlink_playlist(
        &self,
        Parameters(LabelPlaylistParam {
            label_uuid,
            playlist_uuid,
        }): Parameters<LabelPlaylistParam>,
    ) -> String {
        match LabelTools::unlink_playlist(&self.auth, &label_uuid, &playlist_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ SHARED PLAYLIST TOOLS ============

    #[tool(description = "List shared playlists.")]
    fn shared_playlist_list(&self) -> String {
        match SharedPlaylistTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Share a playlist with another team.")]
    fn shared_playlist_create(
        &self,
        Parameters(SharedPlaylistParam {
            playlist_uuid,
            team_uuid,
        }): Parameters<SharedPlaylistParam>,
    ) -> String {
        match SharedPlaylistTools::create(&self.auth, &playlist_uuid, &team_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Unshare a playlist from a team.")]
    fn shared_playlist_delete(
        &self,
        Parameters(SharedPlaylistParam {
            playlist_uuid,
            team_uuid,
        }): Parameters<SharedPlaylistParam>,
    ) -> String {
        match SharedPlaylistTools::delete(&self.auth, &playlist_uuid, &team_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    // ============ EDGE APP TOOLS ============

    #[tool(description = "List all Edge Apps.")]
    fn edge_app_list(&self) -> String {
        match EdgeAppTools::list(&self.auth) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "List settings for an Edge App.")]
    fn edge_app_list_settings(
        &self,
        Parameters(AppUuidParam { app_uuid }): Parameters<AppUuidParam>,
    ) -> String {
        match EdgeAppTools::list_settings(&self.auth, &app_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "List instances of an Edge App.")]
    fn edge_app_list_instances(
        &self,
        Parameters(AppUuidParam { app_uuid }): Parameters<AppUuidParam>,
    ) -> String {
        match EdgeAppTools::list_instances(&self.auth, &app_uuid) {
            Ok(result) => result,
            Err(e) => json!({"error": e}).to_string(),
        }
    }
}

// ============ SERVER HANDLER ============

#[tool_handler]
impl rmcp::ServerHandler for ScreenlyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Screenly MCP Server - Manage digital signage screens, assets, and playlists. \
                Use API_TOKEN environment variable or ~/.screenly file for authentication.\n\n\
                PLAYLIST PREDICATES: Playlists use a predicate DSL for scheduling. Variables: \
                $DATE (Unix ms), $TIME (ms since midnight, 0-86400000), $WEEKDAY (0=Sun..6=Sat). \
                Operators: =, <=, >=, <, >, AND, OR, NOT, BETWEEN {min,max}, IN {values}. \
                Examples: 'TRUE' (always show), '$WEEKDAY IN {1,2,3,4,5}' (weekdays only), \
                '$TIME BETWEEN {32400000, 61200000}' (9AM-5PM), \
                '$TIME >= 32400000 AND $TIME <= 61200000 AND NOT $WEEKDAY IN {0, 6}' (business hours). \
                Time reference: 32400000=9AM, 43200000=12PM, 61200000=5PM, 72000000=8PM."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
