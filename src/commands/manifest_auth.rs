use serde::{Deserialize, Serialize};

use super::{edge_app_settings::Setting, SettingType};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    Basic,
    Bearer,
}

impl AuthType {
    pub fn generate_settings(&self, global: bool) -> Vec<Setting> {
        match self {
            AuthType::Basic => vec![
                Setting::new(
                    SettingType::String,
                    "Username",
                    "screenly_basic_auth_username",
                    "The username for Basic Authentication.",
                    global,
                ),
                Setting::new(
                    SettingType::Secret,
                    "Password",
                    "screenly_basic_auth_password",
                    "The password for Basic Authentication.",
                    global,
                ),
            ],
            AuthType::Bearer => vec![Setting::new(
                SettingType::String,
                "Token",
                "screenly_bearer_token",
                "The Bearer token for authentication.",
                global,
            )],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_settings_when_generated_should_have_correct_properties() {
        let auth_type = AuthType::Basic;
        let settings = auth_type.generate_settings(false);

        assert_eq!(settings.len(), 2);

        let username_setting = &settings[0];
        assert_eq!(username_setting.type_, SettingType::String);
        assert_eq!(username_setting.name, "screenly_basic_auth_username");
        assert_eq!(username_setting.title, Some("Username".to_string()));
        assert!(!username_setting.optional);
        assert!(username_setting
            .help_text
            .contains("username for Basic Authentication"));

        let password_setting = &settings[1];
        assert_eq!(password_setting.type_, SettingType::Secret);
        assert_eq!(password_setting.name, "screenly_basic_auth_password");
        assert_eq!(password_setting.title, Some("Password".to_string()));
        assert!(!password_setting.optional);
        assert!(password_setting
            .help_text
            .contains("password for Basic Authentication"));
    }

    #[test]
    fn test_bearer_auth_settings_when_generated_should_have_correct_properties() {
        let auth_type = AuthType::Bearer;
        let settings = auth_type.generate_settings(false);

        assert_eq!(settings.len(), 1);

        let token_setting = &settings[0];
        assert_eq!(token_setting.type_, SettingType::String);
        assert_eq!(token_setting.name, "screenly_bearer_token");
        assert_eq!(token_setting.title, Some("Token".to_string()));
        assert!(!token_setting.optional);
        assert!(token_setting
            .help_text
            .contains("Bearer token for authentication"));
    }
}
