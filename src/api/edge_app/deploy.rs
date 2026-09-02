use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use log::debug;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::Api;
use crate::commands::CommandError;

const DEPLOY_TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Serialize)]
pub struct DeployPayload {
    pub manifest: Value,
    pub file_tree: HashMap<String, String>,
    pub delete_missing_settings: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct FailedFile {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OutstandingFiles {
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub pending: Vec<String>,
    #[serde(default)]
    pub failed: Vec<FailedFile>,
}

pub fn describe_failed_files(files: &[FailedFile]) -> String {
    files
        .iter()
        .map(|file| format!("{}: {}", file.path, file.error))
        .collect::<Vec<_>>()
        .join("; ")
}

impl fmt::Display for OutstandingFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if !self.missing.is_empty() {
            parts.push(format!("not uploaded: {}", self.missing.join(", ")));
        }
        if !self.pending.is_empty() {
            parts.push(format!("still processing: {}", self.pending.join(", ")));
        }
        if !self.failed.is_empty() {
            parts.push(format!("failed: {}", describe_failed_files(&self.failed)));
        }
        write!(f, "{}", parts.join("; "))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SettingsDiff {
    #[serde(default)]
    pub create: Vec<String>,
    #[serde(default)]
    pub update: Vec<String>,
    #[serde(default)]
    pub delete: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DeployDiff {
    #[serde(default)]
    pub settings: SettingsDiff,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DeployPreview {
    pub deploy_needed: bool,
    #[serde(default)]
    pub outstanding: OutstandingFiles,
    #[serde(default)]
    pub diff: DeployDiff,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DeployResult {
    pub revision: u32,
    pub created: bool,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub channel: String,
}

impl Api {
    pub fn deploy_preview(
        &self,
        app_id: &str,
        payload: &DeployPayload,
    ) -> Result<DeployPreview, CommandError> {
        let (status, body) = self.post_deploy(app_id, "deploy/preview", payload)?;
        if status != StatusCode::OK {
            return Err(CommandError::WrongResponseStatus(status.as_u16()));
        }

        Ok(serde_json::from_value(body)?)
    }

    pub fn deploy(
        &self,
        app_id: &str,
        payload: &DeployPayload,
    ) -> Result<DeployResult, CommandError> {
        #[derive(Deserialize)]
        struct Conflict {
            #[serde(default)]
            outstanding: OutstandingFiles,
        }

        let (status, body) = self.post_deploy(app_id, "deploy", payload)?;
        match status {
            StatusCode::OK => Ok(serde_json::from_value(body)?),
            StatusCode::CONFLICT => {
                let conflict: Conflict = serde_json::from_value(body)?;
                Err(CommandError::DeployRejected(
                    conflict.outstanding.to_string(),
                ))
            }
            _ => Err(CommandError::WrongResponseStatus(status.as_u16())),
        }
    }

    fn post_deploy(
        &self,
        app_id: &str,
        endpoint: &str,
        payload: &DeployPayload,
    ) -> Result<(StatusCode, Value), CommandError> {
        let url = format!(
            "{}/v3/edge-apps/{app_id}/{endpoint}",
            &self.authentication.config.url
        );
        debug!("POST {url}");

        let response = self
            .authentication
            .build_client()?
            .post(&url)
            .timeout(Duration::from_secs(DEPLOY_TIMEOUT_SECONDS))
            .json(payload)
            .send()?;

        let status = response.status();
        debug!("POST {url} -> {status}");

        match status {
            StatusCode::OK | StatusCode::CONFLICT => {
                Ok((status, serde_json::from_str(&response.text()?)?))
            }
            StatusCode::NOT_FOUND => Err(CommandError::AppNotFound(format!(
                "Edge App with ID '{app_id}' not found."
            ))),
            _ => {
                debug!("Response: {:?}", &response.text()?);
                Err(CommandError::WrongResponseStatus(status.as_u16()))
            }
        }
    }
}
