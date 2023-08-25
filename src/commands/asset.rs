use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{Assets, CommandError};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};

use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::time::Duration;

pub struct AssetCommand {
    authentication: Authentication,
}

impl AssetCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn list(&self) -> anyhow::Result<Assets, CommandError> {
        Ok(Assets::new(commands::get(
            &self.authentication,
            "v4/assets",
        )?))
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Assets, CommandError> {
        let endpoint = format!("v4/assets?id=eq.{id}");

        Ok(Assets::new(commands::get(&self.authentication, &endpoint)?))
    }

    fn add_web_asset(
        &self,
        url: &str,
        headers: &HeaderMap,
        payload: &HashMap<&str, &str>,
    ) -> anyhow::Result<Assets, CommandError> {
        let response = self
            .authentication
            .build_client()?
            .post(url)
            .json(payload)
            .headers(headers.clone())
            .send()?;

        if response.status() != StatusCode::CREATED {
            let status = response.status().as_u16();
            return Err(CommandError::WrongResponseStatus(status));
        }

        Ok(Assets::new(serde_json::from_str(&response.text()?)?))
    }

    pub fn add(&self, path: &str, title: &str) -> anyhow::Result<Assets, CommandError> {
        let url = format!("{}/v4/assets", &self.authentication.config.url);

        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

        if path.starts_with("http://") || path.starts_with("https://") {
            let mut payload = HashMap::new();
            payload.insert("title", title);
            payload.insert("source_url", path);
            return self.add_web_asset(&url, &headers, &payload);
        }

        let file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let pb = ProgressBar::new(file_size);
        info!("Uploading asset.");
        if let Ok(template) = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:160.cyan/blue} {percent}% ETA: {eta}",
        ) {
            pb.set_style(template);
        }

        let part = reqwest::blocking::multipart::Part::reader(pb.wrap_read(file)).file_name("file");
        let form = reqwest::blocking::multipart::Form::new()
            .text("title", title.to_owned())
            .part("file", part);

        let response = self
            .authentication
            .build_client()?
            .post(url)
            .multipart(form)
            .headers(headers)
            .timeout(Duration::from_secs(3600)) // timeout is equal to server timeout
            .send()?;

        if response.status() != StatusCode::CREATED {
            let status = response.status().as_u16();
            return Err(CommandError::WrongResponseStatus(status));
        }

        Ok(Assets::new(serde_json::from_str(&response.text()?)?))
    }

    pub fn set_web_asset_headers(
        &self,
        id: &str,
        headers: Vec<(String, String)>,
    ) -> anyhow::Result<(), CommandError> {
        let endpoint = format!("v4/assets?id=eq.{id}");
        let map: HashMap<_, _> = headers.into_iter().collect();
        commands::patch(&self.authentication, &endpoint, &json!({ "headers": map }))?;
        Ok(())
    }

    pub fn update_web_asset_headers(
        &self,
        id: &str,
        headers: Vec<(String, String)>,
    ) -> anyhow::Result<(), CommandError> {
        // if no headers provided update has nothing to do
        if headers.is_empty() {
            return Ok(());
        }

        let mut new_headers: HashMap<_, _> = headers.into_iter().collect();
        let asset = self.get(id)?;
        if let Some(assets) = asset.value.as_array() {
            if assets.is_empty() {
                return Err(CommandError::MissingField);
            }
            let headers = assets[0].get("headers").ok_or(CommandError::MissingField)?;
            let old_headers = serde_json::from_value::<HashMap<String, String>>(headers.clone())?;
            debug!("Old headers {:?}", &old_headers);
            for (key, value) in old_headers {
                new_headers.entry(key).or_insert(value);
            }
        }

        self.set_web_asset_headers(
            id,
            new_headers.into_iter().collect::<Vec<(String, String)>>(),
        )
    }

    pub fn inject_js(&self, id: &str, js_code: &str) -> anyhow::Result<(), CommandError> {
        let endpoint = format!("v4/assets?id=eq.{id}");
        commands::patch(
            &self.authentication,
            &endpoint,
            &json!({ "js_injection": js_code }),
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<(), CommandError> {
        let endpoint = format!("v4/assets?id=eq.{id}");
        commands::delete(&self.authentication, &endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authentication::Config;
    use crate::commands::{Formatter, OutputType};

    use httpmock::Method::{DELETE, GET, PATCH, POST};
    use httpmock::MockServer;

    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_list_assets_should_return_correct_asset_list() {
        let asset_list = json!([{
          "asset_group_id": null,
          "asset_url": "https://us-assets.screenlyapp.com/test13",
          "disable_verification": false,
          "duration": 10.0,
          "headers": {},
          "height": null,
          "id": "0184846d-8f2e-d867-64af-8021cd00a3bc",
          "md5": "5b0db3811985481905566faf7b38f677",
          "meta_data": {},
          "send_metadata": false,
          "source_md5": null,
          "source_size": null,
          "source_url": "https://s3.amazonaws.com/us-assets.screenlyapp.com/assets%2Frow%test",
          "status": "finished",
          "title": "Uploaded via API v4",
          "type": "edge-app",
          "width": null
        },
        {
          "asset_group_id": "01675f41-d468-0000-d807-1a0019f71ce7",
          "asset_url": "https://us-assets.screenlyapp.com/test",
          "disable_verification": false,
          "duration": 33.07,
          "headers": {},
          "height": 1080,
          "id": "0163d007-4f10-0000-cf24-c500181dfcdc",
          "md5": "bb1fb7d464dd8db0b1cea151b4ea2997",
          "meta_data": {},
          "send_metadata": false,
          "source_md5": "90da81a2d0e62a068ac64370e0d55c2b",
          "source_size": 30874052,
          "source_url": "https://us-assets.screenlyapp.com/assets/raw/test/test/test",
          "status": "finished",
          "title": "Big Buck Bunny Trailer",
          "type": "video",
          "width": 1920
        }]);

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(asset_list.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command.list().unwrap();
        assert_eq!(v.value, asset_list);
    }

    #[test]
    fn test_add_asset_when_local_asset_should_send_correct_request() {
        let tmp_dir = tempdir().unwrap();
        fs::write(tmp_dir.path().join("1.html").to_str().unwrap(), "dummy").unwrap();

        let new_asset = json!([
          {
            "asset_group_id": null,
            "asset_url": "",
            "disable_verification": false,
            "duration": null,
            "headers": {},
            "height": null,
            "id": "0184f162-585e-6334-8dae-38a80062a6c2",
            "md5": null,
            "meta_data": {},
            "send_metadata": false,
            "source_md5": null,
            "source_size": null,
            "source_url": "https://s3.amazonaws.com/us-assets.screenlyapp.com/assets%2Frow%2FOZbhHeASzcYCsO8aNWICbpwrSYP2zVwB",
            "status": "none",
            "title": "test3.html",
            "type": null,
            "width": null
          }
        ]);

        let mock_server = MockServer::start();
        let post_mock = mock_server.mock(|when, then| {
            // TODO: figure out how to check the body for multiform content

            when.method(POST)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(201).json_body(new_asset.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command.add(tmp_dir.path().join("1.html").to_str().unwrap(), "test");
        post_mock.assert();

        assert!(v.is_ok());
        assert_eq!(v.unwrap().value, new_asset);
    }

    #[test]
    fn test_add_asset_when_web_asset_should_send_correct_request() {
        let tmp_dir = tempdir().unwrap();
        fs::write(tmp_dir.path().join("1.html").to_str().unwrap(), "dummy").unwrap();

        let new_asset = json!([
          {
            "asset_group_id": null,
            "asset_url": "",
            "disable_verification": false,
            "duration": null,
            "headers": {},
            "height": null,
            "id": "0184f162-585e-6334-8dae-38a80062a6c2",
            "md5": null,
            "meta_data": {},
            "send_metadata": false,
            "source_md5": null,
            "source_size": null,
            "source_url": "https://google.com",
            "status": "none",
            "title": "test3.html",
            "type": null,
            "width": null
          }
        ]);

        let mock_server = MockServer::start();
        let post_mock = mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v4/assets")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({"source_url": "https://google.com", "title": "test"}));
            then.status(201).json_body(new_asset.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command.add("https://google.com", "test");
        assert!(v.is_ok());
        post_mock.assert();
        assert_eq!(v.unwrap().value, new_asset);
    }

    #[test]
    fn test_get_asset_should_return_asset() {
        let asset = json!(  [{
          "asset_group_id": "017b0187-d887-3c79-7b67-18c94098345d",
          "asset_url": "https://vimeo.com/1084537",
          "disable_verification": false,
          "duration": 10.0,
          "headers": {},
          "height": 0,
          "id": "017b0187-d88c-eef6-7d42-e7c4ec7ef30a",
          "md5": "skip_md5",
          "meta_data": {},
          "send_metadata": false,
          "source_md5": null,
          "source_size": null,
          "source_url": "https://vimeo.com/1084537",
          "status": "finished",
          "title": "vimeo.com/1084537",
          "type": "web",
          "width": 0
        }]);

        let mock_server = MockServer::start();
        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("id", "eq.017b0187-d887-3c79-7b67-18c94098345d")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .header("Authorization", "Token token");
            then.status(200).json_body(asset.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);

        let v = asset_command.get("017b0187-d887-3c79-7b67-18c94098345d");
        get_mock.assert();
        assert!(v.is_ok());
        assert_eq!(v.unwrap().value, asset);
    }

    #[test]
    fn test_delete_asset_should_send_correct_request() {
        let mock_server = MockServer::start();
        let delete_mock = mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v4/assets")
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
        let asset_command = AssetCommand::new(authentication);
        let result = asset_command.delete("test-id");
        delete_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_asset_when_human_readable_output_is_set_should_return_correct_formatted_string()
    {
        let asset = Assets::new(json!([
          {
            "asset_group_id": null,
            "asset_url": "",
            "disable_verification": false,
            "duration": null,
            "headers": {},
            "height": null,
            "id": "0184f162-585e-6334-8dae-38a80062a6c2",
            "md5": null,
            "meta_data": {},
            "send_metadata": false,
            "source_md5": null,
            "source_size": null,
            "source_url": "https://s3.amazonaws.com/us-assets.screenlyapp.com/assets%2Frow%2FOZbhHeASzcYCsO8aNWICbpwrSYP2zVwB",
            "status": "none",
            "title": "test3.html",
            "type": null,
            "width": null
          }
        ]));

        println!("{}", asset.format(OutputType::HumanReadable));
        let expected_output =
            "+--------------------------------------+------------+------+--------+\n\
        | Id                                   | Title      | Type | Status |\n\
        +--------------------------------------+------------+------+--------+\n\
        | 0184f162-585e-6334-8dae-38a80062a6c2 | test3.html | N/A  | none   |\n\
        +--------------------------------------+------------+------+--------+\n";

        assert_eq!(asset.format(OutputType::HumanReadable), expected_output);
    }

    #[test]
    fn test_inject_js_should_send_correct_request() {
        let mock_server = MockServer::start();
        let patch_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/assets")
                .query_param("id", "eq.test-id")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({"js_injection": "console.log(1)"}))
                .header("Authorization", "Token token");
            then.status(200);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let result = asset_command.inject_js("test-id", "console.log(1)");
        patch_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_headers_should_send_correct_request() {
        let mock_server = MockServer::start();
        let patch_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/assets")
                .query_param("id", "eq.test-id")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({"headers": {"k": "v"}}))
                .header("Authorization", "Token token");
            then.status(200);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let headers = vec![("k".to_owned(), "v".to_owned())];
        let result = asset_command.set_web_asset_headers("test-id", headers);
        patch_mock.assert();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_headers_should_send_correct_request() {
        let mock_server = MockServer::start();
        let asset = json!(  [{
          "asset_group_id": "017b0187-d887-3c79-7b67-18c94098345d",
          "asset_url": "https://vimeo.com/1084537",
          "disable_verification": false,
          "duration": 10.0,
          "headers": {"a": "b"},
          "height": 0,
          "id": "017b0187-d88c-eef6-7d42-e7c4ec7ef30a",
          "md5": "skip_md5",
          "meta_data": {},
          "send_metadata": false,
          "source_md5": null,
          "source_size": null,
          "source_url": "https://vimeo.com/1084537",
          "status": "finished",
          "title": "vimeo.com/1084537",
          "type": "web",
          "width": 0
        }]);

        let get_mock = mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/assets")
                .query_param("id", "eq.test-id")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .header("Authorization", "Token token");
            then.status(200).json_body(asset);
        });

        let patch_mock = mock_server.mock(|when, then| {
            when.method(PATCH)
                .path("/v4/assets")
                .query_param("id", "eq.test-id")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({"headers": {"k": "v", "a": "b"}}))
                .header("Authorization", "Token token");
            then.status(200);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let asset_command = AssetCommand::new(authentication);
        let headers = vec![("k".to_owned(), "v".to_owned())];
        let result = asset_command.update_web_asset_headers("test-id", headers);

        get_mock.assert();
        patch_mock.assert();

        assert!(result.is_ok());
    }
}
