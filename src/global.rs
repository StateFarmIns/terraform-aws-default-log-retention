use std::{collections::HashMap, time::Duration};

use aws_config::{BehaviorVersion, SdkConfig};
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_smithy_types::retry::{RetryConfig, RetryMode};
use cached::proc_macro::cached;
use log::trace;
use metrics_cloudwatch_embedded::Collector;
use tracing::info_span;

use crate::cloudwatch_logs_traits::CloudWatchLogs;

#[cached]
pub async fn cloudwatch_logs() -> CloudWatchLogs {
    let sdk_config = sdk_config().await;
    CloudWatchLogs::new(CloudWatchLogsClient::new(&sdk_config))
}

#[cached]
async fn sdk_config() -> SdkConfig {
    let retry_config = RetryConfig::standard()
        .with_initial_backoff(Duration::from_millis(500))
        .with_max_attempts(10)
        .with_retry_mode(RetryMode::Adaptive);

    aws_config::defaults(BehaviorVersion::v2023_11_09()).retry_config(retry_config).load().await
}

#[cfg_attr(not(test), cached)] // Disables caching for tests https://github.com/jaemk/cached/issues/130
pub fn retention() -> i32 {
    std::env::var("log_retention_in_days")
        .unwrap_or_else(|_| "30".to_string())
        .parse()
        .unwrap_or(30)
}

#[cfg_attr(not(test), cached)] // Disables caching for tests https://github.com/jaemk/cached/issues/130
pub fn log_group_tags() -> Option<HashMap<String, String>> {
    let log_group_tags = std::env::var("log_group_tags").ok()?;
    let log_group_tags = serde_json::from_str(&log_group_tags).ok()?;
    Some(log_group_tags)
}

#[cached]
pub fn metric_namespace() -> String {
    std::env::var("metric_namespace").unwrap_or_else(|_| "LogRotation".to_string())
}

pub fn aws_partition() -> String {
    std::env::var("aws_partition").unwrap_or_else(|_| "aws".to_string())
}

pub fn initialize_logger() {
    trace!("Initializing logger...");
    env_logger::builder().format_timestamp(None).init();
}

pub fn initialize_metrics() -> &'static Collector {
    trace!("Initializing metrics emitter...");

    let lambda_function_name =
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").expect("Could not determine Lambda function name. Is this code being run in AWS Lambda?");

    metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace(metric_namespace())
        .with_dimension("function", lambda_function_name)
        .with_lambda_request_id("RequestId")
        .lambda_cold_start_metric("ColdStart")
        .lambda_cold_start_span(info_span!("cold start").entered())
        .init()
        .expect("Could not instantiate metric emitter.")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::global::retention;

    use super::{cloudwatch_logs, initialize_logger, log_group_tags};

    #[tokio::test]
    async fn test_cw_logs_client() {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAASDASDQWEF");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "ASIAAFQWEFWEIFJ");

        cloudwatch_logs().await;
    }

    #[test]
    fn test_retention() {
        std::env::set_var("log_retention_in_days", "1");
        assert_eq!(1, retention());
    }

    #[test]
    fn test_retention_invalid() {
        std::env::set_var("log_retention_in_days", "asdasdasd");
        assert_eq!(30, retention());
    }

    #[test]
    fn test_retention_not_set() {
        std::env::remove_var("log_retention_in_days");
        assert_eq!(30, retention());
    }

    #[test]
    fn test_log_group_tags_none() {
        std::env::remove_var("log_group_tags");
        assert_eq!(log_group_tags(), None);
    }

    #[test]
    fn test_log_group_tags_valid() {
        let tags_json = "{\"a\": \"b\", \"c\": \"d\"}";
        std::env::set_var("log_group_tags", tags_json);

        let mut expected = HashMap::new();
        expected.insert("a".to_string(), "b".to_string());
        expected.insert("c".to_string(), "d".to_string());

        assert_eq!(log_group_tags(), Some(expected));
    }

    #[test]
    fn test_log_group_tags_invalid_none() {
        std::env::set_var("log_group_tags", "true");
        assert_eq!(log_group_tags(), None);
    }

    #[test]
    fn test_log_group_tags_empty() {
        std::env::set_var("log_group_tags", "{}");
        assert_eq!(log_group_tags(), Some(HashMap::new()));
    }

    #[test]
    fn test_initialize_logger() {
        // Not much to test here......
        initialize_logger();
    }
}
