use crate::authentication::Authentication;
use crate::commands;
use crate::commands::{CommandError, Formatter, OutputType};
use prettytable::{row, Table};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug)]
pub struct Screens {
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
                                indicatif::HumanDuration(Duration::new(
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

pub struct ScreenCommand {
    authentication: Authentication,
}

impl ScreenCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn list(&self) -> anyhow::Result<Screens, CommandError> {
        Ok(Screens::new(commands::get(
            &self.authentication,
            "v4/screens",
        )?))
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Screens, CommandError> {
        let endpoint = format!("v4/screens?id=eq.{id}");

        Ok(Screens::new(commands::get(
            &self.authentication,
            &endpoint,
        )?))
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
        if response.status() != StatusCode::CREATED {
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
        let endpoint = format!("v3/screens/{id}/");
        commands::delete(&self.authentication, &endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use httpmock::{Method::GET, MockServer};

    use envtestkit::lock::lock_test;
    use envtestkit::set_env;
    use httpmock::Method::{DELETE, POST};
    use std::ffi::OsString;
    use std::fs;

    use crate::authentication::{Authentication, Config};
    use serde_json::{json, Value};
    use tempdir::TempDir;

    #[test]
    fn test_list_screens_should_return_correct_screen_list() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();

        let screens = serde_json::from_str::<Value>("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap();

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v4/screens")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(screens.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let v = screen_command.list().unwrap();
        assert_eq!(v.value, screens);
    }

    #[test]
    fn test_add_screen_should_send_correct_request() {
        let tmp_dir = TempDir::new("test").unwrap();
        let _lock = lock_test();
        let _test = set_env(OsString::from("HOME"), tmp_dir.path().to_str().unwrap());
        fs::write(tmp_dir.path().join(".screenly").to_str().unwrap(), "token").unwrap();
        let new_screen = serde_json::from_str::<Value>("{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Test\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}").unwrap();

        let mock_server = MockServer::start();
        mock_server.mock(|when, then| {
            when.method(POST)
                .path("/v3/screens/")
                .header("Authorization", "Token token")
                .header("content-type", "application/json")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                )
                .json_body(json!({"pin": "test-pin", "name": "test"}));
            then.status(201).json_body(new_screen.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config);
        let screen_command = ScreenCommand::new(authentication);
        let v = screen_command
            .add("test-pin", Some("test".to_string()))
            .unwrap();
        assert_eq!(v.value.as_array().unwrap()[0], new_screen);
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
    fn test_format_screen_when_human_readable_output_is_set_should_return_correct_formatted_string()
    {
        let screen = Screens::new(serde_json::from_str("[{\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb5\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"created_at\":\"2021-06-28T05:07:55+00:00\",\"name\":\"Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2021-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}, {\"id\":\"017a5104-524b-33d8-8026-9087b59e7eb6\",\"team_id\":\"016343c2-82b8-0000-a121-e30f1035875d\",\"created_at\":\"2020-06-28T05:07:55+00:00\",\"name\":\"Not Renat's integrated wired NM\",\"is_enabled\":true,\"coords\":[55.22931, 48.90429],\"last_ping\":\"2020-08-25T06:17:20.728+00:00\",\"last_ip\":null,\"local_ip\":\"192.168.1.146\",\"mac\":\"b8:27:eb:d6:83:6f\",\"last_screenshot_time\":\"2021-08-25T06:09:04.399+00:00\",\"uptime\":\"230728.38\",\"load_avg\":\"0.14\",\"signal_strength\":null,\"interface\":\"eth0\",\"debug\":false,\"location\":\"Kamsko-Ust'inskiy rayon, Russia\",\"team\":\"016343c2-82b8-0000-a121-e30f1035875e\",\"timezone\":\"Europe/Moscow\",\"type\":\"hardware\",\"hostname\":\"srly-4shnfrdc5cd2p0p\",\"ws_open\":false,\"status\":\"Offline\",\"last_screenshot\":\"https://us-assets.screenlyapp.com/01CD1W50NR000A28F31W83B1TY/screenshots/01F98G8MJB6FC809MGGYTSWZNN/5267668e6db35498e61b83d4c702dbe8\",\"in_sync\":false,\"software_version\":\"Screenly 2 Player\",\"hardware_version\":\"Raspberry Pi 3B\",\"config\":{\"hdmi_mode\": 34, \"hdmi_boost\": 2, \"hdmi_drive\": 0, \"hdmi_group\": 0, \"verify_ssl\": true, \"audio_output\": \"hdmi\", \"hdmi_timings\": \"\", \"overscan_top\": 0, \"overscan_left\": 0, \"use_composite\": false, \"display_rotate\": 0, \"overscan_right\": 0, \"overscan_scale\": 0, \"overscan_bottom\": 0, \"disable_overscan\": 0, \"shuffle_playlist\": false, \"framebuffer_width\": 0, \"use_composite_pal\": false, \"framebuffer_height\": 0, \"hdmi_force_hotplug\": true, \"use_composite_ntsc\": false, \"hdmi_pixel_encoding\": 0, \"play_history_enabled\": false}}]").unwrap());
        println!("{}", screen.format(OutputType::HumanReadable));
        let expected_output =
"+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+--------+\n\
| Id                                   | Name                            | Hardware Version | In Sync | Last Ping                     | Uptime |\n\
+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+--------+\n\
| 017a5104-524b-33d8-8026-9087b59e7eb5 | Renat's integrated wired NM     | Raspberry Pi 3B  |   ❌    | 2021-08-25T06:17:20.728+00:00 | 3 days |\n\
+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+--------+\n\
| 017a5104-524b-33d8-8026-9087b59e7eb6 | Not Renat's integrated wired NM | Raspberry Pi 3B  |   ❌    | 2020-08-25T06:17:20.728+00:00 | 3 days |\n\
+--------------------------------------+---------------------------------+------------------+---------+-------------------------------+--------+\n";

        assert_eq!(screen.format(OutputType::HumanReadable), expected_output);
    }
}
