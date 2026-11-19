// Mocks return `aws_sdk_cloudwatchlogs::Error` (160+ bytes) from closures; only relevant in test builds.
#![cfg_attr(test, allow(clippy::result_large_err))]

pub mod cloudwatch_logs_traits;
pub mod config;
pub mod error;
pub mod event;
pub mod global;
pub mod metric_publisher;
pub mod retention_setter;
