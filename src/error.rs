use std::fmt::Display;

use serde::Serialize;

use aws_sdk_cloudwatchlogs::Error as CloudWatchLogsError;

#[derive(Debug, Serialize)]
pub struct Error {
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Error occurred. Message: {}. Severity: {:#?}", &self.message, &self.severity))
    }
}

impl From<CloudWatchLogsError> for Error {
    fn from(e: CloudWatchLogsError) -> Self {
        Self {
            message: e.to_string(),
            severity: Severity::Error,
        }
    }
}
