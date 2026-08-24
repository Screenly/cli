//! Unit tests for MCP tools.

use httpmock::Method::{DELETE, GET, PATCH, POST};
use httpmock::MockServer;
use serde_json::json;

use crate::authentication::{Authentication, Config};
use crate::mcp::tools::asset::AssetTools;
use crate::mcp::tools::asset_group::AssetGroupTools;
use crate::mcp::tools::edge_app::EdgeAppTools;
use crate::mcp::tools::label::LabelTools;
use crate::mcp::tools::playlist::PlaylistTools;
use crate::mcp::tools::playlist_item::PlaylistItemTools;
use crate::mcp::tools::screen::ScreenTools;
use crate::mcp::tools::shared_playlist::SharedPlaylistTools;

fn setup_auth(mock_server: &MockServer) -> Authentication {
    let config = Config::new(mock_server.base_url());
    Authentication::new_with_config(config, "test_token")
}

// ============ SCREEN TESTS ============

#[test]
fn test_screen_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/screens")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "screen-1", "name": "Test Screen"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = ScreenTools::list(&auth);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("screen-1"));
    assert!(body.contains("Test Screen"));
}

#[test]
fn test_screen_get() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/screens")
            .query_param("id", "eq.screen-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "screen-uuid", "name": "My Screen"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = ScreenTools::get(&auth, "screen-uuid");
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("screen-uuid"));
}

// ============ ASSET TESTS ============

#[test]
fn test_asset_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/assets")
            .query_param("type", "neq.edge-app-file")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "asset-1", "title": "Test Asset"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetTools::list(&auth);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("asset-1"));
}

#[test]
fn test_asset_get() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/assets")
            .query_param("id", "eq.asset-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "asset-uuid", "title": "My Asset"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetTools::get(&auth, "asset-uuid");
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("asset-uuid"));
}

#[test]
fn test_asset_create() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/assets")
            .header("Authorization", "Token test_token")
            .json_body(json!({"title": "New Asset", "source_url": "https://example.com"}));
        then.status(201)
            .json_body(json!({"id": "new-asset-id", "title": "New Asset"}));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetTools::create(&auth, "New Asset", "https://example.com");
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("new-asset-id"));
}

#[test]
fn test_asset_update() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(PATCH)
            .path("/v4/assets")
            .query_param("id", "eq.asset-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "asset-uuid", "title": "Updated Title"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetTools::update(
        &auth,
        "asset-uuid",
        Some("Updated Title".to_string()),
        None,
        None,
    );
    assert!(result.is_ok());
}

#[test]
fn test_asset_update_no_fields() {
    let mock_server = MockServer::start();
    let auth = setup_auth(&mock_server);
    let result = AssetTools::update(&auth, "asset-uuid", None, None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No fields to update"));
}

#[test]
fn test_asset_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/assets")
            .query_param("id", "eq.asset-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = AssetTools::delete(&auth, "asset-uuid");
    assert!(result.is_ok());
    assert!(result.unwrap().contains("deleted"));
}

// ============ ASSET GROUP TESTS ============

#[test]
fn test_asset_group_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/asset-groups")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "group-1", "title": "Test Group"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetGroupTools::list(&auth);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("group-1"));
}

#[test]
fn test_asset_group_create() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/asset-groups")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"id": "new-group-id", "title": "New Group"}));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetGroupTools::create(&auth, "New Group");
    assert!(result.is_ok());
}

#[test]
fn test_asset_group_update() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(PATCH)
            .path("/v4/asset-groups")
            .query_param("id", "eq.group-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "group-uuid", "title": "Updated Group"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = AssetGroupTools::update(&auth, "group-uuid", "Updated Group");
    assert!(result.is_ok());
}

#[test]
fn test_asset_group_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/asset-groups")
            .query_param("id", "eq.group-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = AssetGroupTools::delete(&auth, "group-uuid");
    assert!(result.is_ok());
}

// ============ PLAYLIST TESTS ============

#[test]
fn test_playlist_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/playlists")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "playlist-1", "title": "Test Playlist"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistTools::list(&auth);
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("playlist-1"));
}

#[test]
fn test_playlist_create() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/playlists")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"id": "new-playlist-id", "title": "New Playlist"}));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistTools::create(&auth, "New Playlist", None, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_playlist_update() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(PATCH)
            .path("/v4/playlists")
            .query_param("id", "eq.playlist-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "playlist-uuid", "title": "Updated"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistTools::update(
        &auth,
        "playlist-uuid",
        Some("Updated".to_string()),
        None,
        None,
        None,
    );
    assert!(result.is_ok());
}

