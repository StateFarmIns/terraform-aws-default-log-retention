use std::{collections::HashMap, time::Duration};

use aws_config::SdkConfig;
use aws_sdk_cloudwatch::Client as CloudWatchMetricsClient;
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_smithy_types::retry::{RetryConfig, RetryMode};
use cached::proc_macro::cached;

use crate::{cloudwatch_logs_traits::CloudWatchLogs, cloudwatch_metrics_traits::CloudWatchMetrics};

#[cached]
pub async fn cloudwatch_logs() -> CloudWatchLogs {
    let sdk_config = sdk_config().await;
    CloudWatchLogs::new(CloudWatchLogsClient::new(&sdk_config))
}

#[cached]
pub async fn cloudwatch_metrics() -> CloudWatchMetrics {
    let sdk_config = sdk_config().await;
    CloudWatchMetrics::new(CloudWatchMetricsClient::new(&sdk_config))
}

#[cached]
async fn sdk_config() -> SdkConfig {
    let retry_config = RetryConfig::standard()
        .with_initial_backoff(Duration::from_millis(500))
        .with_max_attempts(10)
        .with_retry_mode(RetryMode::Adaptive);

    aws_config::from_env().retry_config(retry_config).load().await
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

pub fn initialize_logger() {
    env_logger::builder().format_timestamp(None).init();
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::global::retention;

    use super::{cloudwatch_logs, cloudwatch_metrics, initialize_logger, log_group_tags};

    #[tokio::test]
    async fn test_cw_logs_client() {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAASDASDQWEF");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "ASIAAFQWEFWEIFJ");

        let client = cloudwatch_logs().await;
        // Remove "base" value from the snapshot because it is a dynamic value
        insta::with_settings!(
            {filters => vec![(r"base: .*\n\s*", "")]},
            {insta::assert_debug_snapshot!(client)}
        )
    }

    #[tokio::test]
    async fn test_cw_metrics_client() {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAASDASDQWEF");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "ASIAAFQWEFWEIFJ");

        let client = cloudwatch_metrics().await;
        // Remove "base" value from the snapshot because it is a dynamic value
        insta::with_settings!(
            {filters => vec![(r"base: .*\n\s*", "")]},
            {insta::assert_debug_snapshot!(client)}
        )
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
