use std::fmt::Display;

use aws_smithy_client::SdkError;
use serde::Serialize;

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

impl<E: std::error::Error + Display, R> From<SdkError<E, R>> for Error {
    fn from(e: SdkError<E, R>) -> Self {
        Self {
            message: e.to_string(),
            severity: Severity::Error,
        }
    }
}