#[test]
fn test_playlist_update_no_fields() {
    let mock_server = MockServer::start();
    let auth = setup_auth(&mock_server);
    let result = PlaylistTools::update(&auth, "playlist-uuid", None, None, None, None);
    assert!(result.is_err());
}

#[test]
fn test_playlist_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/playlists")
            .query_param("id", "eq.playlist-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistTools::delete(&auth, "playlist-uuid");
    assert!(result.is_ok());
}

// ============ PLAYLIST ITEM TESTS ============

#[test]
fn test_playlist_item_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/playlist-items")
            .query_param("playlist_id", "eq.playlist-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "item-1", "asset_id": "asset-1"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistItemTools::list(&auth, "playlist-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_playlist_item_create() {
    let mock_server = MockServer::start();
    // First mock for getting current positions
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/playlist-items")
            .query_param("select", "position")
            .query_param("playlist_id", "eq.playlist-uuid")
            .header("Authorization", "Token test_token");
        then.status(200).json_body(json!([]));
    });
    // Second mock for creating item
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/playlist-items")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!([{"id": "new-item-id", "playlist_id": "playlist-uuid"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistItemTools::create(&auth, "playlist-uuid", "asset-uuid", 30, None);
    assert!(result.is_ok());
}

#[test]
fn test_playlist_item_update() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(PATCH)
            .path("/v4/playlist-items")
            .query_param("playlist_id", "eq.playlist-uuid")
            .query_param("id", "eq.item-uuid")
            .header("Authorization", "Token test_token");
        then.status(200).json_body(json!([{"id": "item-uuid"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistItemTools::update(&auth, "playlist-uuid", "item-uuid", Some(60), None);
    assert!(result.is_ok());
}

#[test]
fn test_playlist_item_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/playlist-items")
            .query_param("playlist_id", "eq.playlist-uuid")
            .query_param("id", "eq.item-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = PlaylistItemTools::delete(&auth, "playlist-uuid", "item-uuid");
    assert!(result.is_ok());
}

// ============ LABEL TESTS ============

#[test]
fn test_label_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/labels")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "label-1", "name": "Test Label"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::list(&auth);
    assert!(result.is_ok());
}

#[test]
fn test_label_create() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/labels")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"id": "new-label-id", "name": "New Label"}));
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::create(&auth, "New Label");
    assert!(result.is_ok());
}

#[test]
fn test_label_update() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(PATCH)
            .path("/v4/labels")
            .query_param("id", "eq.label-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "label-uuid", "name": "Updated Label"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::update(&auth, "label-uuid", "Updated Label");
    assert!(result.is_ok());
}

#[test]
fn test_label_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/labels")
            .query_param("id", "eq.label-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::delete(&auth, "label-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_label_link_screen() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/labels/screens")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"label_id": "label-uuid", "screen_id": "screen-uuid"}));
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::link_screen(&auth, "label-uuid", "screen-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_label_unlink_screen() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/labels/screens")
            .query_param("label_id", "eq.label-uuid")
            .query_param("screen_id", "eq.screen-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::unlink_screen(&auth, "label-uuid", "screen-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_label_link_playlist() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/labels/playlists")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"label_id": "label-uuid", "playlist_id": "playlist-uuid"}));
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::link_playlist(&auth, "label-uuid", "playlist-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_label_unlink_playlist() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/labels/playlists")
            .query_param("label_id", "eq.label-uuid")
            .query_param("playlist_id", "eq.playlist-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = LabelTools::unlink_playlist(&auth, "label-uuid", "playlist-uuid");
    assert!(result.is_ok());
}

// ============ SHARED PLAYLIST TESTS ============

#[test]
fn test_shared_playlist_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/playlists/shared")
            .header("Authorization", "Token test_token");
        then.status(200).json_body(json!([]));
    });

    let auth = setup_auth(&mock_server);
    let result = SharedPlaylistTools::list(&auth);
    assert!(result.is_ok());
}

