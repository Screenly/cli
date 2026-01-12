use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{CommandError, PlaylistFile, PlaylistItem, PlaylistItems, Playlists};

const POSITION_MULTIPLIER: u64 = 100000;
pub struct PlaylistCommand {
    authentication: Authentication,
}

impl PlaylistCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn list(&self) -> Result<Playlists, CommandError> {
        Ok(Playlists::new(commands::get(
            &self.authentication,
            "v4/playlists",
        )?))
    }

    pub fn create(&self, title: &str, predicate: &str) -> Result<Playlists, CommandError> {
        let response = commands::post(
            &self.authentication,
            "v4/playlists",
            &json!({
                "title": title,
                "predicate": predicate,
                "priority": false,
                "is_enabled": true,
                "transitions": true
            }),
        )?;
        Ok(Playlists::new(response))
    }

    fn get_playlist_field(&self, uuid: &str, field_name: &str) -> Result<String, CommandError> {
        let playlists = Playlists::new(commands::get(
            &self.authentication,
            &format!("v4/playlists?id=eq.{uuid}&select=predicate"),
        )?);
        let mut field = String::new();
        if let Some(pl) = playlists.value.as_array() {
            field = pl[0]
                .get(field_name)
                .ok_or(CommandError::MissingField)?
                .as_str()
                .ok_or(CommandError::MissingField)?
                .to_string();
        }
        Ok(field)
    }
    pub fn get_playlist_file(&self, uuid: &str) -> Result<PlaylistFile, CommandError> {
        let predicate = self.get_playlist_field(uuid, "predicate")?;
        let response = commands::get(
            &self.authentication,
            &format!("v4/playlist-items?select=asset_id,duration&playlist_id=eq.{uuid}&order=position.asc"),
        )?;

        PlaylistFile::new(predicate, uuid.to_string(), response)
    }

    pub fn update(&self, playlist: &PlaylistFile) -> Result<PlaylistItems, CommandError> {
        let old_predicate = self.get_playlist_field(&playlist.playlist_id, "predicate")?;
        if old_predicate != playlist.predicate {
            commands::patch(
                &self.authentication,
                &format!("v4/playlists?id=eq.{id}", id = playlist.playlist_id),
                &json!({"predicate": playlist.predicate}),
            )?;
        }

        commands::delete(
            &self.authentication,
            &format!(
                "v4/playlist-items?playlist_id=eq.{id}",
                id = playlist.playlist_id
            ),
        )?;

        let mut new_items = Vec::<serde_json::Value>::new();
        let mut position = 0;
        for item in &playlist.items {
            position += 1;
            let v = json!({
                "playlist_id": playlist.playlist_id,
                "asset_id": item.asset_id,
                "duration": item.duration,
                "position": position * POSITION_MULTIPLIER
            });
            new_items.push(v);
        }

        Ok(PlaylistItems::new(commands::post(
            &self.authentication,
            "v4/playlist-items",
            &json!(new_items),
        )?))
    }

    pub fn delete(&self, uuid: &str) -> Result<(), CommandError> {
        commands::delete(&self.authentication, &format!("v4/playlists?id=eq.{uuid}"))?;
        Ok(())
    }

    pub fn append_asset(
        &self,
        playlist_uuid: &str,
        asset_uuid: &str,
        duration: u32,
    ) -> Result<PlaylistItems, CommandError> {
        // selecting duration and playlist_id just so that we can convert it to PlaylistItem
        let response = commands::get(
            &self.authentication,
            &format!("v4/playlist-items?select=position,asset_id,duration&playlist_id=eq.{playlist_uuid}&order=position.desc&limit=1"),
        )?;

        let playlist_items = serde_json::from_value::<Vec<PlaylistItem>>(response)?;
        let position = if playlist_items.is_empty() {
            POSITION_MULTIPLIER
        } else if playlist_items.len() == 1 {
            playlist_items[0].position + POSITION_MULTIPLIER
        } else {
            return Err(CommandError::MissingField);
        };

        let payload = json!([{
            "playlist_id": playlist_uuid,
            "asset_id": asset_uuid,
            "duration": duration,
            "position": position
        }]);

        Ok(PlaylistItems::new(commands::post(
            &self.authentication,
            "v4/playlist-items",
            &payload,
        )?))
    }

    pub fn prepend_asset(
        &self,
        playlist_uuid: &str,
        asset_uuid: &str,
        duration: u32,
    ) -> Result<PlaylistItems, CommandError> {
        let mut playlist_file = self.get_playlist_file(playlist_uuid)?;
        playlist_file.items.insert(
            0,
            PlaylistItem {
                asset_id: asset_uuid.to_string(),
                duration,
                position: 0,
            },
        );
        let mut position = 0;
        for playlist_item in &mut playlist_file.items {
            position += 1;
            playlist_item.position = position * POSITION_MULTIPLIER;
        }

        self.update(&playlist_file)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use envtestkit::set_env;
    use httpmock::Method::{DELETE, GET, PATCH, POST};
    use httpmock::MockServer;

    use super::*;
    use crate::authentication::Config;

    #[test]
    fn test_create_playlist_should_send_correct_request() {
        let new_playlist_request = json!({
            "title": "Best playlist",
            "predicate": "FALSE",
            "priority": false,
            "is_enabled": true,
            "transitions": true
        });

        let new_playlist_response = json!({
          "id": "01H3M50TFHSRMEP61BBPWXKRCA",
          "is_enabled": true,
          "predicate": "FALSE",
          "priority": false,
          "title": "best-ever",
          "transitions": true
        });

        let mock_server = MockServer::start();
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/playlists")
                .header("Authorization", "Token token")
                .json_body(new_playlist_request);
            then.status(201).json_body(new_playlist_response);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.create("Best playlist", "FALSE");
        post_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_playlists_should_send_correct_request() {
        let _test = set_env(OsString::from("API_TOKEN"), "token");
        let mock_server = MockServer::start();
        let playlists_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlists")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(json!([]));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.list();
        playlists_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_playlist_file_should_send_correct_request_and_return_playlist_file() {
        let playlist_items_response = json!([
          {
            "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
            "duration": 33
          },
          {
            "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
            "duration": 10.0
          },
        ]);

        let playlists_response = json!([{"predicate": "FALSE"}]);

        let mock_server = MockServer::start();
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlists")
                .header("Authorization", "Token token");
            then.status(200).json_body(playlists_response);
        });

        let get_playlist_items_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token");
            then.status(200).json_body(playlist_items_response);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.get_playlist_file("testuuid");

        let expected_playlist_file = json!({
          "predicate": "FALSE",
          "playlist_id": "testuuid",
          "items": [
            {
              "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
              "duration": 33
            },
            {
              "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
              "duration": 10
            },
          ]
        });

        get_mock.assert();
        get_playlist_items_mock.assert();
        assert!(result.is_ok());
        assert_eq!(
            serde_json::from_value::<PlaylistFile>(expected_playlist_file).unwrap(),
            result.unwrap()
        );
    }

    #[test]
    fn test_update_playlist_should_send_correct_request() {
        let updated_playlist = json!({
          "predicate": "FALSE",
          "playlist_id": "test-playlist-id",
          "items": [
            {
              "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
              "duration": 33
            },
            {
              "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
              "duration": 10
            },
          ]
        });

        let items_request = json!([
          {
            "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
            "duration": 33,
            "position": 100000,
            "playlist_id": "test-playlist-id",
          },
          {
            "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
            "duration": 10,
            "position": 200000,
            "playlist_id": "test-playlist-id",
          },
        ]);

        let playlists_response = json!([{"predicate": "TRUE"}]);
        let mock_server = MockServer::start();
        // it will make a request to playlists to get predicate
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlists")
                .header("Authorization", "Token token");
            then.status(200).json_body(playlists_response);
        });

        // then patch request to update playlist predicate
        let patch_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/playlists")
                .header("Authorization", "Token token")
                .json_body(json!({"predicate": "FALSE"}));
            then.status(200).json_body(json!({}));
        });

        // then delete request to delete old playlist items
        let delete_mock = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4/playlist-items")
                .query_param("playlist_id", "eq.test-playlist-id")
                .header("Authorization", "Token token");
            then.status(200).json_body(json!({}));
        });

        // then post request to create new playlist items
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token")
                .json_body(items_request);
            then.status(201).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result =
            command.update(&serde_json::from_value::<PlaylistFile>(updated_playlist).unwrap());

        patch_mock.assert();
        delete_mock.assert();
        post_mock.assert();
        get_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_playlist_should_send_correct_request() {
        let mock_server = MockServer::start();
        let delete_mock = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4/playlists")
                .query_param("id", "eq.test-id")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .header("Authorization", "Token token");
            then.status(200);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.delete("test-id");
        delete_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_append_asset_to_playlist_should_send_correct_request() {
        let items_request = json!([
          {
            "position": 300000,
            "duration": 100,
            "asset_id": "test-asset-id",
            "playlist_id": "test-playlist-id"
          },
        ]);

        let mock_server = MockServer::start();
        // request to get the highest position
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlist-items")
                .query_param("playlist_id", "eq.test-playlist-id")
                .query_param("select", "position,asset_id,duration")
                .query_param("order", "position.desc")
                .query_param("limit", "1")
                .header("Authorization", "Token token");
            then.status(200)
                .json_body(json!([{"position": 200000, "asset_id": "asset-id", "duration": 10}]));
        });

        // then post request to create new playlist items
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token")
                .json_body(items_request);
            then.status(201).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.append_asset("test-playlist-id", "test-asset-id", 100);

        post_mock.assert();
        get_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_append_asset_to_empty_playlist_should_send_correct_request() {
        let items_request = json!([
          {
            "position": 100000,
            "duration": 50,
            "asset_id": "test-asset-id",
            "playlist_id": "test-playlist-id"
          },
        ]);

        let mock_server = MockServer::start();
        // request to get the highest position from empty playlist
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlist-items")
                .query_param("playlist_id", "eq.test-playlist-id")
                .query_param("select", "position,asset_id,duration")
                .query_param("order", "position.desc")
                .query_param("limit", "1")
                .header("Authorization", "Token token");
            then.status(200).json_body(json!([])); // empty array for empty playlist
        });

        // then post request to create new playlist items
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token")
                .json_body(items_request);
            then.status(201).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.append_asset("test-playlist-id", "test-asset-id", 50);

        post_mock.assert();
        get_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepend_asset_to_playlist_should_send_correct_request() {
        let _updated_playlist = json!({
          "predicate": "TRUE",
          "playlist_id": "test-playlist-id",
          "items": [
            {
              "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
              "duration": 33
            },
            {
              "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
              "duration": 10
            },
          ]
        });

        let items_request = json!([
            {
            "asset_id": "test-asset-id",
            "duration": 100,
            "position": 100000,
            "playlist_id": "test-playlist-id",
          },
          {
            "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
            "duration": 33,
            "position": 200000,
            "playlist_id": "test-playlist-id",
          },
          {
            "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
            "duration": 10,
            "position": 300000,
            "playlist_id": "test-playlist-id",
          },
        ]);

        let playlist_items_response = json!([
          {
            "asset_id": "01AWJ47DP0000FXX7R00C5KX3F",
            "duration": 33,
          },
          {
            "asset_id": "01H2QDPVQ5JMKCBYJA78GGSEY4",
            "duration": 10.0
          },
        ]);
        let mock_server = MockServer::start();
        let get_items_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token");
            then.status(200).json_body(playlist_items_response);
        });

        // it will make a request to playlists to get predicate
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/playlists")
                .header("Authorization", "Token token");
            then.status(200).json_body(json!({"predicate": "TRUE"}));
        });

        // then delete request to delete old playlist items
        let delete_mock = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4/playlist-items")
                .query_param("playlist_id", "eq.test-playlist-id")
                .header("Authorization", "Token token");
            then.status(200).json_body(json!({}));
        });

        // then post request to create new playlist items
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/playlist-items")
                .header("Authorization", "Token token")
                .json_body(items_request);
            then.status(201).json_body(json!({}));
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = PlaylistCommand::new(authentication);
        let result = command.prepend_asset("test-playlist-id", "test-asset-id", 100);

        delete_mock.assert();
        post_mock.assert();
        get_mock.assert_calls(2);
        get_items_mock.assert();
        assert!(result.is_ok());
    }
}
