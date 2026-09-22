use prettytable::{Cell, Row};

use crate::authentication::Authentication;
use crate::commands::{self, CommandError, Formatter, OutputType};

pub struct WhoamiCommand {
    authentication: Authentication,
}

#[derive(Debug)]
pub struct WhoamiInfo {
    pub value: serde_json::Value,
}

impl WhoamiInfo {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    fn field(&self, object: &str, key: &str) -> String {
        self.value
            .get(object)
            .and_then(|o| o.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("N/A")
            .to_string()
    }

    fn full_name(&self) -> String {
        let first = self.field("user", "first_name");
        let last = self.field("user", "last_name");
        match (first.as_str(), last.as_str()) {
            ("N/A", "N/A") => "N/A".to_string(),
            ("N/A", last) => last.to_string(),
            (first, "N/A") => first.to_string(),
            (first, last) => format!("{first} {last}"),
        }
    }
}

impl WhoamiCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    pub fn get(&self) -> Result<WhoamiInfo, CommandError> {
        Ok(WhoamiInfo::new(commands::get(
            &self.authentication,
            "v3/me",
        )?))
    }
}

impl Formatter for WhoamiInfo {
    fn supports_csv() -> bool {
        true
    }

    fn format(&self, output_type: OutputType) -> String {
        match output_type {
            OutputType::Json => serde_json::to_string_pretty(&self.value).unwrap(),
            OutputType::HumanReadable => {
                let mut table = prettytable::Table::new();
                table.add_row(Row::from(vec!["Field", "Value"]));
                for (field, value) in [
                    ("Email", self.field("user", "email")),
                    ("Name", self.full_name()),
                    ("User ID", self.field("user", "id")),
                    ("Workspace", self.field("workspace", "name")),
                    ("Workspace ID", self.field("workspace", "id")),
                    ("Workspace URL", self.field("workspace", "url")),
                ] {
                    table.add_row(Row::new(vec![Cell::new(field), Cell::new(&value)]));
                }
                table.to_string()
            }
            OutputType::Csv => {
                let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
                wtr.write_record([
                    "email",
                    "name",
                    "user_id",
                    "workspace",
                    "workspace_id",
                    "workspace_url",
                ])
                .unwrap();
                wtr.write_record([
                    self.field("user", "email"),
                    self.full_name(),
                    self.field("user", "id"),
                    self.field("workspace", "name"),
                    self.field("workspace", "id"),
                    self.field("workspace", "url"),
                ])
                .unwrap();
                String::from_utf8(wtr.into_inner().unwrap()).unwrap()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::authentication::{Authentication, Config};

    fn sample_me() -> serde_json::Value {
        json!({
            "workspace": {
                "id": "01WORKSPACEID0000000000000",
                "name": "Example Workspace",
                "url": "https://example.screenlyapp.com"
            },
            "user": {
                "id": "01USERID000000000000000000",
                "first_name": "Ada",
                "last_name": "Lovelace",
                "email": "ada@example.com"
            }
        })
    }

    #[test]
    fn test_whoami_get_returns_profile() {
        let _tmp_dir = tempdir().unwrap();
        let mock_server = MockServer::start();
        let body = sample_me();
        mock_server.mock(|when, then| {
            when.method(GET)
                .path("/v3/me")
                .header("Authorization", "Token token")
                .header(
                    "user-agent",
                    format!("screenly-cli {}", env!("CARGO_PKG_VERSION")),
                );
            then.status(200).json_body(body.clone());
        });

        let config = Config::new(mock_server.base_url());
        let authentication = Authentication::new_with_config(config, "token");
        let command = WhoamiCommand::new(authentication);
        let info = command.get().unwrap();
        assert_eq!(info.value, body);
    }

    #[test]
    fn test_whoami_human_readable_format() {
        let info = WhoamiInfo::new(sample_me());
        let output = info.format(OutputType::HumanReadable);
        assert!(output.contains("Email"));
        assert!(output.contains("ada@example.com"));
        assert!(output.contains("Ada Lovelace"));
        assert!(output.contains("Example Workspace"));
        assert!(output.contains("https://example.screenlyapp.com"));
    }

    #[test]
    fn test_whoami_json_format() {
        let info = WhoamiInfo::new(sample_me());
        let output = info.format(OutputType::Json);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["user"]["email"], "ada@example.com");
        assert_eq!(parsed["workspace"]["name"], "Example Workspace");
    }

    #[test]
    fn test_whoami_csv_format() {
        let info = WhoamiInfo::new(sample_me());
        let output = info.format(OutputType::Csv);
        let mut lines = output.lines();
        assert_eq!(
            lines.next().unwrap(),
            "email,name,user_id,workspace,workspace_id,workspace_url"
        );
        assert_eq!(
            lines.next().unwrap(),
            "ada@example.com,Ada Lovelace,01USERID000000000000000000,Example Workspace,01WORKSPACEID0000000000000,https://example.screenlyapp.com"
        );
    }
}
