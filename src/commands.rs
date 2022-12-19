use crate::{Authentication, AuthenticationError};
use std::collections::HashMap;

use humantime::format_duration;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use std::fs::File;
use std::time::Duration;
use thiserror::Error;

use prettytable::{row, Table};
use reqwest::header::{HeaderMap, InvalidHeaderValue};

pub enum OutputType {
    HumanReadable,
    Json,
}

pub trait Formatter {
    fn format(&self, output_type: OutputType) -> String;
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("auth error")]
    AuthenticationError(#[from] AuthenticationError),
    #[error("request error")]
    RequestError(#[from] reqwest::Error),
    #[error("parse error")]
    ParseError(#[from] serde_json::Error),
    #[error("unknown error #[0]")]
    WrongResponseStatus(u16),
    #[error("Required field is missing in the response")]
    MissingField,
    #[error("I/O error #[0]")]
    IoError(#[from] std::io::Error),
    #[error("Invalid header value")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
}

#[derive(Debug)]
pub struct Screens {
    pub value: serde_json::Value,
}

#[derive(Debug)]
pub struct Assets {
    pub value: serde_json::Value,
}

impl Screens {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Formatter for Screens {
    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            OutputType::HumanReadable => {
                let mut table = Table::new();
                table.add_row(row!(
                    bl =>
                    "Id",
                    "Name",
                    "Hardware Version",
                    "In Sync",
                    "Last Ping",
                    "Uptime"
                ));

                if !self.value.is_array() {
                    return "".to_owned();
                }

                if let Some(screens) = self.value.as_array() {
                    for screen in screens {
                        let formatted_uptime = if let Some(uptime) = screen["uptime"].as_str() {
                            if uptime.eq("N/A") {
                                uptime.to_owned()
                            } else {
                                format_duration(Duration::new(
                                    uptime.parse::<f64>().unwrap_or(0.0) as u64,
                                    0,
                                ))
                                .to_string()
                            }
                        } else {
                            "N/A".to_owned()
                        };

                        table.add_row(row!(
                            screen["id"].as_str().unwrap_or("N/A"),
                            screen["name"].as_str().unwrap_or("N/A"),
                            screen["hardware_version"].as_str().unwrap_or("N/A"),
                            c -> if screen["in_sync"].as_bool().unwrap_or(false) {
                                "✅"
                            } else {
                                "❌"
                            },
                            screen["last_ping"].as_str().unwrap_or("N/A"),
                            r -> formatted_uptime,
                        ));
                    }
                }
                table.to_string()
            }
            OutputType::Json => {
                serde_json::to_string_pretty(&self.value).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }
}

impl Assets {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Formatter for Assets {
    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            OutputType::HumanReadable => {
                let mut table = Table::new();
                table.add_row(row!("Id", "Title", "Type", "Status",));

                if !self.value.is_array() {
                    return "".to_owned();
                }

                if let Some(assets) = self.value.as_array() {
                    for asset in assets {
                        // TODO: actually use dimensions for videos and images?
                        let _dimensions = match (asset["width"].as_str(), asset["height"].as_str())
                        {
                            (Some(width), Some(height)) => format!("{}x{}", width, height),
                            _ => "N/A".to_owned(),
                        };

                        table.add_row(row!(
                            asset["id"].as_str().unwrap_or("N/A"),
                            asset["title"].as_str().unwrap_or("N/A"),
                            asset["type"].as_str().unwrap_or("N/A"),
                            asset["status"].as_str().unwrap_or("N/A"),
                        ));
                    }
                }
                table.to_string()
            }
            OutputType::Json => {
                serde_json::to_string_pretty(&self.value).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }
}
pub struct ScreenCommand {
    authentication: Authentication,
}

pub struct AssetCommand {
    authentication: Authentication,
}

fn get(
    authentication: &Authentication,
    endpoint: &str,
) -> anyhow::Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let response = authentication.build_client()?.get(url).send()?;
    if response.status().as_u16() != 200 {
        return Err(CommandError::WrongResponseStatus(
            response.status().as_u16(),
        ));
    }
    Ok(serde_json::from_str(&response.text()?)?)
}

fn delete(authentication: &Authentication, endpoint: &str) -> anyhow::Result<(), CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let response = authentication.build_client()?.delete(url).send()?;
    if ![200_u16, 204_u16].contains(&response.status().as_u16()) {
        return Err(CommandError::WrongResponseStatus(
            response.status().as_u16(),
        ));
    }
    Ok(())
}

impl ScreenCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn list(&self) -> anyhow::Result<Screens, CommandError> {
        Ok(Screens::new(get(&self.authentication, "v4/screens")?))
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Screens, CommandError> {
        let endpoint = format!("v4/screens?id=eq.{}", id);

        Ok(Screens::new(get(&self.authentication, &endpoint)?))
    }

    pub fn add(
        &self,
        pin: &str,
        maybe_name: Option<String>,
    ) -> anyhow::Result<Screens, CommandError> {
        let url = format!("{}/v3/screens/", &self.authentication.config.url);
        let mut payload = HashMap::new();
        payload.insert("pin".to_string(), pin.to_string());
        if let Some(name) = maybe_name {
            payload.insert("name".to_string(), name);
        }
        let response = self
            .authentication
            .build_client()?
            .post(url)
            .json(&payload)
            .send()?;
        if response.status().as_u16() != 201 {
            return Err(CommandError::WrongResponseStatus(
                response.status().as_u16(),
            ));
        }

        // Our newer endpoints all return arrays so let's just convert the output from v3 to be the same
        let mut array: Vec<serde_json::Value> = Vec::new();
        array.insert(0, serde_json::from_str(&response.text()?)?);
        Ok(Screens::new(serde_json::Value::Array(array)))
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<(), CommandError> {
        let endpoint = format!("v3/screens/{}/", id);
        delete(&self.authentication, &endpoint)
    }
}

impl AssetCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn list(&self) -> anyhow::Result<Assets, CommandError> {
        Ok(Assets::new(get(&self.authentication, "v4/assets")?))
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Assets, CommandError> {
        let endpoint = format!("v4/assets?id=eq.{}", id);

        Ok(Assets::new(get(&self.authentication, &endpoint)?))
    }

    fn add_web_asset(
        &self,
        url: &str,
        headers: &HeaderMap,
        payload: &HashMap<&str, &String>,
    ) -> anyhow::Result<Assets, CommandError> {
        let response = self
            .authentication
            .build_client()?
            .post(url)
            .json(payload)
            .headers(headers.clone())
            .send()?;

        if response.status().as_u16() != 201 {
            let status = response.status().as_u16();
            return Err(CommandError::WrongResponseStatus(status));
        }

        Ok(Assets::new(serde_json::from_str(&response.text()?)?))
    }

    pub fn add(&self, path: String, title: String) -> anyhow::Result<Assets, CommandError> {
        let url = format!("{}/v4/assets", &self.authentication.config.url);

        let mut headers = HeaderMap::new();
        headers.insert("Prefer", "return=representation".parse()?);

        if path.starts_with("http://") || path.starts_with("https://") {
            let mut payload = HashMap::new();
            payload.insert("title", &title);
            payload.insert("source_url", &path);
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
            .text("title", title)
            .part("file", part);

        let response = self
            .authentication
            .build_client()?
            .post(url)
            .multipart(form)
            .headers(headers)
            .send()?;

        if response.status().as_u16() != 201 {
            let status = response.status().as_u16();
            return Err(CommandError::WrongResponseStatus(status));
        }

        Ok(Assets::new(serde_json::from_str(&response.text()?)?))
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<(), CommandError> {
        let endpoint = format!("v4/assets?id=eq.{}", id);
        delete(&self.authentication, &endpoint)
    }
}

#[cfg(test)]
mod tests {
    use crate::authentication::Config;
    use crate::{Authentication, Formatter, OutputType};
    use httpmock::{Method::GET, MockServer};

    use envtestkit::lock::lock_test;
    use envtestkit::set_env;
    use httpmock::Method::{DELETE, POST};
    use std::ffi::OsString;
    use std::fs;

    use crate::commands::{AssetCommand, Assets, ScreenCommand, Screens};
    use serde_json::{json, Value};
    use tempdir::TempDir;

    #[test]
    fn test_list_screens_should_return_correct_screen_list() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/screens")
                .header("Authorization", "Token token")
                .header("user-agent", format!("screenly-cli {}", env!("CARGO_PKG_VERSION")));
            then
                .status(200)
                .body(b"[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let expected = serde_json::from_str::<Value>("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap();
        let v = screen_command.list().unwrap();
        assert_eq!(v.value, expected);
    }

    #[test]
    fn test_list_assets_should_return_correct_asset_list() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
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
        let authentication = Authentication::new_with_config(config);
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command.list().unwrap();
        assert_eq!(v.value, asset_list);
    }

    #[test]
    fn test_add_screen_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v3/screens/")
                .header("Authorization", "Token token")
                .header("content-type", "application/json")
                .header("user-agent", format!("screenly-cli {}", env!("CARGO_PKG_VERSION")))
                .json_body(json!({"pin": "test-pin", "name": "test"}));
            then
                .status(201)
                .body(b"{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Test\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let expected = serde_json::from_str::<Value>("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Test\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap();
        let v = screen_command
            .add("test-pin", Some("test".to_string()))
            .unwrap();
        assert_eq!(v.value, expected);
    }

    #[test]
    fn test_add_asset_when_local_asset_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
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
        mock_server.mock(|when, then| {
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
        let authentication = Authentication::new_with_config(config);
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command
            .add(
                tmp_dir.path().join("1.html").to_str().unwrap().to_string(),
                "test".to_owned(),
            )
            .unwrap();
        assert_eq!(v.value, new_asset);
    }

    #[test]
    fn test_add_asset_when_web_asset_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
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
        mock_server.mock(|when, then| {
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
        let authentication = Authentication::new_with_config(config);
        let asset_command = AssetCommand::new(authentication);
        let v = asset_command
            .add("https://google.com".to_owned(), "test".to_owned())
            .unwrap();
        assert_eq!(v.value, new_asset);
    }

    #[test]
    fn test_list_screens_when_not_authenticated_should_return_error() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/screens")
                .header("Authorization", "Token token")
                .header("user-agent", format!("screenly-cli {}", env!("CARGO_PKG_VERSION")));
            then
                .status(200)
                .body(b"[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let v = screen_command.list();
        assert!(v.is_err());
    }

    #[test]
    fn test_get_screen_should_return_screen() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/screens")
                .query_param("id", "eq.017a5104-524b-33d8-8026-9087b59e7eb5")
                .header("user-agent", format!("screenly-cli {}", env!("CARGO_PKG_VERSION")))
                .header("Authorization", "Token token");
            then
                .status(200)
                .body(b"[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]");
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let expected = serde_json::from_str::<Value>("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap();
        let v = screen_command
            .get("017a5104-524b-33d8-8026-9087b59e7eb5")
            .unwrap();
        assert_eq!(v.value, expected);
    }

    #[test]
    fn test_get_asset_should_return_asset() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();

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
        mock_server.mock(|when, then| {
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
        let authentication = Authentication::new_with_config(config);
        let asset_command = AssetCommand::new(authentication);

        let v = asset_command
            .get("017b0187-d887-3c79-7b67-18c94098345d")
            .unwrap();
        assert_eq!(v.value, asset);
    }

    #[test]
    fn test_delete_screen_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(DELETE)
                .path("/v3/screens/test-id/")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .header("Authorization", "Token token");
            then.status(200);
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        assert!(screen_command.delete("test-id").is_ok());
    }

    #[test]
    fn test_delete_asset_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
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
        let authentication = Authentication::new_with_config(config);
        let asset_command = AssetCommand::new(authentication);
        assert!(asset_command.delete("test-id").is_ok());
    }

    #[test]
    fn test_format_screen_when_human_readable_output_is_set_should_return_correct_formatted_string()
    {
        let screen = Screens::new(serde_json::from_str("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}, {\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb6\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875d\",\"created_at\":\"2020-06-28T05:07:55+00:00\",\"name\":\"Not Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2020-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap());
        println!("{}", screen.format(OutputType::HumanReadable));
        let expected_output = concat!(
"+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+------------------+\n",
"| Id                                   | Name                            | Hardware Version | In Sync | Last Ping                     | Uptime           |\n",
"+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+------------------+\n",
"| 017a5104-524b-33d8-8026-9087b59e7eb5 | Renat's integrated wired NM     | Raspberry Pi 3B  |   ❌    | 2021-08-25T06:17:20.728+00:00 | 2days 16h 5m 28s |\n",
"+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+------------------+\n",
"| 017a5104-524b-33d8-8026-9087b59e7eb6 | Not Renat's integrated wired NM | Raspberry Pi 3B  |   ❌    | 2020-08-25T06:17:20.728+00:00 | 2days 16h 5m 28s |\n",
"+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+------------------+\n"
);
        assert_eq!(screen.format(OutputType::HumanReadable), expected_output);
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
        let expected_output = concat!(
            "+--------------------------------------+------------+------+--------+\n",
            "| Id                                   | Title      | Type | Status |\n",
            "+--------------------------------------+------------+------+--------+\n",
            "| 0184f162-585e-6334-8dae-38a80062a6c2 | test3.html | N/A  | none   |\n",
            "+--------------------------------------+------------+------+--------+\n"
        );
        assert_eq!(asset.format(OutputType::HumanReadable), expected_output);
    }
}