#[test]
fn test_shared_playlist_create() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v4/playlists/shared")
            .header("Authorization", "Token test_token");
        then.status(201)
            .json_body(json!({"playlist_id": "playlist-uuid", "team_id": "team-uuid"}));
    });

    let auth = setup_auth(&mock_server);
    let result = SharedPlaylistTools::create(&auth, "playlist-uuid", "team-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_shared_playlist_delete() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(DELETE)
            .path("/v4/playlists/shared")
            .query_param("playlist_id", "eq.playlist-uuid")
            .query_param("team_id", "eq.team-uuid")
            .header("Authorization", "Token test_token");
        then.status(204);
    });

    let auth = setup_auth(&mock_server);
    let result = SharedPlaylistTools::delete(&auth, "playlist-uuid", "team-uuid");
    assert!(result.is_ok());
}

// ============ EDGE APP TESTS ============

#[test]
fn test_edge_app_list() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4/edge-apps")
            .query_param("select", "id,name")
            .query_param("deleted", "eq.false")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "app-1", "name": "Test App"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = EdgeAppTools::list(&auth);
    assert!(result.is_ok());
}

#[test]
fn test_edge_app_list_settings() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4.1/edge-apps/settings")
            .query_param("app_id", "eq.app-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"name": "setting1", "type": "string"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = EdgeAppTools::list_settings(&auth, "app-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_edge_app_list_instances() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(GET)
            .path("/v4.1/edge-apps/installations")
            .query_param("app_id", "eq.app-uuid")
            .header("Authorization", "Token test_token");
        then.status(200)
            .json_body(json!([{"id": "instance-1", "name": "Test Instance"}]));
    });

    let auth = setup_auth(&mock_server);
    let result = EdgeAppTools::list_instances(&auth, "app-uuid");
    assert!(result.is_ok());
}

#[test]
fn test_edge_app_publish_from_html_rejects_empty_name() {
    let mock_server = MockServer::start();
    let auth = setup_auth(&mock_server);
    let result = EdgeAppTools::publish_from_html(&auth, "  ", "<h1>Hi</h1>", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("name is required"));
}

#[test]
fn test_edge_app_publish_from_html_rejects_empty_html() {
    let mock_server = MockServer::start();
    let auth = setup_auth(&mock_server);
    let result = EdgeAppTools::publish_from_html(&auth, "Lobby Board", "   ", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("html is empty"));
}

/// Guard against the tool catalog drifting between the MCPB manifest and
/// the `#[tool]` definitions in `server.rs` (names + descriptions).
#[test]
fn test_mcpb_manifest_tools_match_server() {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use regex::Regex;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(manifest_dir.join("mcpb/manifest.json"))
            .expect("mcpb/manifest.json should exist"),
    )
    .expect("mcpb/manifest.json should be valid JSON");

    assert_eq!(
        manifest["tools_generated"], false,
        "tools_generated must stay false while the tools array is the source of truth"
    );

    let mut manifest_tools = BTreeMap::new();
    for tool in manifest["tools"]
        .as_array()
        .expect("manifest tools must be an array")
    {
        let name = tool["name"].as_str().expect("tool name").to_string();
        let description = tool["description"]
            .as_str()
            .expect("tool description")
            .to_string();
        assert!(
            manifest_tools.insert(name.clone(), description).is_none(),
            "duplicate tool in manifest: {name}"
        );
    }

    let server_src = fs::read_to_string(manifest_dir.join("src/mcp/server.rs"))
        .expect("src/mcp/server.rs should exist");
    let tool_re = Regex::new(
        r#"(?s)#\[tool\(\s*description\s*=\s*"([^"]+)"[\s\S]*?\)\]\s*(?:async\s+)?fn\s+(\w+)"#,
    )
    .unwrap();

    let mut server_tools = BTreeMap::new();
    for caps in tool_re.captures_iter(&server_src) {
        let description = caps[1].to_string();
        let name = caps[2].to_string();
        assert!(
            server_tools.insert(name.clone(), description).is_none(),
            "duplicate tool in server.rs: {name}"
        );
    }

    assert_eq!(
        server_tools.len(),
        34,
        "expected 34 #[tool] handlers in server.rs, found {}",
        server_tools.len()
    );
    assert_eq!(
        manifest_tools.keys().collect::<Vec<_>>(),
        server_tools.keys().collect::<Vec<_>>(),
        "tool names in mcpb/manifest.json must match src/mcp/server.rs"
    );

    for (name, server_description) in &server_tools {
        assert_eq!(
            manifest_tools.get(name).map(String::as_str),
            Some(server_description.as_str()),
            "description drift for tool `{name}`"
        );
    }
}
