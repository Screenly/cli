use crate::{Authentication, AuthenticationError};
use std::collections::HashMap;

use humantime::format_duration;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use std::fs::File;
use std::time::Duration;
use thiserror::Error;

use prettytable::{row, Table};
use reqwest::header::{HeaderMap, InvalidHeaderValue};
use reqwest::StatusCode;
use serde_json::json;

pub mod asset;
pub mod screen;

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
    Authentication(#[from] AuthenticationError),
    #[error("request error")]
    Request(#[from] reqwest::Error),
    #[error("parse error")]
    Parse(#[from] serde_json::Error),
    #[error("unknown error #[0]")]
    WrongResponseStatus(u16),
    #[error("Required field is missing in the response")]
    MissingField,
    #[error("I/O error #[0]")]
    Io(#[from] std::io::Error),
    #[error("Invalid header value")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
}

pub fn get(
    authentication: &Authentication,
    endpoint: &str,
) -> anyhow::Result<serde_json::Value, CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let response = authentication.build_client()?.get(url).send()?;
    if response.status() != StatusCode::OK {
        return Err(CommandError::WrongResponseStatus(
            response.status().as_u16(),
        ));
    }
    Ok(serde_json::from_str(&response.text()?)?)
}

pub fn delete(authentication: &Authentication, endpoint: &str) -> anyhow::Result<(), CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let response = authentication.build_client()?.delete(url).send()?;
    if ![StatusCode::OK, StatusCode::NO_CONTENT].contains(&response.status()) {
        return Err(CommandError::WrongResponseStatus(
            response.status().as_u16(),
        ));
    }
    Ok(())
}

pub fn patch(
    authentication: &Authentication,
    endpoint: &str,
    payload: &serde_json::Value,
) -> anyhow::Result<(), CommandError> {
    let url = format!("{}/{}", &authentication.config.url, endpoint);
    let mut headers = HeaderMap::new();
    headers.insert("Prefer", "return=representation".parse()?);

    let response = authentication
        .build_client()?
        .patch(url)
        .json(&payload)
        .headers(headers)
        .send()?;

    if response.status() != StatusCode::OK {
        return Err(CommandError::WrongResponseStatus(
            response.status().as_u16(),
        ));
    }

    Ok(())
}
